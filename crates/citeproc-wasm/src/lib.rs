// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

mod utils;

#[macro_use]
extern crate serde_derive;
#[allow(unused_imports)]
#[macro_use]
extern crate log;

use self::utils::ErrorPlaceholder;

use js_sys::{Error, Promise};
use serde::Serialize;
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::futures_0_3::{future_to_promise, JsFuture};

use citeproc::prelude::*;
use citeproc::Processor;
use csl::locale::Lang;

#[wasm_bindgen]
pub struct Driver {
    engine: Rc<RefCell<Processor>>,
    fetcher: Lifecycle,
}

#[wasm_bindgen]
impl Driver {
    /// Creates a new Driver.
    ///
    /// * `style` is a CSL style as a string. Independent styles only.
    /// * `lifecycle` must implement the `Lifecycle` interface
    /// * `format` is one of { "html", "rtf" }
    ///
    /// Throws an error if it cannot parse the style you gave it.
    pub fn new(style: &str, lifecycle: Lifecycle, format: &str) -> Result<Driver, JsValue> {
        utils::set_panic_hook();
        utils::init_log();

        // The Processor gets a "only has en-US, otherwise empty" fetcher.
        let us_fetcher = Arc::new(utils::USFetcher);
        let format = SupportedFormat::from_str(format)
            .map_err(|_| Error::new(&format!("unknown format `{}`", format)))?;
        let engine = Processor::new(style, us_fetcher, true, format)
            .map(RefCell::new)
            .map(Rc::new)
            .map_err(|e| Error::new(&serde_json::to_string(&e).unwrap()))?;

        // The Driver manually adds locales fetched via Lifecycle, which asks the consumer
        // asynchronously.
        Ok(Driver {
            engine,
            fetcher: lifecycle,
        })
    }

    /// Sets the style (which will also cause everything to be recomputed)
    #[wasm_bindgen(js_name = "setStyle")]
    pub fn set_style(&mut self, style_text: &str) -> JsValue {
        self.engine
            .borrow_mut()
            .set_style_text(style_text)
            .map_err(|e| JsValue::from_serde(&e).unwrap())
            .err()
            .unwrap_or(JsValue::UNDEFINED)
    }

    /// Inserts or overwrites references as a batch operation.
    #[wasm_bindgen(js_name = "setReferences")]
    pub fn set_references(&mut self, refs: Box<[JsValue]>) -> Result<(), JsValue> {
        let refs = utils::read_js_array(refs)?;
        Ok(self.engine.borrow_mut().set_references(refs))
    }

    /// Inserts or overwrites a reference.
    ///
    /// * `refr` is a
    #[wasm_bindgen(js_name = "insertReference")]
    pub fn insert_reference(&mut self, refr: JsValue) -> Result<(), JsValue> {
        let refr = refr
            .into_serde()
            .map_err(|_| ErrorPlaceholder::throw("could not parse Reference from host"))?;
        // inserting & replacing are the same
        self.engine.borrow_mut().insert_reference(refr);
        Ok(())
    }

    fn serde_result<T>(&self, f: impl Fn(&Processor) -> T) -> Result<JsValue, JsValue>
    where
        T: Serialize,
    {
        let engine = self.engine.borrow();
        let to_serialize = f(&engine);
        Ok(JsValue::from_serde(&to_serialize).unwrap())
    }

    /// Gets a list of locales in use by the references currently loaded.
    ///
    /// Note that Driver comes pre-loaded with the `en-US` locale.
    #[wasm_bindgen(js_name = "toFetch")]
    pub fn locales_to_fetch(&self) -> Result<JsValue, JsValue> {
        self.serde_result(|engine| {
            engine
                .get_langs_in_use()
                .iter()
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
        })
    }

    /// Inserts or replaces a cluster with a matching `id`.
    #[wasm_bindgen(js_name = "insertCluster")]
    pub fn insert_cluster(&mut self, cluster_id: JsValue) -> Result<(), JsValue> {
        let cluster = cluster_id.into_serde().map_err(|e| {
            ErrorPlaceholder::throw(&format!("could not parse cluster from host: {}", e))
        })?;
        let mut eng = self.engine.borrow_mut();
        Ok(eng.insert_cluster(cluster))
    }

    /// Removes a cluster with a matching `id`
    #[wasm_bindgen(js_name = "removeCluster")]
    pub fn remove_cluster(&mut self, cluster_id: u32) -> Result<(), JsValue> {
        let mut eng = self.engine.borrow_mut();
        Ok(eng.remove_cluster(cluster_id))
    }

