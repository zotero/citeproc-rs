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
  CITEPROC_RS_ERROR_CODE_INDEXING = 9,
  CITEPROC_RS_ERROR_CODE_CLUSTER_NOT_IN_FLOW = 10,
  CITEPROC_RS_ERROR_CODE_INVALID_STYLE = 11,
  CITEPROC_RS_ERROR_CODE_SET_LOGGER = 12,
};
typedef int32_t citeproc_rs_error_code;

enum citeproc_rs_level_filter {
  CITEPROC_RS_LEVEL_FILTER_OFF,
  /**
   * Corresponds to the `Error` log level.
   */
  CITEPROC_RS_LEVEL_FILTER_ERROR,
  /**
   * Corresponds to the `Warn` log level.
   */
  CITEPROC_RS_LEVEL_FILTER_WARN,
  /**
   * Corresponds to the `Info` log level.
   */
  CITEPROC_RS_LEVEL_FILTER_INFO,
  /**
   * Corresponds to the `Debug` log level.
   */
  CITEPROC_RS_LEVEL_FILTER_DEBUG,
  /**
   * Corresponds to the `Trace` log level.
   */
  CITEPROC_RS_LEVEL_FILTER_TRACE,
};
typedef uintptr_t citeproc_rs_level_filter;

enum citeproc_rs_locator_type {
  CITEPROC_RS_LOCATOR_TYPE_BOOK,
  CITEPROC_RS_LOCATOR_TYPE_CHAPTER,
  CITEPROC_RS_LOCATOR_TYPE_COLUMN,
  CITEPROC_RS_LOCATOR_TYPE_FIGURE,
  CITEPROC_RS_LOCATOR_TYPE_FOLIO,
  CITEPROC_RS_LOCATOR_TYPE_ISSUE,
  CITEPROC_RS_LOCATOR_TYPE_LINE,
  CITEPROC_RS_LOCATOR_TYPE_NOTE,
  CITEPROC_RS_LOCATOR_TYPE_OPUS,
  CITEPROC_RS_LOCATOR_TYPE_PAGE,
  CITEPROC_RS_LOCATOR_TYPE_PARAGRAPH,
  CITEPROC_RS_LOCATOR_TYPE_PART,
  CITEPROC_RS_LOCATOR_TYPE_SECTION,
  CITEPROC_RS_LOCATOR_TYPE_SUB_VERBO,
  CITEPROC_RS_LOCATOR_TYPE_VERSE,
  CITEPROC_RS_LOCATOR_TYPE_VOLUME,
  CITEPROC_RS_LOCATOR_TYPE_ARTICLE,
  CITEPROC_RS_LOCATOR_TYPE_SUBPARAGRAPH,
  CITEPROC_RS_LOCATOR_TYPE_RULE,
  CITEPROC_RS_LOCATOR_TYPE_SUBSECTION,
  CITEPROC_RS_LOCATOR_TYPE_SCHEDULE,
  CITEPROC_RS_LOCATOR_TYPE_TITLE,
  CITEPROC_RS_LOCATOR_TYPE_UNPUBLISHED,
  CITEPROC_RS_LOCATOR_TYPE_SUPPLEMENT,
};
typedef uint32_t citeproc_rs_locator_type;

enum citeproc_rs_log_level {
  CITEPROC_RS_LOG_LEVEL_ERROR = 1,
  CITEPROC_RS_LOG_LEVEL_WARN,
  CITEPROC_RS_LOG_LEVEL_INFO,
  CITEPROC_RS_LOG_LEVEL_DEBUG,
  CITEPROC_RS_LOG_LEVEL_TRACE,
};
typedef uintptr_t citeproc_rs_log_level;

enum citeproc_rs_output_format {
  CITEPROC_RS_OUTPUT_FORMAT_HTML,
  CITEPROC_RS_OUTPUT_FORMAT_RTF,
  CITEPROC_RS_OUTPUT_FORMAT_PLAIN,
};
typedef uint8_t citeproc_rs_output_format;

/**
 * An opaque, boxed wrapper for a [citeproc::prelude::Cluster].
 */
