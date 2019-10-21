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

pub use finite_automata::{Dfa, Edge, EdgeData, Nfa, NfaEdge};

pub trait Disambiguation<O: OutputFormat = Markup> {
    fn get_free_conds(&self, _db: &impl IrDatabase) -> FreeCondSets {
        mult_identity()
    }
    fn ref_ir(
        &self,
        _db: &impl IrDatabase,
        _ctx: &RefContext<O>,
        state: &mut IrState,
        _stack: Formatting,
    ) -> (RefIR, GroupVars) {
        unimplemented!()
    }
}

/// For joining 2+ side-by-side FreeCondSets. This is the `sequence` for get_free_conds.
pub fn cross_product(db: &impl IrDatabase, els: &[Element]) -> FreeCondSets {
    // XXX: include layout parts?
    let mut all = fnv_set_with_cap(els.len());
    all.insert(FreeCond::empty());
    let mut f = FreeCondSets(all);
    for el in els {
        f.cross_product(el.get_free_conds(db));
    }
    f
}

/// Like the number 1, but for multiplying FreeCondSets using cross_products.
///
/// The cross product of any set X and mult_identity() is X.
pub fn mult_identity() -> FreeCondSets {
    FreeCondSets::default()
}

/// Creates a Dfa that will match any cite that could have been made by a particular reference.
/// A cite's output matching more than one reference's Dfa is our definition of "ambiguous".
pub fn create_dfa<O: OutputFormat, DB: IrDatabase>(db: &DB, refr: &Reference) -> Dfa {
    let runs = create_ref_ir::<Markup, DB>(db, refr);
    let mut nfa = Nfa::new();
    let fmt = db.get_formatter();
    for (_fc, ir) in runs {
        let first = nfa.graph.add_node(());
        nfa.start.insert(first);
        let last = add_to_graph(db, &fmt, &mut nfa, &ir, first);
        nfa.accepting.insert(last);
    }
    nfa.brzozowski_minimise()
}

pub fn create_single_ref_ir<O: OutputFormat, DB: IrDatabase>(db: &DB, ctx: &RefContext) -> RefIR {
    let style = ctx.style;
    let mut state = IrState::new();
    let (ir, _gv) =
        Disambiguation::<Markup>::ref_ir(style, db, ctx, &mut state, Formatting::default());
    ir
}

/// Sorts the list so that it can be determined not to have changed by Salsa. Also emits a FreeCond
/// so we don't have to re-allocate/collect the list after sorting to exclude it.
pub fn create_ref_ir<O: OutputFormat, DB: IrDatabase>(
    db: &DB,
    refr: &Reference,
) -> Vec<(FreeCond, RefIR)> {
    let style = db.style();
    let locale = db.locale_by_reference(refr.id.clone());
    let ysh_explicit_edge = db.edge(EdgeData::YearSuffixExplicit);
    let ysh_edge = db.edge(EdgeData::YearSuffix);
    let fcs = db.branch_runs();
    let mut vec: Vec<(FreeCond, RefIR)> = fcs
        .0
        .iter()
        .map(|fc| {
            let fmt = db.get_formatter();
            let name_info = db.name_info_citation();
            let ctx =
                RefContext::from_free_cond(*fc, &fmt, &style, &locale, refr, CiteOrBib::Citation);
            let mut state = IrState::new();
            let (mut ir, _gv) = Disambiguation::<Markup>::ref_ir(
                &*style,
                db,
                &ctx,
                &mut state,
                Formatting::default(),
            );
            ir.keep_first_ysh(ysh_explicit_edge, ysh_edge);
            (*fc, ir)
        })
        .collect();
    vec.sort_by_key(|(fc, _)| fc.bits());
    vec
}

use petgraph::graph::NodeIndex;

