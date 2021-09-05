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
use std::collections::HashSet;

macro_rules! smart_format {
    ($lit:literal, $($expr:expr),*) => {
        {
            use std::fmt::Write;
            let mut smart = $crate::prelude::SmartString::new();
            write!(&mut smart, $lit, $($expr),*).expect("a formatting trait implementation returned an error");
            smart
        }
    }
}

mod choose;
mod citation_label;
mod cite_context;
mod cluster;
mod date;
pub mod db;
pub mod disamb;
mod element;
mod group;
mod helpers;
mod ir;
mod names;
mod number;
mod page_range;
mod ref_ir;
mod renderer;
mod sort;
mod tree;
mod walker;

pub use crate::cluster::built_cluster_before_output;
pub use crate::db::bib_item_preview;
pub use crate::db::safe_default;
pub use crate::sort::BibNumber;

pub(crate) mod prelude {
    pub(crate) trait AsRefOptStr {
        fn as_opt_str(&self) -> Option<&str>;
    }
    impl AsRefOptStr for Option<SmartString> {
        fn as_opt_str(&self) -> Option<&str> {
            self.as_ref().map(|x| x.as_str())
        }
    }
    impl<'a> AsRefOptStr for Option<&'a SmartString> {
        fn as_opt_str(&self) -> Option<&str> {
            self.as_ref().map(|x| x.as_str())
        }
    }
    pub use crate::ir::IrSum;
    pub type IrArena<O = Markup> = indextree::Arena<IrSum<O>>;
    pub use crate::cite_context::RenderContext;
    pub use crate::db::{safe_default, ImplementationDetails, IrDatabase};
    pub use crate::renderer::GenericContext;
    pub(crate) use crate::tree::{IrTree, IrTreeMut, IrTreeRef};
    pub use crate::walker::{StyleWalker, WalkerFoldType};

    pub use citeproc_db::{CiteDatabase, CiteId, LocaleDatabase, StyleDatabase};
    pub use citeproc_io::output::markup::Markup;
    pub use citeproc_io::output::OutputFormat;
    pub use citeproc_io::IngestOptions;
    pub use citeproc_io::{NumberLike, NumericValue};
    pub use citeproc_io::{SmartCow, SmartString};
    pub use csl::CiteOrBib;
    pub use indextree::{Node, NodeId};

    pub use csl::{Affixes, DisplayMode, Element, Formatting, TextCase};

    pub use crate::cite_context::CiteContext;
    pub use crate::group::GroupVars;
    pub use crate::ir::*;
    pub use crate::ref_ir::*;
    pub use crate::sort::BibNumber;

    pub(crate) type MarkupBuild = <Markup as OutputFormat>::Build;
    pub(crate) type MarkupOutput = <Markup as OutputFormat>::Output;
    pub(crate) use crate::disamb::{Disambiguation, EdgeData, RefContext};
    pub(crate) use crate::helpers::*;
    pub(crate) use crate::renderer::Renderer;
    pub(crate) use crate::{IrState, Proc};
    pub(crate) const CSL_STYLE_ERROR: &'static str =
        "[CSL STYLE ERROR: reference with no printed form.]";
    pub(crate) const CLUSTER_NO_PRINTED_FORM: &'static str = "[NO_PRINTED_FORM]";
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

pub(crate) trait Proc<'c, O, I>
where
    O: OutputFormat,
    I: OutputFormat,
{
    fn intermediate(
        &self,
        db: &dyn IrDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O, I>,
        arena: &mut IrArena<O>,
    ) -> NodeId;
}

use csl::{Affixes, DisplayMode, Formatting, Name, NameEtAl, NameLabelInput, Names};
use csl::{AnyVariable, DateVariable, NameAsSortOrder, NameVariable, NumberVariable, Variable};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum DidSupplyName {
    NameEl,
    SortKey,
    None,
}

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct NamesInheritance {
    pub name: Name,
    // Name gets merged from context, starting from scratch
    // So if you supply <name/> at all, you start from context.
    did_supply_name: DidSupplyName,
    pub label: Option<NameLabelInput>,
    pub delimiter: Option<SmartString>,
    pub et_al: Option<NameEtAl>,
    pub formatting: Option<Formatting>,
    pub display: Option<DisplayMode>,
    pub affixes: Option<Affixes>,
    // CSL-M: institutions
    // pub with: Option<NameWith>,
    // CSL-M: institutions
    // pub institution: Option<Institution>,
}

use csl::SortKey;

impl NamesInheritance {
    fn override_with(&self, ctx_name: &Name, ctx_delim: &Option<SmartString>, other: Self) -> Self {
        NamesInheritance {
            // Name gets merged from context, starting from scratch
            // So if you supply <name/> at all, you start from context.
            name: match other.did_supply_name {
                DidSupplyName::NameEl => ctx_name.merge(&other.name),
                DidSupplyName::SortKey => self.name.merge(&other.name),
                DidSupplyName::None => self.name.clone(),
            },
            did_supply_name: DidSupplyName::NameEl,
            // The rest will just replace whatever's in the inheritance
            et_al: other.et_al.or_else(|| self.et_al.clone()),
            label: other.label.or_else(|| self.label.clone()),
            delimiter: other
                .delimiter
                .or_else(|| self.delimiter.clone())
                .or_else(|| ctx_delim.as_ref().map(|x| x.clone())),
            formatting: other.formatting.or(self.formatting),
            display: other.display.or(self.display),
            affixes: other.affixes.or_else(|| self.affixes.clone()),
        }
    }
    fn from_names(ctx_name: &Name, ctx_delim: &Option<SmartString>, names: &Names) -> Self {
        NamesInheritance {
            name: ctx_name.merge(names.name.as_ref().unwrap_or(&Name::empty())),
            did_supply_name: if names.name.is_some() {
                DidSupplyName::NameEl
            } else {
                DidSupplyName::None
            },
            label: names.label.clone(),
            delimiter: names
                .delimiter
                .as_opt_str()
                .or(ctx_delim.as_opt_str())
                .map(Into::into),
            et_al: names.et_al.clone(),
            formatting: names.formatting,
            display: names.display,
            affixes: names.affixes.clone(),
        }
    }
    fn from_sort_key(sort_key: &SortKey) -> Self {
        let name_el = Name {
            et_al_min: sort_key.names_min,
            et_al_subsequent_min: sort_key.names_min,
            et_al_use_first: sort_key.names_use_first,
            et_al_subsequent_use_first: sort_key.names_use_first,
            et_al_use_last: sort_key.names_use_last,
            name_as_sort_order: Some(NameAsSortOrder::All),
            ..Default::default()
        };
        NamesInheritance {
            name: name_el,
            did_supply_name: DidSupplyName::SortKey, // makes no difference
            delimiter: None,
            label: None,
            et_al: None,
            formatting: None,
            display: None,
            affixes: None,
        }
    }
}

use fnv::FnvHashSet;

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct IrState {
    /// This can be a set because macros are strictly non-recursive.
    /// So the same macro name anywhere above indicates attempted recursion.
    /// When you exit a frame, delete from the set.
    macro_stack: HashSet<SmartString>,
    pub name_override: NameOverrider,
    suppressed: FnvHashSet<AnyVariable>,
    pub disamb_count: u32,
}

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct NameOverrider {
    name_override: Option<NamesInheritance>,
    pub in_substitute: bool,
}

impl NameOverrider {
    pub fn inherited_names_options(
        &self,
        ctx_name: &Name,
        ctx_delim: &Option<SmartString>,
        own_names: &Names,
    ) -> NamesInheritance {
        let over = NamesInheritance::from_names(ctx_name, ctx_delim, own_names);
        match &self.name_override {
            None => over,
            Some(stacked) => stacked.override_with(ctx_name, ctx_delim, over),
        }
    }

