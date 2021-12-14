// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

#![allow(non_snake_case)]

#[macro_use]
mod utils;
#[macro_use]
pub mod typescript;
#[macro_use]
pub mod errors;
mod options;

pub use errors::Error;
use typescript::{JsonValue, TypescriptDeserialize};

#[allow(unused_imports)]
#[macro_use]
extern crate log;

use js_sys::Promise;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{future_to_promise, JsFuture};

use citeproc::prelude::*;
use citeproc::string_id;
use csl::{Lang, StyleMeta};

/// Parses a CSL style, either independent or dependent, and returns its metadata.
#[wasm_bindgen]
pub fn parseStyleMetadata(style: &str) -> Result<typescript::StyleMeta, Error> {
    let meta = StyleMeta::parse(style)?;
    meta.serialize_jsvalue()
}

#[wasm_bindgen]
pub struct Driver {
    engine: Rc<RefCell<Processor>>,
    fetcher: Option<Fetcher>,
}

#[wasm_bindgen]
impl Driver {
    /// Creates a new Driver. Use the InitOptions object, which has (for example):
    ///
    /// * `style` is a CSL style as a string. Independent styles only.
    /// * `fetcher` must implement the `Fetcher` interface
    /// * `format` is one of { "html", "rtf", "plain" }
    ///
    /// Throws an error if it cannot parse the style you gave it.
    #[wasm_bindgen(constructor)]
    pub fn new(options: typescript::InitOptions) -> Result<Driver, Error> {
        utils::set_panic_hook();
        utils::init_log();

        // The Processor gets a "only has en-US, otherwise empty" fetcher.
        let us_fetcher = Arc::new(utils::USFetcher);
        let fetcher = Fetcher::from_options_object(&options)?;
        let options = options.ts_deserialize()?;
        let csl_features =
            csl::version::read_features(options.csl_features.iter().map(|x| x.as_str()))
                .map_err(|x| Error::UnknownCSLFeature(x.to_owned()))?;
        let init = InitOptions {
            style: options.style.as_ref(),
            fetcher: Some(us_fetcher),
            format: options.format,
            format_options: options.format_options,
            bibliography_no_sort: options.bibliography_no_sort,
            locale_override: options.locale_override,
            test_mode: false,
            csl_features: Some(csl_features),
            ..Default::default()
        };
        let engine = Processor::new(init)?;

        if engine.default_lang() != Lang::en_us() && fetcher.is_none() {
            log::warn!("citeproc-rs was initialized with a locale other than en-US, but without a locale fetcher, using built-in en-US instead.");
        }

        let engine = Rc::new(RefCell::new(engine));
        // The Driver manually adds locales fetched via Fetcher, which asks the consumer
        // asynchronously.
        Ok(Driver { engine, fetcher })
    }

    /// Sets the style (which will also cause everything to be recomputed, use sparingly)
    #[wasm_bindgen(js_name = "setStyle")]
    pub fn set_style(&self, style_text: &str) -> Result<(), Error> {
        let _ = self.engine.borrow_mut().set_style_text(style_text)?;
        Ok(())
    }

    /// Sets the output format (which will also cause everything to be recomputed, use sparingly)
    ///
    /// @param {"html" | "rtf" | "plain"} format The new output format as a string, same as `new Driver`
    ///
    /// @param {FormatOptions | null} options If absent, this is set to the default FormatOptions.
    ///
    #[wasm_bindgen(js_name = "setOutputFormat")]
    pub fn set_output_format(
        &self,
        format: &str,
        options: Option<typescript::FormatOptions>,
    ) -> Result<(), Error> {
        let format = format
            .parse::<SupportedFormat>()
            .map_err(|()| Error::UnknownOutputFormat(format.to_owned()))?;
        let format_options = options
            .map(|fo| fo.ts_deserialize())
            .transpose()?
            .map(|fo| fo.0)
            .unwrap_or_else(Default::default);
        self.engine
            .borrow_mut()
            .set_output_format(format, format_options);
        Ok(())
    }

    /// Completely overwrites the references library.
    /// This **will** delete references that are not in the provided list.
    #[wasm_bindgen(js_name = "resetReferences")]
    pub fn reset_references(&self, refs: Box<[JsValue]>) -> Result<(), Error> {
        let refs = utils::read_js_array_2(refs)?;
        self.engine.borrow_mut().reset_references(refs);
        Ok(())
    }

    /// Inserts or overwrites references as a batch operation.
    /// This **will not** delete references that are not in the provided list.
    #[wasm_bindgen(js_name = "insertReferences")]
    pub fn insert_references(&self, refs: Box<[JsValue]>) -> Result<(), Error> {
        let refs = utils::read_js_array_2(refs)?;
        self.engine.borrow_mut().extend_references(refs);
        Ok(())
    }

