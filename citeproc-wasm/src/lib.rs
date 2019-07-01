// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

#![feature(async_await)]

mod utils;

extern crate wasm_bindgen;
#[macro_use]
extern crate serde_derive;

use serde::de::DeserializeOwned;

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::futures_0_3::JsFuture;
use js_sys::Promise;

use csl::locale::Lang;
use citeproc::LocaleFetcher;
use std::sync::Arc;
use citeproc::input::ClusterId;
// use citeproc::style::element::CslType;
// use citeproc::style::variables::*;
use citeproc::Processor;

// ~2kB gzipped, and prevents the same initial fetch every single time.
const EN_US: &'static str = include_str!("locales-en-US.xml");

#[derive(Serialize)]
pub struct ErrorPlaceholder(String);

impl ErrorPlaceholder {
    fn throw(msg: &str) -> JsValue {
        JsValue::from_serde(&ErrorPlaceholder(msg.to_string())).unwrap()
    }
}

fn read_js_array<T>(js: Box<[JsValue]>) -> Result<Vec<T>, JsValue> where T : DeserializeOwned {
    let xs: Result<Vec<T>, _> = js.iter().map(|x| x.into_serde()).collect();
    xs
        // TODO: remove Debug code
        .map_err(|e| ErrorPlaceholder::throw(&format!("could not deserialize array: {:?}", e)))
}

#[wasm_bindgen]
pub struct Driver {
    engine: Processor,
    // fetcher: PromiseFetcher,
}

#[wasm_bindgen]
impl Driver {
    pub fn new(style: &str, fet: PromiseFetcher) -> Result<Driver, JsValue> {
        utils::set_panic_hook();
        let fetcher = JsFetcher::new(fet.clone());
        let engine = Processor::new(style, Arc::new(fetcher))
            .map_err(|e| JsValue::from_serde(&e).unwrap())?;
        Ok(Driver { engine/*, fetcher: fet.clone()*/ })
    }

    #[wasm_bindgen(js_name = "setReferences")]
    pub fn set_references(&mut self, refs: Box<[JsValue]>) -> Result<(), JsValue> {
        let refs = read_js_array(refs)?;
        Ok(self.engine.set_references(refs))
    }

    #[wasm_bindgen(js_name = "toFetch")]
    pub fn locales_to_fetch(&self) -> JsValue {
        let langs: Vec<String> = self.engine.get_langs_in_use().iter().map(|l| l.to_string()).collect();
        JsValue::from_serde(&langs).unwrap()
    }

    #[wasm_bindgen(js_name = "initClusters")]
    pub fn init_clusters(&mut self, clusters: Box<[JsValue]>) -> Result<(), JsValue> {
        let clusters = read_js_array(clusters)?;
        Ok(self.engine.init_clusters(clusters))
    }

    #[wasm_bindgen(js_name = "builtCluster")]
    pub fn built_cluster(&self, id: ClusterId) -> Result<JsValue, JsValue> {
        let built = (*self.engine.get_cluster(id)).clone();
        Ok(JsValue::from_serde(&built).unwrap())
    }

}

#[wasm_bindgen]
extern "C" {
    /// Yeh
    #[derive(Clone)]
    pub type PromiseFetcher;

    #[wasm_bindgen(method, js_name = "fetchLocale")]
    fn fetch_locale(this: &PromiseFetcher, lang: &str) -> Promise;
}

#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = r#"

/** This interface lets citeproc retrieve locales or modules asynchronously,
    according to which ones are needed. */
export type Fetcher = {
    /** Return locale XML for a particular locale. */
    fetchLocale: async (lang: string) => string,
}; 
"#;


use std::collections::HashMap;

/// Asks the JS side to fetch all of the locales that could be called by the style+refs.
async fn fetch_all(inner: &PromiseFetcher, langs: Vec<Lang>) -> Vec<(Lang, String)> {
    // Must collect to avoid shared + later mutable borrows of self.cache in
    // two different stages of the iterator
    let thunks: Vec<_> = langs
        .into_iter()
        // .filter(|lang| !self.cache.contains_key(&lang))
        .map(|lang| {
            let promised = inner.fetch_locale(&lang.to_string());
            let future = JsFuture::from(promised);
            (lang.clone(), future)
        })
    .collect();

    let mut pairs = Vec::with_capacity(thunks.len());
    for (lang, thunk) in thunks {
        match thunk.await {
            Ok(got) => {
                pairs.push((lang, got.as_string().unwrap()));
                // self.cache.insert(lang, got.as_string().unwrap());
            }
            Err(_e) => panic!("failed to fetch lang {}", lang),
        }
    }
    pairs
}

/// A smart cache that knows how to asynchronously get more locales from JavaScript-land
pub struct JsFetcher {
    inner: PromiseFetcher,
    cache: HashMap<Lang, String>,
}

impl JsFetcher {
    fn new(js: PromiseFetcher) -> Self {
        let mut cache = HashMap::new();
        cache.insert(Lang::en_us(), EN_US.to_string());
        JsFetcher {
            inner: js,
            cache,
        }
    }
}

impl LocaleFetcher for JsFetcher {
    fn fetch_string(&self, lang: &Lang) -> Result<String, std::io::Error> {
        Ok(self.cache.get(lang).cloned().unwrap_or_else(|| {
            String::from(
                r#"<?xml version="1.0" encoding="utf-8"?>
        <locale xmlns="http://purl.org/net/xbiblio/csl" version="1.0" xml:lang="en-US">
        </locale>"#,
            )
        }))
    }
}

