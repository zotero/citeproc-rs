// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use crate::prelude::*;
use citeproc_io::output::html::Html;
use citeproc_io::Reference;

use csl::style::{
    Choose, Cond, CondSet, Conditions, Element, Formatting, Match, Style, TextSource,
};

use super::Nfa;
use citeproc_io::{Date, DateOrRange, Name, NumericValue};
use csl::variables::AnyVariable;
use csl::Atom;
use csl::IsIndependent;
use std::collections::HashSet;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum DisambToken {
    // Should this be an Atom, really? There will not typically be that much reuse going on. It
    // might inflate the cache too much. The size of the disambiguation index is reduced, though.
    Str(Atom),

    /// Significantly simplifies things compared to ultra-localized date output strings.
    /// Reference cannot predict what they'll look like.
    /// `Date` itself can encode the lack of day/month with those fields set to zero.
    Date(Date),

    Num(NumericValue),

    YearSuffix(Atom),
}

pub trait AddDisambTokens {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, indexing: bool);
    #[inline]
    fn add_tokens_index(&self, set: &mut HashSet<DisambToken>) {
        self.add_tokens_ctx(set, true);
    }
    #[inline]
    fn add_tokens(&self, set: &mut HashSet<DisambToken>) {
        self.add_tokens_ctx(set, false);
    }
}

impl AddDisambTokens for Reference {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, indexing: bool) {
        for val in self.ordinary.values() {
            set.insert(DisambToken::Str(val.as_str().into()));
        }
        for val in self.number.values() {
            set.insert(DisambToken::Num(val.clone()));
        }
        for val in self.name.values() {
            for name in val.iter() {
                name.add_tokens_ctx(set, indexing);
            }
        }
        for val in self.date.values() {
            val.add_tokens_ctx(set, indexing);
        }
    }
}

impl AddDisambTokens for Option<String> {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, _indexing: bool) {
        if let Some(ref x) = self {
            set.insert(DisambToken::Str(x.as_str().into()));
        }
    }
}

impl AddDisambTokens for Name {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, indexing: bool) {
        match self {
            Name::Person(ref pn) => {
                pn.family.add_tokens_ctx(set, indexing);
                pn.given.add_tokens_ctx(set, indexing);
                pn.non_dropping_particle.add_tokens_ctx(set, indexing);
                pn.dropping_particle.add_tokens_ctx(set, indexing);
                pn.suffix.add_tokens_ctx(set, indexing);
            }
            Name::Literal { ref literal } => {
                set.insert(DisambToken::Str(literal.as_str().into()));
            }
        }
    }
}

impl AddDisambTokens for DateOrRange {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, indexing: bool) {
        match self {
            DateOrRange::Single(ref single) => {
                single.add_tokens_ctx(set, indexing);
            }
            DateOrRange::Range(d1, d2) => {
                d1.add_tokens_ctx(set, indexing);
                d2.add_tokens_ctx(set, indexing);
            }
            DateOrRange::Literal(ref lit) => {
                set.insert(DisambToken::Str(lit.as_str().into()));
            }
        }
    }
}

impl AddDisambTokens for Date {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, indexing: bool) {
        // when processing a cite, only insert the segments you actually used
        set.insert(DisambToken::Date(*self));
        // for the index, add all possible variations
        if indexing {
            let just_ym = Date {
                year: self.year,
                month: self.month,
                day: 0,
            };
            let just_year = Date {
                year: self.year,
                month: 0,
                day: 0,
            };
            set.insert(DisambToken::Date(just_ym));
            set.insert(DisambToken::Date(just_year));
        }
    }
}

use super::knowledge::Knowledge;

pub trait DisambiguationOld<O: OutputFormat = Html> {
    // fn disambiguation_ir(&self, db: &impl IrDatabase, state: &mut DisambiguationState) -> IrSum<O>;
    // You're gonna need an IrDatabase there
    fn construct_nfa(&self, _db: &impl IrDatabase, _state: &mut DisambiguationState) {}
    fn independent_conds(&self, _db: &impl IrDatabase, _conds: &mut ConditionStack) {
        // most elements don't contain conditionals
    }
}

