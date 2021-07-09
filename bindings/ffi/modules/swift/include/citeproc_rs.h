#define CF_SWIFT_NAME(_name) __attribute__((swift_name(#_name)))

#ifndef _CITEPROC_RS_H
#define _CITEPROC_RS_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <CoreFoundation/CoreFoundation.h>

typedef CF_ENUM(uint8_t, CiteprocRsOutputFormat) {
  CiteprocRsOutputFormatHtml,
  CiteprocRsOutputFormatRtf,
  CiteprocRsOutputFormatPlain,
};

typedef struct CiteprocRsLocaleSlot CiteprocRsLocaleSlot;

/**
 * Wrapper for a Processor, initialized with one style and any required locales
 */
typedef struct CiteprocRsProcessor CiteprocRsProcessor;

typedef void (*CiteprocRsLocaleFetchCallback)(void *context, struct CiteprocRsLocaleSlot *slot, const char*);

typedef struct CiteprocRsInitOptions {
  const char *style;
  uintptr_t style_len;
  void *locale_fetch_context;
  CiteprocRsLocaleFetchCallback locale_fetch_callback;
  CiteprocRsOutputFormat format;
} CiteprocRsInitOptions;

void citeproc_rs_write_locale_slot(struct CiteprocRsLocaleSlot *slot,
                                   const char *locale_xml,
                                   uintptr_t locale_xml_len) CF_SWIFT_NAME(citeproc_rs_write_locale_slot(slot:locale_xml:locale_xml_len:));

/**
 * Creates a new Processor from InitOptions.
 */
struct CiteprocRsProcessor *citeproc_rs_processor_new(struct CiteprocRsInitOptions init) CF_SWIFT_NAME(citeproc_rs_processor_new(init:));

/**
 * Frees a Processor.
 */
void citeproc_rs_processor_free(struct CiteprocRsProcessor *processor) CF_SWIFT_NAME(citeproc_rs_processor_free(processor:));

/**
 * Frees a string returned from  API.
 */
void citeproc_rs_string_free(char *ptr) CF_SWIFT_NAME(citeproc_rs_string_free(ptr:));

/**
 * let reference: [String: Any] = [ "id": "blah", "type": "book", ... ];
 * in Swift, JSONSerialization.data(reference).withUnsafeBytes({ rBytes in
 *     format_one(processor, rBytes.baseAddress, rBytes.count)
 * })
 *
 * May return null.
 */
char *citeproc_rs_processor_format_one(struct CiteprocRsProcessor *processor,
                                       const char *ref_bytes,
                                       uintptr_t ref_bytes_len) CF_SWIFT_NAME(citeproc_rs_processor_format_one(processor:ref_bytes:ref_bytes_len:));

#endif /* _CITEPROC_RS_H */
