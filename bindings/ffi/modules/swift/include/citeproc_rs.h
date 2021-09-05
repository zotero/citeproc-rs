#define CF_SWIFT_NAME(_name) __attribute__((swift_name(#_name)))

#ifndef _CITEPROC_RS_H
#define _CITEPROC_RS_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <CoreFoundation/CoreFoundation.h>

typedef CF_ENUM(int32_t, CRErrorCode) {
  CRErrorCode_None = 0,
  CRErrorCode_NullPointer = 1,
  CRErrorCode_CaughtPanic = 2,
  CRErrorCode_Poisoned = 3,
  CRErrorCode_Utf8 = 4,
  CRErrorCode_Reordering = 5,
  CRErrorCode_BufferOps = 6,
  CRErrorCode_NullByte = 7,
  CRErrorCode_SerdeJson = 8,
  CRErrorCode_Indexing = 9,
  CRErrorCode_ClusterNotInFlow = 10,
  CRErrorCode_InvalidStyle = 11,
  CRErrorCode_SetLogger = 12,
};

typedef CF_ENUM(uintptr_t, CRLevelFilter) {
  CRLevelFilter_Off,
  /**
   * Corresponds to the `Error` log level.
   */
  CRLevelFilter_Error,
  /**
   * Corresponds to the `Warn` log level.
   */
  CRLevelFilter_Warn,
  /**
   * Corresponds to the `Info` log level.
   */
  CRLevelFilter_Info,
  /**
   * Corresponds to the `Debug` log level.
   */
  CRLevelFilter_Debug,
  /**
   * Corresponds to the `Trace` log level.
   */
  CRLevelFilter_Trace,
};

typedef CF_ENUM(uint32_t, CRLocatorType) {
  CRLocatorType_Book,
  CRLocatorType_Chapter,
  CRLocatorType_Column,
  CRLocatorType_Figure,
  CRLocatorType_Folio,
  CRLocatorType_Issue,
  CRLocatorType_Line,
  CRLocatorType_Note,
  CRLocatorType_Opus,
  CRLocatorType_Page,
  CRLocatorType_Paragraph,
  CRLocatorType_Part,
  CRLocatorType_Section,
  CRLocatorType_SubVerbo,
  CRLocatorType_Verse,
  CRLocatorType_Volume,
  CRLocatorType_Article,
  CRLocatorType_Subparagraph,
  CRLocatorType_Rule,
  CRLocatorType_Subsection,
  CRLocatorType_Schedule,
  CRLocatorType_Title,
  CRLocatorType_Unpublished,
  CRLocatorType_Supplement,
};

typedef CF_ENUM(uintptr_t, CRLogLevel) {
  CRLogLevel_Error = 1,
  CRLogLevel_Warn,
  CRLogLevel_Info,
  CRLogLevel_Debug,
  CRLogLevel_Trace,
};

typedef CF_ENUM(uint8_t, CROutputFormat) {
  CROutputFormat_Html,
  CROutputFormat_Rtf,
  CROutputFormat_Plain,
};

/**
 * An opaque, boxed wrapper for a [citeproc::prelude::Cluster].
 */
typedef struct CRCluster CRCluster;

/**
 * Wrapper for a driver, initialized with one style and any required locales.
 *
 * Not thread safe.
 *
 * Contains an Option<citeproc_rs::Processor>, because to survive panics we want to be able to
 * write a safe value that won't be in an inconsistent state after panicking.
 */
typedef struct CRDriver CRDriver;

typedef struct CRLocaleSlot CRLocaleSlot;

/**
 * A callback signature that is expected to write a string into `slot` via
 * [citeproc_rs_locale_slot_write]
 */
typedef void (*CRLocaleFetchCallback)(void *context, struct CRLocaleSlot *slot, const char*);

/**
 * Should write src_len bytes from src into some structure referenced by user_data.
 * The bytes are guaranteed not to contain a zero.
 */
typedef void (*CRWriteCallback)(void *user_data, const uint8_t *src, uintptr_t src_len);

/**
 * Should clear the buffer in the structure referenced by user_data.
 */
typedef void (*CRClearCallback)(void *user_data);

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
typedef struct CRBufferOps {
  CRWriteCallback write;
  CRClearCallback clear;
} CRBufferOps;

