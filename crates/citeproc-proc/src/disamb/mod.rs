// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

#![allow(dead_code)]
use crate::prelude::*;
use citeproc_io::{Cite, Reference};
use csl::style::{
    Choose, Cond, CondSet, Conditions, CslType, Element, Formatting, Match, Position, Style,
    TextSource,
};
use csl::variables::{AnyVariable, NumberVariable, StandardVariable, Variable};
use csl::IsIndependent;
use fnv::{FnvHashMap, FnvHashSet};

mod finite_automata;
mod free;
mod knowledge;
pub mod old;

use knowledge::Knowledge;

pub use finite_automata::{Dfa, Edge, EdgeData, Nfa};

pub trait Disambiguation {
    // fn disambiguation_ir(&self, db: &impl IrDatabase, state: &mut DisambiguationState) -> IrSum<O>;
    // You're gonna need an IrDatabase there
    fn construct_nfa(&self, db: &impl IrDatabase, state: &mut DisambiguationState);
    fn independent_conds(&self, db: &impl IrDatabase, conds: &mut ConditionStack) {
        // most elements don't contain conditionals
    }
}

impl Disambiguation for Style {
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

impl Disambiguation for CondSet {
    fn construct_nfa(&self, db: &impl IrDatabase, state: &mut DisambiguationState) {
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
    fn independent_conds(&self, db: &impl IrDatabase, stack: &mut ConditionStack) {
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

impl Disambiguation for Conditions {
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

impl Disambiguation for Choose {
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

impl Disambiguation for Element {
    fn independent_conds(&self, db: &impl IrDatabase, stack: &mut ConditionStack) {
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
                        .push(Cond::Variable(AnyVariable::Number(*num_var)))
                }
            }
            Element::Text(src, ..) => match src {
                TextSource::Macro(m) => unimplemented!(),
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
    fn construct_nfa(&self, db: &impl IrDatabase, state: &mut DisambiguationState) {
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

use citeproc_io::output::html::Html;

pub struct DisambiguationState<'c> {
    knowledge: Knowledge,
    format_stack: Vec<Formatting>,
    format_current: Formatting,
    nfa: Nfa,
    cite_context: CiteContext<'c, Html>,
}

impl DisambiguationState<'_> {
    pub fn new<'a>(
        reference: &'a Reference,
        cite_id: CiteId,
        cite: &'a Cite<Html>,
        position: Position,
        number: u32,
    ) -> DisambiguationState<'a> {
        let format = Html::default();
        DisambiguationState {
            knowledge: Knowledge::new(),
            format_stack: vec![],
            format_current: Default::default(),
            nfa: Nfa::new(),
            cite_context: CiteContext {
                cite_id,
                reference,
                format,
                cite,
                position,
                citation_number: number,
                disamb_pass: None,
            },
        }
    }
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

#[cfg(test)]
macro_rules! style_layout {
    ($ex:expr) => {{
        use std::str::FromStr;
        ::csl::style::Style::from_str(&format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
    <style class="in-text" version="1.0.1">
        <citation>
            <layout>
                {}
            </layout>
        </citation>
    </style>"#,
            $ex
        ))
        .unwrap()
    }};
}

#[test]
fn independent_conds() {
    use crate::test::MockProcessor;
    let db = MockProcessor::new();
    let style = style_layout!(
        r#"
    <group delimiter=" ">
        <text variable="locator" />
        <text variable="title" font-style="italic"/>
        <choose>
            <if disambiguate="true">
                <text value="..." />
            </if>
            <else-if position="first" type="book">
                <text value="..." />
            </else-if>
            <else-if position="subsequent" variable="title">
                <text value="..." />
            </else-if>
            <else>
                <text variable="first-reference-note-number" />
                <text variable="ISBN" />
            </else>
        </choose>
    </group>
    "#
    );
    let mut refr = Reference::empty("id".into(), CslType::Book);
    refr.ordinary.insert(Variable::Title, "Title".into());
    let mut stack = ConditionStack::from(&refr);
    style.independent_conds(&db, &mut stack);
    let mut result_set = FnvHashSet::default();
    result_set.insert(Cond::Variable(AnyVariable::Number(NumberVariable::Locator)));
    result_set.insert(Cond::Position(Position::First));
    result_set.insert(Cond::Disambiguate(true));
    result_set.insert(Cond::Variable(AnyVariable::Number(
        NumberVariable::FirstReferenceNoteNumber,
    )));
    // does not contain Position::Subsequent as we knew variable="title" was true
    assert_eq!(
        stack.output.into_iter().collect::<FnvHashSet<_>>(),
        result_set,
    )
}

// #[test(ignore)]
// fn element_disamb() {
//     use crate::test::MockProcessor;
//     let db = MockProcessor::new();
//     let style = style_layout!(
//         r#"
//     <group delimiter=" ">
//         <text value="value" />
//         <text value="italic" font-style="italic"/>
//     </group>
//     "#
//     );
//     let group = &style.citation.layout.elements[0];
//     let mut state = DisambiguationState::new();
//     group.construct_nfa(&db, &mut state);
//     let dfa = state.nfa.brzozowski_minimise();
//     let value = db.edge(EdgeData("value".to_string()));
//     let italic = db.edge(EdgeData("<i>italic</i>".to_string()));
//     assert!(dfa.accepts(&[value, italic]));
// }
