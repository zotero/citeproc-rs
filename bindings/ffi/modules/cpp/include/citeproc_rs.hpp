#ifndef _CITEPROC_RS_HPP
#define _CITEPROC_RS_HPP

#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

namespace citeproc_rs {

enum class ErrorCode : int32_t {
  none = 0,
  null_pointer = 1,
  caught_panic = 2,
  poisoned = 3,
  utf8 = 4,
  reordering = 5,
  buffer_ops = 6,
  null_byte = 7,
  serde_json = 8,
};

enum class OutputFormat : uint8_t {
  html,
  rtf,
  plain,
};

/// Wrapper for a driver, initialized with one style and any required locales.
///
/// Not thread safe.
///
/// Contains an Option<citeproc_rs::Processor>, because to survive panics we want to be able to
/// write a safe value that won't be in an inconsistent state after panicking.
struct Driver;

struct LocaleSlot;

/// A callback signature that is expected to write a string into `slot` via
/// [citeproc_rs_locale_slot_write]
using LocaleFetchCallback = void(*)(void *context, LocaleSlot *slot, const char*);

/// Should write src_len bytes from src into some structure referenced by user_data.
/// The bytes are guaranteed not to contain a zero.
using WriteCallback = void(*)(void *user_data, const uint8_t *src, uintptr_t src_len);

/// Should clear the buffer in the structure referenced by user_data.
using ClearCallback = void(*)(void *user_data);

/// A vtable to allow citeproc-rs to manipulate your own kind of buffer for the output.
/// You could define one using realloc and the C standard library's string manipulations with your
/// own zero terminators etc, but you could also just use [MANAGED_BUFFER_OPS] and let Rust's
/// `std::ffi::CString` do the hard work.
///
/// In C++ and other FFI-compatible higher level languages this is much easier. Just use any
/// growable string or buffer type and implement the two functions in a couple of lines each.
///
/// You will get valid UTF-8 if you correctly write out all the bytes.
struct BufferOps {
  WriteCallback write;
  ClearCallback clear;
};

struct InitOptions {
  const char *style;
  uintptr_t style_len;
  void *locale_fetch_context;
  LocaleFetchCallback locale_fetch_callback;
  OutputFormat format;
  BufferOps buffer_ops;
};

struct CoolStruct {
  int32_t field;
};

extern "C" {

/// Initialises the Rust `log` crate globally. No-op when called a second time.
 void citeproc_rs_log_init();


void citeproc_rs_locale_slot_write(LocaleSlot *slot,
                                   const char *locale_xml,
                                   uintptr_t locale_xml_len);

/// Creates a new Processor from InitOptions.
 Driver *citeproc_rs_driver_new(InitOptions init);

/// Frees a Processor.
 void citeproc_rs_driver_free(Driver *driver);

/// Frees a CString returned from an API or one written using [CSTRING_BUFFER_OPS].
 void citeproc_rs_string_free(char *ptr);

 ErrorCode citeproc_rs_driver_format_bibliography(Driver *driver, void *user_buf);

/// Inserts a reference and formats a single cluster with a single cite to that reference using preview_citation_cluster.
/// The reference in the processor will be overwritten and won't be restored afterward.
///
/// Writes the result into user_buf using the buffer_ops interface.
///
/// Returns an error code indicative of what the LAST_ERROR will contain when checked.

ErrorCode citeproc_rs_driver_preview_reference(Driver *driver,
                                               const char *ref_json,
                                               uintptr_t ref_json_len,
                                               void *user_buf);

/// Inserts a reference.
///
/// Returns an error code indicative of what the LAST_ERROR will contain when checked.

ErrorCode citeproc_rs_driver_insert_reference(Driver *driver,
                                              const char *ref_json,
                                              uintptr_t ref_json_len);

/// Clear the `LAST_ERROR`.
 void citeproc_rs_clear_last_error();

/// Returns either [ErrorCode::None] or [ErrorCode::BufferOps].
 ErrorCode citeproc_rs_last_error_utf8(BufferOps buffer_ops, void *user_data);

/// Return the error code for the last error. If you clear the error, this will give you
/// [ErrorCode::None] (= `0`).
 ErrorCode citeproc_rs_last_error_code();

/// Get the length of the last error message in bytes when encoded as UTF-8,
/// including the trailing null. If the error is cleared, this returns 0.
 uintptr_t citeproc_rs_last_error_length();

/// Get the length of the last error message in bytes when encoded as UTF-16,
/// including the trailing null.
 uintptr_t citeproc_rs_last_error_length_utf16();

/// Peek at the most recent error and write its error message (`Display` impl)
/// into the provided buffer as a UTF-8 encoded string.
///
/// This returns the number of bytes written, or `-1` if there was an error.
///
/// # Safety
///
/// The provided buffer must be valid to write up to `length` bytes into.
 intptr_t citeproc_rs_error_message_utf8(char *buf, uintptr_t length);

/// Peek at the most recent error and write its error message (`Display` impl)
/// into the provided buffer as a UTF-16 encoded string.
///
/// This returns the number of bytes written, or `-1` if there was an error.
///
/// # Safety
///
/// The provided buffer must be valid to write `length` bytes into. That's not `length`
/// UTF-16-encoded characters.
 intptr_t citeproc_rs_error_message_utf16(uint16_t *buf, uintptr_t length);

 int32_t viva_la_funcion(CoolStruct *arg, int32_t other_arg);

/// If you use this as your buffer_write_callback, then you must call `citeproc_rs_string_free` on
/// the resulting buffers.
 void citeproc_rs_cstring_write(void *user_data, const uint8_t *src, uintptr_t src_len);

 void citeproc_rs_cstring_clear(void *user_data);

} // extern "C"

static const BufferOps MANAGED_BUFFER_OPS = BufferOps{ /* .write = */ citeproc_rs_cstring_write, /* .clear = */ citeproc_rs_cstring_clear };

} // namespace citeproc_rs

#endif // _CITEPROC_RS_HPP
