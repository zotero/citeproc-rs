use fnv::{FnvHashMap, FnvHashSet};
use std::collections::HashSet;
use std::sync::Arc;

use crate::input::{Cite, CiteContext, CiteId, Cluster, ClusterId, Reference};
use crate::style::db::StyleDatabase;
use crate::style::element::Position;
// use crate::input::{Reference, Cite};
use crate::locale::db::LocaleDatabase;
use crate::output::Pandoc;
use crate::proc::{AddDisambTokens, DisambToken, IrState, IR};
use crate::Atom;

#[salsa::query_group(ReferenceDatabaseStorage)]
pub trait ReferenceDatabase: salsa::Database + LocaleDatabase + StyleDatabase {
    #[salsa::input]
    fn reference_input(&self, key: Atom) -> Arc<Reference>;

    #[salsa::input]
    fn all_keys(&self, key: ()) -> Arc<HashSet<Atom>>;

    /// Filters out keys not in the library
    fn citekeys(&self, key: ()) -> Arc<HashSet<Atom>>;

    #[salsa::input]
    fn all_uncited(&self, key: ()) -> Arc<HashSet<Atom>>;
    /// Filters out keys not in the library
    fn uncited(&self, key: ()) -> Arc<HashSet<Atom>>;

    fn reference(&self, key: Atom) -> Option<Arc<Reference>>;

    fn disamb_tokens(&self, key: Atom) -> Arc<HashSet<DisambToken>>;

    fn inverted_index(&self, key: ()) -> Arc<FnvHashMap<DisambToken, HashSet<Atom>>>;

    fn cite_positions(&self, key: ()) -> Arc<FnvHashMap<CiteId, Position>>;
    fn cite_position(&self, key: CiteId) -> Position;

    #[salsa::input]
    fn cite(&self, key: CiteId) -> Arc<Cite<Pandoc>>;

    #[salsa::input]
    fn cluster_cites(&self, key: ClusterId) -> Arc<Vec<CiteId>>;

    #[salsa::input]
    fn cluster_ids(&self, key: ()) -> Arc<Vec<ClusterId>>;

    fn ir(&self, key: CiteId) -> Arc<IR<Pandoc>>;
}

fn reference(db: &impl ReferenceDatabase, key: Atom) -> Option<Arc<Reference>> {
    if db.all_keys(()).contains(&key) {
        Some(db.reference_input(key))
    } else {
        None
    }
}

// only call with real references please
fn disamb_tokens(db: &impl ReferenceDatabase, key: Atom) -> Arc<HashSet<DisambToken>> {
    let refr = db.reference_input(key);
    let mut set = HashSet::new();
    refr.add_tokens_index(&mut set);
    Arc::new(set)
}

fn inverted_index(
    db: &impl ReferenceDatabase,
    _: (),
) -> Arc<FnvHashMap<DisambToken, HashSet<Atom>>> {
    let mut index = FnvHashMap::default();
    // TODO: build index from (cited U uncited), not ALL keys.
    for key in db.citekeys(()).iter() {
        for tok in db.disamb_tokens(key.clone()).iter() {
            let ids = index.entry(tok.clone()).or_insert_with(|| HashSet::new());
            ids.insert(key.clone());
        }
    }
    Arc::new(index)
}

// make sure there are no keys we wouldn't recognise
fn uncited(db: &impl ReferenceDatabase, _: ()) -> Arc<HashSet<Atom>> {
    let all = db.all_keys(());
    let uncited = db.all_uncited(());
    let merged = all.intersection(&uncited).cloned().collect();
    Arc::new(merged)
}

// make sure there are no keys we wouldn't recognise
// TODO: compute citekeys from the clusters
fn citekeys(db: &impl ReferenceDatabase, _: ()) -> Arc<HashSet<Atom>> {
    let all = db.all_keys(());
    let mut citekeys = HashSet::new();
    let cluster_ids = db.cluster_ids(());
    let clusters = cluster_ids.iter().cloned().map(|id| db.cluster_cites(id));
    for cluster in clusters {
        citekeys.extend(cluster.iter().map(|&id| db.cite(id).ref_id.clone()));
    }
    let merged = all.intersection(&citekeys).cloned().collect();
    Arc::new(merged)
}

// See https://github.com/jgm/pandoc-citeproc/blob/e36c73ac45c54dec381920e92b199787601713d1/src/Text/CSL/Reference.hs#L910
fn cite_positions(db: &impl ReferenceDatabase, _: ()) -> Arc<FnvHashMap<CiteId, Position>> {
    let cluster_ids = db.cluster_ids(());
    let clusters: Vec<_> = cluster_ids
        .iter()
        .cloned()
        .map(|id| db.cluster_cites(id))
        .collect();

    let mut map = FnvHashMap::default();

    // TODO: configure
    let near_note_distance = 5;

    for (i, cluster) in clusters.iter().enumerate() {
        let mut seen = FnvHashMap::default();

        for (j, &id) in cluster.iter().enumerate() {
            let cite = db.cite(id);
            let prev_cite = cluster
                .get(j.wrapping_sub(1))
                .map(|&prev_id| db.cite(prev_id));
            let matching_prev = prev_cite
                .filter(|p| p.ref_id == cite.ref_id)
                .or_else(|| {
                    if let Some(prev_cluster) = (clusters.get(i.wrapping_sub(1))) {
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
                .map(|prev| {
                    if cite.locators.len() > 0 {
                        Position::Subsequent
                    } else if prev.locators == cite.locators {
                        Position::Ibid
                    } else {
                        Position::IbidWithLocator
                    }
                });
            if let Some(pos) = matching_prev {
                map.insert(id, pos);
            } else if let Some(last_id) = seen.get(&cite.ref_id) {
                // TODO: read position="subsequent" as Ibid || FarNote || NearNote
                if i - last_id < near_note_distance {
                    map.insert(id, Position::NearNote);
                } else {
                    map.insert(id, Position::FarNote);
                }
            } else {
                map.insert(id, Position::First);
            }
            seen.insert(cite.ref_id.clone(), i);
        }
    }

    Arc::new(map)
}

fn cite_position(db: &impl ReferenceDatabase, key: CiteId) -> Position {
    db.cite_positions(())
        .get(&key)
        .expect("called cite_position on unknown cite id")
        .clone()
}

fn ir(db: &impl ReferenceDatabase, id: CiteId) -> Arc<IR<Pandoc>> {
    use crate::proc::Proc;
    let style = db.style(());
    let cite = &db.cite(id);
    let fmt = Pandoc::default();
    // TODO: handle missing references
    let ctx = CiteContext {
        cite,
        format: &fmt,
        reference: &db.reference(cite.ref_id.clone()).expect("???"),
        position: db.cite_position(id),
        citation_number: 0, // XXX: from db
    };
    let mut state = IrState::new();
    Arc::new(style.intermediate(db, &mut state, &ctx).0)
}
