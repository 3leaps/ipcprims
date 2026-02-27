//! Tokio async peer API (Unix-only in v0.2.0).
//!
//! Design is governed by `docs/decisions/ADR-0001-async-peer-receive-model.md`:
//! - Arrival-ordered receive path is a single queue (`any_rx`)
//! - Per-channel receivers are opt-in fanout (tee semantics)
//! - Buffering is bounded by both per-channel frame counts and a global byte budget
//!
//! Notes:
//! - The arrival-ordered `any_rx` queue exists by default. If you only consume per-channel
//!   receivers, call `AsyncPeerRx::disable_any_delivery()` immediately to avoid disconnect
//!   from an undrained `any_rx` buffer.
//! - With the `schema` feature enabled, inbound schema validation failures currently cause a hard
//!   disconnect and are surfaced to receivers as `PeerError::Disconnected(String)` (error text).

use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use bytes::BytesMut;
use futures_core::Stream;
use ipcprims_frame::{
    decode_frame, encode_frame, Frame, FrameError, CONTROL, DEFAULT_MAX_PAYLOAD, HEADER_SIZE,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{mpsc, oneshot, watch, Semaphore};
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::control::{
    ControlMessage, CONTROL_PING, CONTROL_PONG, CONTROL_SHUTDOWN_ACK, CONTROL_SHUTDOWN_FORCE,
    CONTROL_SHUTDOWN_REQUEST,
};
use crate::error::{PeerError, Result};
use crate::handshake::HandshakeResult;
use crate::peer::{PeerConfig, SchemaRegistryHandle};

const READ_CHUNK_SIZE: usize = 8 * 1024;

#[derive(Debug)]
struct PermitHolder {
    _permit: tokio::sync::OwnedSemaphorePermit,
}

#[derive(Debug, Clone)]
struct QueuedFrame {
    frame: Frame,
    // Held to release byte-budget permits when the last queued copy is dropped.
    _permit: Arc<PermitHolder>,
}

impl QueuedFrame {
    fn new(frame: Frame, permit: Arc<PermitHolder>) -> Self {
        Self {
            frame,
            _permit: permit,
        }
    }
}

fn disconnect_error(disconnect: &watch::Receiver<Option<PeerError>>) -> PeerError {
    match disconnect.borrow().as_ref() {
        Some(PeerError::BufferFull(ch)) => PeerError::BufferFull(*ch),
        Some(PeerError::UnsupportedChannel(ch)) => PeerError::UnsupportedChannel(*ch),
        Some(PeerError::Timeout(d)) => PeerError::Timeout(*d),
        Some(PeerError::HandshakeFailed(s)) => PeerError::HandshakeFailed(s.clone()),
        Some(PeerError::ShutdownFailed(s)) => PeerError::ShutdownFailed(s.clone()),
        Some(PeerError::Disconnected(s)) => PeerError::Disconnected(s.clone()),
        Some(PeerError::Frame(e)) => PeerError::Disconnected(e.to_string()),
        Some(PeerError::Transport(e)) => PeerError::Disconnected(e.to_string()),
        Some(PeerError::Json(e)) => PeerError::Disconnected(e.to_string()),
        #[cfg(feature = "schema")]
        Some(PeerError::Schema(e)) => PeerError::Disconnected(e.to_string()),
        None => PeerError::Disconnected("connection closed".to_string()),
    }
}

fn closed_receiver<T>() -> mpsc::Receiver<T> {
    let (_tx, rx) = mpsc::channel(1);
    rx
}

/// A receiver of frames in arrival order.
///
/// Tee semantics: if you also subscribe to a per-channel receiver, you will observe
/// the same frames twice by design (once via each path).
pub struct AnyReceiver {
    // Keeps peer alive while this receiver is in use.
    _shared: Arc<Shared>,
    rx: mpsc::Receiver<QueuedFrame>,
    disconnect: watch::Receiver<Option<PeerError>>,
    done: bool,
}

impl AnyReceiver {
    pub async fn recv(&mut self) -> Result<Frame> {
        match self.rx.recv().await {
            Some(q) => Ok(q.frame),
            None => Err(disconnect_error(&self.disconnect)),
        }
    }
}

impl Stream for AnyReceiver {
    type Item = Result<Frame>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.done {
            return Poll::Ready(None);
        }

        match Pin::new(&mut this.rx).poll_recv(cx) {
            Poll::Ready(Some(q)) => Poll::Ready(Some(Ok(q.frame))),
            Poll::Ready(None) => {
                this.done = true;
                Poll::Ready(Some(Err(disconnect_error(&this.disconnect))))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// A receiver for a specific negotiated channel.
///
/// This receiver exists only if the consumer opted in via `AsyncPeerRx::take_channel_receiver`.
/// On drop, it unsubscribes the channel fanout.
pub struct ChannelReceiver {
    // Keeps peer alive while this receiver is in use.
    _shared: Arc<Shared>,
    channel: u16,
    rx: mpsc::Receiver<QueuedFrame>,
    disconnect: watch::Receiver<Option<PeerError>>,
    subscriptions: Arc<Mutex<HashMap<u16, mpsc::Sender<QueuedFrame>>>>,
    done: bool,
}

impl ChannelReceiver {
    pub fn channel(&self) -> u16 {
        self.channel
    }

    pub async fn recv(&mut self) -> Result<Frame> {
        match self.rx.recv().await {
            Some(q) => Ok(q.frame),
            None => Err(disconnect_error(&self.disconnect)),
        }
    }
}

impl Drop for ChannelReceiver {
    fn drop(&mut self) {
        if let Ok(mut subs) = self.subscriptions.lock() {
            subs.remove(&self.channel);
        }
    }
}

impl Stream for ChannelReceiver {
    type Item = Result<Frame>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.done {
            return Poll::Ready(None);
        }

        match Pin::new(&mut this.rx).poll_recv(cx) {
            Poll::Ready(Some(q)) => Poll::Ready(Some(Ok(q.frame))),
            Poll::Ready(None) => {
                this.done = true;
                Poll::Ready(Some(Err(disconnect_error(&this.disconnect))))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

struct Shared {
    id: String,
    negotiated: Vec<u16>,
    negotiated_set: HashSet<u16>,
    writer: tokio::sync::Mutex<OwnedWriteHalf>,
    schema_registry: Option<SchemaRegistryHandle>,
    config: PeerConfig,
    waiter_token: AtomicU64,
    ping_waiter: tokio::sync::Mutex<Option<InFlightWaiter>>,
    shutdown_waiter: tokio::sync::Mutex<Option<InFlightWaiter>>,

    // Cancellation signal for the reader task.
    //
    // This is used for local cancellation (via `AsyncPeer{,Rx}::cancel()`) and to ensure the
    // reader task unblocks when the last peer handle is dropped (via `Shared::drop`).
    //
    // It can also be paired with an optional external structured cancellation token.
    cancel: CancellationToken,

    // Optional external structured cancellation token (brief D8).
    external_cancel: Option<CancellationToken>,
}

#[derive(Debug)]
struct InFlightWaiter {
    token: u64,
    tx: oneshot::Sender<()>,
}

#[derive(Clone, Copy, Debug)]
enum WaiterKind {
    Ping,
    Shutdown,
}

/// Clears the in-flight waiter if the ping/shutdown future is dropped (e.g., wrapped in an outer
/// timeout). Uses token matching so we don't accidentally clear a newer in-flight waiter.
struct WaiterDropGuard {
    shared: Arc<Shared>,
    token: u64,
    kind: WaiterKind,
}

impl Drop for WaiterDropGuard {
    fn drop(&mut self) {
        match self.kind {
            WaiterKind::Ping => {
                if let Ok(mut guard) = self.shared.ping_waiter.try_lock() {
                    if guard.as_ref().is_some_and(|w| w.token == self.token) {
                        let _ = guard.take();
                    }
                }
            }
            WaiterKind::Shutdown => {
                if let Ok(mut guard) = self.shared.shutdown_waiter.try_lock() {
                    if guard.as_ref().is_some_and(|w| w.token == self.token) {
                        let _ = guard.take();
                    }
                }
            }
        }
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        // Ensure the reader task unblocks even if it's waiting on a read.
        self.cancel.cancel();
    }
}

#[derive(Clone)]
pub struct AsyncPeerTx {
    shared: Arc<Shared>,
}

impl AsyncPeerTx {
    pub fn id(&self) -> &str {
        &self.shared.id
    }

    pub fn channels(&self) -> &[u16] {
        &self.shared.negotiated
    }

    pub fn supports_channel(&self, channel: u16) -> bool {
        self.shared.negotiated_set.contains(&channel)
    }

    pub async fn send(&self, channel: u16, payload: &[u8]) -> Result<()> {
        if channel != CONTROL && !self.supports_channel(channel) {
            return Err(PeerError::UnsupportedChannel(channel));
        }

        if payload.len() > DEFAULT_MAX_PAYLOAD {
            return Err(PeerError::Frame(FrameError::PayloadTooLarge {
                size: payload.len(),
                max: DEFAULT_MAX_PAYLOAD,
            }));
        }

        self.validate_send(channel, payload)?;

        let mut buf = BytesMut::new();
        encode_frame(channel, payload, &mut buf).map_err(PeerError::Frame)?;

        let mut writer = self.shared.writer.lock().await;
        writer
            .write_all(&buf)
            .await
            .map_err(|e| PeerError::Frame(FrameError::Io(e)))?;
        writer
            .flush()
            .await
            .map_err(|e| PeerError::Frame(FrameError::Io(e)))?;
        Ok(())
    }

    pub async fn send_json<T: serde::Serialize>(&self, channel: u16, value: &T) -> Result<()> {
        let payload = serde_json::to_vec(value)?;
        self.send(channel, &payload).await
    }

    pub async fn ping(&self) -> Result<Duration> {
        let (tx, rx) = oneshot::channel();
        let token = self.shared.waiter_token.fetch_add(1, Ordering::Relaxed);
        let _drop_guard = WaiterDropGuard {
            shared: Arc::clone(&self.shared),
            token,
            kind: WaiterKind::Ping,
        };
        {
            let mut guard = self.shared.ping_waiter.lock().await;
            if guard.is_some() {
                return Err(PeerError::Disconnected(
                    "ping already in flight".to_string(),
                ));
            }
            *guard = Some(InFlightWaiter { token, tx });
        }

        let start = Instant::now();
        if let Err(e) = self.send_json(CONTROL, &ControlMessage::ping()).await {
            let mut guard = self.shared.ping_waiter.lock().await;
            if guard.as_ref().is_some_and(|w| w.token == token) {
                let _ = guard.take();
            }
            return Err(e);
        }

        let res = tokio::time::timeout(self.shared.config.shutdown_timeout, rx).await;
        // Clear the in-flight waiter on all paths. If the peer responds after a timeout, we
        // intentionally drop the late PONG.
        {
            let mut guard = self.shared.ping_waiter.lock().await;
            if guard.as_ref().is_some_and(|w| w.token == token) {
                let _ = guard.take();
            }
        }
        match res {
            Ok(Ok(())) => Ok(start.elapsed()),
            Ok(Err(_)) => Err(PeerError::Disconnected("ping waiter dropped".to_string())),
            Err(_) => Err(PeerError::Timeout(self.shared.config.shutdown_timeout)),
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let token = self.shared.waiter_token.fetch_add(1, Ordering::Relaxed);
        let _drop_guard = WaiterDropGuard {
            shared: Arc::clone(&self.shared),
            token,
            kind: WaiterKind::Shutdown,
        };
        {
            let mut guard = self.shared.shutdown_waiter.lock().await;
            if guard.is_some() {
                return Err(PeerError::ShutdownFailed(
                    "shutdown already in flight".to_string(),
                ));
            }
            *guard = Some(InFlightWaiter { token, tx });
        }

        if let Err(e) = self
            .send_json(CONTROL, &ControlMessage::shutdown_request(None))
            .await
        {
            let mut guard = self.shared.shutdown_waiter.lock().await;
            if guard.as_ref().is_some_and(|w| w.token == token) {
                let _ = guard.take();
            }
            return Err(e);
        }

        let res = tokio::time::timeout(self.shared.config.shutdown_timeout, rx).await;
        // Clear the in-flight waiter on all paths. If the peer responds after a timeout, we
        // intentionally drop the late ACK.
        {
            let mut guard = self.shared.shutdown_waiter.lock().await;
            if guard.as_ref().is_some_and(|w| w.token == token) {
                let _ = guard.take();
            }
        }
        match res {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(PeerError::ShutdownFailed(
                "shutdown waiter dropped".to_string(),
            )),
            Err(_) => Err(PeerError::ShutdownFailed(
                "timed out waiting for shutdown acknowledgement".to_string(),
            )),
        }
    }

    #[cfg(feature = "schema")]
    fn validate_send(&self, channel: u16, payload: &[u8]) -> Result<()> {
        if let Some(registry) = &self.shared.schema_registry {
            registry.validate(channel, payload)?;
        }
        Ok(())
    }

    #[cfg(not(feature = "schema"))]
    fn validate_send(&self, _channel: u16, _payload: &[u8]) -> Result<()> {
        let _ = &self.shared.schema_registry;
        Ok(())
    }
}

#[cfg(feature = "schema")]
fn validate_recv(shared: &Shared, frame: &Frame) -> Result<()> {
    if let Some(registry) = &shared.schema_registry {
        registry.validate_frame(frame)?;
    }
    Ok(())
}

#[cfg(not(feature = "schema"))]
fn validate_recv(shared: &Shared, _frame: &Frame) -> Result<()> {
    let _ = &shared.schema_registry;
    Ok(())
}

pub struct AsyncPeerRx {
    shared: Arc<Shared>,
    any_rx: Option<mpsc::Receiver<QueuedFrame>>,
    subscriptions: Arc<Mutex<HashMap<u16, mpsc::Sender<QueuedFrame>>>>,
    disconnect: watch::Receiver<Option<PeerError>>,
}

impl AsyncPeerRx {
    pub fn id(&self) -> &str {
        &self.shared.id
    }

    pub fn channels(&self) -> &[u16] {
        &self.shared.negotiated
    }

    pub fn supports_channel(&self, channel: u16) -> bool {
        self.shared.negotiated_set.contains(&channel)
    }

    pub async fn recv(&mut self) -> Result<Frame> {
        let Some(any_rx) = &mut self.any_rx else {
            return Err(PeerError::Disconnected(
                "any receiver disabled; use a channel receiver".to_string(),
            ));
        };
        match any_rx.recv().await {
            Some(q) => Ok(q.frame),
            None => Err(disconnect_error(&self.disconnect)),
        }
    }

    /// Cancel the background reader task (local cancellation).
    pub fn cancel(&self) {
        self.shared.cancel.cancel();
    }

    /// Take ownership of the arrival-ordered receiver.
    pub fn take_any_receiver(&mut self) -> AnyReceiver {
        let rx = self.any_rx.take().unwrap_or_else(closed_receiver);
        AnyReceiver {
            _shared: Arc::clone(&self.shared),
            rx,
            disconnect: self.disconnect.clone(),
            done: false,
        }
    }

    /// Disable the arrival-ordered `any_rx` queue.
    ///
    /// Recommended for "channel-only" consumers. This drops the `any_rx` receiver which closes the
    /// channel; the reader task will then skip enqueueing to the global queue and cannot disconnect
    /// due to `any_rx` buffer pressure.
    ///
    /// If frames are already queued in `any_rx`, they will be dropped.
    pub fn disable_any_delivery(&mut self) {
        let _ = self.any_rx.take();
    }

    /// Subscribe to a negotiated channel and receive frames for that channel.
    ///
    /// Returns `None` if a receiver for this channel was already taken/subscribed.
    pub fn take_channel_receiver(&mut self, channel: u16) -> Option<ChannelReceiver> {
        if channel != CONTROL && !self.supports_channel(channel) {
            return None;
        }

        let mut subs = self.subscriptions.lock().ok()?;
        if subs.contains_key(&channel) {
            return None;
        }

        let cap = self.shared.config.max_buffer_per_channel.max(1);
        let (tx, rx) = mpsc::channel::<QueuedFrame>(cap);
        subs.insert(channel, tx);
        drop(subs);

        Some(ChannelReceiver {
            _shared: Arc::clone(&self.shared),
            channel,
            rx,
            disconnect: self.disconnect.clone(),
            subscriptions: Arc::clone(&self.subscriptions),
            done: false,
        })
    }
}

pub struct AsyncPeer {
    tx: AsyncPeerTx,
    rx: AsyncPeerRx,
    handshake: HandshakeResult,
}

impl AsyncPeer {
    pub fn id(&self) -> &str {
        self.tx.id()
    }

    pub fn channels(&self) -> &[u16] {
        self.tx.channels()
    }

    pub fn supports_channel(&self, channel: u16) -> bool {
        self.tx.supports_channel(channel)
    }

    pub fn handshake_result(&self) -> &HandshakeResult {
        &self.handshake
    }

    pub fn into_split(self) -> (AsyncPeerTx, AsyncPeerRx) {
        (self.tx, self.rx)
    }

    /// Cancel the background reader task (local cancellation).
    pub fn cancel(&self) {
        self.rx.cancel();
    }
}

pub(crate) fn build_async_peer_with_cancel(
    id: String,
    read_half: OwnedReadHalf,
    write_half: OwnedWriteHalf,
    handshake: HandshakeResult,
    schema_registry: Option<SchemaRegistryHandle>,
    config: PeerConfig,
    external_cancel: Option<CancellationToken>,
) -> AsyncPeer {
    let negotiated_set: HashSet<u16> = handshake.negotiated_channels.iter().copied().collect();

    let shared = Arc::new(Shared {
        id: id.clone(),
        negotiated: handshake.negotiated_channels.clone(),
        negotiated_set,
        writer: tokio::sync::Mutex::new(write_half),
        schema_registry,
        config: config.clone(),
        waiter_token: AtomicU64::new(1),
        ping_waiter: tokio::sync::Mutex::new(None),
        shutdown_waiter: tokio::sync::Mutex::new(None),
        cancel: CancellationToken::new(),
        external_cancel,
    });

    let (disconnect_tx, disconnect_rx) = watch::channel::<Option<PeerError>>(None);
    let cap = config.max_buffer_per_channel.max(1);
    let (any_tx, any_rx) = mpsc::channel::<QueuedFrame>(cap);
    let any_rx = if config.enable_any_delivery {
        Some(any_rx)
    } else {
        // Drop the receiver now so the reader task sees `any_tx.is_closed()` from the start.
        drop(any_rx);
        None
    };
    let subscriptions = Arc::new(Mutex::new(HashMap::new()));
    let budget = Arc::new(Semaphore::new(config.max_total_buffered_bytes));

    spawn_reader_task(
        ReaderTaskCtx {
            shared: Arc::downgrade(&shared),
            cancel: shared.cancel.clone(),
            external_cancel: shared.external_cancel.clone(),
            any_tx: any_tx.clone(),
            subscriptions: Arc::clone(&subscriptions),
            disconnect_tx,
            budget,
        },
        read_half,
    );

    AsyncPeer {
        tx: AsyncPeerTx {
            shared: Arc::clone(&shared),
        },
        rx: AsyncPeerRx {
            shared,
            any_rx,
            subscriptions,
            disconnect: disconnect_rx,
        },
        handshake,
    }
}

fn deliver_frame(
    shared: &Arc<Shared>,
    decoded: Frame,
    any_tx: &mpsc::Sender<QueuedFrame>,
    subscriptions: &Arc<Mutex<HashMap<u16, mpsc::Sender<QueuedFrame>>>>,
    disconnect_tx: &watch::Sender<Option<PeerError>>,
    budget: &Arc<Semaphore>,
) -> bool {
    let channel = decoded.channel;

    if channel != CONTROL && !shared.negotiated_set.contains(&channel) {
        let _ = disconnect_tx.send_replace(Some(PeerError::Disconnected(format!(
            "received frame on unnegotiated channel {}",
            channel
        ))));
        return false;
    }

    if let Err(err) = validate_recv(shared, &decoded) {
        let _ = disconnect_tx.send_replace(Some(err));
        return false;
    }

    let frame_bytes = decoded.payload.len().saturating_add(HEADER_SIZE);
    let permits: u32 = match frame_bytes.try_into() {
        Ok(v) => v,
        Err(_) => {
            let _ = disconnect_tx.send_replace(Some(PeerError::BufferFull(channel)));
            return false;
        }
    };

    let permit = match budget.clone().try_acquire_many_owned(permits) {
        Ok(p) => Arc::new(PermitHolder { _permit: p }),
        Err(_) => {
            let _ = disconnect_tx.send_replace(Some(PeerError::BufferFull(channel)));
            return false;
        }
    };

    let mut delivered = false;

    if !any_tx.is_closed() {
        match any_tx.try_send(QueuedFrame::new(decoded.clone(), Arc::clone(&permit))) {
            Ok(()) => delivered = true,
            Err(mpsc::error::TrySendError::Closed(_)) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                let _ = disconnect_tx.send_replace(Some(PeerError::BufferFull(channel)));
                return false;
            }
        }
    }

    if let Ok(subs) = subscriptions.lock() {
        if let Some(tx) = subs.get(&channel) {
            match tx.try_send(QueuedFrame::new(decoded, permit)) {
                Ok(()) => delivered = true,
                Err(mpsc::error::TrySendError::Closed(_)) => {}
                Err(mpsc::error::TrySendError::Full(_)) => {
                    let _ = disconnect_tx.send_replace(Some(PeerError::BufferFull(channel)));
                    return false;
                }
            }
        }
    }

    if !delivered {
        debug!(peer_id=%shared.id, "frame dropped (no active receivers)");
    }

    true
}

fn spawn_reader_task(ctx: ReaderTaskCtx, mut reader: OwnedReadHalf) {
    let ReaderTaskCtx {
        shared,
        cancel,
        external_cancel,
        any_tx,
        subscriptions,
        disconnect_tx,
        budget,
    } = ctx;

    let has_external = external_cancel.is_some();
    let external_token = external_cancel.unwrap_or_default();

    tokio::spawn(async move {
        let mut buf = BytesMut::with_capacity(8 * 1024);
        let mut chunk = [0u8; READ_CHUNK_SIZE];

        let mut control_frames_seen = 0usize;

        loop {
            let read_res = tokio::select! {
                _ = cancel.cancelled() => {
                    let _ = disconnect_tx.send_replace(Some(PeerError::Disconnected(
                        "cancelled".to_string(),
                    )));
                    break;
                }
                _ = external_token.cancelled(), if has_external => {
                    let _ = disconnect_tx.send_replace(Some(PeerError::Disconnected(
                        "cancelled".to_string(),
                    )));
                    break;
                }
                res = reader.read(&mut chunk) => res,
            };

            let n = match read_res {
                Ok(0) => {
                    let _ = disconnect_tx.send_replace(Some(PeerError::Disconnected(
                        "connection closed".to_string(),
                    )));
                    break;
                }
                Ok(n) => n,
                Err(e) => {
                    let _ = disconnect_tx.send_replace(Some(PeerError::Frame(FrameError::Io(e))));
                    break;
                }
            };
            buf.extend_from_slice(&chunk[..n]);

            loop {
                let decoded = match decode_frame(&mut buf, DEFAULT_MAX_PAYLOAD) {
                    Ok(Some(frame)) => frame,
                    Ok(None) => break,
                    Err(err) => {
                        let _ = disconnect_tx.send_replace(Some(PeerError::Frame(err)));
                        return;
                    }
                };

                if decoded.channel == CONTROL {
                    let Some(shared) = shared.upgrade() else {
                        // All peer handles are dropped; exit.
                        return;
                    };

                    control_frames_seen = control_frames_seen.saturating_add(1);
                    if control_frames_seen > shared.config.max_control_frames_per_loop {
                        let _ = disconnect_tx.send_replace(Some(PeerError::Disconnected(
                            "control frame flood detected".to_string(),
                        )));
                        return;
                    }

                    match handle_control(&shared, decoded, &disconnect_tx).await {
                        Ok(None) => continue,
                        Ok(Some(frame)) => {
                            // Unknown CONTROL messages are forwarded when allowed. Reset flood
                            // tracking because we are making progress delivering frames.
                            control_frames_seen = 0;
                            if !deliver_frame(
                                &shared,
                                frame,
                                &any_tx,
                                &subscriptions,
                                &disconnect_tx,
                                &budget,
                            ) {
                                return;
                            }
                            continue;
                        }
                        Err(err) => {
                            let _ = disconnect_tx.send_replace(Some(err));
                            return;
                        }
                    }
                }

                control_frames_seen = 0;

                let Some(shared) = shared.upgrade() else {
                    // All peer handles are dropped; exit.
                    return;
                };
                if !deliver_frame(
                    &shared,
                    decoded,
                    &any_tx,
                    &subscriptions,
                    &disconnect_tx,
                    &budget,
                ) {
                    return;
                }
            }
        }
    });
}

struct ReaderTaskCtx {
    shared: Weak<Shared>,
    cancel: CancellationToken,
    external_cancel: Option<CancellationToken>,
    any_tx: mpsc::Sender<QueuedFrame>,
    subscriptions: Arc<Mutex<HashMap<u16, mpsc::Sender<QueuedFrame>>>>,
    disconnect_tx: watch::Sender<Option<PeerError>>,
    budget: Arc<Semaphore>,
}

async fn handle_control(
    shared: &Arc<Shared>,
    frame: Frame,
    disconnect_tx: &watch::Sender<Option<PeerError>>,
) -> std::result::Result<Option<Frame>, PeerError> {
    let message = match serde_json::from_slice::<ControlMessage>(frame.payload.as_ref()) {
        Ok(m) => m,
        Err(_) => {
            return Err(PeerError::Disconnected(
                "invalid CONTROL JSON payload".to_string(),
            ));
        }
    };

    match message.msg_type.as_str() {
        CONTROL_PING => {
            send_control(shared, &ControlMessage::pong()).await?;
            Ok(None)
        }
        CONTROL_PONG => {
            if let Some(w) = shared.ping_waiter.lock().await.take() {
                let _ = w.tx.send(());
            }
            Ok(None)
        }
        CONTROL_SHUTDOWN_ACK => {
            if let Some(w) = shared.shutdown_waiter.lock().await.take() {
                let _ = w.tx.send(());
            }
            Ok(None)
        }
        CONTROL_SHUTDOWN_REQUEST => {
            send_control(shared, &ControlMessage::shutdown_ack()).await?;
            let msg = "shutdown requested".to_string();
            let _ = disconnect_tx.send_replace(Some(PeerError::Disconnected(msg.clone())));
            Err(PeerError::Disconnected(msg))
        }
        CONTROL_SHUTDOWN_FORCE => {
            if shared.config.allow_shutdown_force {
                let msg = "force shutdown".to_string();
                let _ = disconnect_tx.send_replace(Some(PeerError::Disconnected(msg.clone())));
                Err(PeerError::Disconnected(msg))
            } else {
                Err(PeerError::Disconnected(
                    "received disallowed SHUTDOWN_FORCE".to_string(),
                ))
            }
        }
        _ if shared.config.allow_unknown_control_messages => Ok(Some(frame)),
        _ => Err(PeerError::Disconnected(
            "unknown CONTROL message type".to_string(),
        )),
    }
}

async fn send_control(shared: &Arc<Shared>, message: &ControlMessage) -> Result<()> {
    let payload = serde_json::to_vec(message)?;
    let mut buf = BytesMut::new();
    encode_frame(CONTROL, &payload, &mut buf).map_err(PeerError::Frame)?;

    let mut writer = shared.writer.lock().await;
    writer
        .write_all(&buf)
        .await
        .map_err(|e| PeerError::Frame(FrameError::Io(e)))?;
    writer
        .flush()
        .await
        .map_err(|e| PeerError::Frame(FrameError::Io(e)))?;
    Ok(())
}

#[cfg(all(test, unix, feature = "async"))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    use ipcprims_frame::{COMMAND, DATA, ERROR, TELEMETRY};

    use crate::async_connector::async_connect;
    use crate::async_listener::AsyncPeerListener;

    static TEST_SOCK_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn test_sock_path() -> std::path::PathBuf {
        // Keep path short for macOS UDS length limits.
        let unique = TEST_SOCK_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::path::PathBuf::from(format!("/tmp/icpp-{}-{}.sock", std::process::id(), unique))
    }

    #[tokio::test]
    async fn any_rx_preserves_arrival_order() {
        let sock = test_sock_path();
        let listener = AsyncPeerListener::bind(&sock)
            .unwrap()
            .with_channels(&[1, 2]);
        let sock_client = sock.clone();

        let client_task =
            tokio::spawn(async move { async_connect(sock_client, &[1, 2]).await.unwrap() });
        let server = listener.accept_with_id("server").await.unwrap();
        let client = client_task.await.unwrap();

        let (client_tx, _client_rx) = client.into_split();
        let (_server_tx, mut server_rx) = server.into_split();

        client_tx.send(1, b"a1").await.unwrap();
        client_tx.send(2, b"b1").await.unwrap();
        client_tx.send(1, b"a2").await.unwrap();

        let f1 = server_rx.recv().await.unwrap();
        let f2 = server_rx.recv().await.unwrap();
        let f3 = server_rx.recv().await.unwrap();

        assert_eq!(f1.channel, 1);
        assert_eq!(f1.payload.as_ref(), b"a1");
        assert_eq!(f2.channel, 2);
        assert_eq!(f2.payload.as_ref(), b"b1");
        assert_eq!(f3.channel, 1);
        assert_eq!(f3.payload.as_ref(), b"a2");

        let _ = std::fs::remove_file(&sock);
    }

    #[tokio::test]
    async fn tee_semantics_duplicate_delivery() {
        let sock = test_sock_path();
        let listener = AsyncPeerListener::bind(&sock).unwrap().with_channels(&[1]);
        let sock_client = sock.clone();

        let client_task =
            tokio::spawn(async move { async_connect(sock_client, &[1]).await.unwrap() });
        let server = listener.accept_with_id("server").await.unwrap();
        let client = client_task.await.unwrap();

        let (client_tx, _client_rx) = client.into_split();
        let (_server_tx, mut server_rx) = server.into_split();

        let mut any = server_rx.take_any_receiver();
        let mut ch1 = server_rx
            .take_channel_receiver(1)
            .expect("channel receiver");

        client_tx.send(1, b"hello").await.unwrap();

        let f_any = any.recv().await.unwrap();
        let f_ch = ch1.recv().await.unwrap();

        assert_eq!(f_any.channel, 1);
        assert_eq!(f_any.payload.as_ref(), b"hello");
        assert_eq!(f_ch.channel, 1);
        assert_eq!(f_ch.payload.as_ref(), b"hello");

        let _ = std::fs::remove_file(&sock);
    }

    #[tokio::test]
    async fn allow_unknown_control_messages_forwards_control_frames() {
        let sock = test_sock_path();
        let cfg = PeerConfig {
            allow_unknown_control_messages: true,
            ..Default::default()
        };

        let listener = AsyncPeerListener::bind(&sock)
            .unwrap()
            .with_channels(&[1])
            .with_peer_config(cfg);
        let sock_client = sock.clone();

        let client_task =
            tokio::spawn(async move { async_connect(sock_client, &[1]).await.unwrap() });
        let server = listener.accept_with_id("server").await.unwrap();
        let client = client_task.await.unwrap();

        let (client_tx, _client_rx) = client.into_split();
        let (_server_tx, mut server_rx) = server.into_split();

        let msg = ControlMessage {
            msg_type: "custom".to_string(),
            payload: None,
            timestamp: None,
        };
        client_tx.send_json(CONTROL, &msg).await.unwrap();

        let frame = server_rx.recv().await.unwrap();
        assert_eq!(frame.channel, CONTROL);
        let roundtrip: ControlMessage = serde_json::from_slice(frame.payload.as_ref()).unwrap();
        assert_eq!(roundtrip.msg_type, "custom");

        let _ = std::fs::remove_file(&sock);
    }

    #[tokio::test]
    async fn buffer_full_disconnects_on_any_queue_overflow() {
        let sock = test_sock_path();
        let cfg = PeerConfig {
            max_buffer_per_channel: 1,
            ..Default::default()
        };

        let listener = AsyncPeerListener::bind(&sock)
            .unwrap()
            .with_channels(&[1])
            .with_peer_config(cfg);
        let sock_client = sock.clone();

        let client_task =
            tokio::spawn(async move { async_connect(sock_client, &[1]).await.unwrap() });
        let server = listener.accept_with_id("server").await.unwrap();
        let client = client_task.await.unwrap();

        let (client_tx, _client_rx) = client.into_split();
        let (_server_tx, mut server_rx) = server.into_split();
        let mut any = server_rx.take_any_receiver();

        client_tx.send(1, b"a").await.unwrap();
        client_tx.send(1, b"b").await.unwrap();

        let first = any.recv().await.unwrap();
        assert_eq!(first.payload.as_ref(), b"a");

        let err = any.recv().await.unwrap_err();
        assert!(matches!(err, PeerError::BufferFull(1)));

        let _ = std::fs::remove_file(&sock);
    }

    #[tokio::test]
    async fn disable_any_delivery_at_construction_prevents_any_queue_overflow_disconnect() {
        let sock = test_sock_path();
        let cfg = PeerConfig {
            // If `any_rx` were enabled and undrained, the second send would overflow the global
            // queue before the channel receiver sees the frame.
            max_buffer_per_channel: 1,
            enable_any_delivery: false,
            ..Default::default()
        };

        let listener = AsyncPeerListener::bind(&sock)
            .unwrap()
            .with_channels(&[1])
            .with_peer_config(cfg);
        let sock_client = sock.clone();

        let client_task =
            tokio::spawn(async move { async_connect(sock_client, &[1]).await.unwrap() });
        let server = listener.accept_with_id("server").await.unwrap();
        let client = client_task.await.unwrap();

        let (client_tx, _client_rx) = client.into_split();
        let (_server_tx, mut server_rx) = server.into_split();
        let mut ch1 = server_rx
            .take_channel_receiver(1)
            .expect("channel receiver");

        client_tx.send(1, b"a").await.unwrap();
        let f1 = ch1.recv().await.unwrap();
        assert_eq!(f1.payload.as_ref(), b"a");

        client_tx.send(1, b"b").await.unwrap();
        let f2 = ch1.recv().await.unwrap();
        assert_eq!(f2.payload.as_ref(), b"b");

        let _ = std::fs::remove_file(&sock);
    }

    #[tokio::test]
    async fn external_cancellation_disconnects_recv() {
        let sock = test_sock_path();
        let token = CancellationToken::new();

        let listener = AsyncPeerListener::bind(&sock)
            .unwrap()
            .with_channels(&[1])
            .with_cancellation_token(token.clone());

        let sock_client = sock.clone();
        let client_task =
            tokio::spawn(async move { async_connect(sock_client, &[1]).await.unwrap() });

        let server = listener.accept_with_id("server").await.unwrap();
        let _client = client_task.await.unwrap();
        let (_server_tx, mut server_rx) = server.into_split();

        token.cancel();

        let err = server_rx.recv().await.unwrap_err();
        assert!(matches!(err, PeerError::Disconnected(_)));

        let _ = std::fs::remove_file(&sock);
    }

    #[tokio::test]
    async fn ping_timeout_clears_in_flight_waiter() {
        let sock = test_sock_path();

        // Server cancels its reader task so it never auto-pongs.
        let listener = AsyncPeerListener::bind(&sock).unwrap();
        let server_task = tokio::spawn(async move {
            let peer = listener.accept().await.unwrap();
            peer.cancel();
            peer
        });

        let client_cfg = PeerConfig {
            shutdown_timeout: Duration::from_millis(25),
            ..PeerConfig::default()
        };
        let client = crate::async_connector::async_connect_with_config(
            &sock,
            &[COMMAND, DATA, TELEMETRY, ERROR],
            &crate::handshake::HandshakeConfig::default(),
            None,
            Some(client_cfg),
            None,
        )
        .await
        .unwrap();

        let (tx, _rx) = client.into_split();

        let _server = server_task.await.unwrap();

        let _ = tx.ping().await.unwrap_err();

        // Regression: an error/timeout used to leave the waiter stuck, causing "already in flight".
        let err2 = tx.ping().await.unwrap_err();
        assert!(!matches!(err2, PeerError::Disconnected(s) if s.contains("already in flight")));
    }

    #[tokio::test]
    async fn shutdown_timeout_clears_in_flight_waiter() {
        let sock = test_sock_path();

        // Server cancels its reader task so it never acknowledges shutdown.
        let listener = AsyncPeerListener::bind(&sock).unwrap();
        let server_task = tokio::spawn(async move {
            let peer = listener.accept().await.unwrap();
            peer.cancel();
            peer
        });

        let client_cfg = PeerConfig {
            shutdown_timeout: Duration::from_millis(25),
            ..PeerConfig::default()
        };
        let client = crate::async_connector::async_connect_with_config(
            &sock,
            &[COMMAND, DATA, TELEMETRY, ERROR],
            &crate::handshake::HandshakeConfig::default(),
            None,
            Some(client_cfg),
            None,
        )
        .await
        .unwrap();

        let (tx, _rx) = client.into_split();

        let _server = server_task.await.unwrap();

        let _ = tx.shutdown().await.unwrap_err();

        // Regression: an error/timeout used to leave the waiter stuck, causing "already in flight".
        let err2 = tx.shutdown().await.unwrap_err();
        assert!(!matches!(err2, PeerError::ShutdownFailed(s) if s.contains("already in flight")));
    }
}
