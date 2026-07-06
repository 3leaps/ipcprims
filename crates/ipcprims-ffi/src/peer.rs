use crate::error;
use crate::transport;
use crate::types::{
    IpcAuthToken, IpcFrame, IpcListenerHandle, IpcPeerHandle, IpcResult, ListenerHandle, PeerHandle,
};
use zeroize::{Zeroize, Zeroizing};

fn with_peer_mut<T>(handle: IpcPeerHandle, on_error: T, f: impl FnOnce(&mut PeerHandle) -> T) -> T {
    if handle.is_null() {
        let _ = error::set_invalid_argument("peer handle cannot be null");
        return on_error;
    }

    let peer_handle = {
        // SAFETY: Pointer validity is guaranteed by the caller.
        unsafe { &mut *(handle as *mut PeerHandle) }
    };

    f(peer_handle)
}

fn with_listener<T>(
    handle: IpcListenerHandle,
    on_error: T,
    f: impl FnOnce(&ListenerHandle) -> T,
) -> T {
    if handle.is_null() {
        let _ = error::set_invalid_argument("listener handle cannot be null");
        return on_error;
    }

    let listener_handle = {
        // SAFETY: Pointer validity is guaranteed by the caller.
        unsafe { &*(handle as *mut ListenerHandle) }
    };

    f(listener_handle)
}

fn write_frame_out(out_frame: *mut IpcFrame, channel: u16, payload: &[u8]) -> IpcResult {
    if out_frame.is_null() {
        return error::set_invalid_argument("out_frame cannot be null");
    }

    let frame_ref = {
        // SAFETY: Pointer validity is guaranteed by the caller.
        unsafe { &mut *out_frame }
    };

    if !frame_ref.data.is_null() {
        let slice_ptr = std::ptr::slice_from_raw_parts_mut(frame_ref.data, frame_ref.len);
        // SAFETY: Existing payload pointers are allocated by this library.
        unsafe {
            drop(Box::from_raw(slice_ptr));
        }
        frame_ref.data = std::ptr::null_mut();
        frame_ref.len = 0;
    }

    let boxed_payload: Box<[u8]> = payload.to_vec().into_boxed_slice();
    let len = boxed_payload.len();
    let ptr = if len == 0 {
        std::ptr::null_mut()
    } else {
        Box::into_raw(boxed_payload) as *mut u8
    };

    frame_ref.channel = channel;
    frame_ref.data = ptr;
    frame_ref.len = len;

    IpcResult::Ok
}

unsafe fn auth_token_arg<'a>(data: *const u8, len: usize) -> Option<Option<&'a [u8]>> {
    if data.is_null() {
        if len == 0 {
            return Some(None);
        }
        let _ = error::set_invalid_argument("auth_token cannot be null when auth_token_len > 0");
        return None;
    }

    if len == 0 {
        let _ =
            error::set_invalid_argument("auth_token_len cannot be 0 when auth_token is non-null");
        return None;
    }

    if len > ipcprims_peer::handshake::MAX_AUTH_TOKEN_LEN {
        let _ = error::set_invalid_argument(format!(
            "auth_token_len exceeds maximum {}",
            ipcprims_peer::handshake::MAX_AUTH_TOKEN_LEN
        ));
        return None;
    }

    // SAFETY: Pointer and length are validated above and owned by caller for the call duration.
    Some(Some(unsafe { std::slice::from_raw_parts(data, len) }))
}

fn zeroize_auth_token_data(token: &mut IpcAuthToken) {
    if !token.data.is_null() {
        let slice_ptr = std::ptr::slice_from_raw_parts_mut(token.data, token.len);
        // SAFETY: Existing token pointers are allocated by this library.
        unsafe {
            let mut boxed = Box::from_raw(slice_ptr);
            boxed.zeroize();
        }
    }

    token.data = std::ptr::null_mut();
    token.len = 0;
    token.present = false;
}

