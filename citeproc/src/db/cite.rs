// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::{LocaleDatabase, StyleDatabase};

use csl::locale::Locale;
use csl::style::{Position, Style};
use fnv::FnvHashMap;
use std::collections::HashSet;
use std::sync::Arc;

use crate::input::{Cite, CiteId, ClusterId, Reference};
use crate::output::Html;
use crate::proc::{AddDisambTokens, DisambToken, ProcDatabase};
use crate::Atom;

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

    fn disamb_tokens(&self, key: Atom) -> Arc<HashSet<DisambToken>>;

    fn inverted_index(&self) -> Arc<FnvHashMap<DisambToken, HashSet<Atom>>>;

    // priv
    #[salsa::input]
    fn cite(&self, key: CiteId) -> Arc<Cite<Html>>;

    #[salsa::input]
    fn cluster_ids(&self) -> Arc<Vec<ClusterId>>;

    #[salsa::input]
    fn cluster_cites(&self, key: ClusterId) -> Arc<Vec<CiteId>>;

    #[salsa::input]
    fn cluster_note_number(&self, key: ClusterId) -> u32;

    // All cite ids, in the order they appear in the document
    fn all_cite_ids(&self) -> Arc<Vec<CiteId>>;

    fn cite_positions(&self) -> Arc<FnvHashMap<CiteId, (Position, Option<u32>)>>;
    fn cite_position(&self, key: CiteId) -> (Position, Option<u32>);

    fn locale_by_cite(&self, id: CiteId) -> Arc<Locale>;

    fn sorted_refs(&self) -> Option<Arc<(Vec<Atom>, FnvHashMap<Atom, u32>)>>;
}

// We don't want too tight a coupling between the salsa DB and the proc module.
// It's just too annoying to refactor any changes here through all the Proc implementations.
impl<T> ProcDatabase for T
where
    T: CiteDatabase,
{
    #[inline]
    fn default_locale(&self) -> Arc<Locale> {
        self.merged_locale(self.style().default_locale.clone())
    }
    #[inline]
    fn locale(&self, id: CiteId) -> Arc<Locale> {
        self.locale_by_cite(id)
    }
    #[inline]
    fn style_el(&self) -> Arc<Style> {
        self.style()
    }
    #[inline]
    fn cite_pos(&self, id: CiteId) -> csl::style::Position {
        self.cite_position(id).0
    }
    #[inline]
    fn cite_frnn(&self, id: CiteId) -> Option<u32> {
        self.cite_position(id).1
    }
    fn bib_number(&self, id: CiteId) -> Option<u32> {
        let cite = self.cite(id);
        if let Some(abc) = self.sorted_refs() {
            let (_, ref lookup) = &*abc;
            lookup.get(&cite.ref_id).cloned()
        } else {
            None
        }
    }
}

fn reference(db: &impl CiteDatabase, key: Atom) -> Option<Arc<Reference>> {
    if db.all_keys().contains(&key) {
        Some(db.reference_input(key))
    } else {
        None
    }
}

fn locale_by_cite(db: &impl CiteDatabase, id: CiteId) -> Arc<Locale> {
    let cite = db.cite(id);
    let refr = db.reference(cite.ref_id.clone());
    refr.and_then(|r| r.language.clone())
        .map(|l| db.merged_locale(l))
        .unwrap_or_else(|| db.default_locale())
}

// only call with real references please
fn disamb_tokens(db: &impl CiteDatabase, key: Atom) -> Arc<HashSet<DisambToken>> {
    let refr = db.reference_input(key);
    let mut set = HashSet::new();
    refr.add_tokens_index(&mut set);
    Arc::new(set)
}

