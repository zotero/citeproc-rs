use fnv::FnvHashMap;
use std::collections::HashSet;
use std::sync::Arc;

use crate::input::Reference;
use crate::proc::{AddDisambTokens, DisambToken};
use crate::Atom;

#[salsa::query_group]
pub trait ReferenceDatabase: salsa::Database {
    #[salsa::input]
    fn reference_input(&self, key: Atom) -> Arc<Reference>;
    #[salsa::input]
    fn citekeys(&self, key: ()) -> Arc<HashSet<Atom>>;

    fn reference(&self, key: Atom) -> Option<Arc<Reference>>;

    fn disamb_tokens(&self, key: Atom) -> Arc<HashSet<DisambToken>>;

    fn inverted_index(&self, key: ()) -> Arc<FnvHashMap<DisambToken, HashSet<Atom>>>;
}

fn reference(db: &impl ReferenceDatabase, key: Atom) -> Option<Arc<Reference>> {
    if db.citekeys(()).contains(&key) {
        Some(db.reference_input(key))
    } else {
        None
    }
}

// only call with real references please
fn disamb_tokens(db: &impl ReferenceDatabase, key: Atom) -> Arc<HashSet<DisambToken>> {
    let refr = db.reference_input(key);
    let mut set = HashSet::new();
    refr.add_disamb_tokens(&mut set);
    Arc::new(set)
}

fn inverted_index(
    db: &impl ReferenceDatabase,
    _: (),
) -> Arc<FnvHashMap<DisambToken, HashSet<Atom>>> {
    let mut index = FnvHashMap::default();
    for key in db.citekeys(()).iter() {
        for tok in db.disamb_tokens(key.clone()).iter() {
            let ids = index.entry(tok.clone()).or_insert_with(|| HashSet::new());
            ids.insert(key.clone());
        }
    }
    Arc::new(index)
}