typedef struct citeproc_rs_cluster citeproc_rs_cluster;

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
 *
 * You could define one using realloc and the C standard library's string manipulations with your
 * own zero terminators etc, but you could also just use [cstring::CSTRING_BUFFER_OPS] and let Rust's
 * `std::ffi::CString` do the hard work.
 *
 * In C++ and other FFI-compatible higher level languages this is much easier. Just use any
 * growable string or buffer type and implement the two functions in a couple of lines each.
 *
 * You will get valid UTF-8 if you correctly write out all the bytes.
 *
 * ## Safety
 *
 * When using BufferOps, the only thing you *must* ensure is that the callback functions access
 * the user data pointer consistently with the actual user data pointers passed to Rust.
 *
 * If your write callback expects a `char **`, then you must supply a `char **`. If your write
 * callback expects a C++ `std::string *`, then you must supply a `std::string *`.
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

/**
 * A number identifying a cluster.
 */
typedef uint32_t citeproc_rs_cluster_id;

typedef struct citeproc_rs_cluster_position {
  bool is_preview_marker;
  /**
   * Ignored if is_preview_marker is set
   */
  citeproc_rs_cluster_id id;
  /**
   * The alternative (false) is to be in-text.
   */
  bool is_note;
  /**
   * Ignored if is_note is NOT set
   */
  uint32_t note_number;
} citeproc_rs_cluster_position;

typedef void (*citeproc_rs_logger_write_callback)(void *user_data, citeproc_rs_log_level level, const uint8_t *module_path, uintptr_t module_path_len, const uint8_t *src, uintptr_t src_len);

typedef void (*citeproc_rs_logger_flush_callback)(void *user_data);

typedef struct citeproc_rs_ffi_logger_v_table {
  citeproc_rs_logger_write_callback write;
  citeproc_rs_logger_flush_callback flush;
} citeproc_rs_ffi_logger_v_table;

/**
 * Either a positive-or-0 u32, or a negative ErrorCode. Represented as an i64.
 *
 * ```ignore
 *
 * // Laborious but thoroughly correct example
 * citeproc_rs_error_code code = CITEPROC_RS_ERROR_CODE_NONE;
 * char *error_message;
 * uint32_t result;
 *
 * int64_t ret = some_api(...);
 *
 * if (ret < 0) {
 *     code = (citeproc_rs_error_code)(-ret);
 *     citeproc_rs_last_error_utf8(citeproc_rs_cstring_buffer_ops, &error_message);
 *     printf("%s\n", error_message);
 *     citeproc_rs_cstring_free(error_message);
 *     return -1;
 * } else {
 *     result = (int32_t) ret;
 * }
 * ```
 */
typedef int64_t citeproc_rs_u32_or_error;

/**
 * Initialises the Rust `log` crate globally. No-op when called a second time.
 */
void citeproc_rs_log_init(void);

/**
 * Write an XML string into a LocaleSlot. Returns an error code if the XML does not parse cleanly.
 *
 * # Safety:
 *
 * Only safe to use inside a [LocaleFetchCallback]. You must pass the slot pointer from the
 * arguments to the callback.
 */
citeproc_rs_error_code citeproc_rs_locale_slot_write(struct citeproc_rs_locale_slot *slot,
                                                     const char *locale_xml,
                                                     uintptr_t locale_xml_len);

/**
 * Creates a new Processor from InitOptions. Free with [citeproc_rs_driver_free].
 */
struct citeproc_rs_driver *citeproc_rs_driver_new(struct citeproc_rs_init_options init);

/**
 * Frees a [Driver].
 *
 * # Safety
 *
 * The driver must either be from [citeproc_rs_driver_new] or be null.
 */
void citeproc_rs_driver_free(struct citeproc_rs_driver *driver);

/**
 * [citeproc::Processor::set_cluster_order], but using an ffi-compatible [ClusterPosition]
 *
 * # Safety
 *
 * Driver must be a valid pointer to a Driver.
 *
 * positions/positions_len must point to a valid array of ClusterPosition.
 */
citeproc_rs_error_code citeproc_rs_driver_set_cluster_order(struct citeproc_rs_driver *driver,
                                                            const struct citeproc_rs_cluster_position *positions,
                                                            uintptr_t positions_len);

/**
 * Writes a formatted cluster ([citeproc::Processor::get_cluster]) into a buffer.
 *
 * # Safety
 *
 *
 */
citeproc_rs_error_code citeproc_rs_driver_format_cluster(struct citeproc_rs_driver *driver,
                                                         citeproc_rs_cluster_id cluster_id,
                                                         void *user_buf);

/**
 * Writes a bibliography into a buffer, using [citeproc::Processor::get_bibliography]
 */
