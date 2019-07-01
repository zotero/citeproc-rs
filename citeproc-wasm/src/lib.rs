// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

#![feature(async_await)]

mod utils;

#[macro_use]
extern crate serde_derive;
#[allow(unused_imports)]
#[macro_use]
extern crate log;

use js_sys::Promise;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::futures_0_3::{future_to_promise, JsFuture};

use citeproc::input::ClusterId;
use citeproc::Processor;
use csl::locale::Lang;

#[wasm_bindgen]
pub struct Driver {
    engine: Rc<RefCell<Processor>>,
    fetcher: PromiseFetcher,
}

#[wasm_bindgen]
impl Driver {
    pub fn new(style: &str, promise_fetcher: PromiseFetcher) -> Result<Driver, JsValue> {
        utils::set_panic_hook();
        utils::init_log();

        // The Processor gets a "only has en-US, otherwise empty" fetcher.
        let us_fetcher = Arc::new(utils::USFetcher::new());
        let engine = Processor::new(style, us_fetcher)
            .map(RefCell::new)
            .map(Rc::new)
            .map_err(|e| JsValue::from_serde(&e).unwrap())?;

        // The Driver manually adds locales fetched via PromiseFetcher, which asks the consumer
        // asynchronously.
        Ok(Driver {
            engine,
            fetcher: promise_fetcher,
        })
    }

    #[wasm_bindgen(js_name = "setReferences")]
    pub fn set_references(&mut self, refs: Box<[JsValue]>) -> Result<(), JsValue> {
        let refs = utils::read_js_array(refs)?;
        Ok(self.engine.borrow_mut().set_references(refs))
    }

    #[wasm_bindgen(js_name = "toFetch")]
    pub fn locales_to_fetch(&self) -> JsValue {
        let langs: Vec<String> = self
            .engine
            .borrow()
            .get_langs_in_use()
            .iter()
            .map(|l| l.to_string())
            .collect();
        JsValue::from_serde(&langs).unwrap()
    }

    #[wasm_bindgen(js_name = "initClusters")]
    pub fn init_clusters(&mut self, clusters: Box<[JsValue]>) -> Result<(), JsValue> {
        let clusters = utils::read_js_array(clusters)?;
        Ok(self.engine.borrow_mut().init_clusters(clusters))
    }

    #[wasm_bindgen(js_name = "builtCluster")]
    pub fn built_cluster(&self, id: ClusterId) -> Result<JsValue, JsValue> {
        let built = (*self.engine.borrow().get_cluster(id)).clone();
        Ok(JsValue::from_serde(&built).unwrap())
    }

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
            }
            Err(_e) => panic!("failed to fetch lang {}", lang),
        }
    }
    pairs
}
