use std::sync::Arc;
use std::collections::HashSet;
use salsa::Database;

use crate::Atom;
use crate::input::Reference;

#[salsa::query_group]
pub trait ReferenceDatabase: salsa::Database {
    #[salsa::input]
    fn reference(&self, key: Atom) -> Arc<Reference>;
    #[salsa::input]
    fn citekeys(&self, key: ()) -> Arc<HashSet<Atom>>;
}


