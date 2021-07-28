#define CF_SWIFT_NAME(_name) __attribute__((swift_name(#_name)))

#ifndef _CITEPROC_RS_H
#define _CITEPROC_RS_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <CoreFoundation/CoreFoundation.h>

typedef CF_ENUM(int32_t, CiteprocRsErrorCode) {
  CiteprocRsErrorCodeNone = 0,
  CiteprocRsErrorCodeNullPointer = 1,
  CiteprocRsErrorCodeCaughtPanic = 2,
  CiteprocRsErrorCodePoisoned = 3,
  CiteprocRsErrorCodeUtf8 = 4,
  CiteprocRsErrorCodeReordering = 5,
};

typedef CF_ENUM(uint8_t, CiteprocRsOutputFormat) {
  CiteprocRsOutputFormatHtml,
  CiteprocRsOutputFormatRtf,
  CiteprocRsOutputFormatPlain,
};

/**
 * Wrapper for a driver, initialized with one style and any required locales
 * Contains an Option<citeproc_rs::Processor>, because to survive panics we want to be able to
 * write a safe value that won't be in an inconsistent state after panicking.
 */
typedef struct CiteprocRsDriver CiteprocRsDriver;

typedef struct CiteprocRsLocaleSlot CiteprocRsLocaleSlot;

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
struct CiteprocRsDriver *citeproc_rs_driver_new(struct CiteprocRsInitOptions init) CF_SWIFT_NAME(citeproc_rs_driver_new(init:));

/**
 * Frees a Processor.
 */
void citeproc_rs_driver_free(struct CiteprocRsDriver *driver) CF_SWIFT_NAME(citeproc_rs_driver_free(driver:));

/**
 * Frees a string returned from  API.
 */
void citeproc_rs_string_free(char *ptr) CF_SWIFT_NAME(citeproc_rs_string_free(ptr:));

/**
 * let reference: [String: Any] = [ "id": "blah", "type": "book", ... ];
 * in Swift, JSONSerialization.data(reference).withUnsafeBytes({ rBytes in
 *     format_one(driver, rBytes.baseAddress, rBytes.count)
 * })
 *
 * May return null.
 */
char *citeproc_rs_driver_format_one(struct CiteprocRsDriver *driver,
                                    const char *ref_bytes,
                                    uintptr_t ref_bytes_len) CF_SWIFT_NAME(citeproc_rs_driver_format_one(driver:ref_bytes:ref_bytes_len:));

/**
 * Clear the `LAST_ERROR`.
 */
void citeproc_rs_clear_last_error(void) CF_SWIFT_NAME(citeproc_rs_clear_last_error());

/**
 * Return the error code for the last error. If you clear the error, this will give you
 * [ErrorCode::None] (= `0`).
 */
CiteprocRsErrorCode citeproc_rs_last_error_code(void) CF_SWIFT_NAME(citeproc_rs_last_error_code());

/**
 * Get the length of the last error message in bytes when encoded as UTF-8,
 * including the trailing null. If the error is cleared, this returns 0.
 */
uintptr_t citeproc_rs_last_error_length(void) CF_SWIFT_NAME(citeproc_rs_last_error_length());

/**
 * Get the length of the last error message in bytes when encoded as UTF-16,
 * including the trailing null.
 */
uintptr_t citeproc_rs_last_error_length_utf16(void) CF_SWIFT_NAME(citeproc_rs_last_error_length_utf16());

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
intptr_t citeproc_rs_error_message_utf8(char *buf,
                                        uintptr_t length) CF_SWIFT_NAME(citeproc_rs_error_message_utf8(buf:length:));

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
intptr_t citeproc_rs_error_message_utf16(uint16_t *buf,
                                         uintptr_t length) CF_SWIFT_NAME(citeproc_rs_error_message_utf16(buf:length:));

int32_t viva_la_funcion(struct CiteprocRsCoolStruct *arg,
                        int32_t other_arg) CF_SWIFT_NAME(viva_la_funcion(arg:other_arg:));

#endif /* _CITEPROC_RS_H */
