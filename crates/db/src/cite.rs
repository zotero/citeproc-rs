// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::xml::{LocaleDatabase, StyleDatabase};

use csl::Locale;
use csl::Position;
use fnv::FnvHashMap;
use std::collections::HashSet;
use std::sync::Arc;

use citeproc_io::output::markup::Markup;
use citeproc_io::{Cite, ClusterId, ClusterNumber, IntraNote, Reference};
use csl::Atom;

#[salsa::query_group(CiteDatabaseStorage)]
pub trait CiteDatabase: LocaleDatabase + StyleDatabase {
    #[salsa::input]
    fn reference_input(&self, key: Atom) -> Arc<Reference>;
    fn reference(&self, key: Atom) -> Option<Arc<Reference>>;

    #[salsa::input]
    fn all_keys(&self) -> Arc<HashSet<Atom>>;

    #[salsa::input]
    fn all_uncited(&self) -> Arc<HashSet<Atom>>;
    /// Filters out keys not in the library
    fn uncited(&self) -> Arc<HashSet<Atom>>;

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

    // All cite ids, in the order they appear in the document
    fn all_cite_ids(&self) -> Arc<Vec<CiteId>>;

    fn cite_positions(&self) -> Arc<FnvHashMap<CiteId, (Position, Option<u32>)>>;

    /// The first element is a [`Position`]; first, ibid, subsequent, etc
    ///
    /// The second is the 'First Reference Note Number' -- the number of the footnote containing the first cite
    /// referring to this cite's reference. This is None for a [`Position::First`].
    fn cite_position(&self, key: CiteId) -> (Position, Option<u32>);

    fn locale_by_cite(&self, id: CiteId) -> Arc<Locale>;
    fn locale_by_reference(&self, ref_id: Atom) -> Arc<Locale>;

    #[salsa::interned]
    fn cite(&self, cluster: ClusterId, index: u32, cite: Arc<Cite<Markup>>) -> CiteId;
    #[salsa::input]
    fn cluster_cites(&self, key: ClusterId) -> Arc<Vec<CiteId>>;
    fn clusters_sorted(&self) -> Arc<Vec<ClusterData>>;
    // #[salsa::input]
    // fn cluster_positions(&self) -> Arc<Vec<ClusterPosition>>;
}

#[macro_export]
macro_rules! intern_key {
    ($vis:vis $name:ident) => {
        #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
        $vis struct $name(u32);
        impl ::salsa::InternKey for $name {
            fn from_intern_id(v: ::salsa::InternId) -> Self {
                $name(u32::from(v))
            }
            fn as_intern_id(&self) -> ::salsa::InternId {
                self.0.into()
            }
        }
    };
}

intern_key!(pub CiteId);

