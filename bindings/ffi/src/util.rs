use libc::c_char;

use crate::errors;
use crate::nullable::FromErrorCode;
use crate::ErrorCode;
use crate::FFIError;

/// Either a positive-or-0 u32, or a negative ErrorCode. Represented as an i64.
///
/// ```ignore
///
/// // Laborious but thoroughly correct example
/// citeproc_rs_error_code code = CITEPROC_RS_ERROR_CODE_NONE;
/// char *error_message;
/// uint32_t result;
///
/// int64_t ret = some_api(...);
///
/// if (ret < 0) {
///     code = (citeproc_rs_error_code)(-ret);
///     citeproc_rs_last_error_utf8(citeproc_rs_cstring_buffer_ops, &error_message);
///     printf("%s\n", error_message);
///     citeproc_rs_cstring_free(error_message);
///     return -1;
/// } else {
///     result = (int32_t) ret;
/// }
/// ```
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct U32OrError(pub i64);

impl FromErrorCode for U32OrError {
    fn from_error_code(code: ErrorCode) -> Self {
        // ErrorCode is #[repr(i32)]
        let int = code as i32 as i64;
        Self(int)
    }
}

pub fn result_to_error_code<F, T>(mut closure: F) -> T
where
    F: FnMut() -> Result<T, FFIError>,
    T: FromErrorCode,
{
    let result = closure();
    match result {
        Ok(value) => {
            errors::clear_last_error();
            return value;
        }
        Err(e) => {
            let code = errors::update_last_error_return_code(e);
            T::from_error_code(code)
        }
    }
}

use core::ptr::NonNull;

pub unsafe fn borrow_raw_ptr<'a, T>(ptr: *const T) -> Result<&'a T, FFIError> {
    if ptr.is_null() {
        return Err(FFIError::NullPointer);
    } else {
        Ok(&*ptr)
    }
}

pub unsafe fn borrow_raw_ptr_mut<'a, T>(ptr: *mut T) -> Result<&'a mut T, FFIError> {
    let ptr = NonNull::new(ptr).ok_or(FFIError::NullPointer)?;
    Ok(&mut *ptr.as_ptr())
}

pub unsafe fn borrow_slice<'a, T>(ptr: *const T, len: usize) -> Result<&'a [T], FFIError> {
    if len == 0 {
        return Ok(&[]);
    }
    if !ptr.is_null() {
        let slice = core::slice::from_raw_parts(ptr, len);
        Ok(slice)
    } else {
        Err(FFIError::NullPointer)
    }
}

pub unsafe fn borrow_utf8_slice<'a>(ptr: *const c_char, len: usize) -> Result<&'a str, FFIError> {
    if len == 0 {
        return Ok("");
    }
    if !ptr.is_null() {
        let ptr = ptr.cast::<u8>();
        let slice = core::slice::from_raw_parts(ptr, len);
        let string = core::str::from_utf8(slice)?;
        Ok(string)
    } else {
        Err(FFIError::NullPointer)
    }
}
