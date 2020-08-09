// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

// For the salsa macro expansion
#![allow(clippy::large_enum_variant)]
#![allow(clippy::enum_variant_names)]

pub mod update;

#[cfg(test)]
mod test;

use crate::prelude::*;

use self::update::{
    BibliographyMeta, BibliographyUpdate, DocUpdate, SecondFieldAlign, UpdateSummary,
};
use citeproc_db::{CiteDatabaseStorage, HasFetcher, LocaleDatabaseStorage, StyleDatabaseStorage};
use citeproc_proc::db::IrDatabaseStorage;

use salsa::Durability;
#[cfg(feature = "rayon")]
use salsa::{ParallelDatabase, Snapshot};
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;

use csl::Lang;
use csl::Style;
use csl::StyleError;

use citeproc_io::output::{markup::Markup, OutputFormat};
use citeproc_io::{Cite, Cluster, ClusterId, ClusterNumber, Reference};
use csl::Atom;

#[allow(dead_code)]
type MarkupBuild = <Markup as OutputFormat>::Build;
#[allow(dead_code)]
type MarkupOutput = <Markup as OutputFormat>::Output;
use fnv::FnvHashMap;

struct SavedBib {
    sorted_refs: Arc<(Vec<Atom>, FnvHashMap<Atom, u32>)>,
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

// Need to keep this index in sync with the order in which the group storage structs appear below
const IR_GROUP_INDEX: u16 = 3;

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
    queue: Arc<Mutex<Vec<DocUpdate>>>,
    save_updates: bool,
    last_bibliography: Arc<Mutex<SavedBib>>,
}

/// This impl tells salsa where to find the salsa runtime.
impl salsa::Database for Processor {

    /// A way to extract imperative update sequences from a "here's the entire world" API. An
    /// editor might require simple instructions to update a document; modify this footnote,
    /// replace that bibliography entry. We will use Salsa WillExecute events to determine which
    /// things were recomputed, and assume a recomputation means re-rendering is necessary.
    fn salsa_event(&self, event: salsa::Event) {
        if !self.save_updates {
            return;
        }
        use salsa::EventKind::*;
        if let WillExecute { database_key: db_key } = event.kind {
            use citeproc_proc::db::BuiltClusterQuery;
            if db_key.group_index() == IR_GROUP_INDEX
                && db_key.query_index() == <BuiltClusterQuery as salsa::Query>::QUERY_INDEX {

                // TODO: this is a massive hack. Get a key index lookup function into Salsa.
                let formatted = format!("{:?}", db_key.debug(self));
                // The format is "built_cluster(123)".
                // log::error!("{:?}", db_key.debug(self));
                let id: u32 = formatted.strip_prefix("built_cluster(").unwrap().trim_end_matches(')').parse().unwrap();

                let mut q = self.queue.lock().unwrap();
                let upd = DocUpdate::Cluster(id);
                // info!("produced update, {:?}", upd);
                q.push(upd);
            }
        }
    }
}

#[cfg(feature = "rayon")]
impl ParallelDatabase for Processor {
    fn snapshot(&self) -> Snapshot<Self> {
        Snapshot::new(Processor {
            storage: self.storage.snapshot(),
            fetcher: self.fetcher.clone(),
            queue: self.queue.clone(),
            save_updates: self.save_updates,
            formatter: self.formatter.clone(),
            last_bibliography: self.last_bibliography.clone(),
        })
    }
}

impl HasFetcher for Processor {
    fn get_fetcher(&self) -> Arc<dyn LocaleFetcher> {
        self.fetcher.clone()
    }
}

impl HasFormatter for Processor {
    fn get_formatter(&self) -> Markup {
        self.formatter.clone()
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SupportedFormat {
    Html,
    Rtf,
    Plain,
    TestHtml,
}

impl FromStr for SupportedFormat {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "html" => Ok(SupportedFormat::Html),
            "rtf" => Ok(SupportedFormat::Rtf),
            "plain" => Ok(SupportedFormat::Plain),
            _ => Err(()),
        }
    }
}