    pub fn inherited_names_options_sort_key(
        &self,
        ctx_name: &Name,
        ctx_delim: &Option<SmartString>,
        sort_key: &SortKey,
    ) -> NamesInheritance {
        let over = NamesInheritance::from_sort_key(sort_key);
        match &self.name_override {
            None => over,
            Some(stacked) => stacked.override_with(ctx_name, ctx_delim, over),
        }
    }

    pub fn replace_name_overrides(
        &mut self,
        inheritance: NamesInheritance,
    ) -> Option<NamesInheritance> {
        std::mem::replace(&mut self.name_override, Some(inheritance))
    }

    pub fn replace_name_overrides_for_substitute(
        &mut self,
        inheritance: NamesInheritance,
    ) -> Option<NamesInheritance> {
        self.in_substitute = true;
        std::mem::replace(&mut self.name_override, Some(inheritance))
    }

    pub fn restore_name_overrides(&mut self, old: Option<NamesInheritance>) {
        if old.is_none() {
            self.in_substitute = false;
        }
        self.name_override = old;
    }
}

impl IrState {
    pub fn maybe_suppress_name_vars(&mut self, vars: &[NameVariable]) {
        if self.name_override.in_substitute {
            for &var in vars {
                self.suppressed.insert(AnyVariable::Name(var));
            }
        }
    }

    pub fn maybe_suppress_date<T: Default>(
        &mut self,
        var: DateVariable,
        mut f: impl FnMut(&mut Self) -> T,
    ) -> T {
        if self.is_suppressed_date(var) {
            Default::default()
        } else {
            if self.name_override.in_substitute {
                self.suppressed.insert(AnyVariable::Date(var));
            }
            f(self)
        }
    }

    pub fn maybe_suppress<T>(
        &mut self,
        var: Variable,
        mut f: impl FnMut(&mut Self) -> Option<T>,
    ) -> Option<T> {
        if self.is_suppressed_ordinary(var) {
            None
        } else {
            if self.name_override.in_substitute {
                self.suppressed.insert(AnyVariable::Ordinary(var));
                if var == Variable::Title {
                    self.suppressed
                        .insert(AnyVariable::Ordinary(Variable::TitleShort));
                } else if var == Variable::TitleShort {
                    self.suppressed
                        .insert(AnyVariable::Ordinary(Variable::Title));
                }
            }
            f(self)
        }
    }

    pub fn maybe_suppress_num<T>(
        &mut self,
        var: NumberVariable,
        mut f: impl FnMut(&mut Self) -> Option<T>,
    ) -> Option<T> {
        if self.is_suppressed_num(var) {
            None
        } else {
            if self.name_override.in_substitute {
                self.suppressed.insert(AnyVariable::Number(var));
            }
            f(self)
        }
    }

    pub fn is_suppressed_name(&self, var: NameVariable) -> bool {
        self.suppressed.contains(&AnyVariable::Name(var))
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

    pub fn push_macro(&mut self, macro_name: &SmartString) {
        if self.macro_stack.contains(macro_name) {
            panic!(
                "foiled macro recursion: {} called from within itself; exiting",
                macro_name
            );
        }
        self.macro_stack.insert(macro_name.clone());
    }

    pub fn pop_macro(&mut self, macro_name: &SmartString) {
        self.macro_stack.remove(macro_name);
    }
}