typedef struct CRInitOptions {
  const char *style;
  uintptr_t style_len;
  void *locale_fetch_context;
  CRLocaleFetchCallback locale_fetch_callback;
  CROutputFormat format;
  struct CRBufferOps buffer_ops;
} CRInitOptions;

/**
 * A number identifying a cluster.
 */
typedef uint32_t CRClusterId;

typedef struct CRClusterPosition {
  bool is_preview_marker;
  /**
   * Ignored if is_preview_marker is set
   */
  CRClusterId id;
  /**
   * The alternative (false) is to be in-text.
   */
  bool is_note;
  /**
   * Ignored if is_note is NOT set
   */
  uint32_t note_number;
} CRClusterPosition;

typedef void (*CRLoggerWriteCallback)(void *user_data, CRLogLevel level, const uint8_t *module_path, uintptr_t module_path_len, const uint8_t *src, uintptr_t src_len);

typedef void (*CRLoggerFlushCallback)(void *user_data);

typedef struct CRFFILoggerVTable {
  CRLoggerWriteCallback write;
  CRLoggerFlushCallback flush;
} CRFFILoggerVTable;

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
typedef int64_t CRU32OrError;

/**
 * Initialises the Rust `log` crate globally. No-op when called a second time.
 */
void citeproc_rs_log_init(void) CF_SWIFT_NAME(citeproc_rs_log_init());

/**
 * Write an XML string into a LocaleSlot. Returns an error code if the XML does not parse cleanly.
 *
 * # Safety:
 *
 * Only safe to use inside a [LocaleFetchCallback]. You must pass the slot pointer from the
 * arguments to the callback.
 */
CRErrorCode citeproc_rs_locale_slot_write(struct CRLocaleSlot *slot,
                                          const char *locale_xml,
                                          uintptr_t locale_xml_len) CF_SWIFT_NAME(citeproc_rs_locale_slot_write(slot:locale_xml:locale_xml_len:));

/**
 * Creates a new Processor from InitOptions. Free with [citeproc_rs_driver_free].
 */
struct CRDriver *citeproc_rs_driver_new(struct CRInitOptions init) CF_SWIFT_NAME(citeproc_rs_driver_new(init:));

/**
 * Frees a [Driver].
 *
 * # Safety
 *
 * The driver must either be from [citeproc_rs_driver_new] or be null.
 */
void citeproc_rs_driver_free(struct CRDriver *driver) CF_SWIFT_NAME(citeproc_rs_driver_free(driver:));

/**
 * [citeproc::Processor::set_cluster_order], but using an ffi-compatible [ClusterPosition]
 *
 * # Safety
 *
 * Driver must be a valid pointer to a Driver.
 *
 * positions/positions_len must point to a valid array of ClusterPosition.
 */
CRErrorCode citeproc_rs_driver_set_cluster_order(struct CRDriver *driver,
                                                 const struct CRClusterPosition *positions,
                                                 uintptr_t positions_len) CF_SWIFT_NAME(citeproc_rs_driver_set_cluster_order(driver:positions:positions_len:));

/**
 * Writes a formatted cluster ([citeproc::Processor::get_cluster]) into a buffer.
 *
 * # Safety
 *
 *
 */
CRErrorCode citeproc_rs_driver_format_cluster(struct CRDriver *driver,
                                              CRClusterId cluster_id,
                                              void *user_buf) CF_SWIFT_NAME(citeproc_rs_driver_format_cluster(driver:cluster_id:user_buf:));

/**
 * Writes a bibliography into a buffer, using [citeproc::Processor::get_bibliography]
 */
CRErrorCode citeproc_rs_driver_format_bibliography(struct CRDriver *driver,
                                                   void *user_buf) CF_SWIFT_NAME(citeproc_rs_driver_format_bibliography(driver:user_buf:));

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
CRErrorCode citeproc_rs_driver_preview_reference(struct CRDriver *driver,
                                                 const char *ref_json,
                                                 uintptr_t ref_json_len,
                                                 CROutputFormat format,
                                                 void *user_buf) CF_SWIFT_NAME(citeproc_rs_driver_preview_reference(driver:ref_json:ref_json_len:format:user_buf:));

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
CRErrorCode citeproc_rs_driver_insert_reference(struct CRDriver *driver,
                                                const char *ref_json,
                                                uintptr_t ref_json_len) CF_SWIFT_NAME(citeproc_rs_driver_insert_reference(driver:ref_json:ref_json_len:));

