use std::sync::Mutex;

use napi::Result;
use napi_derive::napi;

use crate::error::{invalid_state, to_napi_error};
use crate::peer::Peer;

#[napi(object)]
pub struct ListenerOptions {
    pub channels: Option<Vec<u16>>,
    pub schema_dir: Option<String>,
}

#[napi]
pub struct Listener {
    inner: Mutex<Option<ipcprims_peer::PeerListener>>,
}

#[napi]
impl Listener {
    #[napi(factory)]
    pub fn bind(path: String, options: Option<ListenerOptions>) -> Result<Self> {
        let mut listener = ipcprims_peer::PeerListener::bind(&path)
            .map_err(|err| to_napi_error("listener bind failed", err))?;

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
            inner: Mutex::new(Some(listener)),
        })
    }

    #[napi]
    pub fn accept(&self) -> Result<Peer> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| invalid_state("listener lock poisoned"))?;

        let listener = guard
            .as_mut()
            .ok_or_else(|| invalid_state("listener is closed"))?;

        let peer = listener
            .accept()
            .map_err(|err| to_napi_error("listener accept failed", err))?;

        Ok(Peer::from_inner(peer))
    }

    #[napi]
    pub fn close(&self) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| invalid_state("listener lock poisoned"))?;
        let _ = guard.take();
        Ok(())
    }
}
