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

#[derive(Serialize)]
pub struct ErrorPlaceholder(String);

impl ErrorPlaceholder {
    pub fn throw(msg: &str) -> JsValue {
        JsValue::from_serde(&ErrorPlaceholder(msg.to_string())).unwrap()
    }
}

#[allow(clippy::boxed_local)]
pub fn read_js_array<T>(js: Box<[JsValue]>) -> Result<Vec<T>, JsValue>
where
    T: DeserializeOwned,
{
    let xs: Result<Vec<T>, _> = js.iter().map(|x| x.into_serde()).collect();
    xs
        // TODO: remove Debug code
        .map_err(|e| ErrorPlaceholder::throw(&format!("could not deserialize array: {:?}", e)))
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
