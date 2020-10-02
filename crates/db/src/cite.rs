// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::xml::{LocaleDatabase, StyleDatabase};

use csl::Locale;
use std::collections::HashSet;
use std::sync::Arc;

use citeproc_io::output::markup::Markup;
use citeproc_io::{Cite, ClusterId, ClusterNumber, Reference};
use csl::Atom;

#[salsa::query_group(CiteDatabaseStorage)]
pub trait CiteDatabase: LocaleDatabase + StyleDatabase {
    #[salsa::input]
    fn reference_input(&self, key: Atom) -> Arc<Reference>;
    fn reference(&self, key: Atom) -> Option<Arc<Reference>>;

    #[salsa::input]
    fn all_keys(&self) -> Arc<HashSet<Atom>>;

    #[salsa::input]
    fn all_uncited(&self) -> Arc<Uncited>;
    fn uncited_ordered(&self) -> Arc<Vec<Atom>>;

    /// Filters out keys not in the library
    fn cited_keys(&self) -> Arc<HashSet<Atom>>;

    /// Equal to `all.intersection(cited U uncited)`
    /// Also represents "the refs that will be in the bibliography if we generate one"
    fn disamb_participants(&self) -> Arc<HashSet<Atom>>;

    /// All the names that may need to be disambiguated among themselves
    fn names_to_disambiguate(&self) -> Arc<Vec<Name>>;

    #[salsa::input]
    fn cluster_ids(&self) -> Arc<Vec<ClusterId>>;

    #[salsa::input]
    fn cluster_note_number(&self, key: ClusterId) -> Option<ClusterNumber>;

    // All cite ids, in the order they are cited in the document
    fn all_cite_ids(&self) -> Arc<Vec<CiteId>>;

    fn locale_by_cite(&self, id: CiteId) -> Arc<Locale>;
    fn locale_by_reference(&self, ref_id: Atom) -> Arc<Locale>;

    /// Create ghost cites for disambiguation only as needed.
    /// These are subsequently interned into CiteIds.
    fn ghost_cite(&self, ref_id: Atom) -> Arc<Cite<Markup>>;

    #[salsa::interned]
    fn cite(&self, data: CiteData) -> CiteId;

    #[salsa::input]
    fn cluster_cites(&self, key: ClusterId) -> Arc<Vec<CiteId>>;

    fn clusters_sorted(&self) -> Arc<Vec<ClusterData>>;
}

#[macro_export]
macro_rules! intern_key {
    ($vis:vis $name:ident) => {
        #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
        $vis struct $name(salsa::InternId);
        impl ::salsa::InternKey for $name {
            fn from_intern_id(v: ::salsa::InternId) -> Self {
                $name(v)
            }
            fn as_intern_id(&self) -> ::salsa::InternId {
                self.0
            }
        }
    };
}

intern_key!(pub CiteId);