impl CiteId {
    pub fn lookup(self, db: &impl CiteDatabase) -> Arc<Cite<Markup>> {
        let (_cluster_id, _index, cite) = db.lookup_cite(self);
        cite
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct ClusterData {
    pub id: ClusterId,
    pub number: ClusterNumber,
    pub cites: Arc<Vec<CiteId>>,
}

fn reference(db: &impl CiteDatabase, key: Atom) -> Option<Arc<Reference>> {
    if db.all_keys().contains(&key) {
        Some(db.reference_input(key))
    } else {
        None
    }
}

fn locale_by_cite(db: &impl CiteDatabase, id: CiteId) -> Arc<Locale> {
    let cite = id.lookup(db);
    db.locale_by_reference(cite.ref_id.clone())
}

fn locale_by_reference(db: &impl CiteDatabase, ref_id: Atom) -> Arc<Locale> {
    let refr = db.reference(ref_id);
    refr.and_then(|r| r.language.clone())
        .map(|l| db.merged_locale(l))
        .unwrap_or_else(|| db.default_locale())
}

// make sure there are no keys we wouldn't recognise
fn uncited(db: &impl CiteDatabase) -> Arc<HashSet<Atom>> {
    let all = db.all_keys();
    let uncited = db.all_uncited();
    let merged = all.intersection(&uncited).cloned().collect();
    Arc::new(merged)
}

fn cited_keys(db: &impl CiteDatabase) -> Arc<HashSet<Atom>> {
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

fn disamb_participants(db: &impl CiteDatabase) -> Arc<HashSet<Atom>> {
    let cited = db.cited_keys();
    let uncited = db.uncited();
    // make sure there are no keys we wouldn't recognise
    let merged = cited.union(&uncited).cloned().collect();
    Arc::new(merged)
}

use citeproc_io::Name;
use csl::GivenNameDisambiguationRule;
fn names_to_disambiguate(db: &impl CiteDatabase) -> Arc<Vec<Name>> {
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

fn all_cite_ids(db: &impl CiteDatabase) -> Arc<Vec<CiteId>> {
    let mut ids = Vec::new();
    let clusters = db.clusters_sorted();
    for cluster in clusters.iter() {
        ids.extend(cluster.cites.iter().cloned());
    }
    Arc::new(ids)
}

fn get_cluster_data(db: &impl CiteDatabase, id: ClusterId) -> Option<ClusterData> {
    db.cluster_note_number(id)
        .map(|number| ClusterData {
            id,
            number,
            cites: db.cluster_cites(id),
        })
}

fn clusters_sorted(db: &impl CiteDatabase) -> Arc<Vec<ClusterData>> {
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

// See https://github.com/jgm/pandoc-citeproc/blob/e36c73ac45c54dec381920e92b199787601713d1/src/Text/CSL/Reference.hs#L910
fn cite_positions(db: &impl CiteDatabase) -> Arc<FnvHashMap<CiteId, (Position, Option<u32>)>> {
    let clusters = db.clusters_sorted();

    let mut map = FnvHashMap::default();

    let style = db.style();
    let near_note_distance = style.citation.near_note_distance;
    warn!("near_note_distance {}", near_note_distance);

    // Backref table for FRNN
    // No entries for first ref == an in-text reference, only first time it appeared in a
    // footnote. This makes sense because note styles usually have a near-bibliography level of
    // detail, but in-text styles are often just author-date or a bibligraphy item number.
    let mut first_seen: FnvHashMap<Atom, ClusterNumber> = FnvHashMap::default();

    let mut last_note_num = None;
    let mut clusters_in_last_note: Vec<u32> = Vec::new();

    let mut prev_in_text: Option<&ClusterData> = None;
    let mut prev_note: Option<&ClusterData> = None;

    for (i, cluster) in clusters.iter().enumerate() {
        let prev_in_group = if let ClusterNumber::Note(_) = cluster.number {
            !clusters_in_last_note.is_empty()
        } else {
            false
        };
        let is_near = move |n: u32| {
            cluster
                .number
                .sub_note(IntraNote::Single(n))
                .map_or(false, |d| d <= near_note_distance)
        };
        let in_text = match cluster.number {
            ClusterNumber::InText(n) => Some(n),
            _ => None,
        };
        for (j, &cite_id) in cluster.cites.iter().enumerate() {
            let cite = cite_id.lookup(db);
            let prev_cite = cluster
                .cites
                // 0 - 1 == usize::MAX is never going to come up with anything
                .get(j.wrapping_sub(1))
                .map(|&prev_id| prev_id.lookup(db));
            enum Where {
                SameCluster(Arc<Cite<Markup>>),
                // Note Number here, so we can selectively apply near-note
                // There could be a bunch of non-cluster footnotes in between,
                // so we can't just assume two neighbouring clusters are actually next to each
                // other in the document.
                PrevCluster(Arc<Cite<Markup>>, Option<u32>),
            }
            let matching_prev = prev_cite
                .and_then(|p| {
                    if p.ref_id == cite.ref_id {
                        Some(Where::SameCluster(p))
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    if let Some(prev_cluster) = match cluster.number {
                        ClusterNumber::InText(_) => prev_in_text,
                        ClusterNumber::Note(_) => prev_note,
                    } {
                        let prev_number = match prev_cluster.number {
                            ClusterNumber::Note(intra) => Some(intra.note_number()),
                            _ => None,
                        };
                        let cites_all_same = if prev_in_group && in_text.is_none() {
                            // { id: 1, note: 4, cites: [A] },
                            // { id: 2, note: 4, cites: [B] },
                            // { id: 3: note: 5, cites: [B] } => subsequent
                            // (because prev note wasn't homogenous)
                            clusters_in_last_note
                                .iter()
                                .filter_map(|&cluster_id| get_cluster_data(db, cluster_id))
                                .flat_map(|cluster| (*cluster.cites).clone().into_iter())
                                .all(|cite_id| cite_id.lookup(db).ref_id == cite.ref_id)
                        } else {
                            prev_cluster
                                .cites
                                .iter()
                                .all(|&pid| pid.lookup(db).ref_id == cite.ref_id)
                        };
                        // Even if len was 0, fine because last() will end up with None anyway
                        if cites_all_same {
                            // Pick the last one to match locators against
                            prev_cluster
                                .cites
                                .last()
                                .map(|&pid| Where::PrevCluster(pid.lookup(db), prev_number))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .map(|prev| {
                    enum Num {
                        SameCluster,
                        PrevButInText,
                        PrevNote(u32),
                    }
                    let nn = match &prev {
                        Where::SameCluster(_) => Num::SameCluster,
                        Where::PrevCluster(_, None) => Num::PrevButInText,
                        Where::PrevCluster(_, Some(n)) => Num::PrevNote(*n),
                    };
                    let near = match nn {
                        Num::SameCluster => true,
                        Num::PrevButInText => false,
                        Num::PrevNote(n) => is_near(n),
                    };
                    let prev = match &prev {
                        Where::SameCluster(prev) | Where::PrevCluster(prev, _) => prev,
                    };
                    match (prev.locators.as_ref(), cite.locators.as_ref(), near) {
                        (None, None, false) => Position::Ibid,
                        (None, None, true) => Position::IbidNear,
                        (None, Some(_cur), false) => Position::IbidWithLocator,
                        (None, Some(_cur), true) => Position::IbidWithLocatorNear,
                        // Despite "position can only be subsequent", we get
                        // near/far note, as they imply subsequent.
                        (Some(_pre), None, x) => {
                            if x {
                                Position::NearNote
                            } else {
                                Position::FarNote
                            }
                        }
                        (Some(pre), Some(cur), x) if pre == cur => {
                            if x {
                                Position::IbidNear
                            } else {
                                Position::Ibid
                            }
                        }
                        (_, _, x) => {
                            if x {
                                Position::IbidWithLocatorNear
                            } else {
                                Position::IbidWithLocator
                            }
                        }
                    }
                });
            let seen = first_seen.get(&cite.ref_id).cloned();
            match seen {
                Some(ClusterNumber::Note(first_note_number)) => {
                    let first_number = ClusterNumber::Note(first_note_number);
                    assert!(
                        cluster.number >= first_number,
                        "note numbers not monotonic: {:?} came after but was less than {:?}",
                        cluster.number,
                        first_note_number,
                    );
                    let unsigned = first_note_number.note_number();
                    if let Some(pos) = matching_prev {
                        map.insert(cite_id, (pos, Some(unsigned)));
                    } else if cluster.number == first_number || is_near(unsigned) {
                        // XXX: not sure about this one
                        // unimplemented!("cite position for same number, but different cluster");
                        map.insert(cite_id, (Position::NearNote, Some(unsigned)));
                    } else {
                        map.insert(cite_id, (Position::FarNote, Some(unsigned)));
                    }
                }
                Some(ClusterNumber::InText(seen_in_text_num)) => {
                    // First seen was an in-text reference. Can be overwritten with a note cluster.
                    match cluster.number {
                        ClusterNumber::Note(_) => {
                            // Overwrite
                            first_seen.insert(cite.ref_id.clone(), cluster.number);
                            // First 'full' cite.
                            map.insert(cite_id, (Position::First, None));
                        }
                        ClusterNumber::InText(itnum) => {
                            let diff = itnum.wrapping_sub(seen_in_text_num);
                            let pos = if let Some(pos) = matching_prev {
                                pos
                            } else if diff <= near_note_distance {
                                Position::NearNote
                            } else {
                                Position::FarNote
                            };
                            map.insert(cite_id, (pos, None));
                        }
                    }
                }
                None => {
                    map.insert(cite_id, (Position::First, None));
                    first_seen.insert(cite.ref_id.clone(), cluster.number);
                }
            }
        }

        if let ClusterNumber::Note(n) = cluster.number {
            let n = n.note_number();
            if last_note_num != Some(n) {
                last_note_num = Some(n);
                clusters_in_last_note.clear();
            }
            clusters_in_last_note.push(cluster.id);
        }
        prev_in_text = if let ClusterNumber::InText(_i) = cluster.number {
            Some(cluster)
        } else {
            None
        };
        prev_note = if let ClusterNumber::Note(_i) = cluster.number {
            Some(cluster)
        } else {
            None
        };
    }

    Arc::new(map)
}

fn cite_position(db: &impl CiteDatabase, key: CiteId) -> (Position, Option<u32>) {
    if let Some(x) = db.cite_positions().get(&key) {
        *x
    } else {
        panic!("called cite_position on unknown cite id, {:?}", key);
    }
}
