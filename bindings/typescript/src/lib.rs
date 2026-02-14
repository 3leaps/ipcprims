mod error;
mod frame;
mod listener;
mod peer;
mod schema;

pub use listener::{Listener, ListenerOptions};
pub use peer::Peer;
pub use schema::SchemaRegistry;

#[napi_derive::napi]
pub fn control() -> u16 {
    ipcprims_frame::CONTROL
}

#[napi_derive::napi]
pub fn command() -> u16 {
    ipcprims_frame::COMMAND
}

#[napi_derive::napi]
pub fn data() -> u16 {
    ipcprims_frame::DATA
}

#[napi_derive::napi]
pub fn telemetry() -> u16 {
    ipcprims_frame::TELEMETRY
}

#[napi_derive::napi]
pub fn error() -> u16 {
    ipcprims_frame::ERROR
}
