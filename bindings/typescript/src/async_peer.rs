use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use napi::bindgen_prelude::Buffer;
use napi::{Env, JsFunction, JsObject, JsUnknown, Ref, Result, Status};
use napi_derive::napi;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::error::{invalid_state, to_napi_error};
use crate::frame::JsFrame;
use crate::listener::ListenerOptions;

type AsyncPeerRx = ipcprims_peer::async_peer::AsyncPeerRx;
type AsyncPeerTx = ipcprims_peer::async_peer::AsyncPeerTx;
type DispatchResult = std::result::Result<ipcprims_frame::Frame, String>;
type ReceiveResult = std::result::Result<JsFrame, napi::Error>;

const MAX_DISPATCH_BUFFERED_FRAMES: usize = 256;
const MAX_DISPATCH_BUFFERED_BYTES: usize = 16 * 1024 * 1024;

#[napi(object)]
pub struct RecvAsyncOptions {
    #[napi(ts_type = "AbortSignal")]
    pub signal: Option<JsObject>,
}

enum DispatchCommand {
    Recv {
        id: u64,
        channel: Option<u16>,
        tx: oneshot::Sender<DispatchResult>,
    },
    Cancel {
        id: u64,
    },
    Close,
}

struct Waiter {
    id: u64,
    channel: Option<u16>,
    tx: oneshot::Sender<DispatchResult>,
}

struct RecvCancellation {
    token: Option<CancellationToken>,
    abort: Option<AbortRegistration>,
}

struct AbortRegistration {
    signal: Ref<()>,
    listener: Ref<()>,
}

#[napi]
pub struct AsyncPeer {
    tx: Arc<StdMutex<Option<AsyncPeerTx>>>,
    dispatch: mpsc::UnboundedSender<DispatchCommand>,
    next_waiter: Arc<AtomicU64>,
    closed: Arc<AtomicBool>,
}

impl AsyncPeer {
    fn from_inner(peer: ipcprims_peer::async_peer::AsyncPeer) -> Self {
        let (tx, rx) = peer.into_split();
        let (dispatch, commands) = mpsc::unbounded_channel();
        tokio::spawn(dispatch_recv(rx, commands));
        Self {
            tx: Arc::new(StdMutex::new(Some(tx))),
            dispatch,
            next_waiter: Arc::new(AtomicU64::new(1)),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }

    fn ensure_open(closed: &AtomicBool) -> Result<()> {
        if closed.load(Ordering::SeqCst) {
            return Err(invalid_state("async peer is closed"));
        }
        Ok(())
    }

    fn next_waiter_id(&self) -> u64 {
        self.next_waiter.fetch_add(1, Ordering::Relaxed)
    }
}

#[napi]
impl AsyncPeer {
    #[napi(factory)]
    pub async fn connect(path: String, channels: Vec<u16>) -> Result<Self> {
        let peer = ipcprims_peer::async_connect(&path, &channels)
            .await
            .map_err(|err| to_napi_error("async connect failed", err))?;
        Ok(Self::from_inner(peer))
    }

    #[napi(ts_return_type = "Promise<void>")]
    pub fn send(&self, env: Env, channel: u16, data: Buffer) -> Result<JsObject> {
        Self::ensure_open(&self.closed)?;
        let tx = self.tx_handle()?;
        let payload = data.to_vec();
        env.execute_tokio_future(
            async move {
                tx.send(channel, &payload)
                    .await
                    .map_err(|err| to_napi_error("async send failed", err))
            },
            |_, ()| Ok(()),
        )
    }

    #[napi(ts_return_type = "Promise<JsFrame>")]
    pub fn recv_async(&self, env: Env, options: Option<RecvAsyncOptions>) -> Result<JsObject> {
        Self::ensure_open(&self.closed)?;
        let id = self.next_waiter_id();
        execute_recv_future(env, self.dispatch.clone(), id, None, options)
    }

    #[napi(ts_return_type = "Promise<JsFrame>")]
    pub fn recv_on_async(
        &self,
        env: Env,
        channel: u16,
        options: Option<RecvAsyncOptions>,
    ) -> Result<JsObject> {
        Self::ensure_open(&self.closed)?;
        let id = self.next_waiter_id();
        execute_recv_future(env, self.dispatch.clone(), id, Some(channel), options)
    }

