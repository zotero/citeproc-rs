// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

//! The aim of this module is to disambiguate names and cites.
//!
//! This is done by constructing a finite automaton to represent all the different possible outputs
//! a Reference could produce when formatting any particular citation. If a cite's own output
//! matches more than one of these, it is ambiguous.
//!
//! If the Conditions are satisfied, then the contents are rendered, i.e. an edge is added.
//! If the conditions are unsatisfied, then no edge is added.
//! No epsilon edges are added, because we use multiple passes over the IR to do that.
//! If you used an epsilon to represent 'branch not taken', then
//!
//! ```txt
//! if cond
//!   A
//! if !cond
//!   B
//! ```
//!
//! would result in
//!
//!
//! ```txt
//! * ---- A ---> . ---- B ---> $
//!   \___ e __/^   \___ e __/^
//! ```
//!
//! equivalent to `A or AB or B or nothing`, when in fact it should be `A or B`.
//!
//! So we do two passes, one where cond is true for the whole style, and one where cond is
//! false for the whole style.
//!
//! ```txt
//! * ---- A ---> $
//! * ---- B ---> $
//! ```
//!
//! This is a valid NFA, as NFAs can have multiple start states in addition to multiple accepting
//! states. Now we minimise this NFA into a DFA that can pretty quickly calculate if a cite matches
//! or not.
//!
//! ```txt
//! * ---- A ---> $
//!   \___ B __/^
//! ```
//!
//! So a cite where `cond` is true matches the `A` path, and a cite where it is false matches `B`.
//!
//! One important optimisation is that not every `cond` can change between cites. Most variables
//! are set in stone for a particular reference. So we pick out the ones that *can* change, and
//! produce one possibility for each combination contemplated by the style.
//!

use crate::prelude::*;
use citeproc_io::output::markup::Markup;
use citeproc_io::Reference;
use fnv::FnvHashMap;
use petgraph::visit::EdgeRef;

// first so the macros are defined before the other modules
#[cfg(test)]
#[macro_use]
pub(crate) mod test;

mod finite_automata;
mod free;
pub(crate) mod implementation;
// pub(crate) mod knowledge;
pub(crate) mod names;
mod ref_context;

pub use free::{FreeCond, FreeCondSets};
pub use names::{DisambName, DisambNameData};
pub use ref_context::RefContext;

pub use finite_automata::{Dfa, EdgeData, Nfa, NfaEdge};

use csl::{
    variables::*, BodyDate, Choose, Cond, Conditions, IfThen, IsIndependent, LabelElement, Match,
    Names, NumberElement, Position, TextElement, VariableForm,
};

pub fn get_free_conds(db: &dyn IrDatabase) -> FreeCondSets {
    let mut walker = FreeCondWalker::new(db);
    walker.walk_citation(&db.style())
}

struct FreeCondWalker<'a> {
    db: &'a dyn IrDatabase,
    state: IrState,
}

impl<'a> FreeCondWalker<'a> {
    fn new(db: &'a dyn IrDatabase) -> Self {
        FreeCondWalker {
            db,
            state: IrState::new(),
        }
    }
}

impl<'a> StyleWalker for FreeCondWalker<'a> {
    type Output = FreeCondSets;
    type Checker = crate::choose::UselessCondChecker;

    fn default(&mut self) -> Self::Output {
        FreeCondSets::default()
    }
    /// For joining 2+ side-by-side FreeCondSets. This is the `sequence` for get_free_conds.
    fn fold(&mut self, elements: &[Element], _fold_type: WalkerFoldType) -> Self::Output {
        // TODO: keep track of which empty variables caused GroupVars to not render, if
        // they are indeed free variables.
        // XXX: include layout parts?
        let mut all = fnv_set_with_cap(elements.len());
        all.insert(FreeCond::empty());
        let mut f = FreeCondSets(all);
        for el in elements {
            f.cross_product(self.element(el));
        }
        f
    }

    fn text_macro(&mut self, text: &TextElement, name: &SmartString) -> Self::Output {
        // TODO: same todos as in Proc
        let style = self.db.style();
        let macro_elements = style
            .macros
            .get(name)
            .expect("undefined macro should not be valid CSL");

        self.state.push_macro(name);
        let ret = self.fold(macro_elements, WalkerFoldType::Macro(text));
        self.state.pop_macro(name);
        ret
    }