fn write_auth_token_out(
    out_token: *mut IpcAuthToken,
    token: Option<Zeroizing<Vec<u8>>>,
) -> IpcResult {
    if out_token.is_null() {
        return error::set_invalid_argument("out_token cannot be null");
    }

    let token_ref = {
        // SAFETY: Pointer validity is guaranteed by the caller.
        unsafe { &mut *out_token }
    };

    token_ref.data = std::ptr::null_mut();
    token_ref.len = 0;
    token_ref.present = false;

    let Some(token) = token else {
        return IpcResult::Ok;
    };

    let boxed_token: Box<[u8]> = token.to_vec().into_boxed_slice();
    let len = boxed_token.len();
    let ptr = Box::into_raw(boxed_token) as *mut u8;

    token_ref.data = ptr;
    token_ref.len = len;
    token_ref.present = true;
    IpcResult::Ok
}

/// Bind a peer listener at `path`.
///
/// # Safety
/// `path` must be a non-null pointer to a valid UTF-8, NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ipc_listener_bind(path: *const std::os::raw::c_char) -> IpcListenerHandle {
    crate::ffi_boundary(std::ptr::null_mut(), || {
        error::clear_error_state();

        let path = {
            // SAFETY: We validate null and UTF-8 in helper.
            match unsafe { transport::required_str_arg(path, "path") } {
                Some(v) => v,
                None => return std::ptr::null_mut(),
            }
        };

        match ipcprims_peer::PeerListener::bind(path) {
            Ok(listener) => {
                let handle = ListenerHandle { listener };
                Box::into_raw(Box::new(handle)) as IpcListenerHandle
            }
            Err(err) => {
                let _ = error::map_peer_error(&err);
                std::ptr::null_mut()
            }
        }
    })
}

/// Accept an incoming peer connection.
///
/// # Safety
/// `listener` must be a valid listener handle returned by `ipc_listener_bind`.
#[no_mangle]
pub unsafe extern "C" fn ipc_listener_accept(listener: IpcListenerHandle) -> IpcPeerHandle {
    crate::ffi_boundary(std::ptr::null_mut(), || {
        error::clear_error_state();

        with_listener(
            listener,
            std::ptr::null_mut(),
            |listener_handle| match listener_handle.listener.accept() {
                Ok(peer) => {
                    let handle = PeerHandle { peer: Some(peer) };
                    Box::into_raw(Box::new(handle)) as IpcPeerHandle
                }
                Err(err) => {
                    let _ = error::map_peer_error(&err);
                    std::ptr::null_mut()
                }
            },
        )
    })
}

/// Free a listener handle.
///
/// # Safety
/// `listener` must be null or a handle previously returned by `ipc_listener_bind`.
#[no_mangle]
pub unsafe extern "C" fn ipc_listener_free(listener: IpcListenerHandle) {
    crate::ffi_boundary((), || {
        if listener.is_null() {
            return;
        }

        // SAFETY: Caller guarantees this handle was allocated by ipc_listener_bind.
        unsafe {
            drop(Box::from_raw(listener as *mut ListenerHandle));
        }
    });
}

/// Connect to a listener path with an optional channel list.
///
/// # Safety
/// `path` must be a non-null UTF-8 C string. If `num_channels > 0`, `channels` must be non-null
/// and point to `num_channels` readable `uint16_t` values.
#[no_mangle]
pub unsafe extern "C" fn ipc_connect(
    path: *const std::os::raw::c_char,
    channels: *const u16,
    num_channels: usize,
) -> IpcPeerHandle {
    crate::ffi_boundary(std::ptr::null_mut(), || {
        error::clear_error_state();

        let path = {
            // SAFETY: We validate null and UTF-8 in helper.
            match unsafe { transport::required_str_arg(path, "path") } {
                Some(v) => v,
                None => return std::ptr::null_mut(),
            }
        };

        let channels = {
            // SAFETY: We validate pointer/length pairing in helper.
            match unsafe { transport::channels_arg(channels, num_channels) } {
                Some(v) => v,
                None => return std::ptr::null_mut(),
            }
        };

        match ipcprims_peer::connect(path, channels) {
            Ok(peer) => {
                let handle = PeerHandle { peer: Some(peer) };
                Box::into_raw(Box::new(handle)) as IpcPeerHandle
            }
            Err(err) => {
                let _ = error::map_peer_error(&err);
                std::ptr::null_mut()
            }
        }
    })
}

