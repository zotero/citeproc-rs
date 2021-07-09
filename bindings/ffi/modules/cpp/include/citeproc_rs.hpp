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
struct Processor;

using LocaleFetchCallback = void(*)(void *context, LocaleSlot *slot, const char*);

struct InitOptions {
  const char *style;
  uintptr_t style_len;
  void *locale_fetch_context;
  LocaleFetchCallback locale_fetch_callback;
  OutputFormat format;
};

extern "C" {


void citeproc_rs_write_locale_slot(LocaleSlot *slot,
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

} // extern "C"

} // namespace citeproc_rs

#endif // _CITEPROC_RS_HPP
