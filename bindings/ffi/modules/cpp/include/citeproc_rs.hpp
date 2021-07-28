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
};

enum class OutputFormat : uint8_t {
  html,
  rtf,
  plain,
};

/// Wrapper for a driver, initialized with one style and any required locales
/// Contains an Option<citeproc_rs::Processor>, because to survive panics we want to be able to
/// write a safe value that won't be in an inconsistent state after panicking.
struct Driver;

struct LocaleSlot;

/// A callback signature that is expected to write a string into `slot` via
/// [citeproc_rs_locale_slot_write]
using LocaleFetchCallback = void(*)(void *context, LocaleSlot *slot, const char*);

struct InitOptions {
  const char *style;
  uintptr_t style_len;
  void *locale_fetch_context;
  LocaleFetchCallback locale_fetch_callback;
  OutputFormat format;
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

/// Frees a string returned from  API.
 void citeproc_rs_string_free(char *ptr);

/// let reference: [String: Any] = [ "id": "blah", "type": "book", ... ];
/// in Swift, JSONSerialization.data(reference).withUnsafeBytes({ rBytes in
///     format_one(driver, rBytes.baseAddress, rBytes.count)
/// })
///
/// May return null.

char *citeproc_rs_driver_format_one(Driver *driver,
                                    const char *ref_bytes,
                                    uintptr_t ref_bytes_len);

/// Clear the `LAST_ERROR`.
 void citeproc_rs_clear_last_error();

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

} // extern "C"

} // namespace citeproc_rs

#endif // _CITEPROC_RS_HPP
