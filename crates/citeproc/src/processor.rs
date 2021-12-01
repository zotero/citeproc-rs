// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

// For the salsa macro expansion
#![allow(clippy::large_enum_variant)]
#![allow(clippy::enum_variant_names)]

use crate::prelude::*;

use crate::api::{
    string_id, BibEntry, BibliographyMeta, BibliographyUpdate, ClusterPosition, IncludeUncited,
    ReorderingError, SecondFieldAlign, UpdateSummary,
};
use citeproc_db::{
    CiteData, CiteDatabaseStorage, HasFetcher, LocaleDatabaseStorage, StyleDatabaseStorage, Uncited,
};
use citeproc_io::output::markup::FormatOptions;
use citeproc_proc::db::IrDatabaseStorage;
use citeproc_proc::BibNumber;
use indexmap::set::IndexSet;

use salsa::{Database, Durability, SweepStrategy};
#[cfg(feature = "rayon")]
use salsa::{ParallelDatabase, Snapshot};
use std::sync::Arc;
use std::sync::{Mutex, RwLock};

use csl::{Lang, Style, StyleError};

use citeproc_io::output::{markup::Markup, OutputFormat};
use citeproc_io::{Cite, ClusterMode, Reference, SmartString};
use csl::Atom;

use string_interner::{backend::StringBackend, StringInterner};
pub(crate) type Interner =
    StringInterner<ClusterId, StringBackend<ClusterId>, std::collections::hash_map::RandomState>;

#[allow(dead_code)]
type MarkupBuild = <Markup as OutputFormat>::Build;
#[allow(dead_code)]
type MarkupOutput = <Markup as OutputFormat>::Output;
use fnv::{FnvHashMap, FnvHashSet};

struct SavedBib {
    sorted_refs: Arc<(Vec<Atom>, FnvHashMap<Atom, BibNumber>)>,
    bib_entries: Arc<FnvHashMap<Atom, Arc<MarkupOutput>>>,
}

impl SavedBib {
    fn new() -> Self {
        SavedBib {
            sorted_refs: Arc::new(Default::default()),
            bib_entries: Arc::new(Default::default()),
        }
    }
}

#[salsa::database(
    StyleDatabaseStorage,
    LocaleDatabaseStorage,
    CiteDatabaseStorage,
    IrDatabaseStorage
)]
pub struct Processor {
    storage: salsa::Storage<Self>,
    pub fetcher: Arc<dyn LocaleFetcher>,
    pub formatter: Markup,
    format_options: FormatOptions,
    last_bibliography: Arc<Mutex<SavedBib>>,
    last_clusters: Arc<Mutex<FnvHashMap<ClusterId, Arc<SmartString>>>>,
    interner: Arc<RwLock<Interner>>,
    preview_cluster_id: ClusterId,
}

impl Database for Processor {}

#[cfg(feature = "rayon")]
impl ParallelDatabase for Processor {
    fn snapshot(&self) -> Snapshot<Self> {
        Snapshot::new(Processor {
            storage: self.storage.snapshot(),
            fetcher: self.fetcher.clone(),
            format_options: self.format_options.clone(),
            formatter: self.formatter.clone(),
            last_bibliography: self.last_bibliography.clone(),
            last_clusters: self.last_clusters.clone(),
            interner: self.interner.clone(),
            preview_cluster_id: self.preview_cluster_id,
        })
    }
}

impl HasFetcher for Processor {
    fn get_fetcher(&self) -> Arc<dyn LocaleFetcher> {
        self.fetcher.clone()
    }
}

impl ImplementationDetails for Processor {
    fn get_formatter(&self) -> Markup {
        self.formatter()
    }
    fn lookup_cluster_id(&self, symbol: ClusterId) -> Option<SmartString> {
        let reader = self.interner.read().unwrap();
        reader.resolve(symbol).map(SmartString::from)
    }
}

// need a Clone impl for map_with
// thanks to rust-analyzer for the tip
#[cfg(feature = "rayon")]
struct Snap(pub salsa::Snapshot<Processor>);
#[cfg(feature = "rayon")]
impl Clone for Snap {
    fn clone(&self) -> Self {
        Snap(self.0.snapshot())
    }
}

