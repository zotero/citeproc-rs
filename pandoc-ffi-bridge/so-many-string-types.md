## The Haskell or Rust tourists's guide to foreign strings

Non-FFI types

- Haskell Strings are [Char]
- Haskell Data.Text is like Rust String: UTF8.
- Rust CString is a pointer+len+capacity to a null-terminated String
- Rust &CStr is a null-terminated &str
- Neither CString nor CStr are repr(C), so do not pass them over FFI.
- Obviously same goes for Haskell String and Data.Text.

FFI-able string types

- Haskell Foreign.C.String.CString is a typedef for `Ptr CChar`
- Haskell Data.ByteStrings are always stored in pinned memory, so they are a 
  bit like Pin<P> in Rust. This means you can safely pass one over FFI without 
  worrying that the garbage collector will move the underlying array to a new 
  heap. There are also variants that do not include an automatic finalizer.
- https://ghc.haskell.org/trac/ghc/wiki/Commentary/Rts/Storage/GC/Pinned
- Rust *const c_char and *mut c_char are identical to char *pointer in C
- So Haskell CString ~= *const c_char or *mut c_char
- THAT is how you pass types.

Allocation, deallocation, GC and Rust lifetimes

- Rust Cstr::as_ptr() is NOT a borrow, it's just a pointer. So you have to
  keep the lifetime it's meant to be tied to alive manually, and force
  the borrow/drop checker to keep it around.
- I don't yet know how GHC knows when to GC anything. There are a few
  variations on Ptr that may be helpful (like StablePtr).