    /// Resets all the clusters in the processor to a new list.
    ///
    /// * `clusters` is a Cluster[]
    #[wasm_bindgen(js_name = "initClusters")]
    pub fn init_clusters(&mut self, clusters: Box<[JsValue]>) -> Result<(), JsValue> {
        let clusters = utils::read_js_array(clusters)?;
        Ok(self.engine.borrow_mut().init_clusters(clusters))
    }

    /// Returns the formatted citation cluster for `cluster_id`.
    ///
    /// Prefer `batchedUpdates` to avoid serializing unchanged clusters on every edit. This is
    /// still useful for initialization.
    #[wasm_bindgen(js_name = "builtCluster")]
    pub fn built_cluster(&self, id: ClusterId) -> Result<JsValue, JsValue> {
        let built = (*self.engine.borrow().get_cluster(id)).clone();
        Ok(JsValue::from_serde(&built).unwrap())
    }

    #[wasm_bindgen(js_name = "makeBibliography")]
    pub fn full_bibliography(&self) -> Result<JsValue, JsValue> {
        self.serde_result(|engine| engine.get_bibliography())
    }

    #[wasm_bindgen(js_name = "bibliographyMeta")]
    pub fn bibliography_meta(&self) -> Result<JsValue, JsValue> {
        self.serde_result(|engine| engine.get_bibliography_meta())
    }

    /// Replaces cluster numberings in one go.
    ///
    /// * `mappings` is an `Array<[ ClusterId, ClusterNumber ]>` where `ClusterNumber`
    ///   is, e.g. `{ note: 1 }`, `{ note: [3, 1] }` or `{ inText: 5 }` in the same way a
    ///   Cluster must contain one of those three numberings.
    ///
    /// Not every ClusterId must appear in the array, just the ones you wish to renumber.
    ///
    /// The library consumer is responsible for ensuring that clusters are well-ordered. Clusters
    /// are sorted for determining cite positions (ibid, subsequent, etc). If a footnote is
    /// deleted, you will likely need to shift all cluster numbers after it back by one.
    ///
    /// The second note numbering, `{note: [3, 1]}`, is for having multiple clusters in a single
    /// footnote. This is possible in many editors. The second number acts as a second sorting
    /// key.
    ///
    /// The third note numbering, `{ inText: 5 }`, is for ordering in-text references that appear
    /// within the body of a document. These will be sorted but won't cause
    /// `first-reference-note-number` to become available.
    ///
    #[wasm_bindgen(js_name = "renumberClusters")]
    pub fn renumber_clusters(&mut self, mappings: Box<[JsValue]>) -> Result<(), JsValue> {
        let mappings: Vec<(ClusterId, ClusterNumber)> = utils::read_js_array(mappings)?;
        let mut eng = self.engine.borrow_mut();
        eng.renumber_clusters(&mappings);
        Ok(())
    }

    /// Retrieve any clusters that have been touched since last time `batchedUpdates` was
    /// called. Intended to be called every time an edit has been made. Every cluster in the
    /// returned summary should then be reflected in any UI.
    ///
    /// Some built clusters may occasionally have identical contents to before.
    ///
    /// * returns an `UpdateSummary`
    #[wasm_bindgen(js_name = "batchedUpdates")]
    pub fn batched_updates(&self) -> JsValue {
        let eng = self.engine.borrow();
        let summary = eng.batched_updates();
        JsValue::from_serde(&summary).unwrap()
    }

    /// Drains the `batchedUpdates` queue manually. Use it to avoid serializing an unneeded
    /// `UpdateSummary`.
    #[wasm_bindgen(js_name = "drain")]
    pub fn drain(&self) {
        let mut eng = self.engine.borrow_mut();
        eng.drain();
    }

    /// Asynchronously fetches all the locales that may be required, and saves them into the
    /// engine. Uses your provided `Lifecycle.fetchLocale` function.
    #[wasm_bindgen(js_name = "fetchAll")]
    pub fn fetch_all(&self) -> Promise {
        let rc = self.engine.clone();
        let fetcher = self.fetcher.clone();
        let future = async move {
            // Keep these two RefCell borrows short-lived. The { scope } ensures the first borrow
            // ends before the JS code runs indefinitely. Just in case someone calls the Driver
            // while a request is in-flight. The ultimate effect is that the RefCell is only
            // borrowed for the time that the Rust code is in control.
            let langs: Vec<Lang> = {
                let eng = rc.borrow();
                eng.get_langs_in_use()
                    .iter()
                    // we definitely have en-US, it's statically included
                    .filter(|l| **l != Lang::en_us() && !eng.has_cached_locale(l))
                    .cloned()
                    .collect()
            };
            let pairs = fetch_all(&fetcher, langs).await;
            let mut eng = rc.borrow_mut();
            eng.store_locales(pairs);
            Ok(JsValue::null())
        };
        future_to_promise(future)
    }