impl CiteId {
    pub fn lookup<DB: CiteDatabase + ?Sized>(self, db: &DB) -> Arc<Cite<Markup>> {
        match db.lookup_cite(self) {
            CiteData::RealCite { cite, .. } => cite,
            CiteData::BibliographyGhost { cite, .. } => cite,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum CiteData {
    /// This represents an actual cite in an actual cluster in the document.
    RealCite {
        cluster: ClusterId,
        index: u32,
        cite: Arc<Cite<Markup>>,
    },
    /// These are created as necessary when uncited items need to be rendered for disambiguation.
    /// The Arc<Cite> is the null object pattern, used merely to hold a reference id but keep the
    /// cite IR rendering identical for ghost and real cites.
    BibliographyGhost { cite: Arc<Cite<Markup>> },
}

fn ghost_cite(_db: &dyn CiteDatabase, ref_id: Atom) -> Arc<Cite<Markup>> {
    Arc::new(Cite::basic(ref_id))
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClusterData {
    pub id: ClusterId,
    pub number: ClusterNumber,
    pub cites: Arc<Vec<CiteId>>,
}

fn reference(db: &dyn CiteDatabase, key: Atom) -> Option<Arc<Reference>> {
    if db.all_keys().contains(&key) {
        Some(db.reference_input(key))
    } else {
        None
    }
}

fn locale_by_cite(db: &dyn CiteDatabase, id: CiteId) -> Arc<Locale> {
    let cite = id.lookup(db);
    db.locale_by_reference(cite.ref_id.clone())
}

fn locale_by_reference(db: &dyn CiteDatabase, ref_id: Atom) -> Arc<Locale> {
    let refr = db.reference(ref_id);
    refr.and_then(|r| r.language.clone())
        .map(|l| db.merged_locale(l))
        .unwrap_or_else(|| db.default_locale())
}

/// Type to represent which references should appear in a bibiliography even if they are not cited
/// in the document. The default is that references only appear if they are cited.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Uncited {
    /// Every single reference known to citeproc-rs will appear in the bibliography.
    All,
    /// A set of reference IDs, merged with the known references, to appear in the bibliography no
    /// matter what.
    Enumerated(Vec<Atom>),
}

/// Default is to have no uncited references present in bibliography.
impl Default for Uncited {
    fn default() -> Self {
        Uncited::Enumerated(Vec::new())
    }
}

fn uncited_ordered(db: &dyn CiteDatabase) -> Arc<Vec<Atom>> {
    let all = db.all_keys();
    let cited = db.cited_keys();
    let uncited = db.all_uncited();
    match &*uncited {
        Uncited::All => {
            // Determinism for year suffixes: sort by key id.
            let remainder = all.difference(&cited).cloned();
            let mut list: Vec<_> = remainder.collect();
            list.sort();
            Arc::new(list)
        }
        Uncited::Enumerated(list) => {
            let list = list
                .iter()
                .filter(|&x| !cited.contains(x) && all.contains(x))
                .cloned()
                .collect();
            Arc::new(list)
        }
    }
}

fn cited_keys(db: &dyn CiteDatabase) -> Arc<HashSet<Atom>> {
    let all = db.all_keys();
    let mut keys = HashSet::new();
    let all_cite_ids = db.all_cite_ids();
    for &id in all_cite_ids.iter() {
        keys.insert(id.lookup(db).ref_id.clone());
    }
    // make sure there are no keys we wouldn't recognise
    let merged = all.intersection(&keys).cloned().collect();
    Arc::new(merged)
}

fn disamb_participants(db: &dyn CiteDatabase) -> Arc<HashSet<Atom>> {
    let cited = db.cited_keys();
    let all = db.all_keys();
    let uncited = db.all_uncited();
    match &*uncited {
        Uncited::All => all.clone(),
        Uncited::Enumerated(specific) => {
            let mut merged = HashSet::with_capacity(cited.len() + specific.len());
            merged.extend(cited.iter().cloned());
            for key in specific {
                // make sure there are no keys we wouldn't recognise
                if all.contains(key) {
                    merged.insert(key.clone());
                }
            }
            Arc::new(merged)
        }
    }
}

use citeproc_io::Name;
use csl::GivenNameDisambiguationRule;
fn names_to_disambiguate(db: &dyn CiteDatabase) -> Arc<Vec<Name>> {
    let style = db.style();
    if GivenNameDisambiguationRule::ByCite == style.citation.givenname_disambiguation_rule {
        return Arc::new(Vec::new());
    }
    let uncited = db.disamb_participants();
    let mut v = Vec::new();
    for atom in uncited.iter() {
        if let Some(refr) = db.reference(atom.clone()) {
            for (_var, names) in refr.name.iter() {
                for name in names.iter() {
                    v.push(name.clone());
                }
            }
        }
    }
    Arc::new(v)
}

fn clusters_sorted(db: &dyn CiteDatabase) -> Arc<Vec<ClusterData>> {
    let cluster_ids = db.cluster_ids();
    let mut clusters: Vec<_> = cluster_ids
        .iter()
        // No number? Not considered to be in document, position participant.
        // Although may be disamb participant.
        .filter_map(|&id| get_cluster_data(db, id))
        .collect();
    clusters.sort_by_key(|cluster| cluster.number);
    Arc::new(clusters)
}

fn all_cite_ids(db: &dyn CiteDatabase) -> Arc<Vec<CiteId>> {
    let mut ids = Vec::new();
    let clusters = db.clusters_sorted();
    for cluster in clusters.iter() {
        ids.extend(cluster.cites.iter().cloned());
    }
    Arc::new(ids)
}

pub fn get_cluster_data(db: &dyn CiteDatabase, id: ClusterId) -> Option<ClusterData> {
    db.cluster_note_number(id).map(|number| ClusterData {
        id,
        number,
        cites: db.cluster_cites(id),
    })
}
