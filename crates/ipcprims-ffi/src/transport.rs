use std::ffi::CStr;
use std::os::raw::c_char;

use crate::error;

/// Convert a required C string argument into UTF-8 `&str`.
///
/// # Safety
/// `value` must be null or point to a valid NUL-terminated C string.
pub(crate) unsafe fn required_str_arg<'a>(value: *const c_char, name: &str) -> Option<&'a str> {
    if value.is_null() {
        let _ = error::set_invalid_argument(format!("{name} cannot be null"));
        return None;
    }

    let as_cstr = {
        // SAFETY: The caller guarantees `value` points to a valid NUL-terminated C string.
        unsafe { CStr::from_ptr(value) }
    };

    match as_cstr.to_str() {
        Ok(v) => Some(v),
        Err(_) => {
            let _ = error::set_invalid_argument(format!("{name} must be valid UTF-8"));
            None
        }
    }
}

/// Convert an optional channels pointer + length into a slice.
///
/// # Safety
/// If `num_channels > 0`, `channels` must be non-null and readable for that many elements.
pub(crate) unsafe fn channels_arg<'a>(
    channels: *const u16,
    num_channels: usize,
) -> Option<&'a [u16]> {
    if num_channels == 0 {
        return Some(&[]);
    }
    if channels.is_null() {
        let _ = error::set_invalid_argument("channels cannot be null when num_channels > 0");
        return None;
    }

    // SAFETY: Pointer and length are validated above and owned by caller for the call duration.
    Some(unsafe { std::slice::from_raw_parts(channels, num_channels) })
}

/// Convert an optional byte pointer + length into a slice.
///
/// # Safety
/// If `len > 0`, `data` must be non-null and readable for `len` bytes.
pub(crate) unsafe fn bytes_arg<'a>(data: *const u8, len: usize, name: &str) -> Option<&'a [u8]> {
    if len == 0 {
        return Some(&[]);
    }
    if data.is_null() {
        let _ = error::set_invalid_argument(format!("{name} cannot be null when len > 0"));
        return None;
    }

    // SAFETY: Pointer and length are validated above and owned by caller for the call duration.
    Some(unsafe { std::slice::from_raw_parts(data, len) })
}
