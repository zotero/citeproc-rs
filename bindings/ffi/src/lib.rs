#![allow(non_camel_case_types)]

// #[macro_use]
// extern crate ffi_helpers;

use citeproc::prelude::{InitOptions as RsInitOptions, *};
use csl::{Lang, Locale};

#[macro_use]
mod errors;
#[macro_use]
mod macros;
use macros::nullify_on_panic;
mod buffer;
mod nullable;

use nullable::FromErrorCode;
use thiserror::Error;

#[derive(Debug, Error)]
#[repr(C)]
pub enum FFIError {
    #[error("a null pointer was passed in where it wasn't expected")]
    NullPointer,
    #[error("caught panic unwinding: {message}")]
    CaughtPanic {
        message: String,
        #[cfg(feature = "backtrace")]
        backtrace: Option<std::backtrace::Backtrace>,
    },
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

use libc::{c_char, c_void};
use std::ffi::{CStr, CString};
use std::sync::Arc;

use crate::buffer::BufferWriter;

/// Wrapper for a driver, initialized with one style and any required locales.
///
/// Not thread safe.
///
/// Contains an Option<citeproc_rs::Processor>, because to survive panics we want to be able to
/// write a safe value that won't be in an inconsistent state after panicking.
pub struct Driver {
    processor: Option<Processor>,
    buffer_ops: buffer::BufferOps,
}

/// This writes the safe state (None) and also drops the processor and all its memory. If you
/// attempt to use it again, you'll get an error.
impl macros::MakeUnwindSafe for Driver {
    fn make_unwind_safe(&mut self) {
        self.processor = None;
    }
}

/// A callback signature that is expected to write a string into `slot` via
/// [citeproc_rs_locale_slot_write]
type LocaleFetchCallback =
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

ffi_fn! {
    fn citeproc_rs_locale_slot_write(slot: *mut LocaleSlot, locale_xml: *const c_char, locale_xml_len: usize) {
        null_pointer_check!(slot);
        // Safety: we asked for this to be passed back transparently.
        let slot = unsafe { &mut *slot };
        // Safety: we asked folks to give us an XML string.
        let locale_xml = unsafe { utf8_from_raw!(locale_xml, locale_xml_len) };
        // We'll parse it here as well so you catch errors before they become invisible as
        // mysteriously missing locales
        let _ = Locale::parse(locale_xml).expect("could not parse locale xml");
        null_pointer_check!(slot.storage);
        null_pointer_check!(slot.lang);
        // Safety: we control slot
        let storage = unsafe { &mut *slot.storage };
        let lang = unsafe { &*slot.lang };
        storage.locales.push((lang.clone(), locale_xml.to_owned()));
        // println!("added locale for lang: {}", lang);
    }
}

#[repr(u8)]
pub enum OutputFormat {
    Html,
    Rtf,
    Plain,
}

#[repr(C)]
pub struct InitOptions {
    style: *const c_char,
    style_len: usize,
    locale_fetch_context: *mut libc::c_void,
    locale_fetch_callback: LocaleFetchCallback,
    format: OutputFormat,
    buffer_ops: buffer::BufferOps,
}

ffi_fn! {
    /// Creates a new Processor from InitOptions.
    fn citeproc_rs_driver_new(init: InitOptions) -> *mut Driver {
        let style = unsafe { utf8_from_raw!(init.style, init.style_len) };
        let rs_init = RsInitOptions {
            format: match init.format {
                OutputFormat::Html => SupportedFormat::Html,
                OutputFormat::Rtf => SupportedFormat::Rtf,
                OutputFormat::Plain => SupportedFormat::Plain,
            },
            style,
            fetcher: Some(Arc::new(PredefinedLocales::bundled_en_us())),
            ..Default::default()
        };
        let mut proc = match Processor::new(rs_init) {
            Ok(p) => p,
            Err(e) => panic!("{}", e),
        };
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
        Box::into_raw(Box::new(Driver {
            processor: Some(proc),
            buffer_ops: init.buffer_ops,
        }))
    }
}

ffi_fn! {
    /// Frees a Processor.
    fn citeproc_rs_driver_free(driver: *mut Driver) {
        if !driver.is_null() {
            drop(unsafe { Box::from_raw(driver) });
        }
    }
}

ffi_fn! {
    /// Frees a CString returned from an API or one written using [CSTRING_BUFFER_OPS].
    fn citeproc_rs_string_free(ptr: *mut c_char) {
        if !ptr.is_null() {
            drop(unsafe { CString::from_raw(ptr) });
        }
    }
}

fn result_to_error_code<F, T>(mut closure: F) -> T
where
    F: FnMut() -> Result<T, FFIError>,
    T: FromErrorCode,
{
    let result = closure();
    match result {
        Ok(value) => {
            errors::clear_last_error();
            return value;
        }
        Err(e) => {
            let code = errors::update_last_error_return_code(e);
            T::from_error_code(code)
        }
    }
}

use core::ptr::NonNull;

unsafe fn borrow_raw_ptr_mut<'a, T>(ptr: *mut T) -> Result<&'a mut T, FFIError> {
    let ptr = NonNull::new(ptr).ok_or(FFIError::NullPointer)?;
    Ok(&mut *ptr.as_ptr())
}

unsafe fn utf8_slice_non_null<'a, T>(ptr: *const T, len: usize) -> Result<&'a [T], FFIError> {
    if !ptr.is_null() {
        Ok(core::slice::from_raw_parts(ptr, len))
    } else {
        Err(FFIError::NullPointer)
    }
}

unsafe fn borrow_utf8_slice_non_null<'a>(
    ptr: *const c_char,
    len: usize,
) -> Result<&'a str, FFIError> {
    if !ptr.is_null() {
        let ptr = ptr.cast::<u8>();
        let slice = core::slice::from_raw_parts(ptr, len);
        let string = core::str::from_utf8(slice)?;
        Ok(string)
    } else {
        Err(FFIError::NullPointer)
    }
}