/// ```
/// use citeproc::InitOptions;
///
/// let _opts = InitOptions { style: "...", ..Default::default() };
/// ```
#[derive(Clone, Default)]
pub struct InitOptions<'a> {
    pub format: SupportedFormat,
    pub format_options: FormatOptions,
    /// A full independent style.
    pub style: &'a str,
    /// You might get this from a dependent style via `StyleMeta::parse(dependent_xml_string)`
    pub locale_override: Option<Lang>,
    /// Mechanism for fetching the locale you provide, if necessary.
    pub fetcher: Option<Arc<dyn LocaleFetcher>>,

    /// Which csl features to enable globally. Using the `<features>` declaration is highly
    /// preferred, but unfortunately it is not part of CSL yet.
    pub csl_features: Option<csl::Features>,

    /// Disables some formalities for test suite operation
    pub test_mode: bool,

    /// Disables sorting on the bibliography (enabled by default)
    pub bibliography_no_sort: bool,

    #[doc(hidden)]
    pub use_default_default: private::CannotConstruct,
}

mod private {
    #[derive(Clone, Default)]
    #[non_exhaustive]
    pub struct CannotConstruct;
}

impl Processor {
    pub(crate) fn safe_default(fetcher: Arc<dyn LocaleFetcher>) -> Self {
        let interner = Interner::with_capacity(40);
        let preview_cluster_id = ClusterId::new(u32::MAX);
        let mut db = Processor {
            storage: Default::default(),
            fetcher,
            formatter: Markup::default(),
            format_options: FormatOptions::default(),
            last_bibliography: Arc::new(Mutex::new(SavedBib::new())),
            last_clusters: Arc::new(Mutex::new(Default::default())),
            // This uses DefaultBackend, which is
            interner: Arc::new(RwLock::new(interner)),
            preview_cluster_id,
        };
        citeproc_db::safe_default(&mut db);
        citeproc_proc::safe_default(&mut db);
        // XXX: currently impossible to preview a cluster with a ClusterMode applied
        db.set_cluster_mode(preview_cluster_id, None);
        db
    }

    pub fn new(options: InitOptions) -> Result<Self, StyleError> {
        // The only thing you need from a dependent style is the override language, which may well
        // be none.
        let InitOptions {
            style,
            locale_override,
            fetcher,
            format,
            format_options,
            csl_features,
            test_mode,
            bibliography_no_sort,
            use_default_default: _,
        } = options;

        let fetcher =
            fetcher.unwrap_or_else(|| Arc::new(citeproc_db::PredefinedLocales::bundled_en_us()));
        let mut db = Processor::safe_default(fetcher);
        let style = Style::parse_with_opts(
            &style,
            csl::ParseOptions {
                allow_no_info: test_mode,
                features: csl_features,
                ..Default::default()
            },
        )?;
        db.set_style_with_durability(Arc::new(style), Durability::HIGH);
        db.set_output_format(format, format_options);
        db.set_default_lang_override_with_durability(locale_override, Durability::HIGH);
        db.set_bibliography_no_sort_with_durability(bibliography_no_sort, Durability::HIGH);
        Ok(db)
    }

    /// Sets the output format. Will require nearly everything to be recomputed, so call sparingly.
    pub fn set_output_format(&mut self, format: SupportedFormat, options: FormatOptions) {
        self.format_options = options;
        let formatter = format.make_markup(options);
        if self.formatter == formatter {
            // Avoid recomputing everything if possible
            return;
        }
        self.formatter = formatter.clone();
        self.set_formatter_with_durability(formatter, Durability::HIGH);
    }

    /// Sets the CSL style to be used. Will require nearly everything to be recomputed, so call sparingly.
    pub fn set_style_text(&mut self, style_text: &str) -> Result<(), StyleError> {
        let style = Style::parse(style_text)?;
        self.set_style_with_durability(Arc::new(style), Durability::HIGH);
        Ok(())
    }

    #[cfg(feature = "rayon")]
    fn snap(&self) -> Snap {
        Snap(self.snapshot())
    }

