//! from crates.io/crates/ffi_helpers, with failure replaced with std::error::Error

use crate::nullable::Nullable;
use libc::c_char;
use std::{cell::RefCell, slice};

use crate::{ErrorCode, FFIError};

thread_local! {
    static LAST_ERROR: RefCell<Option<FFIError>> = RefCell::new(None);
}

/// Clear the `LAST_ERROR`.
#[no_mangle]
pub extern fn citeproc_rs_clear_last_error() {
    let _ = take_last_error();
}

/// Take the most recent error, clearing `LAST_ERROR` in the process.
pub fn take_last_error() -> Option<FFIError> {
    LAST_ERROR.with(|prev| prev.borrow_mut().take())
}

/// Update the `thread_local` error, taking ownership of the `Error`.
pub fn update_last_error<N: Nullable>(err: FFIError) -> N {
    LAST_ERROR.with(|prev| *prev.borrow_mut() = Some(err));
    N::NULL
}

/// Return the error code for the last error. If you clear the error, this will give you
/// [ErrorCode::None] (= `0`).
#[no_mangle]
pub extern fn citeproc_rs_last_error_code() -> ErrorCode {
    LAST_ERROR.with(|current| current.borrow().as_ref().map_or(ErrorCode::None, |x| x.code()))
}

/// Get the length of the last error message in bytes when encoded as UTF-8,
/// including the trailing null. If the error is cleared, this returns 0.
#[no_mangle]
pub extern fn citeproc_rs_last_error_length() -> usize {
    LAST_ERROR.with(|prev| {
        prev.borrow()
            .as_ref()
            .map(|e| e.to_string().len() + 1)
            .unwrap_or(0)
    })
}

/// Get the length of the last error message in bytes when encoded as UTF-16,
/// including the trailing null.
#[no_mangle]
pub extern fn citeproc_rs_last_error_length_utf16() -> usize {
    LAST_ERROR.with(|prev| {
        prev.borrow()
            .as_ref()
            .map(|e| e.to_string().encode_utf16().count() + 1)
            .unwrap_or(0)
    })
}

/// Peek at the most recent error and get its error message as a Rust `String`.
pub fn error_message() -> Option<String> {
    LAST_ERROR.with(|prev| prev.borrow().as_ref().map(|e| e.to_string()))
}

/// Peek at the most recent error and write its error message (`Display` impl)
/// into the provided buffer as a UTF-8 encoded string.
///
/// This returns the number of bytes written, or `-1` if there was an error.
///
/// # Safety
///
/// The provided buffer must be valid to write up to `length` bytes into.
#[no_mangle]
pub unsafe extern fn citeproc_rs_error_message_utf8(buf: *mut c_char, length: usize) -> isize {
    crate::null_pointer_check!(buf);
    let buffer = slice::from_raw_parts_mut(buf as *mut u8, length as usize);

    copy_error_into_buffer(buffer, |msg| msg.into())
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
pub unsafe extern fn citeproc_rs_error_message_utf16(buf: *mut u16, length: usize) -> isize {
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
    use thiserror::Error;

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
    fn get_the_last_error_messages_length() {
        clear_last_error();

        let err_msg = FFIError::NullPointer.to_string();
        let should_be = err_msg.len() + 1;

        let () = update_last_error(FFIError::NullPointer);

        // Get a valid error message's length
        let got = citeproc_rs_last_error_length();
        assert_eq!(got, should_be);

        // Then clear the error message and make sure we get 0
        clear_last_error();
        let got = citeproc_rs_last_error_length();
        assert_eq!(got, 0);
    }

    #[test]
    fn write_the_last_error_message_into_a_buffer() {
        clear_last_error();

        let err_msg = FFIError::NullPointer.to_string();

        let () = update_last_error(FFIError::NullPointer);

        let mut buffer: Vec<u8> = vec![0; 100];
        let bytes_written = unsafe {
            citeproc_rs_error_message_utf8(buffer.as_mut_ptr() as *mut c_char, buffer.len() as _)
        };

        assert!(bytes_written > 0);
        assert_eq!(bytes_written as usize, err_msg.len() + 1);

        let msg = str::from_utf8(&buffer[..bytes_written as usize - 1]).unwrap();
        assert_eq!(msg, err_msg);
    }
}
