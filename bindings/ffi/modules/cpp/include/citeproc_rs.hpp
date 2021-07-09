#ifndef _CITEPROC_RS_HPP
#define _CITEPROC_RS_HPP

#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

namespace citeproc_rs {

enum class OutputFormat : uint8_t {
  html,
  rtf,
  plain,
};

struct LocaleSlot;

/// Wrapper for a Processor, initialized with one style and any required locales
/// Contains an Option<citeproc_rs::Processor>, because to survive panics we want to be able to
/// write a safe value that won't be in an inconsistent state after panicking.
struct Processor;

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
 Processor *citeproc_rs_processor_new(InitOptions init);

/// Frees a Processor.
 void citeproc_rs_processor_free(Processor *processor);

/// Frees a string returned from  API.
 void citeproc_rs_string_free(char *ptr);

/// let reference: [String: Any] = [ "id": "blah", "type": "book", ... ];
/// in Swift, JSONSerialization.data(reference).withUnsafeBytes({ rBytes in
///     format_one(processor, rBytes.baseAddress, rBytes.count)
/// })
///
/// May return null.

char *citeproc_rs_processor_format_one(Processor *processor,
                                       const char *ref_bytes,
                                       uintptr_t ref_bytes_len);

 int32_t viva_la_funcion(CoolStruct *arg, int32_t other_arg);

 void citeproc_rs_clear_last_error();

 uintptr_t citeproc_rs_last_error_length();

 uintptr_t citeproc_rs_last_error_length_utf16();

 intptr_t citeproc_rs_error_message_utf8(char *buf, uintptr_t length);

 intptr_t citeproc_rs_error_message_utf16(uint16_t *buf, size_t length);

} // extern "C"

} // namespace citeproc_rs

#endif // _CITEPROC_RS_HPP