    // TODO: This might not play extremely well with Salsa's garbage collector,
    // which will have a new revision number for each built_cluster call.
    // Probably better to have this as a real query.
    pub fn compute(&self) -> Vec<(ClusterId, Arc<SmartString>)> {
        fn upsert_diff(
            into_h: &mut FnvHashMap<ClusterId, Arc<SmartString>>,
            id: ClusterId,
            built: Arc<SmartString>,
        ) -> Option<(ClusterId, Arc<SmartString>)> {
            let mut diff = None;
            into_h
                .entry(id)
                .and_modify(|existing| {
                    if built != *existing {
                        diff = Some((id, built.clone()));
                    }
                    *existing = built.clone();
                })
                .or_insert_with(|| {
                    diff = Some((id, built.clone()));
                    built
                });
            diff
        }

        let clusters = self.clusters_cites_sorted();

        #[cfg(feature = "rayon")]
        let result = {
            use rayon::prelude::*;
            use std::ops::DerefMut;

            let cite_ids = self.all_cite_ids();
            // compute ir2s, so the first year_suffixes call doesn't trigger all ir2s on a
            // single rayon thread
            cite_ids
                .par_iter()
                .for_each_with(self.snap(), |snap, &cite_id| {
                    snap.0.ir_gen2_add_given_name(cite_id);
                });
            self.year_suffixes();
            clusters
                .par_iter()
                .map_with(self.snap(), |snap, cluster| {
                    let built = snap.0.built_cluster(cluster.id);
                    let mut into_hashmap = snap.0.last_clusters.lock().unwrap();
                    upsert_diff(into_hashmap.deref_mut(), cluster.id, built)
                })
                .filter_map(|x| x)
                .collect()
        };
        #[cfg(not(feature = "rayon"))]
        let result = {
            let mut into_hashmap = self.last_clusters.lock().unwrap();
            clusters
                .iter()
                .filter_map(|cluster| {
                    let built = self.built_cluster(cluster.id);
                    upsert_diff(&mut into_hashmap, cluster.id, built)
                })
                .collect()
        };

        // Run salsa GC.
        self.sweep_all(SweepStrategy::discard_outdated());
        result
    }

    pub fn batched_updates(&self) -> UpdateSummary {
        let delta = self.compute();
        UpdateSummary {
            clusters: delta,
            bibliography: self.save_and_diff_bibliography(),
        }
    }

    pub fn batched_updates_str(&self) -> string_id::UpdateSummary {
        let delta = self.compute();
        let mut delta_str = Vec::with_capacity(delta.len());
        let interner = self.interner.read().unwrap();
        for (cid, neu) in delta {
            if let Some(resolved) = interner.resolve(cid) {
                delta_str.push((SmartString::from(resolved), neu));
            }
        }
        string_id::UpdateSummary {
            clusters: delta_str,
            bibliography: self.save_and_diff_bibliography(),
        }
    }

    pub fn drain(&mut self) {
        let _ = self.compute();
    }

    pub fn clear_references(&mut self) {
        self.set_all_keys_with_durability(Arc::new(IndexSet::new()), Durability::MEDIUM);
    }

    /// Gives you an interned cluster id to work with. Use this to insert cites, call
    /// `set_cluster_order`, and generally identify clusters in your document.
    ///
    /// ```
    /// use citeproc::prelude::*;
    /// let options = InitOptions {
    ///     style: r#"<style class="in-text"><citation><layout></layout></citation></style>"#,
    ///     test_mode: true,
    ///     ..Default::default()
    /// };
    /// let mut processor = Processor::new(options).unwrap();
    /// let a = processor.cluster_id("cluster-A");
    /// let b = processor.cluster_id("cluster-B");
    /// processor.insert_cites(a, &[Cite::basic("nonexistent-reference")]);
    /// processor.insert_cites(b, &[Cite::basic("nonexistent-reference")]);
    /// processor.set_cluster_order(&[
    ///     ClusterPosition::in_text(a),
    ///     ClusterPosition::in_text(b),
    /// ]);
    /// ```
    pub fn cluster_id(&self, string: impl AsRef<str>) -> ClusterId {
        let mut w = self.interner.write().unwrap();
        w.get_or_intern(string)
    }

    /// Returns a random cluster id, with an extra guarantee that it isn't already in use.
    pub fn random_cluster_id_str(&self) -> SmartString {
        let interner = self.interner.read().unwrap();
        loop {
            let smart_string = crate::random_cluster_id();
            if interner.get(&smart_string).is_none() {
                return smart_string;
            }
        }
    }

    /// Returns a random cluster id, with an extra guarantee that it isn't already in use.
    pub fn random_cluster_id(&self) -> ClusterId {
        let rand_id = self.random_cluster_id_str();
        self.interner.write().unwrap().get_or_intern(rand_id)
    }

    pub fn reset_references(&mut self, refs: Vec<Reference>) {
        let keys: IndexSet<Atom> = refs.iter().map(|r| r.id.clone()).collect();
        for r in refs {
            self.set_reference_input_with_durability(r.id.clone(), Arc::new(r), Durability::MEDIUM);
        }
        self.set_all_keys_with_durability(Arc::new(keys), Durability::MEDIUM);
    }

    pub fn extend_references(&mut self, refs: Vec<Reference>) {
        let keys = self.all_keys();
        let mut keys = IndexSet::clone(&keys);
        for r in refs {
            keys.insert(r.id.clone());
            self.set_reference_input_with_durability(r.id.clone(), Arc::new(r), Durability::MEDIUM);
        }
        self.set_all_keys_with_durability(Arc::new(keys), Durability::MEDIUM);
    }