CRErrorCode test_panic(void) CF_SWIFT_NAME(test_panic());

CRErrorCode test_panic_poison_driver(struct CRDriver *_driver) CF_SWIFT_NAME(test_panic_poison_driver(_driver:));

/**
 * Clear the last error (thread local).
 */
void citeproc_rs_last_error_clear(void) CF_SWIFT_NAME(citeproc_rs_last_error_clear());

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
CRErrorCode citeproc_rs_last_error_utf8(struct CRBufferOps buffer_ops,
                                        void *user_data) CF_SWIFT_NAME(citeproc_rs_last_error_utf8(buffer_ops:user_data:));

/**
 * Return the error code for the last error. If you clear the error, this will give you
 * [ErrorCode::None] (= `0`).
 */
CRErrorCode citeproc_rs_last_error_code(void) CF_SWIFT_NAME(citeproc_rs_last_error_code());

/**
 * Get the length of the last error message (thread local) in bytes when encoded as UTF-16,
 * including the trailing null.
 */
uintptr_t citeproc_rs_last_error_length_utf16(void) CF_SWIFT_NAME(citeproc_rs_last_error_length_utf16());

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
intptr_t citeproc_rs_last_error_utf16(uint16_t *buf,
                                      uintptr_t length) CF_SWIFT_NAME(citeproc_rs_last_error_utf16(buf:length:));

/**
 *
 * # Safety
 *
 * Instance must remain alive for the rest of the program's execution, and it also must be safe to
 * send across threads and to access from multiple concurrent threads.
 */
CRErrorCode citeproc_rs_set_logger(void *instance,
                                   struct CRFFILoggerVTable vtable,
                                   CRLevelFilter min_severity,
                                   const char *filters,
                                   uintptr_t filters_len) CF_SWIFT_NAME(citeproc_rs_set_logger(instance:vtable:min_severity:filters:filters_len:));

void test_log_msg(CRLogLevel level,
                  const char *msg,
                  uintptr_t msg_len) CF_SWIFT_NAME(test_log_msg(level:msg:msg_len:));

/**
 * Frees a FFI-consumer-owned CString written using [CSTRING_BUFFER_OPS].
 */
void citeproc_rs_cstring_free(char *ptr) CF_SWIFT_NAME(citeproc_rs_cstring_free(ptr:));

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
void citeproc_rs_cstring_write(void *user_data,
                               const uint8_t *src,
                               uintptr_t src_len) CF_SWIFT_NAME(citeproc_rs_cstring_write(user_data:src:src_len:));

/**
 * Provides BufferOps.clear for the CString implementation.
 *
 * ## Safety
 */
void citeproc_rs_cstring_clear(void *user_data) CF_SWIFT_NAME(citeproc_rs_cstring_clear(user_data:));

/**
 * Creates a new cluster with the given cluster id. Free with [citeproc_rs_cluster_free].
 */
struct CRCluster *citeproc_rs_cluster_new(CRClusterId id) CF_SWIFT_NAME(citeproc_rs_cluster_new(id:));

/**
 * Deallocates a cluster.
 *
 * # Safety
 *
 * The cluster must be from [citeproc_rs_cluster_new] and not freed.
 */
CRErrorCode citeproc_rs_cluster_free(struct CRCluster *cluster) CF_SWIFT_NAME(citeproc_rs_cluster_free(cluster:));

/**
 * Removes all data and sets a new id on the cluster.
 *
 * # Safety
 *
 * The cluster must be from [citeproc_rs_cluster_new] and not freed.
 */
CRErrorCode citeproc_rs_cluster_reset(struct CRCluster *cluster,
                                      CRClusterId new_id) CF_SWIFT_NAME(citeproc_rs_cluster_reset(cluster:new_id:));

/**
 * Sets the id of the given cluster object.
 *
 * # Safety
 *
 * The cluster must be from [citeproc_rs_cluster_new] and not freed.
 */
CRErrorCode citeproc_rs_cluster_set_id(struct CRCluster *cluster,
                                       CRClusterId id) CF_SWIFT_NAME(citeproc_rs_cluster_set_id(cluster:id:));