    fn text_variable(
        &mut self,
        _text: &TextElement,
        sv: StandardVariable,
        _form: VariableForm,
    ) -> Self::Output {
        let mut implicit_var_test = FreeCondSets::mult_identity();
        if sv.is_independent() {
            let cond = Cond::Variable((&sv).into());
            implicit_var_test.scalar_multiply_cond(cond, true);
        } else if sv == StandardVariable::Ordinary(Variable::CitationLabel) {
            // citation labels typically have a year on the end, so we add year suffixes
            // to them
            let cond = Cond::Variable(AnyVariable::Ordinary(Variable::YearSuffix));
            implicit_var_test.scalar_multiply_cond(cond, true);
        }
        implicit_var_test
    }

    fn date(&mut self, _date: &BodyDate) -> Self::Output {
        let mut base = FreeCondSets::mult_identity();
        let cond = Cond::Variable(AnyVariable::Ordinary(Variable::YearSuffix));
        base.scalar_multiply_cond(cond, true);
        base
    }

    fn names(&mut self, names: &Names) -> Self::Output {
        let mut base = if let Some(subst) = &names.substitute {
            // TODO: drill down into the substitute logic here
            self.fold(&subst.0, WalkerFoldType::Substitute)
        } else {
            FreeCondSets::mult_identity()
        };
        // Position may be involved for NASO and primary disambiguation
        let cond = Cond::Position(Position::First);
        base.scalar_multiply_cond(cond, true);
        base
    }

    fn number(&mut self, number: &NumberElement) -> Self::Output {
        let num_var = number.variable;
        if num_var.is_independent() {
            let mut implicit_var_test = FreeCondSets::mult_identity();
            let cond = Cond::Variable(AnyVariable::Number(num_var));
            implicit_var_test.scalar_multiply_cond(cond, true);
            implicit_var_test
        } else {
            FreeCondSets::mult_identity()
        }
    }
    fn label(&mut self, label: &LabelElement) -> Self::Output {
        let num_var = label.variable;
        if num_var.is_independent() {
            let mut implicit_var_test = FreeCondSets::mult_identity();
            let cond = Cond::Variable(AnyVariable::Number(num_var));
            implicit_var_test.scalar_multiply_cond(cond, true);
            implicit_var_test
        } else {
            FreeCondSets::mult_identity()
        }
    }

    fn choose(&mut self, choose: &Choose) -> Self::Output {
        use std::iter;
        let Choose(ifthen, elseifs, else_) = choose;
        let IfThen(if_conditions, if_els) = ifthen;
        let Conditions(ifc_match, ifc_cond_set) = if_conditions;
        // Other kinds (CSL-M) not yet supported
        assert!(*ifc_match == Match::All);
        assert!(ifc_cond_set.len() == 1);
        let ifthen = (&ifc_cond_set[0], self.fold(if_els, WalkerFoldType::IfThen));
        let first: Vec<_> = iter::once(ifthen)
            .chain(elseifs.iter().map(|fi: &IfThen| {
                let IfThen(if_conditions, if_els) = fi;
                let Conditions(ifc_match, ifc_cond_set) = if_conditions;
                // Other kinds (CSL-M) not yet supported
                assert!(*ifc_match == Match::All);
                assert!(ifc_cond_set.len() == 1);
                (&ifc_cond_set[0], self.fold(if_els, WalkerFoldType::IfThen))
            }))
            .collect();
        FreeCondSets::all_branches(
            first.into_iter(),
            if !else_.0.is_empty() {
                Some(self.fold(&else_.0, WalkerFoldType::Else))
            } else {
                None
            },
        )
    }
}

pub trait Disambiguation<O: OutputFormat = Markup> {
    fn ref_ir(
        &self,
        _db: &dyn IrDatabase,
        _ctx: &RefContext<O>,
        _state: &mut IrState,
        _stack: Formatting,
    ) -> (RefIR, GroupVars);
}