pub fn graph_with_stack(
    db: &impl IrDatabase,
    fmt: &Markup,
    nfa: &mut Nfa,
    formatting: &Option<Formatting>,
    affixes: &Affixes,
    mut spot: NodeIndex,
    f: impl FnOnce(&mut Nfa, NodeIndex) -> NodeIndex,
) -> NodeIndex {
    let stack = fmt.tag_stack(formatting.unwrap_or_else(Default::default));
    let mut open_tags = String::new();
    let mut close_tags = String::new();
    fmt.stack_preorder(&mut open_tags, &stack);
    fmt.stack_postorder(&mut close_tags, &stack);
    let mkedge = |s: &str| {
        RefIR::Edge(if s.len() > 0 {
            Some(db.edge(EdgeData::Output(
                fmt.output_in_context(fmt.plain(s), Default::default()),
            )))
        } else {
            None
        })
    };
    let open_tags = &mkedge(&*open_tags);
    let close_tags = &mkedge(&*close_tags);
    let pre = &mkedge(&*affixes.prefix);
    let suf = &mkedge(&*affixes.suffix);
    spot = add_to_graph(db, fmt, nfa, pre, spot);
    spot = add_to_graph(db, fmt, nfa, open_tags, spot);
    spot = f(nfa, spot);
    spot = add_to_graph(db, fmt, nfa, close_tags, spot);
    spot = add_to_graph(db, fmt, nfa, suf, spot);
    spot
}

pub fn add_to_graph(
    db: &impl IrDatabase,
    fmt: &Markup,
    nfa: &mut Nfa,
    ir: &RefIR,
    spot: NodeIndex,
) -> NodeIndex {
    match ir {
        RefIR::Edge(None) => spot,
        RefIR::Edge(Some(e)) => {
            let to = nfa.graph.add_node(());
            nfa.graph.add_edge(spot, to, NfaEdge::Token(*e));
            to
        }
        RefIR::Seq(ref seq) => {
            let RefIrSeq {
                contents,
                formatting,
                affixes,
                delimiter,
            } = seq;
            let mkedge = |s: &str| {
                RefIR::Edge(if s.len() > 0 {
                    Some(db.edge(EdgeData::Output(
                        fmt.output_in_context(fmt.plain(s), Default::default()),
                    )))
                } else {
                    None
                })
            };
            let delim = &mkedge(&*delimiter);
            graph_with_stack(db, fmt, nfa, formatting, affixes, spot, |nfa, mut spot| {
                let mut seen = false;
                for x in contents {
                    if x != &RefIR::Edge(None) {
                        if seen {
                            spot = add_to_graph(db, fmt, nfa, delim, spot);
                        }
                        seen = true;
                    }
                    spot = add_to_graph(db, fmt, nfa, x, spot);
                }
                spot
            })
        }
        RefIR::Name(_nvar, name_nfa) => {
            // We're going to graft the names_nfa onto our own by translating all the node_ids, and
            // adding the same edges between them.
            let mut node_mapping = FnvHashMap::default();
            let mut get_node = |nfa: &mut Nfa, incoming: NodeIndex| {
                node_mapping
                    .entry(incoming)
                    .or_insert_with(|| nfa.graph.add_node(()))
                    .clone()
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
    let _ = env_logger::init();
    use crate::test::MockProcessor;
    let mut db = MockProcessor::new();
    let fmt = db.get_formatter();
    let aa = db.edge(EdgeData::Output("aa".into()));
    let bb = db.edge(EdgeData::Output("bb".into()));

    let make_dfa = || {
        let mut nfa = Nfa::new();
        for ir in &[RefIR::Edge(Some(aa)), RefIR::Edge(Some(bb))] {
            let first = nfa.graph.add_node(());
            nfa.start.insert(first);
            let last = add_to_graph(&db, &fmt, &mut nfa, ir, first);
            nfa.accepting.insert(last);
        }
        nfa.brzozowski_minimise()
    };

    let mut count = 0;
    for _ in 0..100 {
        let dfa = make_dfa();
        debug!("{}", dfa.debug_graph(&db));
        if dfa.accepts_data(&db, &[aa.lookup(&db)]) {
            count += 1;
        }
    }
    assert_eq!(count, 100);
}
