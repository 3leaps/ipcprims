use std::cell::RefCell;
use std::ffi::CString;
use std::os::raw::c_char;

use ipcprims_peer::PeerError;

use crate::types::IpcResult;

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::new("").expect("empty CString should be valid"));
}

pub(crate) fn clear_error_state() {
    LAST_ERROR.with(|state| {
        *state.borrow_mut() = CString::new("").expect("empty CString should be valid");
    });
}

pub(crate) fn set_error_message(message: impl Into<String>) {
    let message = message.into();
    let sanitized = message.replace('\0', "?");
    LAST_ERROR.with(|state| {
        *state.borrow_mut() = CString::new(sanitized)
            .unwrap_or_else(|_| CString::new("internal error").expect("literal is valid"));
    });
}

pub(crate) fn set_invalid_argument(message: impl Into<String>) -> IpcResult {
    set_error_message(message);
    IpcResult::InvalidArgument
}

pub(crate) fn set_panic_error() {
    set_error_message("panic across FFI boundary");
}

pub(crate) fn map_peer_error(err: &PeerError) -> IpcResult {
    set_error_message(err.to_string());
    match err {
        PeerError::Transport(_) => IpcResult::TransportError,
        PeerError::Frame(_) => IpcResult::FrameError,
        PeerError::HandshakeFailed(_) => IpcResult::HandshakeFailed,
        PeerError::Disconnected(_) => IpcResult::Disconnected,
        PeerError::UnsupportedChannel(_) => IpcResult::UnsupportedChannel,
        PeerError::BufferFull(_) => IpcResult::BufferFull,
        PeerError::Json(_) => IpcResult::InvalidArgument,
        #[cfg(feature = "schema")]
        PeerError::Schema(_) => IpcResult::SchemaError,
        PeerError::Timeout(_) => IpcResult::Timeout,
        PeerError::ShutdownFailed(_) => IpcResult::ShutdownFailed,
    }
}

#[cfg(feature = "schema")]
pub(crate) fn map_schema_error(err: &ipcprims_schema::SchemaError) -> IpcResult {
    set_error_message(err.to_string());
    IpcResult::SchemaError
}

pub(crate) fn last_error_ptr() -> *const c_char {
    LAST_ERROR.with(|state| state.borrow().as_ptr())
}