    #[cfg(feature = "dot")]
    /// Spits out a GraphViz DOT-formatted representation of the internal representation of a
    /// Reference constructed for disambiguation purposes.
    #[wasm_bindgen(js_name = "disambiguationDfaDot")]
    pub fn disambiguation_dfa_dot(&self, key: &str) -> String {
        let id = Atom::from(key);
        let eng = self.engine.borrow();
        if let Some(graph) = eng.ref_dfa(id) {
            return graph.debug_graph(&*eng);
        }
        "".to_string()
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone)]
    #[wasm_bindgen(js_name = "Lifecycle")]
    pub type Lifecycle;

    #[wasm_bindgen(method, js_name = "fetchLocale")]
    fn fetch_locale(this: &Lifecycle, lang: &str) -> Promise;

    #[wasm_bindgen(js_name = "error", js_namespace = console)]
    fn log_js_error(val: JsValue);
}

// TODO: include note about free()-ing the Driver before an async fetchLocale() call comes back (in
// which case the Driver reference held to by the promise handler function is now a dangling
// wasm-bindgen pointer).
#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = r#"

/** This interface lets citeproc retrieve locales or modules asynchronously,
    according to which ones are needed. */
export interface Lifecycle {
    /** Return locale XML for a particular locale. */
    fetchLocale(lang: string): Promise<string>;
}

export type DateLiteral = { "literal": string; };
export type DateRaw = { "raw": string; };
export type DatePartsDate = [number] | [number, number] | [number, number, number];
export type DatePartsSingle = { "date-parts": [DatePartsDate]; };
export type DatePartsRange = { "date-parts": [DatePartsDate, DatePartsDate]; };
export type DateParts = DatePartsSingle | DatePartsRange;
export type DateOrRange = DateLiteral | DateRaw | DateParts;

/** Locator type, and a locator string */
export type Locator = {
    label?: string;
    locator?: string;
    locators: undefined;
};

export type CiteLocator = Locator | { locator: undefined; locators: Locator[] };

export type Cite<Affix = string> = {
    id: string;
    prefix?: Affix;
    suffix?: Affix;
    suppression?: "InText" | "Rest" | null;
} & CiteLocator;

export type ClusterNumber = {
    note: number | [number, number]
} | {
    inText: number
};

export type NoteCluster = {
    id: number;
    cites: Cite[];
    note: number | [number, number]
};

export type InTextCluster = {
    id: number;
    cites: Cite[];
    in_text: number;
};

export type Cluster = NoteCluster | InTextCluster;

export type Reference = {
    id: string;
    type: CslType;
    [key: string]: any;
};

export type CslType = "book" | "article" | "legal_case" | "article-journal";

export interface BibliographyUpdate {
    updatedEntries: { [key: string]: string };
    entryIds?: string[];
}

export type UpdateSummary<Output = string> = {
    clusters: [number, Output][];
    bibliography?: BibliographyUpdate;
};

type InvalidCsl = {
    severity: "Error" | "Warning";
    range: {
        start: number;
        end: number;
    };
    message: string;
    hint: string;
};
type ParseError = {
    ParseError: string;
};
type Invalid = {
    Invalid: InvalidCsl[];
};
type StyleError = Partial<ParseError & Invalid>;


"#;

/// Asks the JS side to fetch all of the locales that could be called by the style+refs.
async fn fetch_all(inner: &Lifecycle, langs: Vec<Lang>) -> Vec<(Lang, String)> {
    // Promises are push-, not pull-based, so this kicks all of the requests off at once. If the JS
    // consumer is making HTTP requests for extra locales, they will run in parallel.
    let thunks: Vec<_> = langs
        .into_iter()
        .map(|lang| {
            let promised = inner.fetch_locale(&lang.to_string());
            let future = JsFuture::from(promised);
            (lang.clone(), future)
        })
        // Must collect to avoid shared + later mutable borrows of self.cache in two different
        // stages of the iterator
        .collect();
    let mut pairs = Vec::with_capacity(thunks.len());
    for (lang, thunk) in thunks {
        // And collect them.
        match thunk.await {
            Ok(got) => match got.as_string() {
                Some(string) => pairs.push((lang, string)),
                // JS consumer did not return a string. Assume it was null/undefined/etc, so no
                // locale was available.
                None => {}
            },
            // ~= Promise.catch; some async JS code threw an Error.
            Err(e) => {
                error!("caught: failed to fetch lang {}", lang);
                log_js_error(e);
            }
        }
    }
    pairs
}
