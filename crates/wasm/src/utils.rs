// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use cfg_if::cfg_if;
use citeproc::prelude::*;
use csl::{IsoCountry, IsoLang, Lang, EN_US};
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;

cfg_if! {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    if #[cfg(feature = "console_error_panic_hook")] {
        extern crate console_error_panic_hook;
        pub use self::console_error_panic_hook::set_once as set_panic_hook;
    } else {
        #[inline]
        #[allow(dead_code)]
        pub fn set_panic_hook() {}
    }
}

cfg_if! {
    if #[cfg(feature = "console")] {
        pub fn init_log() {
            use log::LevelFilter;
            fern::Dispatch::new()
                .level(LevelFilter::Warn)
                .level_for("citeproc_proc::db", LevelFilter::Info)
                // .level_for("citeproc_proc::ir", LevelFilter::Info)
                .level_for("salsa", LevelFilter::Warn)
                .level_for("salsa::derived", LevelFilter::Warn)
                .level_for("html5ever", LevelFilter::Off)
                .level_for("citeproc::processor", LevelFilter::Info)
                .format(|out, message, record| {
                    out.finish(format_args!(
                        "[{}][{}] {}",
                        record.level(),
                        record.target(),
                        message
                    ))
                })
                .chain(fern::Output::call(raw_js_log))
                .apply()
                .unwrap_or(());
        }
    } else {
        pub fn init_log() {
        }
    }
}

/// Import JS items
/// 1. the global namespace, under no-modules
/// 2. the src/js/include.js file itself as an ES module in the other setups
#[macro_export]
#[doc(hidden)]
macro_rules! js_import {
    {$($tt:tt)*} => {
        cfg_if::cfg_if! {
            if #[cfg(feature = "zotero")] {
                compile_error!("Cannot use bare js_import for zotero output, it has to be namespaced")
            } else if #[cfg(feature = "no-modules")] {
                #[wasm_bindgen]
                extern "C" {
                    $($tt)*
                }
            } else {
                #[wasm_bindgen(module = "/src/js/include.js")]
                extern "C" {
                    $($tt)*
                }
            }
        }
    };
}

/// Import a class with a constructor that's namespaced e.g. `new Zotero.CiteprocRs.CslStyleError(...)`
/// either from
/// 1. that namespace, under zotero
/// 2. the global namespace, under no-modules
/// 3. the src/js/include.js file itself as an ES module in the other setups
#[macro_export]
#[doc(hidden)]
macro_rules! js_import_class_constructor {
    {
        pub type $name:ident;
        #[wasm_bindgen(constructor)]
        $constructor:item
    } => {
        cfg_if::cfg_if! {
            // We'd rather use ["Zotero", "CiteprocRs"] but we're pinned to a wasm-bindgen that
            // doesn't support that kind of multi-level namespace.
            // So this has to be replaced in the wasm-bindgen glue file as it gets written to
            // _zotero.
            if #[cfg(feature = "zotero")] {
                #[wasm_bindgen]
                extern "C" {
                    pub type $name;
                    #[wasm_bindgen(constructor, js_namespace = CITEPROC_RS_ZOTERO_GLOBAL)]
                    $constructor
                }
            } else {
                js_import! {
                    pub type $name;
                    #[wasm_bindgen(constructor)]
                    $constructor
                }
            }
        }
    };
}

#[allow(clippy::boxed_local)]
pub fn read_js_array_2<T>(js: Box<[JsValue]>) -> serde_json::Result<Vec<T>>
where
    T: DeserializeOwned,
{
    js.iter().map(|x| x.into_serde()).collect()
}

/// A `LocaleFetcher` that statically includes `en-US`, so it never has to be async-fetched, but
/// otherwise returns `None`.
pub struct USFetcher;

impl LocaleFetcher for USFetcher {
    fn fetch_string(&self, lang: &Lang) -> Result<Option<String>, LocaleFetchError> {
        if let Lang::Iso(IsoLang::English, Some(IsoCountry::US)) = lang {
            Ok(Some(String::from(EN_US)))
        } else {
            Ok(None)
        }
    }
}

// A version of the console_log crate that might be smaller than using web_sys

use log::{Level, Record};

#[wasm_bindgen]
extern "C" {
    // Use `js_namespace` here to bind `console.log(..)` instead of just
    // `log(..)`
    #[wasm_bindgen(js_namespace = console)]
    fn debug(s: &str);
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
    #[wasm_bindgen(js_namespace = console)]
    fn info(s: &str);
    #[wasm_bindgen(js_namespace = console)]
    fn warn(s: &str);
    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

#[allow(dead_code)]
pub fn raw_js_log(record: &Record) {
    // pick the console.log() variant for the appropriate logging level
    let console_log = match record.level() {
        Level::Error => error,
        Level::Warn => warn,
        Level::Info => info,
        Level::Debug => log,
        Level::Trace => debug,
    };

    console_log(&format!("{}", record.args()));
}
