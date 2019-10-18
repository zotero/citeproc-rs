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

use csl::style::{
    Affixes, Delimiter, DisplayMode, Formatting, Name, NameEtAl, NameLabel, NameLabelInput, Names,
};
use csl::variables::{AnyVariable, DateVariable, NameVariable, NumberVariable, Variable};

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct NamesInheritance {
    pub name: Name,
    pub label: Option<NameLabelInput>,
    pub delimiter: Option<Atom>,
    pub et_al: Option<NameEtAl>,
    pub formatting: Option<Formatting>,
    pub display: Option<DisplayMode>,
    pub affixes: Option<Affixes>,
    // CSL-M: institutions
    // pub with: Option<NameWith>,
    // CSL-M: institutions
    // pub institution: Option<Institution>,
}

impl NamesInheritance {
    fn override_with(&self, ctx_name: &Name, ctx_delim: &Option<Delimiter>, names: &Names) -> Self {
        NamesInheritance {
            // Name gets merged from context, starting from scratch
            // So if you supply <name/> at all, you start from context.
            name: ctx_name.merge(names.name.as_ref().unwrap_or(&Name::empty())),
            // The rest will just replace whatever's in the inheritance
            et_al: names.et_al.as_ref().or(self.et_al.as_ref()).cloned(),
            label: names.label.as_ref().or(self.label.as_ref()).cloned(),
            delimiter: names
                .delimiter
                .as_ref()
                .map(|x| &x.0)
                .or_else(|| self.delimiter.as_ref())
                .or_else(|| ctx_delim.as_ref().map(|x| &x.0))
                .cloned(),
            formatting: names
                .formatting
                .as_ref()
                .or(self.formatting.as_ref())
                .cloned(),
            display: names.display.as_ref().or(self.display.as_ref()).cloned(),
            affixes: names.affixes.as_ref().or(self.affixes.as_ref()).cloned(),
        }
    }

    fn from_names(ctx_name: &Name, ctx_delim: &Option<Delimiter>, names: &Names) -> Self {
        NamesInheritance {
            name: ctx_name.merge(names.name.as_ref().unwrap_or(&Name::empty())),
            label: names.label.clone(),
            delimiter: names
                .delimiter
                .as_ref()
                .map(|x| &x.0)
                .or_else(|| ctx_delim.as_ref().map(|x| &x.0))
                .cloned(),
            et_al: names.et_al.clone(),
            formatting: names.formatting.clone(),
            display: names.display.clone(),
            affixes: names.affixes.clone(),
        }
    }
}

use fnv::FnvHashSet;

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct IrState {
    /// This can be a set because macros are strictly non-recursive.
    /// So the same macro name anywhere above indicates attempted recursion.
    /// When you exit a frame, delete from the set.
    pub macro_stack: HashSet<Atom>,
    /// Second field is names_delimiter
    name_override: Option<NamesInheritance>,
    suppressed: FnvHashSet<AnyVariable>,
    in_substitute: bool,
}

impl IrState {
    pub fn inherited_names_options(
        &self,
        ctx_name: &Name,
        ctx_delim: &Option<Delimiter>,
        own_names: &Names,
    ) -> NamesInheritance {
        match &self.name_override {
            None => NamesInheritance::from_names(ctx_name, ctx_delim, own_names),
            Some(stacked) => stacked.override_with(ctx_name, ctx_delim, own_names),
        }
    }

    pub fn replace_name_overrides_for_substitute(
        &mut self,
        inheritance: NamesInheritance,
    ) -> Option<NamesInheritance> {
        self.in_substitute = true;
        let old = std::mem::replace(&mut self.name_override, Some(inheritance));
        old
    }

    pub fn restore_name_overrides(&mut self, old: Option<NamesInheritance>) {
        if old.is_none() {
            self.in_substitute = false;
        }
        self.name_override = old;
    }

    pub fn is_name_suppressed(&self, var: NameVariable) -> bool {
        self.suppressed.contains(&AnyVariable::Name(var))
    }

    pub fn maybe_suppress_name_vars(&mut self, vars: &[NameVariable]) {
        if self.in_substitute {
            for &var in vars {
                self.suppressed.insert(AnyVariable::Name(var));
            }
        }
    }

    pub fn maybe_suppress_num(&mut self, var: NumberVariable) {
        if self.in_substitute {
            self.suppressed.insert(AnyVariable::Number(var));
        }
    }

    pub fn maybe_suppress_date(&mut self, var: DateVariable) {
        if self.in_substitute {
            self.suppressed.insert(AnyVariable::Date(var));
        }
    }

    pub fn maybe_suppress_ordinary(&mut self, var: Variable) {
        if self.in_substitute {
            self.suppressed.insert(AnyVariable::Ordinary(var));
        }
    }

    pub fn is_suppressed_ordinary(&self, var: Variable) -> bool {
        self.suppressed.contains(&AnyVariable::Ordinary(var))
    }

    pub fn is_suppressed_num(&self, var: NumberVariable) -> bool {
        self.suppressed.contains(&AnyVariable::Number(var))
    }

    pub fn is_suppressed_date(&self, var: DateVariable) -> bool {
        self.suppressed.contains(&AnyVariable::Date(var))
    }
}

impl IrState {
    pub fn new() -> Self {
        IrState::default()
    }
}