    pub fn insert_reference(&mut self, refr: Reference) {
        let keys = self.all_keys();
        let mut keys = IndexSet::clone(&keys);
        keys.insert(refr.id.clone());
        self.set_reference_input_with_durability(
            refr.id.clone(),
            Arc::new(refr),
            Durability::MEDIUM,
        );
        self.set_all_keys_with_durability(Arc::new(keys), Durability::MEDIUM);
    }

    pub fn remove_reference(&mut self, id: Atom) {
        let keys = self.all_keys();
        let mut keys = IndexSet::clone(&keys);
        keys.remove(&id);
        self.set_all_keys_with_durability(Arc::new(keys), Durability::MEDIUM);
    }

    pub fn include_uncited(&mut self, uncited: IncludeUncited) {
        let db_uncited = match uncited {
            IncludeUncited::All => Uncited::All,
            IncludeUncited::None => Uncited::default(),
            IncludeUncited::Specific(list) => {
                Uncited::Enumerated(list.iter().map(String::as_str).map(Atom::from).collect())
            }
        };
        self.set_all_uncited_with_durability(Arc::new(db_uncited), Durability::MEDIUM);
    }

    pub fn init_clusters(&mut self, clusters: Vec<Cluster>) {
        let mut new_all = FnvHashSet::default();
        new_all.reserve(clusters.len());
        for cluster in clusters {
            let Cluster {
                id: cluster_id,
                cites,
                mode,
            } = cluster;
            let mut ids = Vec::with_capacity(cites.len());
            for (index, cite) in cites.into_iter().enumerate() {
                let cite_id = self.cite(CiteData::RealCite {
                    cluster: cluster_id,
                    index: index as u32,
                    cite: Arc::new(cite),
                });
                ids.push(cite_id);
            }
            self.set_cluster_cites(cluster_id, Arc::new(ids));
            self.set_cluster_note_number(cluster_id, None);
            self.set_cluster_mode(cluster_id, mode);
            new_all.insert(cluster_id);
        }
        self.set_all_cluster_ids(Arc::new(new_all));
    }

    pub fn init_clusters_str(&mut self, clusters: Vec<string_id::Cluster>) {
        let mut new_all = FnvHashSet::default();
        new_all.reserve(clusters.len());
        let interner_arc = self.interner.clone();
        let mut interner = interner_arc.write().unwrap();
        for cluster in clusters {
            let string_id::Cluster {
                id: cluster_id,
                cites,
                mode,
            } = cluster;
            let cluster_id = interner.get_or_intern(cluster_id);
            let mut ids = Vec::with_capacity(cites.len());
            for (index, cite) in cites.into_iter().enumerate() {
                let cite_id = self.cite(CiteData::RealCite {
                    cluster: cluster_id,
                    index: index as u32,
                    cite: Arc::new(cite),
                });
                ids.push(cite_id);
            }
            self.set_cluster_cites(cluster_id, Arc::new(ids));
            self.set_cluster_note_number(cluster_id, None);
            self.set_cluster_mode(cluster_id, mode);
            new_all.insert(cluster_id);
        }
        self.set_all_cluster_ids(Arc::new(new_all));
    }

    // cluster_ids is maintained manually
    // the cluster_cites relation is maintained manually

    pub fn remove_cluster(&mut self, cluster_id: ClusterId) {
        self.set_cluster_cites(cluster_id, Arc::new(Vec::new()));
        self.set_cluster_note_number(cluster_id, None);
        self.set_cluster_mode(cluster_id, None);
        let all_cluster_ids = self.all_cluster_ids();
        let mut new_all = (*all_cluster_ids).clone();
        new_all.remove(&cluster_id);
        self.set_all_cluster_ids(Arc::new(new_all));
    }

    pub fn remove_cluster_str(&mut self, cluster_id: &str) {
        let cid = self.cluster_id(cluster_id);
        self.remove_cluster(cid);
    }

    // Invariant: any cluster in all_cluster_ids also has a cluster_note_number and
    // a cluster_mode.
    fn ensure_cluster_in_all(&mut self, cluster_id: ClusterId) {
        let all_cluster_ids = self.all_cluster_ids();
        if !all_cluster_ids.contains(&cluster_id) {
            let mut new_all = (*all_cluster_ids).clone();
            new_all.insert(cluster_id);
            self.set_all_cluster_ids(Arc::new(new_all));
            // Now initialise the cluster data
            self.set_cluster_note_number(cluster_id, None);
            self.set_cluster_mode(cluster_id, None);
        }
    }

