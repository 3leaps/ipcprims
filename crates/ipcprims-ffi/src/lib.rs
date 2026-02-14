//! ipcprims-ffi: C-ABI exports for ipcprims peer-level APIs.

mod error;
mod frame;
mod peer;
mod schema;
mod transport;
mod types;

use std::panic::AssertUnwindSafe;

pub use frame::ipc_frame_free;
pub use peer::{
    ipc_connect, ipc_listener_accept, ipc_listener_bind, ipc_listener_free, ipc_peer_free,
    ipc_peer_ping, ipc_peer_recv, ipc_peer_recv_on, ipc_peer_send, ipc_peer_shutdown,
};
pub use schema::{
    ipc_schema_registry_free, ipc_schema_registry_from_directory, ipc_schema_registry_validate,
};
pub use types::IpcSchemaRegistryHandle;
pub use types::{
    IpcFrame, IpcListenerHandle, IpcPeerHandle, IpcResult, IPC_CHANNEL_COMMAND,
    IPC_CHANNEL_CONTROL, IPC_CHANNEL_DATA, IPC_CHANNEL_ERROR, IPC_CHANNEL_TELEMETRY,
    IPC_ERR_BUFFER_FULL, IPC_ERR_DISCONNECTED, IPC_ERR_FRAME, IPC_ERR_HANDSHAKE_FAILED,
    IPC_ERR_INTERNAL, IPC_ERR_INVALID_ARGUMENT, IPC_ERR_SCHEMA, IPC_ERR_SHUTDOWN_FAILED,
    IPC_ERR_TIMEOUT, IPC_ERR_TRANSPORT, IPC_ERR_UNSUPPORTED_CHANNEL, IPC_OK,
};

fn ffi_boundary<T>(on_panic: T, f: impl FnOnce() -> T) -> T {
    match std::panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(_) => {
            error::set_panic_error();
            on_panic
        }
    }
}

#[no_mangle]
pub extern "C" fn ipc_init() -> IpcResult {
    ffi_boundary(IpcResult::Internal, || {
        error::clear_error_state();
        IpcResult::Ok
    })
}

#[no_mangle]
pub extern "C" fn ipc_cleanup() {
    ffi_boundary((), || {
        error::clear_error_state();
    });
}

#[no_mangle]
pub extern "C" fn ipc_last_error() -> *const std::os::raw::c_char {
    ffi_boundary(std::ptr::null(), error::last_error_ptr)
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    use super::*;

    #[test]
    fn init_and_cleanup_are_ok() {
        assert_eq!(ipc_init(), IpcResult::Ok);
        ipc_cleanup();
    }

    #[test]
    fn last_error_returns_non_null_pointer() {
        ipc_cleanup();
        let ptr = ipc_last_error();
        assert!(!ptr.is_null());

        // SAFETY: ipc_last_error returns a pointer to a thread-local CString.
        let text = unsafe { CStr::from_ptr(ptr).to_str().unwrap() };
        assert!(text.is_empty());
    }
}
