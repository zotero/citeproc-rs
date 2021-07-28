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
mod nullable;

use thiserror::Error;

#[derive(Debug, Error)]
#[repr(C)]
pub enum FFIError {
    #[error("A null pointer was passed in where it wasn't expected")]
    NullPointer,
    #[error("caught panic unwinding: {message}")]
    CaughtPanic {
        message: String,
        #[cfg(feature = "backtrace")]
        backtrace: Option<std::backtrace::Backtrace>,
    },
    #[error("attempted to use a driver after a panic poisoned it")]
    Poisoned,
    #[error("{0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("{0}")]
    Reordering(#[from] citeproc::ReorderingError),
}

#[repr(i32)]
pub enum ErrorCode {
    None = 0,
    NullPointer = 1,
    CaughtPanic = 2,
    Poisoned = 3,
    Utf8 = 4,
    Reordering = 5,
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
            Self::Reordering(_) => ErrorCode::Reordering,
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

/// Wrapper for a driver, initialized with one style and any required locales
/// Contains an Option<citeproc_rs::Processor>, because to survive panics we want to be able to
/// write a safe value that won't be in an inconsistent state after panicking.
pub struct Driver(Option<Processor>);

/// This writes the safe state (None) and also drops the processor and all its memory. If you
/// attempt to use it again, you'll get an error.
impl macros::MakeUnwindSafe for Driver {
    fn make_unwind_safe(&mut self) {
        self.0 = None;
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
        Box::into_raw(Box::new(Driver(Some(proc))))
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
    /// Frees a string returned from  API.
    fn citeproc_rs_string_free(ptr: *mut c_char) {
        if !ptr.is_null() {
            drop(unsafe { CString::from_raw(ptr) });
        }
    }
}

ffi_fn_nullify! {
    /// let reference: [String: Any] = [ "id": "blah", "type": "book", ... ];
    /// in Swift, JSONSerialization.data(reference).withUnsafeBytes({ rBytes in
    ///     format_one(driver, rBytes.baseAddress, rBytes.count)
    /// })
    ///
    /// May return null.
    fn citeproc_rs_driver_format_one(#[nullify_on_panic] driver: *mut Driver, ref_bytes: *const c_char, ref_bytes_len: usize) -> *mut c_char {
        let ref_json = unsafe { utf8_from_raw!(ref_bytes, ref_bytes_len) };
        let reference: Reference = serde_json::from_str(ref_json).unwrap();
        let id = reference.id.clone();
        let proc = unsafe { &mut (*driver).0 };
        if let Some(proc) = proc.as_mut() {
            proc.insert_reference(reference);
            let cluster = proc.preview_cluster_id();
            let result = proc.preview_citation_cluster(
                &[Cite::basic(id)],
                PreviewPosition::MarkWithZero(&[ClusterPosition { id: cluster, note: None }]),
                None,
            );
            match result {
                Ok(result) => {
                    let c = CString::new(result.as_bytes()).unwrap();
                    c.into_raw()
                },
                Err(e) => {
                    errors::update_last_error(FFIError::Reordering(e))
                }
            }
        } else {
            errors::update_last_error(FFIError::Poisoned)
        }
    }
}