impl<'de> serde::de::Deserialize<'de> for SupportedFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        use serde::de::Error as DeError;
        SupportedFormat::from_str(s.as_str())
            .map_err(|()| DeError::custom(format!("unknown format {}", s.as_str())))
    }
}

impl Processor {
    pub(crate) fn safe_default(fetcher: Arc<dyn LocaleFetcher>) -> Self {
        let mut db = Processor {
            storage: Default::default(),
            fetcher,
            queue: Arc::new(Mutex::new(Default::default())),
            save_updates: false,
            formatter: Markup::default(),
            last_bibliography: Arc::new(Mutex::new(SavedBib::new())),
        };
        citeproc_db::safe_default(&mut db);
        db
    }

    pub fn new(
        style_string: &str,
        fetcher: Arc<dyn LocaleFetcher>,
        save_updates: bool,
        format: SupportedFormat,
    ) -> Result<Self, StyleError> {
        let mut db = Processor::safe_default(fetcher);
        db.save_updates = save_updates;
        db.formatter = match format {
            SupportedFormat::Html => Markup::html(),
            SupportedFormat::Rtf => Markup::rtf(),
            SupportedFormat::Plain => Markup::plain(),
            SupportedFormat::TestHtml => Markup::test_html(),
        };
        let style = Arc::new(Style::from_str(style_string)?);
        db.set_style_with_durability(style, Durability::MEDIUM);
        Ok(db)
    }

    pub fn set_style_text(&mut self, style_text: &str) -> Result<(), StyleError> {
        let style = Style::from_str(style_text)?;
        self.set_style_with_durability(Arc::new(style), Durability::MEDIUM);
        Ok(())
    }

    #[cfg(test)]
    pub fn test_db() -> Self {
        use citeproc_db::PredefinedLocales;
        Processor::safe_default(Arc::new(PredefinedLocales(Default::default())))
    }

    #[cfg(feature = "rayon")]
    fn snap(&self) -> Snap {
        Snap(self.snapshot())
    }

    // TODO: This might not play extremely well with Salsa's garbage collector,
    // which will have a new revision number for each built_cluster call.
    // Probably better to have this as a real query.
    pub fn compute(&self) {
        let clusters = self.clusters_cites_sorted();
        #[cfg(feature = "rayon")]
        {
            use rayon::prelude::*;
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
                .for_each_with(self.snap(), |snap, cluster| {
                    snap.0.built_cluster(cluster.id);
                });
        }
        #[cfg(not(feature = "rayon"))]
        {
            for cluster in clusters.iter() {
                self.built_cluster(cluster.id);
            }
        }
    }

    pub fn batched_updates(&self) -> UpdateSummary {
        if !self.save_updates {
            return UpdateSummary::default();
        }
        self.compute();
        let mut queue = self.queue.lock().unwrap();
        let mut summary = UpdateSummary::summarize(self, &*queue);
        queue.clear();
        // Technically, you should probably have a lock over save_and_diff_bibliography as well, so
        // you get a point-in-time shapshot, but at the moment, that would mean recursively locking
        // queue as computing the bibliography will also create salsa events.
        drop(queue);
        if self.get_style().bibliography.is_some() {
            let bib = self.save_and_diff_bibliography();
            summary.bibliography = bib;
        }
        summary
    }

    pub fn drain(&mut self) {
        self.compute();
        let mut queue = self.queue.lock().unwrap();
        queue.clear();
    }

