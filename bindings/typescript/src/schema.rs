use std::sync::Mutex;

use napi::bindgen_prelude::Buffer;
use napi::Result;
use napi_derive::napi;

use crate::error::{invalid_state, to_napi_error};

#[napi]
pub struct SchemaRegistry {
    inner: Mutex<Option<ipcprims_schema::SchemaRegistry>>,
}

#[napi]
impl SchemaRegistry {
    #[napi(factory)]
    pub fn from_directory(path: String) -> Result<Self> {
        let registry = ipcprims_schema::SchemaRegistry::from_directory(path.as_ref())
            .map_err(|err| to_napi_error("schema registry load failed", err))?;

        Ok(Self {
            inner: Mutex::new(Some(registry)),
        })
    }

    #[napi]
    pub fn validate(&self, channel: u16, data: Buffer) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| invalid_state("schema registry lock poisoned"))?;

        let registry = guard
            .as_mut()
            .ok_or_else(|| invalid_state("schema registry is closed"))?;

        registry
            .validate(channel, data.as_ref())
            .map_err(|err| to_napi_error("schema validation failed", err))
    }

    #[napi]
    pub fn close(&self) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| invalid_state("schema registry lock poisoned"))?;
        let _ = guard.take();
        Ok(())
    }
}
