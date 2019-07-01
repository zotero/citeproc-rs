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
use wasm_bindgen_futures::futures_0_3::{JsFuture, future_to_promise};
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

use std::rc::Rc;
use std::cell::RefCell;

#[wasm_bindgen]
pub struct Driver {
    engine: Rc<RefCell<Processor>>,
    fetcher: PromiseFetcher,
}

#[wasm_bindgen]
impl Driver {
    pub fn new(style: &str, fet: PromiseFetcher) -> Result<Driver, JsValue> {
        utils::set_panic_hook();
        let fetcher = Arc::new(JsFetcher::new());
        let engine = Processor::new(style, fetcher)
            .map(RefCell::new)
            .map(Rc::new)
            .map_err(|e| JsValue::from_serde(&e).unwrap())?;
        Ok(Driver { engine, fetcher: fet })
    }

    #[wasm_bindgen(js_name = "setReferences")]
    pub fn set_references(&mut self, refs: Box<[JsValue]>) -> Result<(), JsValue> {
        let refs = read_js_array(refs)?;
        Ok(self.engine.borrow_mut().set_references(refs))
    }

    #[wasm_bindgen(js_name = "toFetch")]
    pub fn locales_to_fetch(&self) -> JsValue {
        let langs: Vec<String> = self.engine.borrow().get_langs_in_use().iter().map(|l| l.to_string()).collect();
        JsValue::from_serde(&langs).unwrap()
    }

    #[wasm_bindgen(js_name = "initClusters")]
    pub fn init_clusters(&mut self, clusters: Box<[JsValue]>) -> Result<(), JsValue> {
        let clusters = read_js_array(clusters)?;
        Ok(self.engine.borrow_mut().init_clusters(clusters))
    }

    #[wasm_bindgen(js_name = "builtCluster")]
    pub fn built_cluster(&self, id: ClusterId) -> Result<JsValue, JsValue> {
        let built = (*self.engine.borrow().get_cluster(id)).clone();
        Ok(JsValue::from_serde(&built).unwrap())
    }

    // #[wasm_bindgen(js_name = "fetchAll")]
    // pub fn fetch_all(&self) -> Promise {
    //     let langs = self.engine.get_langs_in_use();
    //     let fetcher = self.fetcher.clone();
    //     let future = async move || {
    //         let pairs = fetcher.fetch(langs).await;
    //         fetcher.write(pairs);
    //         Ok::<JsValue, JsValue>(JsValue::null())
    //     };
    //     future_to_promise(future())
    // }

    /// This asynchronously fetches all the locales that may be required, and saves them into the
    /// engine. It holds a mutable borrow for the duration of the fetches, and any other Driver
    /// method will fail until all fetches return.
    ///
    /// This needs improvement -- a fully queued architecture that is aware of any 'locks' would be
    /// better. You could enqueue `SetReferences` (which might trigger fetching) and be notified
    /// when your document needed updating.
    #[wasm_bindgen(js_name = "fetchAll")]
    pub fn fetch_all(&self) -> Promise {
        let rc = self.engine.clone();
        let fetcher = self.fetcher.clone();
        let future = async move || -> Result<JsValue, JsValue> {
            let pairs = {
                let eng = rc.borrow();
                let langs: Vec<Lang> = eng
                    .get_langs_in_use()
                    .iter()
                    // we definitely have en-US, it's statically included
                    .filter(|l| **l != Lang::en_us() && !eng.has_cached_locale(l))
                    .cloned()
                    .collect();
                fetch_all(&fetcher, langs).await
            };
            let mut eng = rc.borrow_mut();
            eng.store_locales(pairs);
            Ok(JsValue::null())
        };
        future_to_promise(future())
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


use std::collections::HashMap;

/// A smart cache that knows how to asynchronously get more locales from JavaScript-land
pub struct JsFetcher {
    cache: HashMap<Lang, String>,
}

impl JsFetcher {
    fn new() -> Self {
        let mut cache = HashMap::new();
        cache.insert(Lang::en_us(), EN_US.to_string());
        JsFetcher {
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

// use std::sync::RwLock;

// pub struct LockFetcher {
//     inner: Arc<RwLock<JsFetcher>>,
// }

// impl LockFetcher {
//     fn new(js: JsFetcher) -> Self {
//         LockFetcher { inner: Arc::new(RwLock::new(js)) }
//     }
//     async fn fetch(&self, langs: Vec<Lang>) -> Vec<(Lang, String)> {
//         let inner = self.inner.read().expect("poisoned LockFetcher rwlock");
//         let pairs = inner.fetch_all(langs).await;
//         pairs
//     }
//     fn write(&self, pairs: Vec<(Lang, String)>) {
//         let mut inner = self.inner.write().expect("poisoned LockFetcher rwlock");
//         for (lang, string) in pairs {
//             inner.cache.insert(lang, string);
//         }
//     }
// }

// impl LocaleFetcher for LockFetcher {
//     fn fetch_string(&self, lang: &Lang) -> Result<String, std::io::Error> {
//         let inner = self.inner.read().expect("poisoned LockFetcher rwlock");
//         inner.fetch_string(lang)
//     }
// }
