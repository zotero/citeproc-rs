#ifndef _CITEPROC_RS_H
#define _CITEPROC_RS_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

enum citeproc_rs_error_code {
  CITEPROC_RS_ERROR_CODE_NONE = 0,
  CITEPROC_RS_ERROR_CODE_NULL_POINTER = 1,
  CITEPROC_RS_ERROR_CODE_CAUGHT_PANIC = 2,
  CITEPROC_RS_ERROR_CODE_POISONED = 3,
  CITEPROC_RS_ERROR_CODE_UTF8 = 4,
  CITEPROC_RS_ERROR_CODE_REORDERING = 5,
};
typedef int32_t citeproc_rs_error_code;

enum citeproc_rs_output_format {
  CITEPROC_RS_OUTPUT_FORMAT_HTML,
  CITEPROC_RS_OUTPUT_FORMAT_RTF,
  CITEPROC_RS_OUTPUT_FORMAT_PLAIN,
};
typedef uint8_t citeproc_rs_output_format;

/**
 * Wrapper for a driver, initialized with one style and any required locales
 * Contains an Option<citeproc_rs::Processor>, because to survive panics we want to be able to
 * write a safe value that won't be in an inconsistent state after panicking.
 */
typedef struct citeproc_rs_driver citeproc_rs_driver;

typedef struct citeproc_rs_locale_slot citeproc_rs_locale_slot;

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
struct citeproc_rs_driver *citeproc_rs_driver_new(struct citeproc_rs_init_options init);

/**
 * Frees a Processor.
 */
void citeproc_rs_driver_free(struct citeproc_rs_driver *driver);

/**
 * Frees a string returned from  API.
 */
void citeproc_rs_string_free(char *ptr);

/**
 * let reference: [String: Any] = [ "id": "blah", "type": "book", ... ];
 * in Swift, JSONSerialization.data(reference).withUnsafeBytes({ rBytes in
 *     format_one(driver, rBytes.baseAddress, rBytes.count)
 * })
 *
 * May return null.
 */
char *citeproc_rs_driver_format_one(struct citeproc_rs_driver *driver,
                                    const char *ref_bytes,
                                    uintptr_t ref_bytes_len);

/**
 * Clear the `LAST_ERROR`.
 */
void citeproc_rs_clear_last_error(void);

/**
 * Return the error code for the last error. If you clear the error, this will give you
 * [ErrorCode::None] (= `0`).
 */
citeproc_rs_error_code citeproc_rs_last_error_code(void);

/**
 * Get the length of the last error message in bytes when encoded as UTF-8,
 * including the trailing null. If the error is cleared, this returns 0.
 */
uintptr_t citeproc_rs_last_error_length(void);

/**
 * Get the length of the last error message in bytes when encoded as UTF-16,
 * including the trailing null.
 */
uintptr_t citeproc_rs_last_error_length_utf16(void);

/**
 * Peek at the most recent error and write its error message (`Display` impl)
 * into the provided buffer as a UTF-8 encoded string.
 *
 * This returns the number of bytes written, or `-1` if there was an error.
 *
 * # Safety
 *
 * The provided buffer must be valid to write up to `length` bytes into.
 */
intptr_t citeproc_rs_error_message_utf8(char *buf, uintptr_t length);

/**
 * Peek at the most recent error and write its error message (`Display` impl)
 * into the provided buffer as a UTF-16 encoded string.
 *
 * This returns the number of bytes written, or `-1` if there was an error.
 *
 * # Safety
 *
 * The provided buffer must be valid to write `length` bytes into. That's not `length`
 * UTF-16-encoded characters.
 */
intptr_t citeproc_rs_error_message_utf16(uint16_t *buf, uintptr_t length);

int32_t viva_la_funcion(struct citeproc_rs_cool_struct *arg, int32_t other_arg);

#endif /* _CITEPROC_RS_H */