    #[napi(ts_return_type = "Promise<AsyncChannelReceiver>")]
    pub fn open_channel(&self, env: Env, channel: u16) -> Result<JsObject> {
        Self::ensure_open(&self.closed)?;
        let receiver = AsyncChannelReceiver {
            channel,
            dispatch: self.dispatch.clone(),
            next_waiter: Arc::clone(&self.next_waiter),
        };
        env.execute_tokio_future(async move { Ok(receiver) }, |_, receiver| Ok(receiver))
    }

    #[napi(ts_return_type = "Promise<number>")]
    pub fn ping(&self, env: Env) -> Result<JsObject> {
        Self::ensure_open(&self.closed)?;
        let tx = self.tx_handle()?;
        env.execute_tokio_future(
            async move {
                let rtt = tx
                    .ping()
                    .await
                    .map_err(|err| to_napi_error("async ping failed", err))?;
                let ms = rtt.as_millis();
                Ok(u32::try_from(ms).unwrap_or(u32::MAX))
            },
            |_, ms| Ok(ms),
        )
    }

    #[napi(ts_return_type = "Promise<void>")]
    pub fn shutdown(&self, env: Env) -> Result<JsObject> {
        Self::ensure_open(&self.closed)?;
        let tx = self.tx_handle()?;
        env.execute_tokio_future(
            async move {
                tx.shutdown()
                    .await
                    .map_err(|err| to_napi_error("async shutdown failed", err))
            },
            |_, ()| Ok(()),
        )
    }

    #[napi]
    pub fn close(&self) -> Result<()> {
        self.closed.store(true, Ordering::SeqCst);
        let _ = self
            .tx
            .lock()
            .map_err(|_| invalid_state("async peer tx lock poisoned"))?
            .take();
        let _ = self.dispatch.send(DispatchCommand::Close);
        Ok(())
    }
}

impl AsyncPeer {
    fn tx_handle(&self) -> Result<AsyncPeerTx> {
        self.tx
            .lock()
            .map_err(|_| invalid_state("async peer tx lock poisoned"))?
            .as_ref()
            .cloned()
            .ok_or_else(|| invalid_state("async peer is closed"))
    }
}

#[napi]
pub struct AsyncChannelReceiver {
    channel: u16,
    dispatch: mpsc::UnboundedSender<DispatchCommand>,
    next_waiter: Arc<AtomicU64>,
}

#[napi]
impl AsyncChannelReceiver {
    #[napi(ts_return_type = "Promise<JsFrame>")]
    pub fn recv_async(&self, env: Env, options: Option<RecvAsyncOptions>) -> Result<JsObject> {
        let id = self.next_waiter.fetch_add(1, Ordering::Relaxed);
        execute_recv_future(env, self.dispatch.clone(), id, Some(self.channel), options)
    }
}

#[napi]
pub struct AsyncListener {
    inner: Arc<ipcprims_peer::AsyncPeerListener>,
    cancel: CancellationToken,
    closed: Arc<AtomicBool>,
}

#[napi]
impl AsyncListener {
    #[napi(factory)]
    pub fn bind(path: String, options: Option<ListenerOptions>) -> Result<Self> {
        let mut listener = ipcprims_peer::AsyncPeerListener::bind(&path)
            .map_err(|err| to_napi_error("async listener bind failed", err))?;

        if let Some(opts) = options {
            if let Some(channels) = opts.channels {
                listener = listener.with_channels(&channels);
            }
            if let Some(schema_dir) = opts.schema_dir {
                let registry = ipcprims_schema::SchemaRegistry::from_directory(schema_dir.as_ref())
                    .map_err(|err| to_napi_error("schema registry load failed", err))?;
                listener = listener.with_schema_registry(std::sync::Arc::new(registry));
            }
        }

        Ok(Self {
            inner: Arc::new(listener),
            cancel: CancellationToken::new(),
            closed: Arc::new(AtomicBool::new(false)),
        })
    }

    #[napi(ts_return_type = "Promise<AsyncPeer>")]
    pub fn accept(&self, env: Env) -> Result<JsObject> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(invalid_state("async listener is closed"));
        }
        let inner = Arc::clone(&self.inner);
        let cancel = self.cancel.clone();
        env.execute_tokio_future(
            async move {
                let peer = tokio::select! {
                    _ = cancel.cancelled() => {
                        return Err(invalid_state("async listener is closed"));
                    }
                    peer = inner.accept() => peer
                        .map_err(|err| to_napi_error("async listener accept failed", err))?,
                };
                Ok(AsyncPeer::from_inner(peer))
            },
            |_, peer| Ok(peer),
        )
    }

    #[napi(ts_return_type = "Promise<void>")]
    pub fn close(&self, env: Env) -> Result<JsObject> {
        self.closed.store(true, Ordering::SeqCst);
        self.cancel.cancel();
        env.execute_tokio_future(async move { Ok(()) }, |_, ()| Ok(()))
    }
}

