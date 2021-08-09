//! Functions for getting and setting the `LAST_ERROR`, a thread-local error slot.
//!
//! FFI consumers can use the extern "C" functions here to access this error,
//!
//! from crates.io/crates/ffi_helpers, with a single FFIError type instead

use crate::nullable::Nullable;
use libc::c_void;
use std::{cell::RefCell, slice};

use crate::buffer;
use crate::{ErrorCode, FFIError};

thread_local! {
    static LAST_ERROR: RefCell<Option<FFIError>> = RefCell::new(None);
}

/// Clear the last error (thread local).
#[no_mangle]
pub extern "C" fn citeproc_rs_last_error_clear() {
    let _ = take_last_error();
}

/// Take the most recent error, clearing `LAST_ERROR` in the process.
pub(crate) fn take_last_error() -> Option<FFIError> {
    LAST_ERROR.with(|prev| prev.borrow_mut().take())
}

/// Update the `thread_local` error, taking ownership of the `Error`.
pub(crate) fn clear_last_error() {
    let _ = take_last_error();
}

/// Update the `thread_local` error, taking ownership of the `Error`.
#[allow(dead_code)]
pub(crate) fn update_last_error<N: Nullable>(err: FFIError) -> N {
    LAST_ERROR.with(|prev| *prev.borrow_mut() = Some(err));
    N::NULL
}

/// Update the `thread_local` error, taking ownership of the `Error`.
pub(crate) fn update_last_error_return_code(err: FFIError) -> ErrorCode {
    let code = err.code();
    LAST_ERROR.with(|prev| *prev.borrow_mut() = Some(err));
    code
}

/// Peek at the last error (thread local) and write its Display string using the [crate::buffer] system.
///
/// Accepts a struct of buffer operations and a pointer to the user's buffer instance.
///
/// Returns either [ErrorCode::None] (success) or [ErrorCode::BufferOps] (failure, because of a
/// nul byte somewhere in the error message itself).
///
/// ## Safety
///
/// Refer to [crate::buffer::BufferOps]
#[no_mangle]
pub unsafe extern "C" fn citeproc_rs_last_error_utf8(
    buffer_ops: buffer::BufferOps,
    user_data: *mut c_void,
) -> ErrorCode {
    let mut buffer = buffer::BufferWriter::new(buffer_ops, user_data);
    buffer.clear();
    let fmt_result = LAST_ERROR.with(|prev| {
        prev.borrow()
            .as_ref()
            .map(|last_error| {
                let formatted = last_error.to_string();
                buffer
                    .copy_to_user_noalloc(formatted.as_bytes())
                    .map(|_| formatted.len())
            })
            .unwrap_or(Ok(0usize))
    });
    match fmt_result {
        Ok(_bytes_written) => ErrorCode::None,
        Err(_nul_error) => ErrorCode::BufferOps,
    }
}

/// Return the error code for the last error. If you clear the error, this will give you
/// [ErrorCode::None] (= `0`).
#[no_mangle]
pub extern "C" fn citeproc_rs_last_error_code() -> ErrorCode {
    LAST_ERROR.with(|current| {
        current
            .borrow()
            .as_ref()
            .map_or(ErrorCode::None, |x| x.code())
    })
}

/// Get the length of the last error message (thread local) in bytes when encoded as UTF-16,
/// including the trailing null.
#[no_mangle]
pub extern "C" fn citeproc_rs_last_error_length_utf16() -> usize {
    LAST_ERROR.with(|prev| {
        prev.borrow()
            .as_ref()
            .map(|e| e.to_string().encode_utf16().count() + 1)
            .unwrap_or(0)
    })
}

/// Peek at the last error (thread local) and get its error message as a Rust `String`.
pub(crate) fn error_message() -> Option<String> {
    LAST_ERROR.with(|prev| prev.borrow().as_ref().map(|e| e.to_string()))
}

/// Peek at the most recent error and write its error message (`Display` impl)
/// into the provided buffer as a UTF-16 encoded string.
///
/// This returns the number of bytes written, or `-1` if there was an error.
///
/// # Safety
///
/// The provided buffer must be valid to write `length` bytes into. That's not `length`
/// UTF-16-encoded characters.
#[no_mangle]
pub unsafe extern "C" fn citeproc_rs_last_error_utf16(buf: *mut u16, length: usize) -> isize {
    crate::null_pointer_check!(buf);
    let buffer = slice::from_raw_parts_mut(buf, length as usize);

    let ret = copy_error_into_buffer(buffer, |msg| msg.encode_utf16().collect());

    if ret > 0 {
        // utf16 uses two bytes per character
        ret * 2
    } else {
        ret
    }
}

fn copy_error_into_buffer<B, F>(buffer: &mut [B], error_msg: F) -> isize
where
    F: FnOnce(String) -> Vec<B>,
    B: Copy + Nullable,
{
    let maybe_error_message: Option<Vec<B>> = error_message().map(|msg| error_msg(msg));

    let err_msg = match maybe_error_message {
        Some(msg) => msg,
        None => return 0,
    };

    if err_msg.len() + 1 > buffer.len() {
        // buffer isn't big enough
        return -1;
    }

    buffer[..err_msg.len()].copy_from_slice(&err_msg);
    // Make sure to add a trailing null in case people use this as a bare char*
    buffer[err_msg.len()] = B::NULL;

    (err_msg.len() + 1) as isize
}

#[doc(hidden)]
#[macro_export]
macro_rules! export_c_symbol {
    (fn $name:ident($( $arg:ident : $type:ty ),*) -> $ret:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name($( $arg : $type),*) -> $ret {
            $crate::errors::$name($( $arg ),*)
        }
    };
    (fn $name:ident($( $arg:ident : $type:ty ),*)) => {
        export_c_symbol!(fn $name($( $arg : $type),*) -> ());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str;

    fn clear_last_error() {
        let _ = LAST_ERROR.with(|e| e.borrow_mut().take());
    }

    #[test]
    fn update_the_error() {
        clear_last_error();

        let () = update_last_error(FFIError::NullPointer);

        let got_err_msg = LAST_ERROR.with(|e| e.borrow_mut().take().unwrap().to_string());
        assert_eq!(got_err_msg, FFIError::NullPointer.to_string());
    }

    #[test]
    fn take_the_last_error() {
        clear_last_error();

        let () = update_last_error(FFIError::NullPointer);

        let got_err_msg = take_last_error().unwrap().to_string();
        assert_eq!(got_err_msg, FFIError::NullPointer.to_string());
    }

    #[test]
    fn write_the_last_error_into_a_buffer() {
        clear_last_error();

        let err_msg = FFIError::NullPointer.to_string();

        let () = update_last_error(FFIError::NullPointer);

        use std::ffi::CString;
        let buffer = CString::new("").unwrap();

        let mut buffer_raw = CString::into_raw(buffer);
        let output = unsafe {
            let buffer_ptr = &mut buffer_raw as *mut *mut i8 as *mut c_void;
            let _ =
                citeproc_rs_last_error_utf8(crate::buffer::cstring::CSTRING_BUFFER_OPS, buffer_ptr);
            CString::from_raw(buffer_raw)
        };

        let bytes_written = output.as_bytes_with_nul().len();
        assert!(bytes_written > 0);
        assert_eq!(bytes_written, err_msg.len() + 1);

        let msg = str::from_utf8(output.as_bytes()).unwrap();
        assert_eq!(msg, err_msg);
    }
}