impl DisambiguationOld for Style {
    fn construct_nfa(&self, db: &impl IrDatabase, state: &mut DisambiguationState) {
        // XXX: include layout parts?
        for el in &self.citation.layout.elements {
            el.construct_nfa(db, state);
        }
    }
    fn independent_conds(&self, db: &impl IrDatabase, conds: &mut ConditionStack) {
        for el in &self.citation.layout.elements {
            el.independent_conds(db, conds);
        }
    }
}

pub struct ConditionStack {
    knowledge: Knowledge,
    output: Vec<Cond>,
}

impl From<&Reference> for ConditionStack {
    fn from(refr: &Reference) -> Self {
        let mut knowledge = Knowledge::new();
        knowledge.know_all_of(
            refr.ordinary
                .keys()
                .map(|&var| (Cond::Variable(AnyVariable::Ordinary(var)), true)),
        );
        knowledge.know_all_of(
            refr.number
                .keys()
                .map(|&var| (Cond::Variable(AnyVariable::Number(var)), true)),
        );
        // TODO: insert Cond::HasYearOnly & friends as well
        knowledge.know_all_of(
            refr.date
                .keys()
                .map(|&var| (Cond::Variable(AnyVariable::Date(var)), true)),
        );
        knowledge.know_all_of(
            refr.name
                .keys()
                .map(|&var| (Cond::Variable(AnyVariable::Name(var)), true)),
        );
        knowledge.push();
        ConditionStack {
            knowledge,
            output: Vec::new(),
        }
    }
}

impl DisambiguationOld for CondSet {
    fn construct_nfa(&self, _db: &impl IrDatabase, state: &mut DisambiguationState) {
        match self.match_type {
            Match::Any => {
                state
                    .knowledge
                    .know_some_of(self.conds.iter().cloned().map(|c| (c, true)));
            }
            Match::All => {
                state
                    .knowledge
                    .know_all_of(self.conds.iter().cloned().map(|c| (c, true)));
            }
            Match::None => {
                state
                    .knowledge
                    .know_all_of(self.conds.iter().cloned().map(|c| (c, false)));
            }
            _ => unimplemented!(),
        }
    }
    fn independent_conds(&self, _db: &impl IrDatabase, stack: &mut ConditionStack) {
        let (indep, ref_based): (Vec<&Cond>, Vec<&Cond>) =
            self.conds.iter().partition(|c| c.is_independent());
        if self.match_type == Match::Any
            && ref_based
                .iter()
                .any(|&c| stack.knowledge.demonstrates(&c, true))
        {
            // this condition block is always going to return TRUE, so it doesn't depend on the
            // independent ones
            return;
        } else if self.match_type == Match::All
            && ref_based
                .iter()
                .any(|&c| stack.knowledge.demonstrates(&c, false))
        {
            // this condition block is always going to return FALSE, so it doesn't depend on the
            // independent ones
            return;
        } else {
            for i in indep {
                stack.output.push(i.clone())
            }
        }
    }
}

impl DisambiguationOld for Conditions {
    fn construct_nfa(&self, db: &impl IrDatabase, state: &mut DisambiguationState) {
        // TODO(CSL-M): handle other match modes than ALL
        for cond_set in &self.1 {
            // push all the relevant knowledge gleaned from the cond_set
            cond_set.construct_nfa(db, state);
        }
    }
    fn independent_conds(&self, db: &impl IrDatabase, stack: &mut ConditionStack) {
        // TODO(CSL-M): handle other match modes than ALL
        for cond_set in &self.1 {
            cond_set.independent_conds(db, stack);
        }
    }
}