/// Creates a Dfa that will match any cite that could have been made by a particular reference.
/// A cite's output matching more than one reference's Dfa is our definition of "ambiguous".
pub fn create_dfa<O: OutputFormat>(db: &dyn IrDatabase, refr: &Reference) -> Dfa {
    let runs = create_ref_ir::<Markup>(db, refr);
    let mut nfa = Nfa::new();
    let fmt = db.get_formatter();
    for (_fc, ir) in runs {
        let first = nfa.graph.add_node(());
        nfa.start.insert(first);
        let last = add_to_graph(&fmt, &mut nfa, &ir, first, None);
        nfa.accepting.insert(last);
    }
    nfa.brzozowski_minimise()
}

pub fn create_single_ref_ir<O: OutputFormat>(db: &dyn IrDatabase, ctx: &RefContext) -> RefIR {
    let style = ctx.style;
    let mut state = IrState::new();
    let (ir, _gv) =
        Disambiguation::<Markup>::ref_ir(style, db, ctx, &mut state, Formatting::default());
    ir
}

/// Sorts the list so that it can be determined not to have changed by Salsa. Also emits a FreeCond
/// so we don't have to re-allocate/collect the list after sorting to exclude it.
pub fn create_ref_ir<O: OutputFormat>(
    db: &dyn IrDatabase,
    refr: &Reference,
) -> Vec<(FreeCond, RefIR)> {
    let style = db.style();
    let locale = db.default_locale();
    let ysh_explicit_edge = EdgeData::YearSuffixExplicit;
    let ysh_plain_edge = EdgeData::YearSuffixPlain;
    let ysh_edge = EdgeData::YearSuffix;
    let fcs = db.branch_runs();
    let fmt = db.get_formatter();
    let mut vec: Vec<(FreeCond, RefIR)> = fcs
        .0
        .iter()
        .cloned()
        .flat_map(|fc| {
            // Now we construct one ctx for every different count of disambiguate="X" checks
            let ctx =
                RefContext::from_free_cond(fc, &fmt, &style, &locale, refr, CiteOrBib::Citation);
            let count = ctx.disamb_count;
            // 0 = none of them enabled
            // 1 = first disambiguate="X" tests as true
            // count = all of the (reachable) checks test as true
            (0..=count).into_iter().map(move |c| {
                let mut cloned = ctx.clone();
                cloned.disamb_count = c;
                (fc, cloned)
            })
        })
        .map(|(fc, cloned)| {
            let mut state = IrState::new();
            let (mut ir, _gv) = Disambiguation::<Markup>::ref_ir(
                &*style,
                db,
                &cloned,
                &mut state,
                Formatting::default(),
            );
            ir.keep_first_ysh(
                ysh_explicit_edge.clone(),
                ysh_plain_edge.clone(),
                ysh_edge.clone(),
            );
            (fc, ir)
        })
        .collect();
    vec.sort_by_key(|(fc, _)| fc.bits());
    vec
}

use petgraph::graph::NodeIndex;

pub fn graph_with_stack(
    fmt: &Markup,
    nfa: &mut Nfa,
    formatting: Option<Formatting>,
    affixes: Option<&Affixes>,
    mut spot: NodeIndex,
    f: impl FnOnce(&mut Nfa, NodeIndex) -> NodeIndex,
) -> NodeIndex {
    let stack = fmt.tag_stack(formatting.unwrap_or_else(Default::default), None);
    let mut open_tags = SmartString::new();
    let mut close_tags = SmartString::new();
    fmt.stack_preorder(&mut open_tags, &stack);
    fmt.stack_postorder(&mut close_tags, &stack);
    let mkedge = |s: SmartString| {
        RefIR::Edge(if !s.is_empty() {
            Some(EdgeData::Output(s))
        } else {
            None
        })
    };
    let mkedge_esc = |s: &str| {
        RefIR::Edge(if !s.is_empty() {
            Some(EdgeData::Output(
                // TODO: fmt.ingest
                fmt.output_in_context(fmt.plain(s), Default::default(), None),
            ))
        } else {
            None
        })
    };
    let open_tags = &mkedge(open_tags);
    let close_tags = &mkedge(close_tags);
    if let Some(pre) = affixes.as_ref().map(|a| mkedge_esc(&*a.prefix)) {
        spot = add_to_graph(fmt, nfa, &pre, spot, None);
    }
    spot = add_to_graph(fmt, nfa, open_tags, spot, None);
    spot = f(nfa, spot);
    spot = add_to_graph(fmt, nfa, close_tags, spot, None);
    if let Some(suf) = affixes.as_ref().map(|a| mkedge_esc(&*a.suffix)) {
        spot = add_to_graph(fmt, nfa, &suf, spot, None);
    }
    spot
}