    // // TODO: make this use a function exported from citeproc_proc
    // pub fn single(&self, ref_id: &Atom) -> <Markup as OutputFormat>::Output {
    //     let fmt = Markup::default();
    //     let refr = match self.reference(ref_id.clone()) {
    //         None => return fmt.output(fmt.plain("Reference not found")),
    //         Some(r) => r,
    //     };
    //     let ctx = CiteContext {
    //         reference: &refr,
    //         cite: &Cite::basic("ok"),
    //         position: Position::First,
    //         format: Markup::default(),
    //         citation_number: 1,
    //         disamb_pass: None,
    //     };
    //     let style = self.style();
    //     let mut state = IrState::new();
    //     use crate::proc::Proc;
    //     let ir = style.intermediate(self, &mut state, &ctx).0;
    //     ir.flatten(&fmt)
    //         .map(|flat| fmt.output(flat))
    //         .unwrap_or(<Markup as OutputFormat>::Output::default())
    // }

    pub fn set_references(&mut self, refs: Vec<Reference>) {
        let keys: HashSet<Atom> = refs.iter().map(|r| r.id.clone()).collect();
        for r in refs {
            self.set_reference_input(r.id.clone(), Arc::new(r));
        }
        self.set_all_keys(Arc::new(keys));
    }

    pub fn insert_reference(&mut self, refr: Reference) {
        self.set_references(vec![refr])
    }

    pub fn init_clusters(&mut self, clusters: Vec<Cluster<Markup>>) {
        let mut cluster_ids = Vec::new();
        for cluster in clusters {
            let Cluster {
                id: cluster_id,
                cites,
            } = cluster;
            let mut ids = Vec::new();
            for (index, cite) in cites.into_iter().enumerate() {
                let cite_id = self.cite(cluster_id, index as u32, Arc::new(cite));
                ids.push(cite_id);
            }
            self.set_cluster_cites(cluster_id, Arc::new(ids));
            self.set_cluster_note_number(cluster_id, None);
            cluster_ids.push(cluster_id);
        }
        self.set_cluster_ids(Arc::new(cluster_ids));
    }

    // cluster_ids is maintained manually
    // the cluster_cites relation is maintained manually

    pub fn remove_cluster(&mut self, cluster_id: ClusterId) {
        self.set_cluster_cites(cluster_id, Arc::new(Vec::new()));
        self.set_cluster_note_number(cluster_id, None);
        let cluster_ids = self.cluster_ids();
        let cluster_ids: Vec<_> = (*cluster_ids)
            .iter()
            .filter(|&i| *i != cluster_id)
            .cloned()
            .collect();
        self.set_cluster_ids(Arc::new(cluster_ids));
    }

    pub fn insert_cluster(&mut self, cluster: Cluster<Markup>) {
        let Cluster {
            id: cluster_id,
            cites,
        } = cluster;
        let cluster_ids = self.cluster_ids();
        if !cluster_ids.contains(&cluster_id) {
            let mut new_cluster_ids = (*cluster_ids).clone();
            new_cluster_ids.push(cluster_id);
            self.set_cluster_ids(Arc::new(new_cluster_ids));
        }

        let mut ids = Vec::new();
        for (index, cite) in cites.iter().enumerate() {
            let cite_id = self.cite(cluster_id, index as u32, Arc::new(cite.clone()));
            ids.push(cite_id);
        }
        self.set_cluster_cites(cluster_id, Arc::new(ids));
        self.set_cluster_note_number(cluster_id, None);
    }

    pub fn renumber_clusters(&mut self, mappings: &[(u32, ClusterNumber)]) {
        for chunk in mappings {
            let cluster_id = chunk.0;
            let n = chunk.1;
            self.set_cluster_note_number(cluster_id, Some(n));
        }
    }

    // Getters, because the query groups have too much exposed to publish.

    pub fn get_cite(&self, id: CiteId) -> Arc<Cite<Markup>> {
        id.lookup(self)
    }

