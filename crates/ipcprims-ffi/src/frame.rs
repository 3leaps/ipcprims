use std::ptr;

use crate::types::IpcFrame;

/// Free payload memory held by an [`IpcFrame`] populated by recv APIs.
///
/// # Safety
/// `frame` must be either null or a valid pointer to an `IpcFrame` created by caller code.
/// If `frame->data` is non-null, it must have originated from this library.
#[no_mangle]
pub unsafe extern "C" fn ipc_frame_free(frame: *mut IpcFrame) {
    crate::ffi_boundary((), || {
        if frame.is_null() {
            return;
        }

        let frame_ref = {
            // SAFETY: Pointer validity is guaranteed by the caller.
            unsafe { &mut *frame }
        };

        if !frame_ref.data.is_null() {
            let slice_ptr = ptr::slice_from_raw_parts_mut(frame_ref.data, frame_ref.len);
            // SAFETY: `data` was allocated by `Box<[u8]>` in FFI receive functions.
            unsafe {
                drop(Box::from_raw(slice_ptr));
            }
        }

        *frame_ref = IpcFrame::default();
    });
}
