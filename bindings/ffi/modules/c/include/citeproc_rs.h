#ifndef _CITEPROC_RS_H
#define _CITEPROC_RS_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

enum citeproc_rs_output_format {
  CITEPROC_RS_OUTPUT_FORMAT_HTML,
  CITEPROC_RS_OUTPUT_FORMAT_RTF,
  CITEPROC_RS_OUTPUT_FORMAT_PLAIN,
};
typedef uint8_t citeproc_rs_output_format;

typedef struct citeproc_rs_locale_slot citeproc_rs_locale_slot;

/**
 * Wrapper for a Processor, initialized with one style and any required locales
 * Contains an Option<citeproc_rs::Processor>, because to survive panics we want to be able to
 * write a safe value that won't be in an inconsistent state after panicking.
 */
typedef struct citeproc_rs_processor citeproc_rs_processor;

/**
 * A callback signature that is expected to write a string into `slot` via
 * [citeproc_rs_locale_slot_write]
 */
typedef void (*citeproc_rs_locale_fetch_callback)(void *context, struct citeproc_rs_locale_slot *slot, const char*);

typedef struct citeproc_rs_init_options {
  const char *style;
  uintptr_t style_len;
  void *locale_fetch_context;
  citeproc_rs_locale_fetch_callback locale_fetch_callback;
  citeproc_rs_output_format format;
} citeproc_rs_init_options;

typedef struct citeproc_rs_cool_struct {
  int32_t field;
} citeproc_rs_cool_struct;

/**
 * Initialises the Rust `log` crate globally. No-op when called a second time.
 */
void citeproc_rs_log_init(void);

void citeproc_rs_locale_slot_write(struct citeproc_rs_locale_slot *slot,
                                   const char *locale_xml,
                                   uintptr_t locale_xml_len);

/**
 * Creates a new Processor from InitOptions.
 */
struct citeproc_rs_processor *citeproc_rs_processor_new(struct citeproc_rs_init_options init);

/**
 * Frees a Processor.
 */
void citeproc_rs_processor_free(struct citeproc_rs_processor *processor);

/**
 * Frees a string returned from  API.
 */
void citeproc_rs_string_free(char *ptr);

/**
 * let reference: [String: Any] = [ "id": "blah", "type": "book", ... ];
 * in Swift, JSONSerialization.data(reference).withUnsafeBytes({ rBytes in
 *     format_one(processor, rBytes.baseAddress, rBytes.count)
 * })
 *
 * May return null.
 */
char *citeproc_rs_processor_format_one(struct citeproc_rs_processor *processor,
                                       const char *ref_bytes,
                                       uintptr_t ref_bytes_len);

int32_t viva_la_funcion(struct citeproc_rs_cool_struct *arg, int32_t other_arg);

void citeproc_rs_clear_last_error(void);

uintptr_t citeproc_rs_last_error_length(void);

uintptr_t citeproc_rs_last_error_length_utf16(void);

intptr_t citeproc_rs_error_message_utf8(char *buf, uintptr_t length);

intptr_t citeproc_rs_error_message_utf16(uint16_t *buf, size_t length);

#endif /* _CITEPROC_RS_H */