/// Connect to a listener path with an optional opaque auth token.
///
/// # Safety
/// `path` must be a non-null UTF-8 C string. If `num_channels > 0`, `channels` must be non-null
/// and point to `num_channels` readable `uint16_t` values. `auth_token == NULL` with
/// `auth_token_len == 0` means no token; non-null token data must be readable for
/// `auth_token_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ipc_connect_with_auth(
    path: *const std::os::raw::c_char,
    channels: *const u16,
    num_channels: usize,
    auth_token: *const u8,
    auth_token_len: usize,
) -> IpcPeerHandle {
    crate::ffi_boundary(std::ptr::null_mut(), || {
        error::clear_error_state();

        let path = {
            // SAFETY: We validate null and UTF-8 in helper.
            match unsafe { transport::required_str_arg(path, "path") } {
                Some(v) => v,
                None => return std::ptr::null_mut(),
            }
        };

        let channels = {
            // SAFETY: We validate pointer/length pairing in helper.
            match unsafe { transport::channels_arg(channels, num_channels) } {
                Some(v) => v,
                None => return std::ptr::null_mut(),
            }
        };

        let token = {
            // SAFETY: We validate pointer/length pairing and auth-specific empty-token rules.
            match unsafe { auth_token_arg(auth_token, auth_token_len) } {
                Some(v) => v,
                None => return std::ptr::null_mut(),
            }
        };

        let mut handshake_config = ipcprims_peer::HandshakeConfig::default();
        if let Some(token) = token {
            handshake_config.auth_token = Some(Zeroizing::new(token.to_vec()));
        }

        match ipcprims_peer::connect_with_config(path, channels, &handshake_config, None, None) {
            Ok(peer) => {
                let handle = PeerHandle { peer: Some(peer) };
                Box::into_raw(Box::new(handle)) as IpcPeerHandle
            }
            Err(err) => {
                let _ = error::map_peer_error(&err);
                std::ptr::null_mut()
            }
        }
    })
}

/// Send payload bytes on a negotiated channel.
///
/// # Safety
/// `peer` must be a valid peer handle. If `len > 0`, `data` must be non-null and readable for
/// `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ipc_peer_send(
    peer: IpcPeerHandle,
    channel: u16,
    data: *const u8,
    len: usize,
) -> IpcResult {
    crate::ffi_boundary(IpcResult::Internal, || {
        error::clear_error_state();

        let payload = {
            // SAFETY: We validate pointer/length pairing in helper.
            match unsafe { transport::bytes_arg(data, len, "data") } {
                Some(v) => v,
                None => return IpcResult::InvalidArgument,
            }
        };

        with_peer_mut(peer, IpcResult::InvalidArgument, |peer_handle| {
            let peer = match peer_handle.peer.as_mut() {
                Some(peer) => peer,
                None => return error::set_invalid_argument("peer handle has been closed"),
            };

            match peer.send(channel, payload) {
                Ok(()) => IpcResult::Ok,
                Err(err) => error::map_peer_error(&err),
            }
        })
    })
}

/// Receive the next non-control frame.
///
/// # Safety
/// `peer` must be a valid peer handle and `out_frame` must be a valid writable pointer.
/// If `out_frame->data` already contains a prior payload from this library, it is freed first.
#[no_mangle]
pub unsafe extern "C" fn ipc_peer_recv(peer: IpcPeerHandle, out_frame: *mut IpcFrame) -> IpcResult {
    crate::ffi_boundary(IpcResult::Internal, || {
        error::clear_error_state();

        with_peer_mut(peer, IpcResult::InvalidArgument, |peer_handle| {
            let peer = match peer_handle.peer.as_mut() {
                Some(peer) => peer,
                None => return error::set_invalid_argument("peer handle has been closed"),
            };

            match peer.recv() {
                Ok(frame) => write_frame_out(out_frame, frame.channel, frame.payload.as_ref()),
                Err(err) => error::map_peer_error(&err),
            }
        })
    })
}

/// Receive the next frame on a specific channel.
///
/// # Safety
/// `peer` must be a valid peer handle and `out_frame` must be a valid writable pointer.
/// If `out_frame->data` already contains a prior payload from this library, it is freed first.
#[no_mangle]
pub unsafe extern "C" fn ipc_peer_recv_on(
    peer: IpcPeerHandle,
    channel: u16,
    out_frame: *mut IpcFrame,
) -> IpcResult {
    crate::ffi_boundary(IpcResult::Internal, || {
        error::clear_error_state();

        with_peer_mut(peer, IpcResult::InvalidArgument, |peer_handle| {
            let peer = match peer_handle.peer.as_mut() {
                Some(peer) => peer,
                None => return error::set_invalid_argument("peer handle has been closed"),
            };

            match peer.recv_on(channel) {
                Ok(frame) => write_frame_out(out_frame, frame.channel, frame.payload.as_ref()),
                Err(err) => error::map_peer_error(&err),
            }
        })
    })
}

