use std::sync::Mutex;

use napi::bindgen_prelude::Buffer;
use napi::Result;
use napi_derive::napi;

use crate::error::{invalid_state, to_napi_error};
use crate::frame::JsFrame;

#[napi]
pub struct Peer {
    inner: Mutex<Option<ipcprims_peer::Peer>>,
}

impl Peer {
    pub(crate) fn from_inner(peer: ipcprims_peer::Peer) -> Self {
        Self {
            inner: Mutex::new(Some(peer)),
        }
    }

    fn with_peer_mut<T>(&self, f: impl FnOnce(&mut ipcprims_peer::Peer) -> Result<T>) -> Result<T> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| invalid_state("peer lock poisoned"))?;
        let peer = guard
            .as_mut()
            .ok_or_else(|| invalid_state("peer is closed"))?;
        f(peer)
    }
}

#[napi]
impl Peer {
    #[napi(factory)]
    pub fn connect(path: String, channels: Vec<u16>) -> Result<Self> {
        let peer = ipcprims_peer::connect(&path, &channels)
            .map_err(|err| to_napi_error("connect failed", err))?;
        Ok(Self::from_inner(peer))
    }

    #[napi]
    pub fn send(&self, channel: u16, data: Buffer) -> Result<()> {
        self.with_peer_mut(|peer| {
            peer.send(channel, data.as_ref())
                .map_err(|err| to_napi_error("send failed", err))
        })
    }

    #[napi]
    pub fn recv(&self) -> Result<JsFrame> {
        self.with_peer_mut(|peer| {
            let frame = peer
                .recv()
                .map_err(|err| to_napi_error("recv failed", err))?;
            Ok(JsFrame {
                channel: frame.channel,
                payload: frame.payload.to_vec().into(),
            })
        })
    }

    #[napi]
    pub fn recv_on(&self, channel: u16) -> Result<JsFrame> {
        self.with_peer_mut(|peer| {
            let frame = peer
                .recv_on(channel)
                .map_err(|err| to_napi_error("recvOn failed", err))?;
            Ok(JsFrame {
                channel: frame.channel,
                payload: frame.payload.to_vec().into(),
            })
        })
    }

    #[napi]
    pub fn ping(&self) -> Result<u32> {
        self.with_peer_mut(|peer| {
            let rtt = peer
                .ping()
                .map_err(|err| to_napi_error("ping failed", err))?;
            let ms = rtt.as_millis();
            Ok(u32::try_from(ms).unwrap_or(u32::MAX))
        })
    }

    #[napi]
    pub fn shutdown(&self) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| invalid_state("peer lock poisoned"))?;
        let peer = guard
            .take()
            .ok_or_else(|| invalid_state("peer is closed"))?;
        peer.shutdown()
            .map_err(|err| to_napi_error("shutdown failed", err))
    }

    #[napi]
    pub fn close(&self) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| invalid_state("peer lock poisoned"))?;
        let _ = guard.take();
        Ok(())
    }
}