citeproc_rs_error_code citeproc_rs_driver_format_bibliography(struct citeproc_rs_driver *driver,
                                                              void *user_buf);

/**
 * Formats a bibliography entry for a given reference.
 *
 * Writes the result into user_buf using the buffer_ops interface.
 *
 * Returns an error code indicative of what the LAST_ERROR will contain when checked.
 *
 * # Safety
 *
 * Same as [citeproc_rs_driver_insert_reference], but `user_buf` must also match the expected user data in the BufferOps struct passed to driver's init call.
 */
citeproc_rs_error_code citeproc_rs_driver_preview_reference(struct citeproc_rs_driver *driver,
                                                            const char *ref_json,
                                                            uintptr_t ref_json_len,
                                                            citeproc_rs_output_format format,
                                                            void *user_buf);

/**
 * Inserts a reference. [citeproc::Processor::insert_reference]
 *
 * Returns an error code.
 *
 * # Safety
 *
 * `driver` must be a valid pointer to a Driver.
 *
 * Either `ref_json` must refer to a byte array of length `ref_json_len`, or `ref_json_len` must be zero.
 */
citeproc_rs_error_code citeproc_rs_driver_insert_reference(struct citeproc_rs_driver *driver,
                                                           const char *ref_json,
                                                           uintptr_t ref_json_len);

/**
 * Clear the last error (thread local).
 */
void citeproc_rs_last_error_clear(void);

/**
 * Peek at the last error (thread local) and write its Display string using the [crate::buffer] system.
 *
 * Accepts a struct of buffer operations and a pointer to the user's buffer instance.
 *
 * Returns either [ErrorCode::None] (success) or [ErrorCode::BufferOps] (failure, because of a
 * nul byte somewhere in the error message itself).
 *
 * ## Safety
 *
 * Refer to [crate::buffer::BufferOps]
 */
citeproc_rs_error_code citeproc_rs_last_error_utf8(struct citeproc_rs_buffer_ops buffer_ops,
                                                   void *user_data);

/**
 * Return the error code for the last error. If you clear the error, this will give you
 * [ErrorCode::None] (= `0`).
 */
citeproc_rs_error_code citeproc_rs_last_error_code(void);

/**
 * Get the length of the last error message (thread local) in bytes when encoded as UTF-16,
 * including the trailing null.
 */
uintptr_t citeproc_rs_last_error_length_utf16(void);

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
intptr_t citeproc_rs_last_error_utf16(uint16_t *buf, uintptr_t length);

/**
 *
 * # Safety
 *
 * Instance must remain alive for the rest of the program's execution, and it also must be safe to
 * send across threads and to access from multiple concurrent threads.
 */
citeproc_rs_error_code citeproc_rs_set_logger(void *instance,
                                              struct citeproc_rs_ffi_logger_v_table vtable,
                                              citeproc_rs_level_filter min_severity,
                                              const char *filters,
                                              uintptr_t filters_len);

/**
 * Frees a FFI-consumer-owned CString written using [CSTRING_BUFFER_OPS].
 */
void citeproc_rs_cstring_free(char *ptr);

/**
 * Provides BufferOps.write for the CString implementation.
 *
 * ## Safety
 *
 * Only safe to call with a `user_data` that is a **valid pointer to a pointer**. The inner
 * pointer should be either
 *
 * * `NULL`; or
 * * a pointer returned from `CString::into_raw`.
 *
 * The src/src_len must represent a valid &[u8] structure.
 */
void citeproc_rs_cstring_write(void *user_data, const uint8_t *src, uintptr_t src_len);

/**
 * Provides BufferOps.clear for the CString implementation.
 *
 * ## Safety
 */
void citeproc_rs_cstring_clear(void *user_data);

/**
 * Creates a new cluster with the given cluster id. Free with [citeproc_rs_cluster_free].
 */
struct citeproc_rs_cluster *citeproc_rs_cluster_new(citeproc_rs_cluster_id id);

/**
 * Deallocates a cluster.
 *
 * # Safety
 *
 * The cluster must be from [citeproc_rs_cluster_new] and not freed.
 */
citeproc_rs_error_code citeproc_rs_cluster_free(struct citeproc_rs_cluster *cluster);

/**
 * Removes all data and sets a new id on the cluster.
 *
 * # Safety
 *
 * The cluster must be from [citeproc_rs_cluster_new] and not freed.
 */