    fn insert_cites_only(&mut self, cluster_id: ClusterId, cites: Vec<Cite<Markup>>) {
        let mut ids = Vec::new();
        for (index, cite) in cites.into_iter().enumerate() {
            let cite_id = self.cite(CiteData::RealCite {
                cluster: cluster_id,
                index: index as u32,
                cite: Arc::new(cite),
            });
            ids.push(cite_id);
        }
        self.set_cluster_cites(cluster_id, Arc::new(ids));
    }

    pub fn insert_cluster(&mut self, cluster: Cluster) {
        log::debug!("insert_cluster, {:?}", cluster);
        let Cluster {
            id: cluster_id,
            cites,
            mode,
        } = cluster;
        self.ensure_cluster_in_all(cluster_id);
        self.insert_cites_only(cluster_id, cites);
        self.set_cluster_mode(cluster_id, mode);
    }

    fn intern_cluster(&mut self, cluster: string_id::Cluster) -> Cluster {
        let string_id::Cluster { id, cites, mode } = cluster;
        let interned = self.cluster_id(id);
        Cluster {
            id: interned,
            cites,
            mode,
        }
    }

    pub fn insert_cluster_str(&mut self, cluster: string_id::Cluster) {
        let cluster = self.intern_cluster(cluster);
        self.insert_cluster(cluster)
    }

    pub fn insert_cites(&mut self, cluster_id: ClusterId, cites: &[Cite<Markup>]) {
        let cites = cites.to_owned();
        self.ensure_cluster_in_all(cluster_id);
        self.insert_cites_only(cluster_id, cites);
    }

    pub fn insert_cites_str(&mut self, cluster_id: &str, cites: &[Cite<Markup>]) {
        let interned = self.cluster_id(cluster_id);
        self.insert_cites(interned, cites);
    }

    // Getters, because the query groups have too much exposed to publish.

    /// Returns None if the cluster has not been assigned a position in the document.
    pub fn get_cluster(&self, cluster_id: ClusterId) -> Option<Arc<MarkupOutput>> {
        if self.cluster_note_number(cluster_id).is_some() {
            Some(self.built_cluster(cluster_id))
        } else {
            None
        }
    }

    pub fn get_cluster_note_number(&self, cluster_id: ClusterId) -> Option<ClusterNumber> {
        self.cluster_note_number(cluster_id)
    }

    /// Returns None if the cluster has not been assigned a position in the document.
    pub fn get_cluster_str(&self, cluster_id: &str) -> Option<Arc<MarkupOutput>> {
        let id = self.cluster_id(cluster_id);
        self.get_cluster(id)
    }

    pub fn get_bib_item(&self, ref_id: Atom) -> Arc<MarkupOutput> {
        self.bib_item(ref_id)
    }

    pub fn get_bibliography_meta(&self) -> Option<BibliographyMeta> {
        let style = self.get_style();
        style.bibliography.as_ref().map(|bib| {
            BibliographyMeta {
                // TODO
                max_offset: 0,
                entry_spacing: bib.entry_spacing,
                line_spacing: bib.line_spacing,
                hanging_indent: bib.hanging_indent,
                // To avoid serde derive in csl
                second_field_align: bib.second_field_align.as_ref().map(|s| match s {
                    csl::style::SecondFieldAlign::Flush => SecondFieldAlign::Flush,
                    csl::style::SecondFieldAlign::Margin => SecondFieldAlign::Margin,
                }),
                format_meta: self.get_formatter().meta(),
            }
        })
    }

    fn save_and_diff_bibliography(&self) -> Option<BibliographyUpdate> {
        if self.get_style().bibliography.is_none() {
            return None;
        }
        let mut last_bibliography = self.last_bibliography.lock().unwrap();
        let new = self.get_bibliography_map();
        let old = std::mem::replace(&mut *last_bibliography, SavedBib::new());
        let mut update = BibliographyUpdate::new();
        for (k, v) in new.iter() {
            let old_v = old.bib_entries.get(k);
            if Some(v) != old_v {
                update.updated_entries.insert(k.clone(), v.clone());
            }
        }
        last_bibliography.bib_entries = new;
        let sorted_refs = self.sorted_refs();
        if sorted_refs.0 != old.sorted_refs.0 {
            update.entry_ids = Some(sorted_refs.0.clone());
        }
        last_bibliography.sorted_refs = sorted_refs;
        if update.updated_entries.is_empty() && update.entry_ids.is_none() {
            None
        } else {
            Some(update)
        }
    }

