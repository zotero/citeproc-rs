use libc::c_void;
use std::ffi::CString;

/// A vtable to allow citeproc-rs to manipulate your own kind of buffer for the output.
/// You could define one using realloc and the C standard library's string manipulations with your
/// own zero terminators etc, but you could also just use [MANAGED_BUFFER_OPS] and let Rust's
/// `std::ffi::CString` do the hard work.
///
/// In C++ and other FFI-compatible higher level languages this is much easier. Just use any
/// growable string or buffer type and implement the two functions in a couple of lines each.
///
/// You will get valid UTF-8 if you correctly write out all the bytes.
#[derive(Copy, Clone)]
#[repr(C)]
pub struct BufferOps {
    write: WriteCallback,
    clear: ClearCallback,
}

pub const MANAGED_BUFFER_OPS: BufferOps = BufferOps {
    write: citeproc_rs_cstring_write,
    clear: citeproc_rs_cstring_clear,
};

/// Should write src_len bytes from src into some structure referenced by user_data.
/// The bytes are guaranteed not to contain a zero.
pub type WriteCallback = unsafe extern "C" fn(user_data: *mut c_void, src: *const u8, src_len: usize);
/// Should clear the buffer in the structure referenced by user_data.
pub type ClearCallback = unsafe extern "C" fn(user_data: *mut c_void);

pub struct BufferWriter {
    callbacks: BufferOps,
    user_buf_ptr: *mut c_void,
}

#[derive(Debug)]
pub struct NulError(usize, Vec<u8>);

impl std::error::Error for NulError {}
impl core::fmt::Display for NulError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "cannot write string with null byte at position {}: {:?}",
            self.0, self.1
        )
    }
}

/// If you use this as your buffer_write_callback, then you must call `citeproc_rs_string_free` on
/// the resulting buffers.
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
    let user_data_ptr = user_data.cast::<*mut libc::c_char>();
    // SAFETY: we checked it for null.
    let user_data_contents = unsafe { *user_data_ptr };
    let cstring = if user_data_contents.is_null() {
        None
    } else {
        Some(CString::from_raw(user_data_contents))
    };

    // SAFETY: This is safe as long as we call this with a Rust slice decomposition.
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

#[no_mangle]
#[allow(unused_unsafe)]
pub unsafe extern "C" fn citeproc_rs_cstring_clear(
    user_data: *mut c_void,
) {
    // user_data shouldn't be null but we will fail gracefully.
    if user_data.is_null() {
        return;
    }
    let user_data_ptr = user_data.cast::<*mut libc::c_char>();
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

    // Write null to it. W
    // SAFETY: we checked user_data for null before.
    unsafe {
        core::ptr::write(user_data_ptr, cstring.into_raw());
    }
}

impl BufferWriter {
    // # Safety
    //
    // The FFI end-user must call various APIs with these requirements met, in order for them to be
    // satisfied when the BufferWriter is eventually constructed.
    //
    // First, user_buf_ptr can technically be null, but it will not be very useful if that's the
    // case.
    //
    // But if it is not null, and the callback is Some, then the callback must not misuse the
    // pointer. The callback should essentially cast the void into the same type as user_buf_ptr
    // is. It can mutate the contents of the pointer within the bounds of the object it refers to.
    pub unsafe fn new(callbacks: BufferOps, user_buf_ptr: *mut c_void) -> Self {
        Self {
            callbacks,
            user_buf_ptr,
        }
    }

    fn using_callback(&mut self, bytes: &[u8]) {
        // user_buf_ptr is whatever the callback wants it to be
        // SAFETY: see fn new()
        unsafe {
            (self.callbacks.write)(self.user_buf_ptr, bytes.as_ptr(), bytes.len());
        }
    }

    /// Same as `write_bytes()`.
    pub fn write_str(&mut self, str: &str) -> Result<(), NulError> {
        self.write_bytes(str.as_bytes())
    }

    /// Uses the callback and the user data to write these bytes somewhere user-accessible.
    ///
    /// Returns Err(NulError) if there are any 0x0 characters in `bytes`, because we want to be
    /// C-compatible and that's not very friendly.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), NulError> {
        use memchr::memchr;
        if let Some(ix) = memchr(0u8, bytes) {
            return Err(NulError(ix, bytes.to_owned()));
        }
        Ok(self.using_callback(bytes))
    }

    /// Same as copy_to_user, except doesn't allocate for NulError, replacing it with `()`.
    pub fn copy_to_user_noalloc(&mut self, bytes: &[u8]) -> Result<(), ()> {
        use memchr::memchr;
        if let Some(_) = memchr(0u8, bytes) {
            return Err(());
        }
        Ok(self.using_callback(bytes))
    }

    pub fn clear(&mut self) {
        unsafe {
            (self.callbacks.clear)(self.user_buf_ptr);
        }
    }
}

impl core::fmt::Write for BufferWriter {
    fn write_str(&mut self, string: &str) -> core::fmt::Result {
        self.copy_to_user_noalloc(string.as_bytes())
            .map_err(|_| core::fmt::Error)
    }
}

use std::io;
impl io::Write for BufferWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let len = bytes.len();
        self.write_bytes(bytes)
            .map(|_| len)
            .map_err(|nul_err| io::Error::new(io::ErrorKind::InvalidData, nul_err))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // noop. Let's hope the FFI consumer doesn't need to flush the stream.
        Ok(())
    }
}
