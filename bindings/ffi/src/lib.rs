#![allow(non_camel_case_types)]

// #[macro_use]
// extern crate ffi_helpers;

use citeproc::prelude as rust;
use csl::{Lang, Locale};
use rust::{Processor, Reference};

#[macro_use]
mod errors;
#[macro_use]
mod macros;
pub mod logger;
use macros::nullify_on_panic;
pub(crate) mod util;
use util::*;
pub mod buffer;
mod nullable;

mod clusters;
pub use clusters::*;
pub use errors::*;

use thiserror::Error;

use libc::{c_char, c_void};
use std::ffi::CStr;
use std::sync::Arc;

use crate::buffer::BufferWriter;

#[derive(Debug, Error)]
#[repr(C)]
pub enum FFIError {
    #[error("a null pointer was passed in where it wasn't expected")]
    NullPointer,
    #[error("caught panic unwinding: {message}")]
    CaughtPanic { message: String },
    #[error("poisoned: attempted to use a driver after a panic poisoned it")]
    Poisoned,
    #[error("utf8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("reordering error: {0}")]
    Reordering(#[from] citeproc::ReorderingError),
    #[error("buffer ops error: no buffer ops set or general  buffer write error")]
    BufferOps,
    #[error("null byte error: {0}")]
    NullByte(#[from] buffer::NulError),
    #[error("serde json conversion error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("index {index} out of bounds, len = {len}")]
    Indexing { index: usize, len: usize },
    #[error("cluster not in flow: {0:?} does not have a position in the document")]
    ClusterNotInFlow(rust::ClusterId),
    #[error("style error: {0}")]
    InvalidStyle(#[from] csl::StyleError),
    #[error("could not set logger: {0}")]
    SetLogger(#[from] log::SetLoggerError),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum ErrorCode {
    None = 0,
    NullPointer = 1,
    CaughtPanic = 2,
    Poisoned = 3,
    Utf8 = 4,
    Reordering = 5,
    BufferOps = 6,
    NullByte = 7,
    SerdeJson = 8,
    Indexing = 9,
    ClusterNotInFlow = 10,
    InvalidStyle = 11,
    SetLogger = 12,
}

impl FFIError {
    pub(crate) fn from_caught_panic(e: Box<dyn std::any::Any + Send>) -> Self {
        let message = e.downcast::<String>().map(|x| *x).unwrap_or_else(|x| {
            x.downcast_ref::<&str>()
                .map(|x| *x)
                .unwrap_or("<catch_unwind held non-string object>")
                .into()
        });
        FFIError::CaughtPanic { message }
    }
    pub(crate) fn code(&self) -> ErrorCode {
        match self {
            Self::NullPointer => ErrorCode::NullPointer,
            Self::CaughtPanic { .. } => ErrorCode::CaughtPanic,
            Self::Poisoned => ErrorCode::Poisoned,
            Self::Utf8(_) => ErrorCode::Utf8,
            Self::NullByte(..) => ErrorCode::NullByte,
            Self::Reordering(_) => ErrorCode::Reordering,
            Self::BufferOps => ErrorCode::BufferOps,
            Self::SerdeJson(_) => ErrorCode::SerdeJson,
            Self::Indexing { .. } => ErrorCode::Indexing,
            Self::ClusterNotInFlow(_) => ErrorCode::ClusterNotInFlow,
            Self::InvalidStyle(_) => ErrorCode::InvalidStyle,
            Self::SetLogger(_) => ErrorCode::SetLogger,
        }
    }
}

use std::sync::Once;
static INITIALISED_LOG_CRATE: Once = Once::new();

/// Initialises the Rust `log` crate globally. No-op when called a second time.
#[no_mangle]
pub extern "C" fn citeproc_rs_log_init() {
    INITIALISED_LOG_CRATE.call_once(|| {
        env_logger::init();
    });
}

/// Wrapper for a driver, initialized with one style and any required locales.
///
/// Not thread safe.
///
/// Contains an Option<citeproc_rs::Processor>, because to survive panics we want to be able to
/// write a safe value that won't be in an inconsistent state after panicking.
pub struct Driver {
    processor: Option<Processor>,
    buffer_ops: buffer::BufferOps,
    /// Scratch buffer for translating FFI cluster positions
    positions_scratch: Vec<rust::ClusterPosition>,
}

/// This writes the safe state (None) and also drops the processor and all its memory. If you
/// attempt to use it again, you'll get an error.
impl macros::MakeUnwindSafe for Driver {
    fn make_unwind_safe(&mut self) {
        log::error!("making Driver unwind safe, by setting its Processor instance to None");
        self.processor = None;
    }
}

/// A callback signature that is expected to write a string into `slot` via
/// [citeproc_rs_locale_slot_write]
pub type LocaleFetchCallback =
    Option<unsafe extern "C" fn(context: *mut c_void, slot: *mut LocaleSlot, *const c_char)>;

pub struct LocaleSlot {
    storage: *mut LocaleStorage,
    lang: *const Lang,
}

struct LocaleFetcher {
    context: *mut c_void,
    callback: LocaleFetchCallback,
    storage: LocaleStorage,
}

impl LocaleFetcher {
    /// get `to_fetch` from `Processor::get_langs_in_use()`
    fn build(mut self, to_fetch: &[Lang]) -> LocaleStorage {
        use std::io::Write;
        let mut string_repr = Vec::<u8>::with_capacity(20);
        for lang in to_fetch {
            if *lang == Lang::en_us() {
                continue;
            }
            write!(&mut string_repr, "{}\0", lang).expect("in-memory write should not fail");
            let lang_str_ref = CStr::from_bytes_with_nul(string_repr.as_ref())
                .expect("definitely formatted thistruct s CStr with a null byte?");
            let mut slot = LocaleSlot {
                storage: &mut self.storage,
                lang,
            };
            if let Some(callback) = self.callback {
                unsafe {
                    callback(self.context, &mut slot, lang_str_ref.as_ptr());
                }
            }
            string_repr.clear();
        }
        self.storage
    }
}

struct LocaleStorage {
    locales: Vec<(Lang, String)>,
}

ffi_fn_nullify! {
    /// Write an XML string into a LocaleSlot. Returns an error code if the XML does not parse cleanly.
    ///
    /// # Safety:
    ///
    /// Only safe to use inside a [LocaleFetchCallback]. You must pass the slot pointer from the
    /// arguments to the callback.
    @safety unsafe fn citeproc_rs_locale_slot_write(slot: *mut LocaleSlot, locale_xml: *const c_char, locale_xml_len: usize) -> ErrorCode {
        result_to_error_code(|| {
            // Safety: we asked for this to be passed back transparently.
            let slot = unsafe { borrow_raw_ptr_mut(slot) } ?;
            // Safety: we asked folks to give us an XML string.
            let locale_xml = unsafe { borrow_utf8_slice(locale_xml, locale_xml_len) } ?;
            // We'll parse it preliminarily so you catch errors before they become invisible as
            // mysteriously missing locales
            let _ = Locale::parse(locale_xml)?;
            // Safety: we control slot, and the only time
            let storage = unsafe { borrow_raw_ptr_mut(slot.storage) } ?;
            // Safety: we control slot
            let lang = unsafe { borrow_raw_ptr(slot.lang) } ?;
            storage.locales.push((lang.clone(), locale_xml.to_owned()));
            // println!("added locale for lang: {}", lang);
            Ok(ErrorCode::None)
        })
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum OutputFormat {
    Html,
    Rtf,
    Plain,
}

#[repr(C)]
pub struct InitOptions {
    pub style: *const c_char,
    pub style_len: usize,
    pub locale_fetch_context: *mut libc::c_void,
    pub locale_fetch_callback: LocaleFetchCallback,
    pub format: OutputFormat,
    pub buffer_ops: buffer::BufferOps,
}

impl OutputFormat {
    fn to_supported_format(&self) -> rust::SupportedFormat {
        match *self {
            OutputFormat::Html => rust::SupportedFormat::Html,
            OutputFormat::Rtf => rust::SupportedFormat::Rtf,
            OutputFormat::Plain => rust::SupportedFormat::Plain,
        }
    }
}

ffi_fn! {
    /// Creates a new Processor from InitOptions. Free with [citeproc_rs_driver_free].
    fn citeproc_rs_driver_new(init: InitOptions) -> *mut Driver {

        result_to_error_code(|| {
            let style = unsafe { borrow_utf8_slice(init.style, init.style_len) }?;
            let rs_init = rust::InitOptions {
                format: init.format.to_supported_format(),
                style,
                fetcher: Some(Arc::new(rust::PredefinedLocales::bundled_en_us())),
                ..Default::default()
            };
            let mut proc = Processor::new(rs_init)?;
            let langs = proc.get_langs_in_use();
            if !langs.is_empty() {
                if let Some(_) = init.locale_fetch_callback {
                    let ffi_locales = LocaleFetcher {
                        callback: init.locale_fetch_callback,
                        context: init.locale_fetch_context,
                        storage: LocaleStorage { locales: Vec::with_capacity(langs.len()) },
                    };
                    let locales = ffi_locales.build(&langs).locales;
                    proc.store_locales(locales)
                }
            }
            Ok(Box::into_raw(Box::new(Driver {
                processor: Some(proc),
                buffer_ops: init.buffer_ops,
                positions_scratch: Vec::new(),
            })))
        })
    }
}

ffi_fn_nullify! {
    /// Frees a [Driver].
    ///
    /// # Safety
    ///
    /// The driver must either be from [citeproc_rs_driver_new] or be null.
    fn citeproc_rs_driver_free(driver: *mut Driver) {
        if !driver.is_null() {
            drop(unsafe { Box::from_raw(driver) });
        }
    }
}

#[repr(C)]
pub struct ClusterPosition {
    pub is_preview_marker: bool,
    /// Ignored if is_preview_marker is set
    pub id: ClusterId,
    /// The alternative (false) is to be in-text.
    pub is_note: bool,
    /// Ignored if is_note is NOT set
    pub note_number: u32,
}

ffi_fn_nullify! {
    /// [citeproc::Processor::set_cluster_order], but using an ffi-compatible [ClusterPosition]
    ///
    /// # Safety
    ///
    /// Driver must be a valid pointer to a Driver.
    ///
    /// positions/positions_len must point to a valid array of ClusterPosition.
    @safety unsafe fn citeproc_rs_driver_set_cluster_order(#[nullify_on_panic] driver: *mut Driver, positions: *const ClusterPosition, positions_len: usize) -> ErrorCode {
        result_to_error_code(|| {
            let driver = unsafe { borrow_raw_ptr_mut(driver) } ?;
            let proc = driver.processor.as_mut().ok_or(FFIError::Poisoned)?;
            let slice = unsafe { borrow_slice(positions, positions_len) } ?;
            driver.positions_scratch.clear();
            driver.positions_scratch.reserve(slice.len());
            for pos in slice {
                let rustpos = rust::ClusterPosition {
                    id: if pos.is_preview_marker {
                        return Err(FFIError::Reordering(citeproc::ReorderingError::ClusterOrderWithZero));
                    } else {
                        Some(rust::ClusterId(pos.id))
                    },
                    note: if pos.is_note {
                        Some(pos.note_number)
                    } else {
                        None
                    },
                };
                driver.positions_scratch.push(rustpos);
            }
            proc.set_cluster_order(&driver.positions_scratch)?;
            Ok(ErrorCode::None)
        })
    }
}

ffi_fn_nullify! {
    /// Writes a formatted cluster ([citeproc::Processor::get_cluster]) into a buffer.
    ///
    /// # Safety
    ///
    ///
    @safety unsafe fn citeproc_rs_driver_format_cluster(#[nullify_on_panic] driver: *mut Driver, cluster_id: ClusterId, user_buf: *mut c_void) -> ErrorCode {
        result_to_error_code(|| {
            let driver = unsafe { borrow_raw_ptr_mut(driver) } ?;
            let proc = driver.processor.as_mut().ok_or(FFIError::Poisoned)?;
            let mut buffer = unsafe { BufferWriter::new(driver.buffer_ops, user_buf) };
            let id = rust::ClusterId(cluster_id);
            let built = proc.get_cluster(id).ok_or(FFIError::ClusterNotInFlow(id))?;
            buffer.clear();
            buffer.write_str(built.as_str())?;
            Ok(ErrorCode::None)
        })
    }
}

ffi_fn_nullify! {
    /// Writes a bibliography into a buffer, using [citeproc::Processor::get_bibliography]
    @safety unsafe fn citeproc_rs_driver_format_bibliography(#[nullify_on_panic] driver: *mut Driver, user_buf: *mut c_void) -> ErrorCode {
        result_to_error_code(|| {
            let driver = unsafe { borrow_raw_ptr_mut(driver) } ?;
            let proc = driver.processor.as_mut().ok_or(FFIError::Poisoned)?;
            let mut buffer = unsafe { BufferWriter::new(driver.buffer_ops, user_buf) };
            buffer.clear();
            let bib_entries = proc.get_bibliography();
            for entry in bib_entries {
                buffer.write_str(entry.value.as_str())?;
                buffer.write_str("\n")?;
            }
            Ok(ErrorCode::None)
        })
    }
}

ffi_fn_nullify! {
    /// Formats a bibliography entry for a given reference.
    ///
    /// Writes the result into user_buf using the buffer_ops interface.
    ///
    /// Returns an error code indicative of what the LAST_ERROR will contain when checked.
    ///
    /// # Safety
    ///
    /// Same as [citeproc_rs_driver_insert_reference], but `user_buf` must also match the expected user data in the BufferOps struct passed to driver's init call.
    @safety unsafe fn citeproc_rs_driver_preview_reference(#[nullify_on_panic] driver: *mut Driver, ref_json: *const c_char, ref_json_len: usize, format: OutputFormat, user_buf: *mut c_void) -> ErrorCode {
        result_to_error_code(|| {
            // SAFETY: We assume people have passed a valid Driver pointer over FFI.
            let driver = unsafe { borrow_raw_ptr_mut(driver) } ?;
            let proc = driver.processor.as_mut().ok_or(FFIError::Poisoned)?;

            // SAFETY: we asked folks to give us a JSON string.
            let ref_json = unsafe { borrow_utf8_slice(ref_json, ref_json_len) } ?;
            let reference: Reference = serde_json::from_str(ref_json)?;

            let mut buffer = unsafe { BufferWriter::new(driver.buffer_ops, user_buf) };

            let format = format.to_supported_format();
            let result = proc.preview_reference(reference, Some(format));
            buffer.clear();
            buffer.write_str(&result)?;

            Ok(ErrorCode::None)
        })
    }
}

ffi_fn_nullify! {
    /// Inserts a reference. [citeproc::Processor::insert_reference]
    ///
    /// Returns an error code.
    ///
    /// # Safety
    ///
    /// `driver` must be a valid pointer to a Driver.
    ///
    /// Either `ref_json` must refer to a byte array of length `ref_json_len`, or `ref_json_len` must be zero.
    @safety unsafe fn citeproc_rs_driver_insert_reference(#[nullify_on_panic] driver: *mut Driver, ref_json: *const c_char, ref_json_len: usize) -> ErrorCode {
        result_to_error_code(|| {
            // SAFETY: We assume people have passed a valid Driver pointer over FFI.
            let driver = unsafe { borrow_raw_ptr_mut(driver) } ?;
            let proc = driver.processor.as_mut().ok_or(FFIError::Poisoned)?;
            // SAFETY: we asked folks to give us a JSON string.
            let ref_json = unsafe { borrow_utf8_slice(ref_json, ref_json_len) } ?;
            let reference: Reference = serde_json::from_str(ref_json)?;
            proc.insert_reference(reference);
            Ok(ErrorCode::None)
        })
    }
}

#[cfg(feature = "testability")]
ffi_fn! {
    fn test_panic() -> ErrorCode {
        panic!("test_panic {}", 755);
    }
}

#[cfg(feature = "testability")]
ffi_fn_nullify! {
    fn test_panic_poison_driver(#[nullify_on_panic] _driver: *mut Driver) -> ErrorCode {
        panic!("test_panic_poison_driver {}", 755);
    }
}
