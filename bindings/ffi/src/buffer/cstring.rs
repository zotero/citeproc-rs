//! An implementation of BufferOps using Rust's [std::ffi::CString].
//!
//! Usage from C:
//!
//! ```ignore,c
//! #define BUF_OPS citeproc_rs_cstring_buffer_ops
//!
//! char *buffer;
//! citeproc_rs_error_code code;
//!
//! /* For example. Note the &buffer. */
//! code = citeproc_rs_last_error_utf8(BUF_OPS, &buffer)
//!
//! /* The resulting string is null terminated, has no nulls inside it, and is valid utf8. */
//! printf("%s\n", buffer);
//!
//! /* You can reuse the buffer, it will be cleared and then grow if necessary */
//! /* ... initialize a driver with { .buffer_ops = BUF_OPS } */
//! code = citeproc_rs_driver_format_cluster(..., &buffer);
//!
//! // You must free it when you're done.
//! citeproc_rs_cstring_free(buffer);
//! ```

use libc::{c_char, c_void};
use std::ffi::CString;

use super::BufferOps;

/// If you use this as your buffer_write_callback, then you must call [citeproc_rs_cstring_free] on
/// the resulting buffers, or the memory will leak.
///
pub const CSTRING_BUFFER_OPS: BufferOps = BufferOps {
    write: citeproc_rs_cstring_write,
    clear: citeproc_rs_cstring_clear,
};

ffi_fn! {
    /// Frees a FFI-consumer-owned CString written using [CSTRING_BUFFER_OPS].
    fn citeproc_rs_cstring_free(ptr: *mut c_char) {
        if !ptr.is_null() {
            drop(unsafe { CString::from_raw(ptr) });
        }
    }
}

/// Provides BufferOps.write for the CString implementation.
///
/// ## Safety
///
/// Only safe to call with a `user_data` that is a **valid pointer to a pointer**. The inner
/// pointer should be either
///
/// * `NULL`; or
/// * a pointer returned from `CString::into_raw`.
///
/// The src/src_len must represent a valid &[u8] structure.
#[no_mangle]
#[allow(unused_unsafe)]
pub unsafe extern "C" fn citeproc_rs_cstring_write(
    user_data: *mut c_void,
    src: *const u8,
    src_len: usize,
) {
    // user_data shouldn't be null but we will fail gracefully.
    if user_data.is_null() || src.is_null() {
        return;
    }
    let user_data_ptr = user_data.cast::<*mut c_char>();
    // SAFETY: we checked it for null.
    let user_data_contents = unsafe { *user_data_ptr };
    let cstring = if user_data_contents.is_null() {
        None
    } else {
        Some(CString::from_raw(user_data_contents))
    };

    // SAFETY: This is safe as long as we call this with a decomposition of a real Rust slice.
    let bytes = unsafe { core::slice::from_raw_parts(src, src_len) };

    let mut vec = cstring.map_or_else(Vec::new, |x| x.into_bytes());
    // don't do two resizes, just one please
    vec.reserve_exact(bytes.len() + 1);
    vec.extend_from_slice(bytes);

    // and convert back.
    // the unchecked part refers to whether it contain any interior zero bytes.
    // SAFETY: we check in `copy_to_user` that there are no zeroes each time we write.
    // this method does push a 0 on the end, unconditionally.
    let cstring = unsafe { CString::from_vec_unchecked(vec) };

    // We have to write out the CString again as it may have reallocated itself.
    // SAFETY: same as the deref in user_data_contents
    unsafe {
        core::ptr::write(user_data_ptr, cstring.into_raw());
    }
}

/// Provides BufferOps.clear for the CString implementation.
///
/// ## Safety
#[no_mangle]
#[allow(unused_unsafe)]
pub unsafe extern "C" fn citeproc_rs_cstring_clear(user_data: *mut c_void) {
    // user_data shouldn't be null but we will fail gracefully.
    if user_data.is_null() {
        return;
    }
    let user_data_ptr = user_data.cast::<*mut c_char>();
    // SAFETY: we checked it for null.
    let user_data_contents = unsafe { *user_data_ptr };
    // *user_data could be null as well. first time use.
    let cstring = if user_data_contents.is_null() {
        None
    } else {
        Some(CString::from_raw(user_data_contents))
    };

    // Clear it.
    // You may be thinking, if CString reads itself from a raw pointer and gets its length using
    // strlen, then how does preserving this memory help us at all? Won't it come back with zero
    // length?
    //
    // Answer -- CString::into_raw uses Box::<[T]>::into_raw, which is a _fat pointer_.
    // It stores a capacity value alongside the pointer somewhere, and recovers it in
    // Box/CString::from_raw.
    let mut vec = cstring.map_or_else(Vec::new, |x| x.into_bytes());
    vec.clear();
    // SAFETY: no internal zeroes, because it's empty.
    let cstring = unsafe { CString::from_vec_unchecked(vec) };

    // Write null to it.
    // SAFETY: we checked user_data for null before.
    unsafe {
        core::ptr::write(user_data_ptr, cstring.into_raw());
    }
}
