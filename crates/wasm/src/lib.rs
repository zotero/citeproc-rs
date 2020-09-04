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

use js_sys::{Error as JsError, Promise};
use serde::Serialize;
use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{future_to_promise, JsFuture};

use citeproc::prelude::*;
use csl::Lang;

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
            .map_err(|_| JsError::new(&format!("unknown format `{}`", format)))?;
        let engine = Processor::new(style, us_fetcher, true, format)
            .map(RefCell::new)
            .map(Rc::new)
            .map_err(|e| JsError::new(&serde_json::to_string(&e).unwrap()))?;

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

    /// Completely overwrites the references library.
    /// This **will** delete references that are not in the provided list.
    #[wasm_bindgen(js_name = "resetReferences")]
    pub fn reset_references(&mut self, refs: Box<[JsValue]>) -> Result<(), JsValue> {
        let refs = utils::read_js_array(refs)?;
        let mut engine = self.engine.borrow_mut();
        engine.reset_references(refs);
        Ok(())
    }

    /// Inserts or overwrites references as a batch operation.
    /// This **will not** delete references that are not in the provided list.
    #[wasm_bindgen(js_name = "setReferences")]
    pub fn set_references(&mut self, refs: Box<[JsValue]>) -> Result<(), JsValue> {
        let refs = utils::read_js_array(refs)?;
        self.engine.borrow_mut().extend_references(refs);
        Ok(())
    }

    /// Inserts or overwrites a reference.
    ///
    /// * `refr` is a Reference object.
    #[wasm_bindgen(js_name = "insertReference")]
    pub fn insert_reference(&mut self, refr: TReference) -> Result<(), JsValue> {
        let refr = refr
            .into_serde()
            .map_err(|_| ErrorPlaceholder::throw("could not parse Reference from host"))?;
        // inserting & replacing are the same
        self.engine.borrow_mut().insert_reference(refr);
        Ok(())
    }

    /// Removes a reference by id. If it is cited, any cites will be dangling. It will also
    /// disappear from the bibliography.
    #[wasm_bindgen(js_name = "removeReference")]
    pub fn remove_reference(&mut self, id: &str) -> Result<(), JsValue> {
        let id = Atom::from(id);
        self.engine.borrow_mut().remove_reference(id);
        Ok(())
    }

    /// Sets the references to be included in the bibliography despite not being directly cited.
    ///
    /// * `refr` is a
    #[wasm_bindgen(js_name = "includeUncited")]
    pub fn include_uncited(&mut self, uncited: TIncludeUncited) -> Result<(), JsValue> {
        let uncited = uncited
            .into_serde()
            .map_err(|_| ErrorPlaceholder::throw("could not parse IncludeUncited from host"))?;
        self.engine.borrow_mut().include_uncited(uncited);
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
        eng.insert_cluster(cluster);
        Ok(())
    }

    /// Removes a cluster with a matching `id`
    #[wasm_bindgen(js_name = "removeCluster")]
    pub fn remove_cluster(&mut self, cluster_id: u32) -> Result<(), JsValue> {
        let mut eng = self.engine.borrow_mut();
        eng.remove_cluster(cluster_id);
        Ok(())
    }

    /// Resets all the clusters in the processor to a new list.
    ///
    /// * `clusters` is a Cluster[]
    #[wasm_bindgen(js_name = "initClusters")]
    pub fn init_clusters(&mut self, clusters: Box<[JsValue]>) -> Result<(), JsValue> {
        let clusters = utils::read_js_array(clusters)?;
        self.engine.borrow_mut().init_clusters(clusters);
        Ok(())
    }

    /// Returns the formatted citation cluster for `cluster_id`.
    ///
    /// Prefer `batchedUpdates` to avoid serializing unchanged clusters on every edit. This is
    /// still useful for initialization.
    #[wasm_bindgen(js_name = "builtCluster")]
    pub fn built_cluster(&self, id: ClusterId) -> Result<JsValue, JsValue> {
        let eng = self.engine.borrow();
        let built = eng.get_cluster(id);
        Ok(built
            .ok_or_else(|| {
                JsError::new(&format!(
                    "Cluster {} has not been assigned a position in the document.",
                    id
                ))
            })
            .and_then(|b| JsValue::from_serde(&b).map_err(|e| JsError::new(e.description())))?)
    }

    /// Returns the formatted citation cluster for `cluster_id`.
    ///
    /// Prefer `batchedUpdates` to avoid serializing unchanged clusters on every edit. This is
    /// still useful for initialization.
    #[wasm_bindgen(js_name = "previewCitationCluster")]
    pub fn preview_citation_cluster(&mut self, cites: Box<[JsValue]>, pieces: Box<[JsValue]>) -> Result<JsValue, JsValue> {
        let cites: Vec<Cite<Markup>> = utils::read_js_array(cites)?;
        let positions: Vec<ClusterPosition> = utils::read_js_array(pieces)?;
        let mut eng = self.engine.borrow_mut();
        let preview = eng.preview_citation_cluster(cites, PreviewPosition::MarkWithZero(&positions));
        preview
            .map_err(|e| {
                JsError::new(&e.to_string())
            })
            .and_then(|b| JsValue::from_serde(&b).map_err(|e| JsError::new(&e.to_string())))
            .map_err(|e| e.into())
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

    /// Specifies which clusters are actually considered to be in the document, and sets their
    /// order. You may insert as many clusters as you like, but the ones provided here are the only
    /// ones used.
    ///
    /// If a piece does not provide a note, it is an in-text reference. Generally, this is what you
    /// should be providing for note styles, such that first-reference-note-number does not gain a
    /// value, but some users put in-text references inside footnotes, and it is unclear what the
    /// processor should do in this situation so you could try providing note numbers there as
    /// well.
    ///
    /// If a piece provides a { note: N } field, then that N must be monotically increasing
    /// throughout the document. Two same-N-in-a-row clusters means they occupy the same footnote,
    /// e.g. this would be two clusters:
    ///
    /// ```text
    /// Some text with footnote.[Prefix @cite, suffix. Second prefix @another_cite, second suffix.]
    /// ```
    ///
    /// This case is recognised and the order they appear in the input here is the order used for
    /// determining cite positions (ibid, subsequent, etc). But the position:first cites within
    /// them will all have the same first-reference-note-number if FRNN is used in later cites.
    ///
    /// May error without having set_cluster_ids, but with some set_cluster_note_number-s executed.
    #[wasm_bindgen(js_name = "setClusterOrder")]
    pub fn set_cluster_order(&mut self, pieces: Box<[JsValue]>) -> Result<(), JsValue> {
        let pieces: Vec<ClusterPosition> = utils::read_js_array(pieces)?;
        let mut eng = self.engine.borrow_mut();
        eng.set_cluster_order(&pieces)
            .map_err(|e| ErrorPlaceholder::throw(&format!("{:?}", e)))?;
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
    pub fn batched_updates(&self) -> TUpdateSummary {
        let eng = self.engine.borrow();
        let summary = eng.batched_updates();
        TUpdateSummary::from(JsValue::from_serde(&summary).unwrap())
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
} & Partial<CiteLocator>;

export type ClusterNumber = {
    note: number | [number, number]
} | {
    inText: number
};

export type Cluster = {
    id: number;
    cites: Cite[];
};

export type ClusterPosition = {
    id: number;
    /** Leaving off this field means this cluster is in-text. */
    note?: number;
}

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

type IncludeUncited = "None" | "All" | { Specific: string[] };
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "UpdateSummary")]
    pub type TUpdateSummary;
    #[wasm_bindgen(typescript_type = "IncludeUncited")]
    pub type TIncludeUncited;
    #[wasm_bindgen(typescript_type = "Reference")]
    pub type TReference;
}

/// Asks the JS side to fetch all of the locales that could be called by the style+refs.
async fn fetch_all(inner: &Lifecycle, langs: Vec<Lang>) -> Vec<(Lang, String)> {
    // Promises are push-, not pull-based, so this kicks all of the requests off at once. If the JS
    // consumer is making HTTP requests for extra locales, they will run in parallel.
    let thunks: Vec<_> = langs
        .into_iter()
        .map(|lang| {
            let promised = inner.fetch_locale(&lang.to_string());
            let future = JsFuture::from(promised);
            (lang, future)
        })
        // Must collect to avoid shared + later mutable borrows of self.cache in two different
        // stages of the iterator
        .collect();
    let mut pairs = Vec::with_capacity(thunks.len());
    for (lang, thunk) in thunks {
        // And collect them.
        match thunk.await {
            #[allow(clippy::single_match)]
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
