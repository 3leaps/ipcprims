use crate::error;
use crate::transport;
use crate::types::{
    IpcFrame, IpcListenerHandle, IpcPeerHandle, IpcResult, ListenerHandle, PeerHandle,
};

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
}