ffi_fn_nullify! {
    fn citeproc_rs_driver_format_bibliography(#[nullify_on_panic] driver: *mut Driver, user_buf: *mut c_void) -> ErrorCode {
        result_to_error_code(|| {
            let driver = unsafe { borrow_raw_ptr_mut(driver) } ?;
            let mut buffer = unsafe { BufferWriter::new(driver.buffer_ops, user_buf) };
            buffer.clear();
            buffer.write_str("<b>success_write</b>")?;
            buffer.write_str("this has a null byte here \0 rest")?;
            Ok(ErrorCode::None)
        })
    }
}

ffi_fn_nullify! {
    /// Inserts a reference and formats a single cluster with a single cite to that reference using preview_citation_cluster.
    /// The reference in the processor will be overwritten and won't be restored afterward.
    ///
    /// Writes the result into user_buf using the buffer_ops interface.
    ///
    /// Returns an error code indicative of what the LAST_ERROR will contain when checked.
    fn citeproc_rs_driver_preview_reference(#[nullify_on_panic] driver: *mut Driver, ref_json: *const c_char, ref_json_len: usize, user_buf: *mut c_void) -> ErrorCode {
        result_to_error_code(|| {
            // SAFETY: We assume people have passed a valid Driver pointer over FFI.
            let driver = unsafe { borrow_raw_ptr_mut(driver) } ?;
            let proc = driver.processor.as_mut().ok_or(FFIError::Poisoned)?;

            // SAFETY: we asked folks to give us a JSON string.
            let ref_json = unsafe { borrow_utf8_slice_non_null(ref_json, ref_json_len) } ?;
            let reference: Reference = serde_json::from_str(ref_json)?;
            let id = reference.id.clone();

            let mut buffer = unsafe { BufferWriter::new(driver.buffer_ops, user_buf) };

            proc.insert_reference(reference);
            let cluster = proc.preview_cluster_id();
            let result = proc.preview_citation_cluster(
                &[Cite::basic(id)],
                PreviewPosition::MarkWithZero(&[ClusterPosition { id: cluster, note: None }]),
                None,
            )?;
            buffer.clear();
            buffer.write_str(&result)?;
            Ok(ErrorCode::None)
        })
    }
}

ffi_fn_nullify! {
    /// Inserts a reference.
    ///
    /// Returns an error code indicative of what the LAST_ERROR will contain when checked.
    fn citeproc_rs_driver_insert_reference(#[nullify_on_panic] driver: *mut Driver, ref_json: *const c_char, ref_json_len: usize) -> ErrorCode {
        result_to_error_code(|| {
            // SAFETY: We assume people have passed a valid Driver pointer over FFI.
            let driver = unsafe { borrow_raw_ptr_mut(driver) } ?;
            let proc = driver.processor.as_mut().ok_or(FFIError::Poisoned)?;
            // SAFETY: we asked folks to give us a JSON string.
            let ref_json = unsafe { borrow_utf8_slice_non_null(ref_json, ref_json_len) } ?;
            let reference: Reference = serde_json::from_str(ref_json)?;
            proc.insert_reference(reference);
            Ok(ErrorCode::None)
        })
    }
}