citeproc_rs_error_code citeproc_rs_cluster_reset(struct citeproc_rs_cluster *cluster,
                                                 citeproc_rs_cluster_id new_id);

/**
 * Sets the id of the given cluster object.
 *
 * # Safety
 *
 * The cluster must be from [citeproc_rs_cluster_new] and not freed.
 */
citeproc_rs_error_code citeproc_rs_cluster_set_id(struct citeproc_rs_cluster *cluster,
                                                  citeproc_rs_cluster_id id);

/**
 * Sets the reference id for a cite.
 *
 * # Safety
 *
 * The cluster must be from [citeproc_rs_cluster_new] and not freed.
 *
 * Either `ref_id` must refer to a byte array of length `ref_id_len`, or `ref_id_len` must be zero.
 */
citeproc_rs_error_code citeproc_rs_cluster_cite_set_ref(struct citeproc_rs_cluster *cluster,
                                                        uintptr_t cite_index,
                                                        const char *ref_id,
                                                        uintptr_t ref_id_len);

/**
 * Returns either an index (>=0) representing the position of a newly created cite within a
 * cluster, or a negative error code.
 *
 * # Safety
 *
 * The cluster must be from [citeproc_rs_cluster_new] and not freed.
 * The ref_id pair must refer to a valid byte array.
 */
citeproc_rs_u32_or_error citeproc_rs_cluster_cite_new(struct citeproc_rs_cluster *cluster,
                                                      const char *ref_id,
                                                      uintptr_t ref_id_len);

/**
 * Interns a cluster id. Returns -1 on error, hence the i64 return type; [ClusterId] is
 * actually a u32, so you can cast it safely after checking for -1.
 *
 * Returns an error code indicative of what the LAST_ERROR will contain when checked.
 *
 * # Safety
 *
 * Either `str` must refer to a byte array of length `str_len`, or `str_len` must be zero.
 */
citeproc_rs_u32_or_error citeproc_rs_driver_intern_cluster_id(struct citeproc_rs_driver *driver,
                                                              const char *str,
                                                              uintptr_t str_len);

/**
 * Writes a random cluster_id into user_buf, and returns a ClusterId that represents it.
 * [citeproc::Processor::random_cluster_id]
 *
 * Useful for allocating string ids to citation clusters in a real document, that need to be
 * read back later.
 */
citeproc_rs_u32_or_error citeproc_rs_driver_random_cluster_id(struct citeproc_rs_driver *driver,
                                                              void *user_buf);

/**
 * Inserts a cluster, overwriting any previously written cluster with that ID.
 * [citeproc::Processor::insert_cluster]
 *
 * # Safety
 *
 * Driver must be from [citeproc_rs_driver_new]. The cluster must be from
 * [citeproc_rs_cluster_new].
 */
citeproc_rs_error_code citeproc_rs_driver_insert_cluster(struct citeproc_rs_driver *driver,
                                                         const struct citeproc_rs_cluster *cluster);

/**
 * Sets the string locator and [LocatorType] for a cite.
 */
citeproc_rs_error_code citeproc_rs_cluster_cite_set_locator(struct citeproc_rs_cluster *cluster,
                                                            uintptr_t cite_index,
                                                            const char *locator,
                                                            uintptr_t locator_len,
                                                            citeproc_rs_locator_type loc_type);

/**
 * Sets the string prefix for a cite.
 */
citeproc_rs_error_code citeproc_rs_cluster_cite_set_prefix(struct citeproc_rs_cluster *cluster,
                                                           uintptr_t cite_index,
                                                           const char *prefix,
                                                           uintptr_t prefix_len);

/**
 * Sets the string suffix on a cite. Pass in a zero length string for no suffix.
 */
citeproc_rs_error_code citeproc_rs_cluster_cite_set_suffix(struct citeproc_rs_cluster *cluster,
                                                           uintptr_t cite_index,
                                                           const char *suffix,
                                                           uintptr_t suffix_len);

/**
 * If you use this as your buffer_write_callback, then you must call [citeproc_rs_cstring_free] on
 * the resulting buffers, or the memory will leak.
 *
 */
#define citeproc_rs_cstring_buffer_ops (citeproc_rs_buffer_ops){ .write = citeproc_rs_cstring_write, .clear = citeproc_rs_cstring_clear }

#endif /* _CITEPROC_RS_H */
