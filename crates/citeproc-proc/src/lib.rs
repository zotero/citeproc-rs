// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

// #[macro_use]
// extern crate serde_derive;

use citeproc_io::output::OutputFormat;
use csl::Atom;
use fnv::FnvHashMap;
use std::collections::HashSet;

mod choose;
mod cite_context;
mod date;
pub mod db;
mod disamb;
mod element;
mod group;
mod helpers;
mod ir;
mod names;
mod unicode;

pub(crate) mod prelude {
    pub use crate::db::IrDatabase;
    pub use citeproc_db::{CiteDatabase, LocaleDatabase, StyleDatabase};
    pub use citeproc_io::output::OutputFormat;
    pub use citeproc_io::IngestOptions;

    pub use crate::cite_context::CiteContext;
    pub use crate::disamb::old::{AddDisambTokens, DisambToken};
    pub use crate::group::GroupVars;
    pub use crate::ir::*;
    pub(crate) use crate::{IrState, Proc};
}

use prelude::*;

#[cfg(test)]
mod test;

pub use self::disamb::old::DisambToken;
pub use self::ir::IR;

// TODO: function to walk the entire tree for a <text variable="year-suffix"> to work out which
// nodes are possibly disambiguate-able in year suffix mode and if such a node should be inserted
// at the end of the layout block before the suffix. (You would only insert an IR node, not in the
// actual style, to keep it immutable and plain-&borrow-thread-shareable).
// TODO: also to figure out which macros are needed
// TODO: juris-m module loading in advance? probably in advance.

// Levels 1-3 will also have to update the ConditionalDisamb's current render

pub(crate) trait Proc<'c, O>
where
    O: OutputFormat,
{
    fn intermediate(
        &self,
        db: &impl IrDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O>,
    ) -> IrSum<O>;
}

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct IrState {
    pub tokens: HashSet<DisambToken>,
    pub name_tokens: FnvHashMap<u64, HashSet<DisambToken>>,
    /// This can be a set because macros are strictly non-recursive.
    /// So the same macro name anywhere above indicates attempted recursion.
    /// When you exit a frame, delete from the set.
    pub macro_stack: HashSet<Atom>,
}

impl IrState {
    pub fn new() -> Self {
        IrState::default()
    }
}