    /// Returns None if the cluster has not been assigned a position in the document.
    pub fn get_cluster(&self, cluster_id: ClusterId) -> Option<Arc<MarkupOutput>> {
        if self.cluster_note_number(cluster_id).is_some() {
            Some(self.built_cluster(cluster_id))
        } else {
            None
        }
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
                line_spacing: bib.line_spaces,
                hanging_indent: bib.hanging_indent,
                // To avoid serde derive in csl
                second_field_align: bib.second_field_align.as_ref().map(|s| match s {
                    csl::style::SecondFieldAlign::Flush => SecondFieldAlign::Flush,
                    csl::style::SecondFieldAlign::Margin => SecondFieldAlign::Margin,
                }),
                format_meta: self.formatter.meta(),
            }
        })
    }

    fn save_and_diff_bibliography(&self) -> Option<BibliographyUpdate> {
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
        if sorted_refs != old.sorted_refs {
            update.entry_ids = Some(sorted_refs.0.clone());
            last_bibliography.sorted_refs = sorted_refs;
            Some(update)
        } else if update.updated_entries.is_empty() {
            None
        } else {
            Some(update)
        }
    }

    pub fn get_bibliography(&self) -> Vec<MarkupOutput> {
        let bib_map = self.get_bibliography_map();
        self.sorted_refs()
            .0
            .iter()
            .filter_map(|k| bib_map.get(&k))
            .filter(|k| !k.is_empty())
            .map(|x| (**x).clone())
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
            self.set_locale_input_xml_with_durability(lang, Arc::new(xml), Durability::MEDIUM);
        }
        self.set_locale_input_langs(Arc::new(langs));
    }

    pub fn get_langs_in_use(&self) -> Vec<Lang> {
        let mut langs: HashSet<Lang> = self
            .all_keys()
            .iter()
            .filter_map(|ref_id| self.reference(ref_id.clone()))
            .filter_map(|refr| refr.language.clone())
            .collect();
        let style = self.style();
        langs.insert(style.default_locale.clone());
        langs.into_iter().collect()
    }

    pub fn has_cached_locale(&self, lang: &Lang) -> bool {
        let langs = self.locale_input_langs();
        langs.contains(lang)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    #[error(
        "set_cluster_order called with a note number {0} that was out of order (e.g. [1, 2, 3, 1])"
    )]
    NonMonotonicNoteNumber(u32),
}

impl Processor {
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
    pub fn set_cluster_order(&mut self, pieces: &[ClusterPosition]) -> Result<(), ErrorKind> {
        let mut cluster_ids = Vec::with_capacity(pieces.len());
        let mut intext_number = 1u32;
        // (note number, next index)
        let mut this_note: Option<(u32, u32)> = None;
        for piece in pieces {
            if let Some(nn) = piece.note {
                if let Some(ref mut note) = this_note {
                    if nn < note.0 {
                        return Err(ErrorKind::NonMonotonicNoteNumber(nn));
                    } else if nn == note.0 {
                        // This note number ended up having more than one index in it;
                        let (num, ref mut index) = *note;
                        let i = *index;
                        *index += 1;
                        self.set_cluster_note_number(
                            piece.id,
                            Some(ClusterNumber::Note(IntraNote::Multi(num, i))),
                        );
                    } else if nn > note.0 {
                        self.set_cluster_note_number(
                            piece.id,
                            Some(ClusterNumber::Note(IntraNote::Multi(nn, 0))),
                        );
                        *note = (nn, 1);
                    }
                } else {
                    // the first note in the document
                    this_note = Some((nn, 1));
                    self.set_cluster_note_number(
                        piece.id,
                        Some(ClusterNumber::Note(IntraNote::Multi(nn, 0))),
                    );
                }
                cluster_ids.push(piece.id);
            } else {
                let num = intext_number;
                intext_number += 1;
                self.set_cluster_note_number(piece.id, Some(ClusterNumber::InText(num)));
                cluster_ids.push(piece.id);
            }
        }
        // This removes any clusters that did not appear.
        self.set_cluster_ids(Arc::new(cluster_ids));
        Ok(())
    }
}