/**
 * Sets the reference id for a cite.
 *
 * # Safety
 *
 * The cluster must be from [citeproc_rs_cluster_new] and not freed.
 *
 * Either `ref_id` must refer to a byte array of length `ref_id_len`, or `ref_id_len` must be zero.
 */
CRErrorCode citeproc_rs_cluster_cite_set_ref(struct CRCluster *cluster,
                                             uintptr_t cite_index,
                                             const char *ref_id,
                                             uintptr_t ref_id_len) CF_SWIFT_NAME(citeproc_rs_cluster_cite_set_ref(cluster:cite_index:ref_id:ref_id_len:));

/**
 * Returns either an index (>=0) representing the position of a newly created cite within a
 * cluster, or a negative error code.
 *
 * # Safety
 *
 * The cluster must be from [citeproc_rs_cluster_new] and not freed.
 * The ref_id pair must refer to a valid byte array.
 */
CRU32OrError citeproc_rs_cluster_cite_new(struct CRCluster *cluster,
                                          const char *ref_id,
                                          uintptr_t ref_id_len) CF_SWIFT_NAME(citeproc_rs_cluster_cite_new(cluster:ref_id:ref_id_len:));

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
CRU32OrError citeproc_rs_driver_intern_cluster_id(struct CRDriver *driver,
                                                  const char *str,
                                                  uintptr_t str_len) CF_SWIFT_NAME(citeproc_rs_driver_intern_cluster_id(driver:str:str_len:));

/**
 * Writes a random cluster_id into user_buf, and returns a ClusterId that represents it.
 * [citeproc::Processor::random_cluster_id]
 *
 * Useful for allocating string ids to citation clusters in a real document, that need to be
 * read back later.
 */
CRU32OrError citeproc_rs_driver_random_cluster_id(struct CRDriver *driver,
                                                  void *user_buf) CF_SWIFT_NAME(citeproc_rs_driver_random_cluster_id(driver:user_buf:));

/**
 * Inserts a cluster, overwriting any previously written cluster with that ID.
 * [citeproc::Processor::insert_cluster]
 *
 * # Safety
 *
 * Driver must be from [citeproc_rs_driver_new]. The cluster must be from
 * [citeproc_rs_cluster_new].
 */
CRErrorCode citeproc_rs_driver_insert_cluster(struct CRDriver *driver,
                                              const struct CRCluster *cluster) CF_SWIFT_NAME(citeproc_rs_driver_insert_cluster(driver:cluster:));

/**
 * Sets the string locator and [LocatorType] for a cite.
 */
CRErrorCode citeproc_rs_cluster_cite_set_locator(struct CRCluster *cluster,
                                                 uintptr_t cite_index,
                                                 const char *locator,
                                                 uintptr_t locator_len,
                                                 CRLocatorType loc_type) CF_SWIFT_NAME(citeproc_rs_cluster_cite_set_locator(cluster:cite_index:locator:locator_len:loc_type:));

/**
 * Sets the string prefix for a cite.
 */
CRErrorCode citeproc_rs_cluster_cite_set_prefix(struct CRCluster *cluster,
                                                uintptr_t cite_index,
                                                const char *prefix,
                                                uintptr_t prefix_len) CF_SWIFT_NAME(citeproc_rs_cluster_cite_set_prefix(cluster:cite_index:prefix:prefix_len:));

/**
 * Sets the string suffix on a cite. Pass in a zero length string for no suffix.
 */
CRErrorCode citeproc_rs_cluster_cite_set_suffix(struct CRCluster *cluster,
                                                uintptr_t cite_index,
                                                const char *suffix,
                                                uintptr_t suffix_len) CF_SWIFT_NAME(citeproc_rs_cluster_cite_set_suffix(cluster:cite_index:suffix:suffix_len:));

/**
 * If you use this as your buffer_write_callback, then you must call [citeproc_rs_cstring_free] on
 * the resulting buffers, or the memory will leak.
 *
 */
#define CRCSTRING_BUFFER_OPS (CRBufferOps){ .write = citeproc_rs_cstring_write, .clear = citeproc_rs_cstring_clear }

#endif /* _CITEPROC_RS_H */
