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
 * Contains an Option<citeproc_rs::Processor>, because to survive panics we want to be able to
 * write a safe value that won't be in an inconsistent state after panicking.
 */
typedef struct CiteprocRsProcessor CiteprocRsProcessor;

/**
 * A callback signature that is expected to write a string into `slot` via
 * [citeproc_rs_locale_slot_write]
 */
typedef void (*CiteprocRsLocaleFetchCallback)(void *context, struct CiteprocRsLocaleSlot *slot, const char*);

typedef struct CiteprocRsInitOptions {
  const char *style;
  uintptr_t style_len;
  void *locale_fetch_context;
  CiteprocRsLocaleFetchCallback locale_fetch_callback;
  CiteprocRsOutputFormat format;
} CiteprocRsInitOptions;

typedef struct CiteprocRsCoolStruct {
  int32_t field;
} CiteprocRsCoolStruct;

/**
 * Initialises the Rust `log` crate globally. No-op when called a second time.
 */
void citeproc_rs_log_init(void) CF_SWIFT_NAME(citeproc_rs_log_init());

void citeproc_rs_locale_slot_write(struct CiteprocRsLocaleSlot *slot,
                                   const char *locale_xml,
                                   uintptr_t locale_xml_len) CF_SWIFT_NAME(citeproc_rs_locale_slot_write(slot:locale_xml:locale_xml_len:));

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

int32_t viva_la_funcion(struct CiteprocRsCoolStruct *arg,
                        int32_t other_arg) CF_SWIFT_NAME(viva_la_funcion(arg:other_arg:));

void citeproc_rs_clear_last_error(void) CF_SWIFT_NAME(citeproc_rs_clear_last_error());

uintptr_t citeproc_rs_last_error_length(void) CF_SWIFT_NAME(citeproc_rs_last_error_length());

uintptr_t citeproc_rs_last_error_length_utf16(void) CF_SWIFT_NAME(citeproc_rs_last_error_length_utf16());

intptr_t citeproc_rs_error_message_utf8(char *buf,
                                        uintptr_t length) CF_SWIFT_NAME(citeproc_rs_error_message_utf8(buf:length:));

intptr_t citeproc_rs_error_message_utf16(uint16_t *buf,
                                         size_t length) CF_SWIFT_NAME(citeproc_rs_error_message_utf16(buf:length:));

#endif /* _CITEPROC_RS_H */
