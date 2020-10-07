// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

#[macro_use]
mod utils;

extern crate serde_derive;
#[allow(unused_imports)]
#[macro_use]
extern crate log;

use js_sys::{Error as JsError, Promise};
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{future_to_promise, JsFuture};

use citeproc::prelude::*;
use citeproc::string_id;
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
            .map_err(|_| anyhow::anyhow!("unknown format `{}`", format));
        let format = js_err!(format);
        let engine = js_err!(Processor::new(style, us_fetcher, format));
        let engine = Rc::new(RefCell::new(engine));

        // The Driver manually adds locales fetched via Lifecycle, which asks the consumer
        // asynchronously.
        Ok(Driver {
            engine,
            fetcher: lifecycle,
        })
    }

    /// Sets the style (which will also cause everything to be recomputed)
    #[wasm_bindgen(js_name = "setStyle")]
    pub fn set_style(&self, style_text: &str) -> Result<(), JsValue> {
        let mut eng = self.engine.borrow_mut();
        js_err!(eng.set_style_text(style_text));
        Ok(())
    }

    /// Completely overwrites the references library.
    /// This **will** delete references that are not in the provided list.
    #[wasm_bindgen(js_name = "resetReferences")]
    pub fn reset_references(&self, refs: Box<[JsValue]>) -> Result<(), JsValue> {
        let refs = utils::read_js_array(refs)?;
        let mut engine = self.engine.borrow_mut();
        engine.reset_references(refs);
        Ok(())
    }

    /// Inserts or overwrites references as a batch operation.
    /// This **will not** delete references that are not in the provided list.
    #[wasm_bindgen(js_name = "insertReferences")]
    pub fn insert_references(&self, refs: Box<[JsValue]>) -> Result<(), JsValue> {
        let refs = utils::read_js_array(refs)?;
        self.engine.borrow_mut().extend_references(refs);
        Ok(())
    }

    /// Inserts or overwrites a reference.
    ///
    /// * `refr` is a Reference object.
    #[wasm_bindgen(js_name = "insertReference")]
    pub fn insert_reference(&self, refr: TReference) -> Result<(), JsValue> {
        let refr = js_err!(refr.into_serde());
        // inserting & replacing are the same
        self.engine.borrow_mut().insert_reference(refr);
        Ok(())
    }

    /// Removes a reference by id. If it is cited, any cites will be dangling. It will also
    /// disappear from the bibliography.
    #[wasm_bindgen(js_name = "removeReference")]
    pub fn remove_reference(&self, id: &str) -> Result<(), JsValue> {
        let id = Atom::from(id);
        self.engine.borrow_mut().remove_reference(id);
        Ok(())
    }

    /// Sets the references to be included in the bibliography despite not being directly cited.
    ///
    /// * `refr` is a
    #[wasm_bindgen(js_name = "includeUncited")]
    pub fn include_uncited(&self, uncited: TIncludeUncited) -> Result<(), JsValue> {
        let uncited = js_err!(uncited.into_serde());
        self.engine.borrow_mut().include_uncited(uncited);
        Ok(())
    }

    /// Gets a list of locales in use by the references currently loaded.
    ///
    /// Note that Driver comes pre-loaded with the `en-US` locale.
    #[wasm_bindgen(js_name = "toFetch")]
    pub fn locales_to_fetch(&self) -> Result<JsValue, JsValue> {
        let eng = self.engine.borrow();
        let langs: Vec<_> = eng
            .get_langs_in_use()
            .iter()
            .map(|l| l.to_string())
            .collect();
        Ok(js_err!(JsValue::from_serde(&langs)))
    }


    /// Returns a random cluster id, with an extra guarantee that it isn't already in use.
    #[wasm_bindgen(js_name = "randomClusterId")]
    pub fn random_cluster_id(&self) -> String {
        let eng = self.engine.borrow();
        eng.random_cluster_id_str().into()
    }

    /// Inserts or replaces a cluster with a matching `id`.
    #[wasm_bindgen(js_name = "insertCluster")]
    pub fn insert_cluster(&self, cluster: JsValue) -> Result<(), JsValue> {
        let cluster: string_id::Cluster<Markup> = js_err!(cluster.into_serde());
        let mut eng = self.engine.borrow_mut();
        eng.insert_cites_str(&cluster.id, &cluster.cites);
        Ok(())
    }

    /// Removes a cluster with a matching `id`
    #[wasm_bindgen(js_name = "removeCluster")]
    pub fn remove_cluster(&self, cluster_id: &str) -> Result<(), JsValue> {
        let mut eng = self.engine.borrow_mut();
        eng.remove_cluster_str(cluster_id);
        Ok(())
    }

    /// Resets all the clusters in the processor to a new list.
    ///
    /// * `clusters` is a Cluster[]
    #[wasm_bindgen(js_name = "initClusters")]
    pub fn init_clusters(&self, clusters: Box<[JsValue]>) -> Result<(), JsValue> {
        let clusters: Vec<_> = utils::read_js_array(clusters)?;
        self.engine.borrow_mut().init_clusters_str(clusters);
        Ok(())
    }

    /// Returns the formatted citation cluster for `cluster_id`.
    ///
    /// Prefer `batchedUpdates` to avoid serializing unchanged clusters on every edit. This is
    /// still useful for initialization.
    #[wasm_bindgen(js_name = "builtCluster")]
    pub fn built_cluster(&self, id: &str) -> Result<JsValue, JsValue> {
        let eng = self.engine.borrow();
        let built = js_err!(eng.get_cluster_str(id).ok_or_else(|| anyhow::anyhow!(
            "cluster {} either does not exist, or has not been assigned a position in the document",
            id
        )));
        let js = js_err!(JsValue::from_serde(&built));
        Ok(js)
    }

    /// Previews a formatted citation cluster, in a particular position.
    ///
    /// - `cites`: The cites to go in the cluster
    /// - `positions`: An array of `ClusterPosition`s as in set_cluster_order, but with a single
    ///   cluster's id set to zero. The cluster with id=0 is the position to preview the cite. It
    ///   can replace another cluster, or be inserted before/after/between existing clusters, in
    ///   any location you can think of.
    ///
    #[wasm_bindgen(js_name = "previewCitationCluster")]
    pub fn preview_citation_cluster(
        &self,
        cites: Box<[JsValue]>,
        positions: Box<[JsValue]>,
        format: &str,
    ) -> Result<JsValue, JsValue> {
        let cites: Vec<Cite<Markup>> = utils::read_js_array(cites)?;
        let positions: Vec<string_id::ClusterPosition> = utils::read_js_array(positions)?;
        let mut eng = self.engine.borrow_mut();
        let preview = eng.preview_citation_cluster(
            &cites,
            PreviewPosition::MarkWithZeroStr(&positions),
            SupportedFormat::from_str(format).ok(),
        );
        preview
            .map_err(|e| JsError::new(&e.to_string()))
            .and_then(|b| JsValue::from_serde(&b).map_err(|e| JsError::new(&e.to_string())))
            .map_err(|e| e.into())
    }

    #[wasm_bindgen(js_name = "makeBibliography")]
    pub fn make_bibliography(&self) -> Result<TBibEntries, JsValue> {
        let eng = self.engine.borrow();
        Ok(js_err!(
            JsValue::from_serde(&eng.get_bibliography()).map(TBibEntries::from)
        ))
    }

    #[wasm_bindgen(js_name = "bibliographyMeta")]
    pub fn bibliography_meta(&self) -> Result<TBibliographyMeta, JsValue> {
        let eng = self.engine.borrow();
        Ok(js_err!(
            JsValue::from_serde(&eng.get_bibliography_meta()).map(TBibliographyMeta::from)
        ))
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
    pub fn set_cluster_order(&self, positions: Box<[JsValue]>) -> Result<(), JsValue> {
        let positions: Vec<string_id::ClusterPosition> = utils::read_js_array(positions)?;
        let mut eng = self.engine.borrow_mut();
        js_err!(eng.set_cluster_order_str(&positions));
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
    pub fn batched_updates(&self) -> Result<TUpdateSummary, JsValue> {
        let eng = self.engine.borrow();
        let summary = eng.batched_updates_str();
        Ok(js_err!(
            JsValue::from_serde(&summary).map(TUpdateSummary::from)
        ))
    }

    /// Returns all the clusters and bibliography entries in the document.
    /// Also drains the queue, just like batchedUpdates().
    /// Use this to rehydrate a document or run non-interactively.
    #[wasm_bindgen(js_name = "fullRender")]
    pub fn full_render(&self) -> Result<TFullRender, JsValue> {
        let mut eng = self.engine.borrow_mut();
        let all_clusters = eng.all_clusters_str();
        let bib_entries = eng.get_bibliography();
        let all = string_id::FullRender {
            all_clusters,
            bib_entries,
        };
        eng.drain();
        Ok(js_err!(JsValue::from_serde(&all).map(TFullRender::from)))
    }

    /// Drains the `batchedUpdates` queue manually.
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
    id: string;
    cites: Cite[];
};

export type ClusterPosition = {
    id: string;
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
    clusters: [string, Output][];
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

type BibEntry = {
    id: string;
    value: string;
};

type BibEntries = BibEntry[];

type FullRender = {
    allClusters: { [clusterId: string]: string },
    bibEntries: BibEntries,
};

type BibliographyMeta = {
    max_offset: number;
    entry_spacing: number;
    line_spacing: number;
    hanging_indent: boolean;
    /** the second-field-align value of the CSL style */
    secondFieldAlign: null  | "flush" | "margin";
    /** Format-specific metadata */
    formatMeta: any,
};
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "UpdateSummary")]
    pub type TUpdateSummary;
    #[wasm_bindgen(typescript_type = "BibEntries")]
    pub type TBibEntries;
    #[wasm_bindgen(typescript_type = "FullRender")]
    pub type TFullRender;
    #[wasm_bindgen(typescript_type = "BibliographyMeta")]
    pub type TBibliographyMeta;
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
