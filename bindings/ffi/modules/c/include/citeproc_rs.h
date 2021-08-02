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
  CITEPROC_RS_ERROR_CODE_BUFFER_OPS = 6,
  CITEPROC_RS_ERROR_CODE_NULL_BYTE = 7,
  CITEPROC_RS_ERROR_CODE_SERDE_JSON = 8,
};
typedef int32_t citeproc_rs_error_code;

enum citeproc_rs_output_format {
  CITEPROC_RS_OUTPUT_FORMAT_HTML,
  CITEPROC_RS_OUTPUT_FORMAT_RTF,
  CITEPROC_RS_OUTPUT_FORMAT_PLAIN,
};
typedef uint8_t citeproc_rs_output_format;

/**
 * Wrapper for a driver, initialized with one style and any required locales.
 *
 * Not thread safe.
 *
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

/**
 * Should write src_len bytes from src into some structure referenced by user_data.
 * The bytes are guaranteed not to contain a zero.
 */
typedef void (*citeproc_rs_write_callback)(void *user_data, const uint8_t *src, uintptr_t src_len);

/**
 * Should clear the buffer in the structure referenced by user_data.
 */
typedef void (*citeproc_rs_clear_callback)(void *user_data);

/**
 * A vtable to allow citeproc-rs to manipulate your own kind of buffer for the output.
 * You could define one using realloc and the C standard library's string manipulations with your
 * own zero terminators etc, but you could also just use [MANAGED_BUFFER_OPS] and let Rust's
 * `std::ffi::CString` do the hard work.
 *
 * In C++ and other FFI-compatible higher level languages this is much easier. Just use any
 * growable string or buffer type and implement the two functions in a couple of lines each.
 *
 * You will get valid UTF-8 if you correctly write out all the bytes.
 */
typedef struct citeproc_rs_buffer_ops {
  citeproc_rs_write_callback write;
  citeproc_rs_clear_callback clear;
} citeproc_rs_buffer_ops;

typedef struct citeproc_rs_init_options {
  const char *style;
  uintptr_t style_len;
  void *locale_fetch_context;
  citeproc_rs_locale_fetch_callback locale_fetch_callback;
  citeproc_rs_output_format format;
  struct citeproc_rs_buffer_ops buffer_ops;
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
 * Frees a CString returned from an API or one written using [CSTRING_BUFFER_OPS].
 */
void citeproc_rs_string_free(char *ptr);

citeproc_rs_error_code citeproc_rs_driver_format_bibliography(struct citeproc_rs_driver *driver,
                                                              void *user_buf);

/**
 * Inserts a reference and formats a single cluster with a single cite to that reference using preview_citation_cluster.
 * The reference in the processor will be overwritten and won't be restored afterward.
 *
 * Writes the result into user_buf using the buffer_ops interface.
 *
 * Returns an error code indicative of what the LAST_ERROR will contain when checked.
 */
citeproc_rs_error_code citeproc_rs_driver_preview_reference(struct citeproc_rs_driver *driver,
                                                            const char *ref_json,
                                                            uintptr_t ref_json_len,
                                                            void *user_buf);

/**
 * Inserts a reference.
 *
 * Returns an error code indicative of what the LAST_ERROR will contain when checked.
 */
citeproc_rs_error_code citeproc_rs_driver_insert_reference(struct citeproc_rs_driver *driver,
                                                           const char *ref_json,
                                                           uintptr_t ref_json_len);

/**
 * Clear the `LAST_ERROR`.
 */
void citeproc_rs_clear_last_error(void);

/**
 * Returns either [ErrorCode::None] or [ErrorCode::BufferOps].
 */
citeproc_rs_error_code citeproc_rs_last_error_utf8(struct citeproc_rs_buffer_ops buffer_ops,
                                                   void *user_data);

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

/**
 * If you use this as your buffer_write_callback, then you must call `citeproc_rs_string_free` on
 * the resulting buffers.
 */
void citeproc_rs_cstring_write(void *user_data, const uint8_t *src, uintptr_t src_len);

void citeproc_rs_cstring_clear(void *user_data);

#define citeproc_rs_managed_buffer_ops (citeproc_rs_buffer_ops){ .write = citeproc_rs_cstring_write, .clear = citeproc_rs_cstring_clear }

#endif /* _CITEPROC_RS_H */