fn inverted_index(db: &impl CiteDatabase) -> Arc<FnvHashMap<DisambToken, HashSet<Atom>>> {
    let mut index = FnvHashMap::default();
    for key in db.disamb_participants().iter() {
        for tok in db.disamb_tokens(key.clone()).iter() {
            let ids = index.entry(tok.clone()).or_insert_with(|| HashSet::new());
            ids.insert(key.clone());
        }
    }
    Arc::new(index)
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
        keys.insert(db.cite(id).ref_id.clone());
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

fn all_cite_ids(db: &impl CiteDatabase) -> Arc<Vec<CiteId>> {
    let mut ids = Vec::new();
    let cluster_ids = db.cluster_ids();
    let clusters = cluster_ids.iter().cloned().map(|id| db.cluster_cites(id));
    for cluster in clusters {
        ids.extend(cluster.iter().cloned());
    }
    Arc::new(ids)
}

// See https://github.com/jgm/pandoc-citeproc/blob/e36c73ac45c54dec381920e92b199787601713d1/src/Text/CSL/Reference.hs#L910
fn cite_positions(db: &impl CiteDatabase) -> Arc<FnvHashMap<CiteId, (Position, Option<u32>)>> {
    let cluster_ids = db.cluster_ids();
    let clusters: Vec<_> = cluster_ids
        .iter()
        .map(|&id| (id, db.cluster_cites(id)))
        .collect();

    let mut map = FnvHashMap::default();

    // TODO: configure
    let near_note_distance = 5;

    let mut seen = FnvHashMap::default();

    for (i, (cluster_id, cluster)) in clusters.iter().enumerate() {
        let note_number = db.cluster_note_number(*cluster_id);
        for (j, &cite_id) in cluster.iter().enumerate() {
            let cite = db.cite(cite_id);
            let prev_cite = cluster
                .get(j.wrapping_sub(1))
                .map(|&prev_id| db.cite(prev_id));
            let matching_prev = prev_cite
                .filter(|p| p.ref_id == cite.ref_id)
                .or_else(|| {
                    if let Some((_, prev_cluster)) = clusters.get(i.wrapping_sub(1)) {
                        if prev_cluster.len() > 0
                            && prev_cluster
                                .iter()
                                .all(|&pid| db.cite(pid).ref_id == cite.ref_id)
                        {
                            // Pick the last one to match locators against
                            prev_cluster.last().map(|&pid| db.cite(pid))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .map(|prev| match (&prev.locators[..], &cite.locators[..]) {
                    (&[], &[]) => Position::Ibid,
                    (&[], _cur) => Position::IbidWithLocator,
                    (_pre, &[]) => Position::Subsequent,
                    (pre, cur) if pre == cur => Position::Ibid,
                    _ => Position::IbidWithLocator,
                });
            if let Some(&first_note_number) = seen.get(&cite.ref_id) {
                assert!(note_number >= first_note_number, "note numbers not monotonic: {} came after but was less than {}", note_number, first_note_number);
                if let Some(pos) = matching_prev {
                    map.insert(cite_id, (pos, Some(first_note_number)));
                } else if note_number == first_note_number {
                    // TODO: same footnote!
                    unimplemented!("cite position for same note_number, but different cluster");
                } else if note_number - first_note_number < near_note_distance {
                    map.insert(cite_id, (Position::NearNote, Some(first_note_number)));
                } else {
                    map.insert(cite_id, (Position::FarNote, Some(first_note_number)));
                }
            } else {
                map.insert(cite_id, (Position::First, None));
                seen.insert(cite.ref_id.clone(), note_number);
            }
        }
    }

    Arc::new(map)
}

fn cite_position(db: &impl CiteDatabase, key: CiteId) -> (Position, Option<u32>) {
    db.cite_positions()
        .get(&key)
        .expect("called cite_position on unknown cite id")
        .clone()
}

use csl::style::{Sort, SortSource};
use csl::variables::*;
use std::cmp::Ordering;

/// Creates a total ordering of References from a Sort element.
fn bib_ordering(a: &Reference, b: &Reference, sort: &Sort, _style: &Style) -> Ordering {
    //
    fn compare_demoting_none<T: Ord>(aa: Option<&T>, bb: Option<&T>) -> Ordering {
        match (aa, bb) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(aaa), Some(bbb)) => aaa.cmp(bbb),
        }
    }
    let mut ord = Ordering::Equal;
    for key in sort.keys.iter() {
        // If an ordering is found, you don't need to tie-break any further with more sort keys.
        if ord != Ordering::Equal {
            break;
        }
        ord = match key.sort_source {
            // TODO: implement macro-based sorting using a new Proc method
            SortSource::Macro(_) => Ordering::Equal,
            // For variables, we're not going to use the CiteContext wrappers, because if a
            // variable is not defined directly on the reference, it shouldn't be sortable-by, so
            // will just come back as None from reference.xxx.get() and produce Equal.
            SortSource::Variable(any) => match any {
                AnyVariable::Ordinary(v) => {
                    compare_demoting_none(a.ordinary.get(&v), b.ordinary.get(&v))
                }
                AnyVariable::Number(v) => compare_demoting_none(a.number.get(&v), b.number.get(&v)),
                AnyVariable::Name(_) => Ordering::Equal,
                AnyVariable::Date(_) => Ordering::Equal,
            },
        };
    }
    ord
}

fn sorted_refs(db: &impl CiteDatabase) -> Option<Arc<(Vec<Atom>, FnvHashMap<Atom, u32>)>> {
    let style = db.style();
    // TODO: also return None to avoid work if no bibliography was requested by the user
    let bib = match style.bibliography {
        None => return None,
        Some(ref b) => b,
    };

    let mut citation_numbers = FnvHashMap::default();

    // only the references that exist go in the bibliography
    // first, compute refs in the order that they are cited.
    // stable sorting will cause this to be the final tiebreaker.
    let all = db.all_keys();
    let mut preordered = Vec::new();
    let all_cite_ids = db.all_cite_ids();
    let mut i = 1;
    for &id in all_cite_ids.iter() {
        let ref_id = &db.cite(id).ref_id;
        if all.contains(ref_id) && !citation_numbers.contains_key(ref_id) {
            preordered.push(ref_id.clone());
            citation_numbers.insert(ref_id.clone(), i as u32);
            i += 1;
        }
    }
    let refs = if let Some(ref sort) = bib.sort {
        // dbg!(sort);
        preordered.sort_by(|a, b| {
            let ar = db.reference_input(a.clone());
            let br = db.reference_input(b.clone());
            bib_ordering(&ar, &br, sort, &style)
        });
        for (i, ref_id) in preordered.iter().enumerate() {
            citation_numbers.insert(ref_id.clone(), (i + 1) as u32);
        }
        preordered
    } else {
        // In the absence of cs:sort, cites and bibliographic entries appear in the order in which
        // they are cited.
        preordered
    };
    // dbg!(&refs);
    Some(Arc::new((refs, citation_numbers)))
}