    pub fn all_clusters(&self) -> FnvHashMap<ClusterId, Arc<MarkupOutput>> {
        let all_cluster_ids = self.all_cluster_ids();
        let mut mapping = FnvHashMap::default();
        mapping.reserve(all_cluster_ids.len());
        for &cid in all_cluster_ids.iter() {
            if let Some(built) = self.get_cluster(cid) {
                mapping.insert(cid, built);
            }
        }
        mapping
    }

    pub fn all_clusters_str(&self) -> FnvHashMap<SmartString, Arc<MarkupOutput>> {
        let all_cluster_ids = self.all_cluster_ids();
        let interner = self.interner.read().unwrap();
        let mut mapping = FnvHashMap::default();
        mapping.reserve(all_cluster_ids.len());
        for &cid in all_cluster_ids.iter() {
            if let Some(built) = self.get_cluster(cid) {
                if let Some(resolved) = interner.resolve(cid) {
                    mapping.insert(SmartString::from(resolved), built);
                }
            }
        }
        mapping
    }

    pub fn get_bibliography(&self) -> Vec<BibEntry> {
        let bib_map = self.get_bibliography_map();
        self.sorted_refs()
            .0
            .iter()
            .filter_map(|k| bib_map.get(k).map(|v| (k, v)))
            .map(|(k, v)| BibEntry {
                id: k.clone(),
                value: if v.is_empty() {
                    Arc::new(SmartString::from(
                        "[CSL STYLE ERROR: reference with no printed form.]",
                    ))
                } else {
                    v.clone()
                },
            })
            .collect()
    }

    pub fn get_reference(&self, ref_id: Atom) -> Option<Arc<Reference>> {
        self.reference(ref_id)
    }

    pub fn get_style(&self) -> Arc<Style> {
        self.style()
    }

    pub fn store_locales(&mut self, locales: Vec<(Lang, String)>) {
        let mut langs = (*self.locale_input_langs()).clone();
        for (lang, xml) in locales {
            langs.insert(lang.clone());
            self.set_locale_input_xml_with_durability(lang, Arc::new(xml), Durability::HIGH);
        }
        self.set_locale_input_langs(Arc::new(langs));
    }

    pub fn get_langs_in_use(&self) -> Vec<Lang> {
        let dl = self.default_lang();
        let mut vec: Vec<Lang> = dl.iter_fetchable_langs().collect();
        vec.sort();
        vec.dedup();
        vec
    }

    pub fn has_cached_locale(&self, lang: &Lang) -> bool {
        let langs = self.locale_input_langs();
        langs.contains(lang)
    }
}

/// Stores all the relevant #[salsa::input] entries from CiteDatabase.
/// They are all Arcs, so this is cheap.
#[derive(Debug)]
struct ClusterState {
    clusters_ordered: Arc<Vec<ClusterId>>,
    relevant_one: Option<OneClusterState>,
    /// Unrelated to clusters but still has to be restored
    old_positions: Option<Vec<(ClusterId, Option<ClusterNumber>)>>,
}
#[derive(Debug)]
struct OneClusterState {
    my_id: ClusterId,
    /// The entry for my_id
    cluster_note_number: Option<ClusterNumber>,
    /// The entry for my_id
    cluster_cites: Arc<Vec<CiteId>>,
    cluster_mode: Option<ClusterMode>,
}

impl Processor {
    fn save_cluster_state(&self, relevant_cluster: Option<ClusterId>) -> ClusterState {
        let clusters_ordered = self.clusters_ordered();
        let relevant_one = relevant_cluster
            .filter(|rc| clusters_ordered.contains(&rc))
            .map(|rc| OneClusterState {
                my_id: rc,
                cluster_note_number: self.cluster_note_number(rc),
                cluster_cites: self.cluster_cites(rc),
                cluster_mode: self.cluster_mode(rc),
            });
        ClusterState {
            clusters_ordered,
            relevant_one,
            old_positions: None,
        }
    }

    fn restore_cluster_state(&mut self, state: ClusterState) {
        let ClusterState {
            clusters_ordered,
            relevant_one,
            old_positions,
        } = state;
        if let Some(relevant) = relevant_one {
            let OneClusterState {
                my_id,
                cluster_cites,
                cluster_note_number,
                cluster_mode,
            } = relevant;
            self.set_cluster_cites(my_id, cluster_cites);
            self.set_cluster_note_number(my_id, cluster_note_number);
            self.set_cluster_mode(my_id, cluster_mode);
        }
        if let Some(old_pos) = old_positions {
            for (id, num) in old_pos {
                self.set_cluster_note_number(id, num);
            }
        }
        self.set_clusters_ordered(clusters_ordered);
    }