fn execute_recv_future(
    env: Env,
    dispatch: mpsc::UnboundedSender<DispatchCommand>,
    id: u64,
    channel: Option<u16>,
    options: Option<RecvAsyncOptions>,
) -> Result<JsObject> {
    let cancellation = cancellation_from_options(&env, options)?;
    env.execute_tokio_future(
        async move {
            let result = recv_via_dispatch(dispatch, id, channel, cancellation.token)
                .await
                .map(frame_to_js);
            Ok((result, cancellation.abort))
        },
        |env, (result, abort): (ReceiveResult, Option<AbortRegistration>)| {
            let cleanup = cleanup_abort_listener(env, abort);
            match result {
                Ok(frame) => {
                    cleanup?;
                    Ok(frame)
                }
                Err(err) => {
                    let _ = cleanup;
                    Err(err)
                }
            }
        },
    )
}

async fn recv_via_dispatch(
    dispatch: mpsc::UnboundedSender<DispatchCommand>,
    id: u64,
    channel: Option<u16>,
    cancel: Option<CancellationToken>,
) -> Result<ipcprims_frame::Frame> {
    let (tx, rx) = oneshot::channel();
    dispatch
        .send(DispatchCommand::Recv { id, channel, tx })
        .map_err(|_| invalid_state("async peer receive path is closed"))?;

    match cancel {
        Some(token) => {
            tokio::select! {
                _ = token.cancelled() => {
                    let _ = dispatch.send(DispatchCommand::Cancel { id });
                    Err(napi::Error::new(Status::Cancelled, "receive aborted"))
                }
                res = rx => dispatch_result_to_napi(res),
            }
        }
        None => dispatch_result_to_napi(rx.await),
    }
}

fn dispatch_result_to_napi(
    res: std::result::Result<DispatchResult, oneshot::error::RecvError>,
) -> Result<ipcprims_frame::Frame> {
    match res {
        Ok(Ok(frame)) => Ok(frame),
        Ok(Err(message)) => Err(invalid_state(&message)),
        Err(_) => Err(invalid_state("async peer receive path is closed")),
    }
}

async fn dispatch_recv(
    mut rx: AsyncPeerRx,
    mut commands: mpsc::UnboundedReceiver<DispatchCommand>,
) {
    let mut waiters = VecDeque::<Waiter>::new();
    let mut buffered = VecDeque::<ipcprims_frame::Frame>::new();
    let mut buffered_bytes = 0usize;

    loop {
        tokio::select! {
            command = commands.recv() => {
                match command {
                    Some(DispatchCommand::Recv { id, channel, tx }) => {
                        if let Some(channel) = channel {
                            if channel != ipcprims_frame::CONTROL && !rx.supports_channel(channel) {
                                let _ = tx.send(Err(format!("channel is not negotiated: {channel}")));
                                continue;
                            }
                        }
                        if let Some(frame) =
                            take_buffered(channel, &mut buffered, &mut buffered_bytes)
                        {
                            let _ = tx.send(Ok(frame));
                        } else {
                            waiters.push_back(Waiter { id, channel, tx });
                        }
                    }
                    Some(DispatchCommand::Cancel { id }) => {
                        if let Some(index) = waiters.iter().position(|waiter| waiter.id == id) {
                            waiters.remove(index);
                        }
                    }
                    Some(DispatchCommand::Close) | None => {
                        rx.cancel();
                        fail_waiters(&mut waiters, "async peer is closed");
                        break;
                    }
                }
            }
            frame = rx.recv() => {
                match frame {
                    Ok(frame) => {
                        if let Some(index) = matching_waiter(&waiters, frame.channel) {
                            let waiter = waiters.remove(index).expect("waiter index came from position");
                            let _ = waiter.tx.send(Ok(frame));
                        } else {
                            let frame_bytes = frame.payload.len().saturating_add(ipcprims_frame::HEADER_SIZE);
                            if buffered.len() >= MAX_DISPATCH_BUFFERED_FRAMES
                                || buffered_bytes.saturating_add(frame_bytes) > MAX_DISPATCH_BUFFERED_BYTES
                            {
                                rx.cancel();
                                fail_waiters(&mut waiters, "async receive buffer limit exceeded");
                                break;
                            }
                            buffered_bytes = buffered_bytes.saturating_add(frame_bytes);
                            buffered.push_back(frame);
                        }
                    }
                    Err(err) => {
                        fail_waiters(&mut waiters, &err.to_string());
                        break;
                    }
                }
            }
        }
    }
}