    /// Inserts or overwrites a reference.
    ///
    /// * `refr` is a Reference object.
    #[wasm_bindgen(js_name = "insertReference")]
    pub fn insert_reference(&self, refr: typescript::Reference) -> Result<(), Error> {
        let refr = refr.into_serde()?;
        // inserting & replacing are the same
        self.engine.borrow_mut().insert_reference(refr);
        Ok(())
    }

    /// Removes a reference by id. If it is cited, any cites will be dangling. It will also
    /// disappear from the bibliography.
    #[wasm_bindgen(js_name = "removeReference")]
    pub fn remove_reference(&self, id: &str) -> Result<(), Error> {
        let id = Atom::from(id);
        self.engine.borrow_mut().remove_reference(id);
        Ok(())
    }

    /// Sets the references to be included in the bibliography despite not being directly cited.
    ///
    /// * `refr` is a
    #[wasm_bindgen(js_name = "includeUncited")]
    pub fn include_uncited(&self, uncited: typescript::IncludeUncited) -> Result<(), Error> {
        let uncited = uncited.into_serde()?;
        self.engine.borrow_mut().include_uncited(uncited);
        Ok(())
    }

    /// Gets a list of locales in use by the references currently loaded.
    ///
    /// Note that Driver comes pre-loaded with the `en-US` locale.
    #[wasm_bindgen(js_name = "toFetch")]
    pub fn locales_to_fetch(&self) -> Result<typescript::StringArray, Error> {
        let eng = self.engine.borrow();
        let langs: Vec<_> = eng
            .get_langs_in_use()
            .iter()
            .map(|l| l.to_string())
            .collect();
        langs.serialize_jsvalue()
    }

    /// Returns a random cluster id, with an extra guarantee that it isn't already in use.
    #[wasm_bindgen(js_name = "randomClusterId")]
    pub fn random_cluster_id(&self) -> String {
        let eng = self.engine.borrow();
        eng.random_cluster_id_str().into()
    }

    /// Inserts or replaces a cluster with a matching `id`.
    #[wasm_bindgen(js_name = "insertCluster")]
    pub fn insert_cluster(&self, cluster: typescript::Cluster) -> Result<(), Error> {
        let cluster: string_id::Cluster = cluster.into_serde()?;
        let mut eng = self.engine.borrow_mut();
        eng.insert_cluster_str(cluster);
        Ok(())
    }

    /// Removes a cluster with a matching `id`
    #[wasm_bindgen(js_name = "removeCluster")]
    pub fn remove_cluster(&self, cluster_id: &str) -> Result<(), Error> {
        let mut eng = self.engine.borrow_mut();
        eng.remove_cluster_str(cluster_id);
        Ok(())
    }

    /// Resets all the clusters in the processor to a new list.
    ///
    /// * `clusters` is a Cluster[]
    #[wasm_bindgen(js_name = "initClusters")]
    pub fn init_clusters(&self, clusters: Box<[JsValue]>) -> Result<(), Error> {
        let clusters: Vec<string_id::Cluster> = utils::read_js_array_2(clusters)?;
        self.engine.borrow_mut().init_clusters_str(clusters);
        Ok(())
    }

    /// Returns the formatted citation cluster for `cluster_id`.
    ///
    /// Prefer `batchedUpdates` to avoid serializing unchanged clusters on every edit. This is
    /// still useful for initialization.
    #[wasm_bindgen(js_name = "builtCluster")]
    pub fn built_cluster(&self, id: &str) -> Result<String, Error> {
        let eng = self.engine.borrow();
        let built = eng
            .get_cluster_str(id)
            .map(|arc| arc.to_string())
            .ok_or_else(|| Error::NonExistentCluster(id.into()))?;
        Ok(built)
    }

    /// @deprecated Use `previewCluster` instead
    #[wasm_bindgen(js_name = "previewCitationCluster")]
    pub fn preview_citation_cluster(
        &self,
        cites: Box<[JsValue]>,
        positions: Box<[JsValue]>,
        format: Option<String>,
    ) -> Result<String, Error> {
        let cites = utils::read_js_array_2(cites)?;
        self.preview_cluster_inner(PreviewCluster::new(cites, None), positions, format)
            .map(|arc| arc.to_string())
    }

    /// Previews a formatted citation cluster, in a particular position.
    ///
    /// - `cluster`: A cluster, without an `id` field. You'll want this to contain some cites.
    /// - `positions`: An array of `ClusterPosition`s as in set_cluster_order, but with a single
    ///   cluster's id set to zero. The cluster with id=0 is the position to preview the cite. It
    ///   can replace another cluster, or be inserted before/after/between existing clusters, in
    ///   any location you can think of.
    /// - `format`: an optional argument, an output format as a string, that is used only for this
    ///   preview.
    ///
    #[wasm_bindgen(js_name = "previewCluster")]
    pub fn preview_cluster(
        &self,
        preview_cluster: typescript::PreviewCluster,
        positions: Box<[JsValue]>,
        format: Option<String>,
    ) -> Result<String, Error> {
        let preview_cluster: PreviewCluster = preview_cluster.into_serde()?;
        self.preview_cluster_inner(preview_cluster, positions, format)
            .map(|arc| arc.to_string())
    }

