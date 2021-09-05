use libc::c_void;

pub mod cstring;

/// A vtable to allow citeproc-rs to manipulate your own kind of buffer for the output.
///
/// You could define one using realloc and the C standard library's string manipulations with your
/// own zero terminators etc, but you could also just use [cstring::CSTRING_BUFFER_OPS] and let Rust's
/// `std::ffi::CString` do the hard work.
///
/// In C++ and other FFI-compatible higher level languages this is much easier. Just use any
/// growable string or buffer type and implement the two functions in a couple of lines each.
///
/// You will get valid UTF-8 if you correctly write out all the bytes.
///
/// ## Safety
///
/// When using BufferOps, the only thing you *must* ensure is that the callback functions access
/// the user data pointer consistently with the actual user data pointers passed to Rust.
///
/// If your write callback expects a `char **`, then you must supply a `char **`. If your write
/// callback expects a C++ `std::string *`, then you must supply a `std::string *`.
#[derive(Copy, Clone)]
#[repr(C)]
pub struct BufferOps {
    pub write: WriteCallback,
    pub clear: ClearCallback,
}

/// Should write src_len bytes from src into some structure referenced by user_data.
/// The bytes are guaranteed not to contain a zero.
pub type WriteCallback =
    unsafe extern "C" fn(user_data: *mut c_void, src: *const u8, src_len: usize);
/// Should clear the buffer in the structure referenced by user_data.
pub type ClearCallback = unsafe extern "C" fn(user_data: *mut c_void);

/// A way to invoke BufferOps on matching user data.
pub(crate) struct BufferWriter {
    callbacks: BufferOps,
    user_buf_ptr: *mut c_void,
}

/// An error wherein citeproc attempted to write a null byte into a user buffer.
///
/// Just a clone of [std::ffi::NulError] that has a constructor. For some reason the only way to
/// construct that is by creating a CString, but we want to avoid nulls in user-supplied
/// implementations.
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