fn take_buffered(
    channel: Option<u16>,
    buffered: &mut VecDeque<ipcprims_frame::Frame>,
    buffered_bytes: &mut usize,
) -> Option<ipcprims_frame::Frame> {
    let index = match channel {
        Some(channel) => buffered.iter().position(|frame| frame.channel == channel),
        None => {
            if buffered.is_empty() {
                None
            } else {
                Some(0)
            }
        }
    }?;
    let frame = buffered
        .remove(index)
        .expect("buffer index came from position");
    *buffered_bytes = buffered_bytes.saturating_sub(
        frame
            .payload
            .len()
            .saturating_add(ipcprims_frame::HEADER_SIZE),
    );
    Some(frame)
}

fn matching_waiter(waiters: &VecDeque<Waiter>, channel: u16) -> Option<usize> {
    waiters
        .iter()
        .position(|waiter| waiter.channel.is_none())
        .or_else(|| {
            waiters
                .iter()
                .position(|waiter| waiter.channel == Some(channel))
        })
}

fn fail_waiters(waiters: &mut VecDeque<Waiter>, message: &str) {
    while let Some(waiter) = waiters.pop_front() {
        let _ = waiter.tx.send(Err(message.to_string()));
    }
}

fn frame_to_js(frame: ipcprims_frame::Frame) -> JsFrame {
    JsFrame {
        channel: frame.channel,
        payload: frame.payload.to_vec().into(),
    }
}

fn cancellation_from_options(
    env: &Env,
    options: Option<RecvAsyncOptions>,
) -> Result<RecvCancellation> {
    let Some(options) = options else {
        return Ok(RecvCancellation {
            token: None,
            abort: None,
        });
    };
    let Some(signal) = options.signal else {
        return Ok(RecvCancellation {
            token: None,
            abort: None,
        });
    };
    let token = CancellationToken::new();
    if signal
        .get_named_property::<bool>("aborted")
        .unwrap_or(false)
    {
        token.cancel();
        return Ok(RecvCancellation {
            token: Some(token),
            abort: None,
        });
    }

    let abort_token = token.clone();
    let onabort: JsFunction = env.create_function_from_closure("onabort", move |_| {
        abort_token.cancel();
        Ok(())
    })?;
    let mut signal_ref = env.create_reference(&signal)?;
    let mut listener_ref = env.create_reference(&onabort)?;
    let add_event_listener: JsFunction = signal.get_named_property("addEventListener")?;
    let mut options = env.create_object()?;
    options.set_named_property("once", true)?;
    let args: Vec<JsUnknown> = vec![
        env.create_string("abort")?.into_unknown(),
        onabort.into_unknown(),
        options.into_unknown(),
    ];
    if let Err(err) = add_event_listener.call(Some(&signal), &args) {
        let _ = listener_ref.unref(*env);
        let _ = signal_ref.unref(*env);
        return Err(err);
    }
    Ok(RecvCancellation {
        token: Some(token),
        abort: Some(AbortRegistration {
            signal: signal_ref,
            listener: listener_ref,
        }),
    })
}

fn cleanup_abort_listener(env: &Env, abort: Option<AbortRegistration>) -> Result<()> {
    let Some(mut abort) = abort else {
        return Ok(());
    };
    let signal: JsObject = env.get_reference_value_unchecked(&abort.signal)?;
    let listener: JsFunction = env.get_reference_value_unchecked(&abort.listener)?;
    let remove_event_listener: JsFunction = signal.get_named_property("removeEventListener")?;
    let args: Vec<JsUnknown> = vec![
        env.create_string("abort")?.into_unknown(),
        listener.into_unknown(),
    ];
    let remove_result = remove_event_listener.call(Some(&signal), &args).map(|_| ());
    let listener_unref = abort.listener.unref(*env).map(|_| ());
    let signal_unref = abort.signal.unref(*env).map(|_| ());
    remove_result?;
    listener_unref?;
    signal_unref?;
    Ok(())
}