    /// Previews a citation as if it was inserted and positioned in the document.
    ///
    /// The position must be to either replace a single cluster, or to supply a complete document
    /// re-ordering with exactly one id set to 0. If you supply a PreviewPosition::MarkWithZero
    /// with only one position total, then it is as if the document only has one cluster. Prefer
    /// generating a complete reordering, with one position edited or inserted.
    ///
    /// Format defaults (if None) to the processor's native format, but may be set to another
    /// format. Note this is output only, so any disambiguation specialisation for a particular
    /// format's limitations/features is kept even though the output format is different. For
    /// example, a native HTML processor (set with `Processor::new`) can disambiguate with italics,
    /// but a native plain text processor cannot, and this will show up in whatever output format
    /// is chosen here.
    pub fn preview_citation_cluster<'a>(
        &mut self,
        preview_cluster: PreviewCluster,
        position: PreviewPosition<'a>,
        format: Option<SupportedFormat>,
    ) -> Result<Arc<MarkupOutput>, ReorderingError> {
        let (id, state) = match position {
            PreviewPosition::ReplaceCluster(cluster_id) => {
                let ids = self.all_cluster_ids();
                if !ids.contains(&cluster_id) {
                    return Err(ReorderingError::NonExistentCluster(cluster_id));
                }
                (cluster_id, self.save_cluster_state(Some(cluster_id)))
            }
            PreviewPosition::MarkWithZeroStr(positions) => {
                let positions: Vec<_> = positions
                    .into_iter()
                    .map(|x| {
                        let cid = x.id.as_ref().map(|id| self.cluster_id(id));
                        ClusterPosition {
                            id: cid,
                            note: x.note,
                        }
                    })
                    .collect();
                self.preview_marked_init(&positions)?
            }
            PreviewPosition::MarkWithZero(positions) => self.preview_marked_init(positions)?,
        };

        // insert the preview cluster data
        log::debug!("previewing cluster id {:?}", id);
        // we don't want the preview cluster in all_cluster_ids, so don't ensure_cluster_in_all
        // you could put it in there, but that's just needlessly thrashing all_cluster_ids
        self.insert_cites_only(id, preview_cluster.cites);
        self.set_cluster_mode(id, preview_cluster.mode);
        // we do set_cluster_note_number in preview_marked_init

        let formatter = format
            .map(|fmt| fmt.make_markup(self.format_options))
            .unwrap_or_else(|| self.get_formatter());
        let markup = citeproc_proc::db::built_cluster_preview(self, id, &formatter);
        let cluster_cites_sorted = self.cluster_cites_sorted(id);
        let nn = self.cluster_note_number(id);
        log::debug!("cluster_cites_sorted: {:?}", cluster_cites_sorted);
        log::debug!("cluster_note_number: {:?}", nn);
        self.restore_cluster_state(state);
        Ok(markup)
    }

    pub fn preview_reference(
        &mut self,
        mut refr: Reference,
        format: Option<SupportedFormat>,
    ) -> SmartString {
        const PREVIEW_REFERENCE_ID: &'static str = "REFERENCE-2b4e3fe4429cb";
        let preview_ref_id = Atom::from(PREVIEW_REFERENCE_ID);
        refr.id = preview_ref_id.clone();
        let arc = Arc::new(refr);
        self.set_reference_input(preview_ref_id.clone(), arc.clone());
        let formatter = format
            .map(|fmt| fmt.make_markup(self.format_options))
            .unwrap_or_else(|| self.get_formatter().clone());
        citeproc_proc::bib_item_preview(self, preview_ref_id.clone(), arc.as_ref(), &formatter)
    }

    fn preview_marked_init<'a>(
        &mut self,
        positions: &[ClusterPosition],
    ) -> Result<(ClusterId, ClusterState), ReorderingError> {
        if positions.iter().filter(|pos| pos.id.is_none()).count() != 1 {
            return Err(ReorderingError::DidNotSupplyZeroPosition);
        }

        let mut old_positions = Vec::with_capacity(positions.len() + 1);

        // Save state first so we don't clobber its cluster_ids store
        let mut state = self.save_cluster_state(None);
        self.set_cluster_order_inner(positions.iter(), |id, num| old_positions.push((id, num)))?;

        // schedule setting the preview cluster's position to None at the end
        old_positions.push((self.preview_cluster_id, None));
        state.old_positions = Some(old_positions);
        Ok((self.preview_cluster_id, state))
    }
}

// static PREVIEW_CLUSTER_ID: &'static str = "PREVIEW-7b2b4e3fe4429cb";