pub fn add_to_graph(
    fmt: &Markup,
    nfa: &mut Nfa,
    ir: &RefIR,
    spot: NodeIndex,
    override_delim: Option<&str>,
) -> NodeIndex {
    match ir {
        RefIR::Edge(None) => spot,
        RefIR::Edge(Some(e)) => {
            let to = nfa.graph.add_node(());
            nfa.graph.add_edge(spot, to, NfaEdge::Token(e.clone()));
            to
        }
        RefIR::Seq(ref seq) => {
            let RefIrSeq {
                formatting,
                ref contents,
                ref affixes,
                ref delimiter,
                should_inherit_delim,
                // TODO: use these
                quotes: _,
                text_case: _,
            } = *seq;
            let affixes = affixes.as_ref();
            let mkedge = |s: &str| {
                RefIR::Edge(if !s.is_empty() {
                    Some(EdgeData::Output(fmt.output_in_context(
                        fmt.plain(s),
                        Default::default(),
                        None,
                    )))
                } else {
                    None
                })
            };
            let delim = override_delim
                .filter(|_| should_inherit_delim)
                .or(delimiter.as_opt_str())
                .map(|d| mkedge(d));
            graph_with_stack(fmt, nfa, formatting, affixes, spot, |nfa, mut spot| {
                let mut seen = false;
                for x in contents {
                    if !matches!(x, RefIR::Edge(None)) {
                        if seen {
                            if let Some(d) = &delim {
                                spot = add_to_graph(fmt, nfa, d, spot, None);
                            }
                        }
                        seen = true;
                    }
                    spot = add_to_graph(fmt, nfa, x, spot, delimiter.as_opt_str());
                }
                spot
            })
        }
        RefIR::Name(_nvar, name_nfa) => {
            // We're going to graft the names_nfa onto our own by translating all the node_ids, and
            // adding the same edges between them.
            let mut node_mapping = FnvHashMap::default();
            let mut get_node = |nfa: &mut Nfa, incoming: NodeIndex| {
                *node_mapping
                    .entry(incoming)
                    .or_insert_with(|| nfa.graph.add_node(()))
            };
            // collected because iterator uses a mutable reference to nfa
            let incoming_edges: Vec<_> = name_nfa
                .graph
                .edge_references()
                .map(|e| {
                    (
                        get_node(nfa, e.source()),
                        get_node(nfa, e.target()),
                        e.weight(),
                    )
                })
                .collect();
            nfa.graph.extend_with_edges(incoming_edges.into_iter());
            for &start_node in &name_nfa.start {
                let start_node = get_node(nfa, start_node);
                nfa.graph.add_edge(spot, start_node, NfaEdge::Epsilon);
            }
            let finish = nfa.graph.add_node(());
            for &acc_node in &name_nfa.accepting {
                let acc_node = get_node(nfa, acc_node);
                nfa.graph.add_edge(acc_node, finish, NfaEdge::Epsilon);
            }
            finish
        }
    }
}

#[test]
fn test_determinism() {
    env_logger::init();
    use crate::test::MockProcessor;
    let db = MockProcessor::new();
    let fmt = db.get_formatter();
    let aa = || EdgeData::Output("aa".into());
    let bb = || EdgeData::Output("bb".into());

    let make_dfa = || {
        let mut nfa = Nfa::new();
        for ir in &[RefIR::Edge(Some(aa())), RefIR::Edge(Some(bb()))] {
            let first = nfa.graph.add_node(());
            nfa.start.insert(first);
            let last = add_to_graph(&fmt, &mut nfa, ir, first, None);
            nfa.accepting.insert(last);
        }
        nfa.brzozowski_minimise()
    };

    let mut count = 0;
    for _ in 0..100 {
        let dfa = make_dfa();
        debug!("{}", dfa.debug_graph(&db));
        if dfa.accepts_data(&[aa()]) {
            count += 1;
        }
    }
    assert_eq!(count, 100);
}