/// Send a control ping and return round-trip time in nanoseconds.
///
/// # Safety
/// `peer` must be a valid peer handle and `out_rtt_ns` must be a non-null writable pointer.
#[no_mangle]
pub unsafe extern "C" fn ipc_peer_ping(peer: IpcPeerHandle, out_rtt_ns: *mut u64) -> IpcResult {
    crate::ffi_boundary(IpcResult::Internal, || {
        error::clear_error_state();

        if out_rtt_ns.is_null() {
            return error::set_invalid_argument("out_rtt_ns cannot be null");
        }

        with_peer_mut(peer, IpcResult::InvalidArgument, |peer_handle| {
            let peer = match peer_handle.peer.as_mut() {
                Some(peer) => peer,
                None => return error::set_invalid_argument("peer handle has been closed"),
            };

            match peer.ping() {
                Ok(rtt) => {
                    let nanos = rtt.as_nanos();
                    let rtt_ns = u64::try_from(nanos).unwrap_or(u64::MAX);

                    // SAFETY: Pointer was checked for null above.
                    unsafe {
                        *out_rtt_ns = rtt_ns;
                    }
                    IpcResult::Ok
                }
                Err(err) => error::map_peer_error(&err),
            }
        })
    })
}

/// Gracefully shutdown a peer connection.
///
/// # Safety
/// `peer` must be a valid peer handle.
#[no_mangle]
pub unsafe extern "C" fn ipc_peer_shutdown(peer: IpcPeerHandle) -> IpcResult {
    crate::ffi_boundary(IpcResult::Internal, || {
        error::clear_error_state();

        with_peer_mut(peer, IpcResult::InvalidArgument, |peer_handle| {
            let peer = match peer_handle.peer.take() {
                Some(peer) => peer,
                None => return error::set_invalid_argument("peer handle has been closed"),
            };

            match peer.shutdown() {
                Ok(()) => IpcResult::Ok,
                Err(err) => error::map_peer_error(&err),
            }
        })
    })
}

/// Take and clear the client auth token observed during handshake.
///
/// # Safety
/// `peer` must be a valid peer handle and `out_token` must be a valid writable pointer.
/// `out_token` is overwritten without reading its previous contents. If reusing an `IpcAuthToken`
/// that already owns token data from this library, call `ipc_auth_token_free` before passing it
/// here or that prior allocation will be leaked. Returned token data must be released with
/// `ipc_auth_token_free`.
#[no_mangle]
pub unsafe extern "C" fn ipc_peer_take_client_auth_token(
    peer: IpcPeerHandle,
    out_token: *mut IpcAuthToken,
) -> IpcResult {
    crate::ffi_boundary(IpcResult::Internal, || {
        error::clear_error_state();

        with_peer_mut(peer, IpcResult::InvalidArgument, |peer_handle| {
            let peer = match peer_handle.peer.as_mut() {
                Some(peer) => peer,
                None => return error::set_invalid_argument("peer handle has been closed"),
            };

            write_auth_token_out(out_token, peer.take_client_auth_token())
        })
    })
}

/// Zeroize and free token memory returned by `ipc_peer_take_client_auth_token`.
///
/// # Safety
/// `token` must be null or a valid pointer to an `IpcAuthToken` created by caller code.
/// If `token->data` is non-null, it must have originated from this library.
#[no_mangle]
pub unsafe extern "C" fn ipc_auth_token_free(token: *mut IpcAuthToken) {
    crate::ffi_boundary((), || {
        if token.is_null() {
            return;
        }

        let token_ref = {
            // SAFETY: Pointer validity is guaranteed by the caller.
            unsafe { &mut *token }
        };
        zeroize_auth_token_data(token_ref);
    });
}