impl Processor {
    /// Specifies which clusters are actually considered to be in the document, and sets their
    /// order. You may insert as many clusters as you like, but the ones provided here are the only
    /// ones used.
    ///
    /// If a position does not provide a note, it is an in-text reference. Generally, this is what you
    /// should be providing for note styles, such that first-reference-note-number does not gain a
    /// value, but some users put in-text references inside footnotes, and it is unclear what the
    /// processor should do in this situation so you could try providing note numbers there as
    /// well.
    ///
    /// If a position provides a { note: N } field, then that N must be monotically increasing
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
    /// May error without having set_clusters_ordered, but with some set_cluster_note_number-s executed.
    pub fn set_cluster_order(
        &mut self,
        positions: &[ClusterPosition],
    ) -> Result<(), ReorderingError> {
        self.set_cluster_order_inner(positions.iter(), |_, _| {})
    }

    pub fn set_cluster_order_str(
        &mut self,
        positions: &[string_id::ClusterPosition],
    ) -> Result<(), string_id::ReorderingError> {
        let positions = positions.iter().map({
            // Move a clone of the arc into the iterator.
            let interner = self.interner.clone();
            move |pos| {
                let mut interner = interner.write().unwrap();
                let string_id::ClusterPosition { id, note } = pos;
                let interned_id = id.as_ref().map(|id| interner.get_or_intern(id));
                ClusterPosition {
                    id: interned_id,
                    note: *note,
                }
            }
        });
        self.set_cluster_order_inner(positions, |_, _| {})
            .map_err(|e| {
                let reader = self.interner.read().unwrap();
                e.to_external(&reader)
            })
    }

    /// Variant of the above that allows logging the changes.
    pub fn set_cluster_order_inner<T: std::borrow::Borrow<ClusterPosition>>(
        &mut self,
        positions: impl ExactSizeIterator<Item = T> + Clone,
        mut mods: impl FnMut(ClusterId, Option<ClusterNumber>),
    ) -> Result<(), ReorderingError> {
        log::debug!(
            "set_cluster_order_inner, {:?}",
            positions
                .clone()
                .map(|x| x.borrow().clone())
                .collect::<Vec<_>>()
        );
        let all_cluster_ids: Arc<FnvHashSet<ClusterId>> = self.all_cluster_ids();
        let mut new_clusters_ordered = Vec::with_capacity(positions.len());
        let mut intext_number = 1u32;
        // (note number, next index)
        let mut this_note: Option<(u32, u32)> = None;
        for piece in positions {
            let piece = piece.borrow();
            log::debug!("piece: {:?}", piece);
            if let Some(id) = piece.id {
                if !all_cluster_ids.contains(&id) {
                    log::debug!(
                        "non-existent cluster: {:?} not in old_cluster_ids {:?}",
                        id,
                        all_cluster_ids
                    );
                    return Err(ReorderingError::NonExistentCluster(id));
                }
            }
            let id_or: ClusterId = piece.id.unwrap_or(self.preview_cluster_id);
            if let Some(nn) = piece.note {
                if let Some(ref mut note) = this_note {
                    if nn < note.0 {
                        log::error!(
                            "reordering error: non-monotonic note number: {:?} < {:?}",
                            nn,
                            note.0
                        );
                        return Err(ReorderingError::NonMonotonicNoteNumber(nn));
                    }
                    if let Some(id) = piece.id {
                        mods(id, self.cluster_note_number(id));
                    }
                    if nn == note.0 {
                        // This note number ended up having more than one index in it;
                        let (num, ref mut index) = *note;
                        let i = *index;
                        *index += 1;
                        self.set_cluster_note_number(
                            id_or,
                            Some(ClusterNumber::Note(IntraNote::Multi(num, i))),
                        );
                    } else if nn > note.0 {
                        self.set_cluster_note_number(
                            id_or,
                            Some(ClusterNumber::Note(IntraNote::Multi(nn, 0))),
                        );
                        *note = (nn, 1);
                    }
                } else {
                    // the first note in the document
                    this_note = Some((nn, 1));
                    self.set_cluster_note_number(
                        id_or,
                        Some(ClusterNumber::Note(IntraNote::Multi(nn, 0))),
                    );
                }
                new_clusters_ordered.push(id_or);
            } else {
                let num = intext_number;
                intext_number += 1;
                self.set_cluster_note_number(id_or, Some(ClusterNumber::InText(num)));
                new_clusters_ordered.push(id_or);
            }
        }
        // This removes any clusters that did not appear.
        self.set_clusters_ordered(Arc::new(new_clusters_ordered));
        Ok(())
    }
}
