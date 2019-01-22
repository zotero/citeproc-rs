use std::collections::HashSet;
use std::sync::Arc;

use crate::input::Reference;
use crate::Atom;

#[salsa::query_group]
pub trait ReferenceDatabase: salsa::Database {
    #[salsa::input]
    fn reference_input(&self, key: Atom) -> Arc<Reference>;
    #[salsa::input]
    fn citekeys(&self, key: ()) -> Arc<HashSet<Atom>>;

    fn reference(&self, key: Atom) -> Option<Arc<Reference>>;
}

fn reference(db: &impl ReferenceDatabase, key: Atom) -> Option<Arc<Reference>> {
    if db.citekeys(()).contains(&key) {
        Some(db.reference_input(key.clone()))
    } else {
        None
    }
}