// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

#[macro_use]
extern crate log;

// #[macro_use]
extern crate citeproc_db;

use citeproc_io::output::OutputFormat;
use csl::Atom;

use std::collections::HashSet;

mod choose;
mod cite_context;
mod date;
pub mod db;
pub mod disamb;
mod element;
mod group;
mod helpers;
mod ir;
mod names;
mod number;
mod renderer;
mod unicode;

pub(crate) mod prelude {
    pub use crate::db::{HasFormatter, IrDatabase};
    pub use crate::renderer::GenericContext;
    pub use citeproc_db::{CiteDatabase, CiteId, LocaleDatabase, StyleDatabase};
    pub use citeproc_io::output::markup::Markup;
    pub use citeproc_io::output::OutputFormat;
    pub use citeproc_io::IngestOptions;

    pub use csl::style::{Affixes, Element, Formatting};

    pub use crate::cite_context::CiteContext;
    pub use crate::group::GroupVars;
    pub use crate::ir::*;

    pub(crate) use crate::disamb::{
        cross_product, Disambiguation, Edge, EdgeData, FreeCondSets, RefContext,
    };
    pub(crate) use crate::helpers::*;
    pub(crate) use crate::renderer::Renderer;
    pub(crate) use crate::{IrState, Proc};

    pub type MarkupBuild = <Markup as OutputFormat>::Build;
}

use prelude::*;

#[cfg(test)]
mod test;

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

use csl::style::{Delimiter, Name, NameLabel, NameLabelInput};

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct IrState {
    /// This can be a set because macros are strictly non-recursive.
    /// So the same macro name anywhere above indicates attempted recursion.
    /// When you exit a frame, delete from the set.
    pub macro_stack: HashSet<Atom>,
    /// Second field is names_delimiter
    pub name_override: Option<(Name, NameLabelInput, Atom)>,
}

impl IrState {

    fn inherited_name_el(&self, ctx_name: &Name, own: &Option<Name>) -> Name {
        let inherited = if let Some((ref name_el, ..)) = self.name_override.as_ref() {
            name_el
        } else {
            ctx_name
        };
        inherited.merge(own.as_ref().unwrap_or(&Name::empty()))
    }

    fn inherited_label_el(&self, own: &Option<NameLabelInput>) -> NameLabelInput {
        let empty = NameLabelInput::empty();
        let inherited = if let Some((_, ref name_el, _)) = self.name_override.as_ref() {
            name_el
        } else {
            &empty
        };
        inherited.merge(own.as_ref().unwrap_or(&NameLabelInput::empty()))
    }

    fn inherited_names_delimiter(
        &self,
        ctx_delim: &Option<Delimiter>,
        own: &Option<Delimiter>,
    ) -> Atom {
        let own_names_delim = own.as_ref().map(|x| &x.0);
        let inherited_names_delim = self
            .name_override
            .as_ref()
            .map(|x| &x.2)
            .or(ctx_delim.as_ref().map(|x| &x.0));
        own_names_delim
            .or(inherited_names_delim)
            .map(|d| d.clone())
            .unwrap_or_else(|| Atom::from(""))
    }

    pub fn inherited_names_options(
        &self,
        ctx_name: &Name,
        own_name: &Option<Name>,
        own_label: &Option<NameLabelInput>,
        ctx_delim: &Option<Delimiter>,
        own_delim: &Option<Delimiter>,
    ) -> (Name, NameLabelInput, Atom) {
        (
            self.inherited_name_el(ctx_name, own_name),
            self.inherited_label_el(own_label),
            self.inherited_names_delimiter(ctx_delim, own_delim),
        )
    }

    pub fn replace_name_overrides(&mut self, name: Name, label: NameLabelInput, delim: Atom) -> Option<(Name, NameLabelInput, Atom)> {
        let old = std::mem::replace(&mut self.name_override, Some((name, label, delim)));
        old
    }

    pub fn restore_name_overrides(&mut self, old: Option<(Name, NameLabelInput, Atom)>) {
        self.name_override = old;
    }
}

impl IrState {
    pub fn new() -> Self {
        IrState::default()
    }
}
