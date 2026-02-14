use crate::error;
#[cfg(feature = "schema")]
use crate::transport;
use crate::types::{IpcResult, IpcSchemaRegistryHandle};

/// Load schema registry from a directory.
///
/// # Safety
/// `path` must be a non-null pointer to a valid UTF-8, NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ipc_schema_registry_from_directory(
    path: *const std::os::raw::c_char,
) -> IpcSchemaRegistryHandle {
    crate::ffi_boundary(std::ptr::null_mut(), || {
        error::clear_error_state();

        #[cfg(feature = "schema")]
        {
            let path = {
                // SAFETY: We validate null and UTF-8 in helper.
                match unsafe { transport::required_str_arg(path, "path") } {
                    Some(v) => v,
                    None => return std::ptr::null_mut(),
                }
            };

            match ipcprims_schema::SchemaRegistry::from_directory(std::path::Path::new(path)) {
                Ok(registry) => {
                    let handle = crate::types::SchemaRegistryHandle { registry };
                    Box::into_raw(Box::new(handle)) as IpcSchemaRegistryHandle
                }
                Err(err) => {
                    let _ = error::map_schema_error(&err);
                    std::ptr::null_mut()
                }
            }
        }
        #[cfg(not(feature = "schema"))]
        {
            let _ = path;
            error::set_error_message("schema support is not enabled in this ipcprims-ffi build");
            std::ptr::null_mut()
        }
    })
}

/// Validate payload bytes for a channel against the loaded schema.
///
/// # Safety
/// `registry` must be a valid handle. If `len > 0`, `data` must be non-null and readable.
#[no_mangle]
pub unsafe extern "C" fn ipc_schema_registry_validate(
    registry: IpcSchemaRegistryHandle,
    channel: u16,
    data: *const u8,
    len: usize,
) -> IpcResult {
    crate::ffi_boundary(IpcResult::Internal, || {
        error::clear_error_state();

        #[cfg(feature = "schema")]
        {
            if registry.is_null() {
                return error::set_invalid_argument("registry handle cannot be null");
            }

            let payload = {
                // SAFETY: We validate pointer/length pairing in helper.
                match unsafe { transport::bytes_arg(data, len, "data") } {
                    Some(v) => v,
                    None => return IpcResult::InvalidArgument,
                }
            };

            let registry_handle = {
                // SAFETY: Caller guarantees this handle was allocated by ipc_schema_registry_from_directory.
                unsafe { &*(registry as *mut crate::types::SchemaRegistryHandle) }
            };

            match registry_handle.registry.validate(channel, payload) {
                Ok(()) => IpcResult::Ok,
                Err(err) => error::map_schema_error(&err),
            }
        }
        #[cfg(not(feature = "schema"))]
        {
            let _ = registry;
            let _ = channel;
            let _ = data;
            let _ = len;
            error::set_error_message("schema support is not enabled in this ipcprims-ffi build");
            IpcResult::Internal
        }
    })
}

/// Free a schema registry handle.
///
/// # Safety
/// `registry` must be null or a handle returned by `ipc_schema_registry_from_directory`.
#[no_mangle]
pub unsafe extern "C" fn ipc_schema_registry_free(registry: IpcSchemaRegistryHandle) {
    crate::ffi_boundary((), || {
        #[cfg(not(feature = "schema"))]
        {
            let _ = registry;
            return;
        }

        #[cfg(feature = "schema")]
        if registry.is_null() {
            return;
        }

        #[cfg(feature = "schema")]
        // SAFETY: Caller guarantees this handle was allocated by ipc_schema_registry_from_directory.
        unsafe {
            drop(Box::from_raw(
                registry as *mut crate::types::SchemaRegistryHandle,
            ));
        }
    });
}
