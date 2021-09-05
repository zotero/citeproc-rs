#ifndef _CITEPROC_RS_HPP
#define _CITEPROC_RS_HPP

#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

namespace citeproc_rs {

enum class ErrorCode : int32_t {
  none = 0,
  null_pointer = 1,
  caught_panic = 2,
  poisoned = 3,
  utf8 = 4,
  reordering = 5,
  buffer_ops = 6,
  null_byte = 7,
  serde_json = 8,
  indexing = 9,
  cluster_not_in_flow = 10,
  invalid_style = 11,
  set_logger = 12,
};

enum class LevelFilter : uintptr_t {
  off,
  /// Corresponds to the `Error` log level.
  error,
  /// Corresponds to the `Warn` log level.
  warn,
  /// Corresponds to the `Info` log level.
  info,
  /// Corresponds to the `Debug` log level.
  debug,
  /// Corresponds to the `Trace` log level.
  trace,
};

enum class LocatorType : uint32_t {
  book,
  chapter,
  column,
  figure,
  folio,
  issue,
  line,
  note,
  opus,
  page,
  paragraph,
  part,
  section,
  sub_verbo,
  verse,
  volume,
  article,
  subparagraph,
  rule,
  subsection,
  schedule,
  title,
  unpublished,
  supplement,
};

enum class LogLevel : uintptr_t {
  error = 1,
  warn,
  info,
  debug,
  trace,
};

enum class OutputFormat : uint8_t {
  html,
  rtf,
  plain,
};

/// An opaque, boxed wrapper for a [citeproc::prelude::Cluster].
struct Cluster;

/// Wrapper for a driver, initialized with one style and any required locales.
///
/// Not thread safe.
///
/// Contains an Option<citeproc_rs::Processor>, because to survive panics we want to be able to
/// write a safe value that won't be in an inconsistent state after panicking.
struct Driver;

struct LocaleSlot;

/// A callback signature that is expected to write a string into `slot` via
/// [citeproc_rs_locale_slot_write]
using LocaleFetchCallback = void(*)(void *context, LocaleSlot *slot, const char*);

/// Should write src_len bytes from src into some structure referenced by user_data.
/// The bytes are guaranteed not to contain a zero.
using WriteCallback = void(*)(void *user_data, const uint8_t *src, uintptr_t src_len);

/// Should clear the buffer in the structure referenced by user_data.
using ClearCallback = void(*)(void *user_data);

/// A vtable to allow citeproc-rs to manipulate your own kind of buffer for the output.
///
/// You could define one using realloc and the C standard library's string manipulations with your
/// own zero terminators etc, but you could also just use [cstring::CSTRING_BUFFER_OPS] and let Rust's
/// `std::ffi::CString` do the hard work.
///
/// In C++ and other FFI-compatible higher level languages this is much easier. Just use any
/// growable string or buffer type and implement the two functions in a couple of lines each.
///
/// You will get valid UTF-8 if you correctly write out all the bytes.
///
/// ## Safety
///
/// When using BufferOps, the only thing you *must* ensure is that the callback functions access
/// the user data pointer consistently with the actual user data pointers passed to Rust.
///
/// If your write callback expects a `char **`, then you must supply a `char **`. If your write
/// callback expects a C++ `std::string *`, then you must supply a `std::string *`.
struct BufferOps {
  WriteCallback write;
  ClearCallback clear;
};

struct InitOptions {
  const char *style;
  uintptr_t style_len;
  void *locale_fetch_context;
  LocaleFetchCallback locale_fetch_callback;
  OutputFormat format;
  BufferOps buffer_ops;
};

/// A number identifying a cluster.
using ClusterId = uint32_t;

struct ClusterPosition {
  bool is_preview_marker;
  /// Ignored if is_preview_marker is set
  ClusterId id;
  /// The alternative (false) is to be in-text.
  bool is_note;
  /// Ignored if is_note is NOT set
  uint32_t note_number;
};

using LoggerWriteCallback = void(*)(void *user_data, LogLevel level, const uint8_t *module_path, uintptr_t module_path_len, const uint8_t *src, uintptr_t src_len);

using LoggerFlushCallback = void(*)(void *user_data);

struct FFILoggerVTable {
  LoggerWriteCallback write;
  LoggerFlushCallback flush;
};

/// Either a positive-or-0 u32, or a negative ErrorCode. Represented as an i64.
///
/// ```ignore
///
/// // Laborious but thoroughly correct example
/// citeproc_rs_error_code code = CITEPROC_RS_ERROR_CODE_NONE;
/// char *error_message;
/// uint32_t result;
///
/// int64_t ret = some_api(...);
///
/// if (ret < 0) {
///     code = (citeproc_rs_error_code)(-ret);
///     citeproc_rs_last_error_utf8(citeproc_rs_cstring_buffer_ops, &error_message);
///     printf("%s\n", error_message);
///     citeproc_rs_cstring_free(error_message);
///     return -1;
/// } else {
///     result = (int32_t) ret;
/// }
/// ```
using U32OrError = int64_t;

extern "C" {

/// Initialises the Rust `log` crate globally. No-op when called a second time.
 void citeproc_rs_log_init();

/// Write an XML string into a LocaleSlot. Returns an error code if the XML does not parse cleanly.
///
/// # Safety:
///
/// Only safe to use inside a [LocaleFetchCallback]. You must pass the slot pointer from the
/// arguments to the callback.

ErrorCode citeproc_rs_locale_slot_write(LocaleSlot *slot,
                                        const char *locale_xml,
                                        uintptr_t locale_xml_len);

/// Creates a new Processor from InitOptions. Free with [citeproc_rs_driver_free].
 Driver *citeproc_rs_driver_new(InitOptions init);

/// Frees a [Driver].
///
/// # Safety
///
/// The driver must either be from [citeproc_rs_driver_new] or be null.
 void citeproc_rs_driver_free(Driver *driver);

/// [citeproc::Processor::set_cluster_order], but using an ffi-compatible [ClusterPosition]
///
/// # Safety
///
/// Driver must be a valid pointer to a Driver.
///
/// positions/positions_len must point to a valid array of ClusterPosition.

ErrorCode citeproc_rs_driver_set_cluster_order(Driver *driver,
                                               const ClusterPosition *positions,
                                               uintptr_t positions_len);

/// Writes a formatted cluster ([citeproc::Processor::get_cluster]) into a buffer.
///
/// # Safety
///
///
 ErrorCode citeproc_rs_driver_format_cluster(Driver *driver, ClusterId cluster_id, void *user_buf);

/// Writes a bibliography into a buffer, using [citeproc::Processor::get_bibliography]
 ErrorCode citeproc_rs_driver_format_bibliography(Driver *driver, void *user_buf);

/// Formats a bibliography entry for a given reference.
///
/// Writes the result into user_buf using the buffer_ops interface.
///
/// Returns an error code indicative of what the LAST_ERROR will contain when checked.
///
/// # Safety
///
/// Same as [citeproc_rs_driver_insert_reference], but `user_buf` must also match the expected user data in the BufferOps struct passed to driver's init call.

ErrorCode citeproc_rs_driver_preview_reference(Driver *driver,
                                               const char *ref_json,
                                               uintptr_t ref_json_len,
                                               OutputFormat format,
                                               void *user_buf);

/// Inserts a reference. [citeproc::Processor::insert_reference]
///
/// Returns an error code.
///
/// # Safety
///
/// `driver` must be a valid pointer to a Driver.
///
/// Either `ref_json` must refer to a byte array of length `ref_json_len`, or `ref_json_len` must be zero.

ErrorCode citeproc_rs_driver_insert_reference(Driver *driver,
                                              const char *ref_json,
                                              uintptr_t ref_json_len);

/// Clear the last error (thread local).
 void citeproc_rs_last_error_clear();

/// Peek at the last error (thread local) and write its Display string using the [crate::buffer] system.
///
/// Accepts a struct of buffer operations and a pointer to the user's buffer instance.
///
/// Returns either [ErrorCode::None] (success) or [ErrorCode::BufferOps] (failure, because of a
/// nul byte somewhere in the error message itself).
///
/// ## Safety
///
/// Refer to [crate::buffer::BufferOps]

ErrorCode citeproc_rs_last_error_utf8(BufferOps buffer_ops,
                                      void *user_data);

/// Return the error code for the last error. If you clear the error, this will give you
/// [ErrorCode::None] (= `0`).
 ErrorCode citeproc_rs_last_error_code();

/// Get the length of the last error message (thread local) in bytes when encoded as UTF-16,
/// including the trailing null.
 uintptr_t citeproc_rs_last_error_length_utf16();

/// Peek at the most recent error and write its error message (`Display` impl)
/// into the provided buffer as a UTF-16 encoded string.
///
/// This returns the number of bytes written, or `-1` if there was an error.
///
/// # Safety
///
/// The provided buffer must be valid to write `length` bytes into. That's not `length`
/// UTF-16-encoded characters.
 intptr_t citeproc_rs_last_error_utf16(uint16_t *buf, uintptr_t length);

///
/// # Safety
///
/// Instance must remain alive for the rest of the program's execution, and it also must be safe to
/// send across threads and to access from multiple concurrent threads.

ErrorCode citeproc_rs_set_logger(void *instance,
                                 FFILoggerVTable vtable,
                                 LevelFilter min_severity,
                                 const char *filters,
                                 uintptr_t filters_len);

/// Frees a FFI-consumer-owned CString written using [CSTRING_BUFFER_OPS].
 void citeproc_rs_cstring_free(char *ptr);

/// Provides BufferOps.write for the CString implementation.
///
/// ## Safety
///
/// Only safe to call with a `user_data` that is a **valid pointer to a pointer**. The inner
/// pointer should be either
///
/// * `NULL`; or
/// * a pointer returned from `CString::into_raw`.
///
/// The src/src_len must represent a valid &[u8] structure.
 void citeproc_rs_cstring_write(void *user_data, const uint8_t *src, uintptr_t src_len);

/// Provides BufferOps.clear for the CString implementation.
///
/// ## Safety
 void citeproc_rs_cstring_clear(void *user_data);

/// Creates a new cluster with the given cluster id. Free with [citeproc_rs_cluster_free].
 Cluster *citeproc_rs_cluster_new(ClusterId id);

/// Deallocates a cluster.
///
/// # Safety
///
/// The cluster must be from [citeproc_rs_cluster_new] and not freed.
 ErrorCode citeproc_rs_cluster_free(Cluster *cluster);

/// Removes all data and sets a new id on the cluster.
///
/// # Safety
///
/// The cluster must be from [citeproc_rs_cluster_new] and not freed.
 ErrorCode citeproc_rs_cluster_reset(Cluster *cluster, ClusterId new_id);

/// Sets the id of the given cluster object.
///
/// # Safety
///
/// The cluster must be from [citeproc_rs_cluster_new] and not freed.
 ErrorCode citeproc_rs_cluster_set_id(Cluster *cluster, ClusterId id);

/// Sets the reference id for a cite.
///
/// # Safety
///
/// The cluster must be from [citeproc_rs_cluster_new] and not freed.
///
/// Either `ref_id` must refer to a byte array of length `ref_id_len`, or `ref_id_len` must be zero.

ErrorCode citeproc_rs_cluster_cite_set_ref(Cluster *cluster,
                                           uintptr_t cite_index,
                                           const char *ref_id,
                                           uintptr_t ref_id_len);

/// Returns either an index (>=0) representing the position of a newly created cite within a
/// cluster, or a negative error code.
///
/// # Safety
///
/// The cluster must be from [citeproc_rs_cluster_new] and not freed.
/// The ref_id pair must refer to a valid byte array.

U32OrError citeproc_rs_cluster_cite_new(Cluster *cluster,
                                        const char *ref_id,
                                        uintptr_t ref_id_len);

/// Interns a cluster id. Returns -1 on error, hence the i64 return type; [ClusterId] is
/// actually a u32, so you can cast it safely after checking for -1.
///
/// Returns an error code indicative of what the LAST_ERROR will contain when checked.
///
/// # Safety
///
/// Either `str` must refer to a byte array of length `str_len`, or `str_len` must be zero.

U32OrError citeproc_rs_driver_intern_cluster_id(Driver *driver,
                                                const char *str,
                                                uintptr_t str_len);

/// Writes a random cluster_id into user_buf, and returns a ClusterId that represents it.
/// [citeproc::Processor::random_cluster_id]
///
/// Useful for allocating string ids to citation clusters in a real document, that need to be
/// read back later.
 U32OrError citeproc_rs_driver_random_cluster_id(Driver *driver, void *user_buf);

/// Inserts a cluster, overwriting any previously written cluster with that ID.
/// [citeproc::Processor::insert_cluster]
///
/// # Safety
///
/// Driver must be from [citeproc_rs_driver_new]. The cluster must be from
/// [citeproc_rs_cluster_new].
 ErrorCode citeproc_rs_driver_insert_cluster(Driver *driver, const Cluster *cluster);

/// Sets the string locator and [LocatorType] for a cite.

ErrorCode citeproc_rs_cluster_cite_set_locator(Cluster *cluster,
                                               uintptr_t cite_index,
                                               const char *locator,
                                               uintptr_t locator_len,
                                               LocatorType loc_type);

/// Sets the string prefix for a cite.

ErrorCode citeproc_rs_cluster_cite_set_prefix(Cluster *cluster,
                                              uintptr_t cite_index,
                                              const char *prefix,
                                              uintptr_t prefix_len);

/// Sets the string suffix on a cite. Pass in a zero length string for no suffix.

ErrorCode citeproc_rs_cluster_cite_set_suffix(Cluster *cluster,
                                              uintptr_t cite_index,
                                              const char *suffix,
                                              uintptr_t suffix_len);

} // extern "C"

/// If you use this as your buffer_write_callback, then you must call [citeproc_rs_cstring_free] on
/// the resulting buffers, or the memory will leak.
///
static const BufferOps CSTRING_BUFFER_OPS = BufferOps{ /* .write = */ citeproc_rs_cstring_write, /* .clear = */ citeproc_rs_cstring_clear };

} // namespace citeproc_rs

#endif // _CITEPROC_RS_HPP