impl DisambiguationOld for Choose {
    fn construct_nfa(&self, db: &impl IrDatabase, state: &mut DisambiguationState) {
        let &Choose(ref ift, ref elifs, ref elset) = self;
        state.knowledge.push();
        ift.0.construct_nfa(db, state);
        for el in &ift.1 {
            el.construct_nfa(db, state);
        }
        state.knowledge.pop();
        for elif in elifs {
            state.knowledge.push();
            elif.0.construct_nfa(db, state);
            for el in &elif.1 {
                el.construct_nfa(db, state);
            }
            state.knowledge.pop();
        }
        for el in &elset.0 {
            el.construct_nfa(db, state);
        }
    }
    fn independent_conds(&self, db: &impl IrDatabase, stack: &mut ConditionStack) {
        let &Choose(ref ift, ref elifs, ref elset) = self;
        stack.knowledge.push();
        ift.0.independent_conds(db, stack);
        for el in &ift.1 {
            el.independent_conds(db, stack);
        }
        // TODO: you can assert a bunch of knowledge about what was not true because the previous
        // branches didn't match
        // TODO: make the push() API return a GenerationIndex and let you rollback() to that index
        stack.knowledge.pop();
        for elif in elifs {
            stack.knowledge.push();
            elif.0.independent_conds(db, stack);
            for el in &elif.1 {
                el.independent_conds(db, stack);
            }
            stack.knowledge.pop();
        }
        for el in &elset.0 {
            el.independent_conds(db, stack);
        }
    }
}

impl DisambiguationOld for Element {
    fn independent_conds(&self, db: &impl IrDatabase, stack: &mut ConditionStack) {
        // let mut output = FnvHashSet::default();
        match self {
            Element::Group(g) => {
                // TODO: create a new conds vec, and extend the main conds if GroupVars is a hit
                for el in &g.elements {
                    el.independent_conds(db, stack);
                }
            }
            Element::Names(n) => {
                if let Some(subst) = &n.substitute {
                    for el in &subst.0 {
                        el.independent_conds(db, stack);
                    }
                }
            }
            Element::Choose(c) => c.independent_conds(db, stack),
            Element::Number(num_var, ..) | Element::Label(num_var, ..) => {
                if num_var.is_independent() {
                    stack
                        .output
                        .push(Cond::Variable(AnyVariable::Number(*num_var)));
                }
            }
            Element::Text(src, ..) => match src {
                TextSource::Macro(_m) => unimplemented!(),
                TextSource::Variable(sv, ..) => {
                    if sv.is_independent() {
                        stack.output.push(Cond::Variable(sv.into()));
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    fn construct_nfa(&self, _db: &impl IrDatabase, _state: &mut DisambiguationState) {
        match self {
            Element::Text(..) => {
                // let ir = self
                //     .intermediate(db, ir_state, state.cite_context)
                //     .flatten();
                // state.cite_context.format()
            }
            _ => {}
        }
    }
}

pub struct DisambiguationState<'c> {
    knowledge: Knowledge,
    format_stack: Vec<Formatting>,
    format_current: Formatting,
    nfa: Nfa,
    cite_context: CiteContext<'c, Html>,
}

impl DisambiguationState<'_> {
    /// if None, you should exit out of DFADisambiguate
    fn digest(&mut self, _token: EdgeData) -> Option<()> {
        // ... match against internal DFA, using formatting from the stack.
        None
    }
    pub fn push_fmt(&mut self, fmt: Formatting) {
        self.format_stack.push(fmt);
        self.coalesce_fmt();
    }
    pub fn pop_fmt(&mut self) {
        self.format_stack.pop();
        self.coalesce_fmt();
    }
    fn coalesce_fmt(&mut self) {
        self.format_current = Formatting::default();
        for fmt in self.format_stack.iter() {
            self.format_current.font_style = fmt.font_style;
            self.format_current.font_weight = fmt.font_weight;
            self.format_current.font_variant = fmt.font_variant;
            self.format_current.text_decoration = fmt.text_decoration;
            self.format_current.vertical_alignment = fmt.vertical_alignment;
        }
    }
}