/// Free a peer handle.
///
/// # Safety
/// `peer` must be null or a handle returned by `ipc_connect` or `ipc_listener_accept`.
#[no_mangle]
pub unsafe extern "C" fn ipc_peer_free(peer: IpcPeerHandle) {
    crate::ffi_boundary((), || {
        if peer.is_null() {
            return;
        }

        // SAFETY: Caller guarantees this handle was allocated by ipc_connect/ipc_listener_accept.
        unsafe {
            drop(Box::from_raw(peer as *mut PeerHandle));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_frame_out_populates_output() {
        let mut frame = IpcFrame::default();
        let result = write_frame_out(&mut frame as *mut IpcFrame, 7, b"abc");
        assert_eq!(result, IpcResult::Ok);
        assert_eq!(frame.channel, 7);
        assert_eq!(frame.len, 3);
        assert!(!frame.data.is_null());

        // SAFETY: `frame` was populated by `write_frame_out`.
        unsafe { crate::ipc_frame_free(&mut frame as *mut IpcFrame) };
    }

    #[test]
    fn write_frame_out_reuse_is_safe() {
        let mut frame = IpcFrame::default();
        assert_eq!(
            write_frame_out(&mut frame as *mut IpcFrame, 1, b"old"),
            IpcResult::Ok
        );
        assert_eq!(
            write_frame_out(&mut frame as *mut IpcFrame, 2, b"newer"),
            IpcResult::Ok
        );
        assert_eq!(frame.channel, 2);
        assert_eq!(frame.len, 5);

        // SAFETY: `frame` was populated by `write_frame_out`.
        unsafe { crate::ipc_frame_free(&mut frame as *mut IpcFrame) };
    }

    #[cfg(unix)]
    #[test]
    fn ffi_auth_token_take_reports_present_then_absent() {
        use std::ffi::CString;
        use std::thread;
        use std::time::{SystemTime, UNIX_EPOCH};

        let dir = std::env::temp_dir().join(format!(
            "ipcp-ffi-auth-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir should be creatable");
        let sock_path = dir.join("listener.sock");
        let c_path = CString::new(sock_path.to_string_lossy().as_bytes())
            .expect("socket path should not contain NUL");

        let listener = unsafe { ipc_listener_bind(c_path.as_ptr()) };
        assert!(!listener.is_null());

        let listener_addr = listener as usize;
        let server = thread::spawn(move || {
            let listener = listener_addr as IpcListenerHandle;
            let peer = unsafe { ipc_listener_accept(listener) };
            assert!(!peer.is_null());

            let mut token = std::mem::MaybeUninit::<IpcAuthToken>::uninit();
            let result = unsafe { ipc_peer_take_client_auth_token(peer, token.as_mut_ptr()) };
            assert_eq!(result, IpcResult::Ok);
            let mut token = unsafe { token.assume_init() };
            assert!(token.present);
            assert_eq!(token.len, 7);
            let bytes = unsafe { std::slice::from_raw_parts(token.data, token.len) };
            assert_eq!(bytes, b"abc\0xyz");
            unsafe { ipc_auth_token_free(&mut token as *mut IpcAuthToken) };
            assert!(!token.present);
            assert!(token.data.is_null());
            assert_eq!(token.len, 0);

            let mut token = std::mem::MaybeUninit::<IpcAuthToken>::uninit();
            let result = unsafe { ipc_peer_take_client_auth_token(peer, token.as_mut_ptr()) };
            assert_eq!(result, IpcResult::Ok);
            let token = unsafe { token.assume_init() };
            assert!(!token.present);
            assert!(token.data.is_null());
            assert_eq!(token.len, 0);

            unsafe { ipc_peer_free(peer) };
        });

        let auth_token = b"abc\0xyz";
        let channels = [ipcprims_frame::COMMAND];
        let peer = unsafe {
            ipc_connect_with_auth(
                c_path.as_ptr(),
                channels.as_ptr(),
                channels.len(),
                auth_token.as_ptr(),
                auth_token.len(),
            )
        };
        assert!(!peer.is_null());
        unsafe { ipc_peer_free(peer) };

        server.join().expect("server thread should finish");
        unsafe { ipc_listener_free(listener) };
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn auth_token_arg_rejects_empty_present_token() {
        let byte = 0u8;
        let token = unsafe { auth_token_arg(&byte as *const u8, 0) };
        assert!(token.is_none());
    }
}