    fn preview_cluster_inner(
        &self,
        preview_cluster: PreviewCluster,
        positions: Box<[JsValue]>,
        format: Option<String>,
    ) -> Result<String, Error> {
        let positions: Vec<string_id::ClusterPosition> = utils::read_js_array_2(positions)?;
        let mut eng = self.engine.borrow_mut();
        let preview = eng.preview_citation_cluster(
            preview_cluster,
            PreviewPosition::MarkWithZeroStr(&positions),
            format
                .map(|frmt| {
                    frmt.parse::<SupportedFormat>()
                        .map_err(|()| Error::UnknownOutputFormat(frmt))
                })
                .transpose()?,
        )?;
        Ok(preview.to_string())
    }

    #[wasm_bindgen(js_name = "makeBibliography")]
    pub fn make_bibliography(&self) -> Result<typescript::BibEntries, Error> {
        let eng = self.engine.borrow();
        let bib = eng.get_bibliography();
        bib.serialize_jsvalue()
    }

    #[wasm_bindgen(js_name = "bibliographyMeta")]
    pub fn bibliography_meta(&self) -> Result<typescript::BibliographyMeta, Error> {
        let eng = self.engine.borrow();
        let meta = eng.get_bibliography_meta();
        meta.serialize_jsvalue()
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
    pub fn set_cluster_order(&self, positions: Box<[JsValue]>) -> Result<(), Error> {
        let positions: Vec<string_id::ClusterPosition> = utils::read_js_array_2(positions)?;
        let mut eng = self.engine.borrow_mut();
        eng.set_cluster_order_str(&positions)?;
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
    pub fn batched_updates(&self) -> Result<typescript::UpdateSummary, Error> {
        let eng = self.engine.borrow();
        let summary = eng.batched_updates_str();
        summary.serialize_jsvalue()
    }

    /// Returns all the clusters and bibliography entries in the document.
    /// Also drains the queue, just like batchedUpdates().
    /// Use this to rehydrate a document or run non-interactively.
    #[wasm_bindgen(js_name = "fullRender")]
    pub fn full_render(&self) -> Result<typescript::FullRender, Error> {
        let mut eng = self.engine.borrow_mut();
        let all_clusters = eng.all_clusters_str();
        let bib_entries = eng.get_bibliography();
        let all = string_id::FullRender {
            all_clusters,
            bib_entries,
        };
        eng.drain();
        all.serialize_jsvalue()
    }

    /// Drains the `batchedUpdates` queue manually.
    #[wasm_bindgen(js_name = "drain")]
    pub fn drain(&self) {
        let mut eng = self.engine.borrow_mut();
        eng.drain();
    }

    /// Asynchronously fetches all the locales that may be required, and saves them into the
    /// engine. Uses your provided `Fetcher.fetchLocale` function.
    #[wasm_bindgen(js_name = "fetchLocales")]
    pub fn fetch_locales(&self) -> Promise {
        let rc = self.engine.clone();
        let langs: Vec<Lang> = {
            let eng = rc.borrow();
            eng.get_langs_in_use()
                .iter()
                // we definitely have en-US, it's statically included
                .filter(|l| **l != Lang::en_us() && !eng.has_cached_locale(l))
                .cloned()
                .collect()
        };
        if langs.is_empty() {
            return Promise::resolve(&JsValue::UNDEFINED);
        }
        let fetcher = if let Some(f) = self.fetcher.clone() {
            f
        } else {
            log::warn!("citeproc-rs was initialized without a locale fetcher, but reqested to fetchLocales() required locales {:?}, bailing out", langs);
            return Promise::resolve(&JsValue::UNDEFINED);
        };
        future_to_promise(async move {
            let pairs = fetch_all(&fetcher, langs).await;
            let mut eng = rc.borrow_mut();
            eng.store_locales(pairs);
            Ok(JsValue::UNDEFINED)
        })
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
    #[wasm_bindgen(js_name = "Fetcher")]
    pub type Fetcher;

    #[wasm_bindgen(method, js_name = "fetchLocale")]
    fn fetch_locale(this: &Fetcher, lang: &str) -> Promise;

    #[wasm_bindgen(js_name = "error", js_namespace = console)]
    fn log_js_error(val: JsValue);
}

/// Asks the JS side to fetch all of the locales that could be called by the style+refs.
async fn fetch_all(inner: &Fetcher, langs: Vec<Lang>) -> Vec<(Lang, String)> {
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
