// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::prelude::*;

use citeproc_io::output::markup::Markup;
use citeproc_io::output::LocalizedQuotes;
use csl::Atom;
use csl::{Affixes, DisplayMode, Element, Formatting};

pub fn sequence_basic<'c, O, I>(
    db: &dyn IrDatabase,
    state: &mut IrState,
    ctx: &CiteContext<'c, O, I>,
    arena: &mut IrArena<O>,
    els: &[Element],
) -> NodeId
where
    O: OutputFormat,
    I: OutputFormat,
{
    sequence(
        db,
        state,
        ctx,
        arena,
        els,
        "".into(),
        None,
        None,
        None,
        None,
        TextCase::None,
        false,
    )
}

pub fn sequence<'c, O, I>(
    db: &dyn IrDatabase,
    state: &mut IrState,
    ctx: &CiteContext<'c, O, I>,
    arena: &mut IrArena<O>,
    els: &[Element],
    delimiter: Atom,
    formatting: Option<Formatting>,
    affixes: Option<&Affixes>,
    display: Option<DisplayMode>,
    // Only because <text macro="xxx" /> supports quotes.
    quotes: Option<LocalizedQuotes>,
    text_case: TextCase,
    is_group: bool,
) -> NodeId
where
    O: OutputFormat,
    I: OutputFormat,
{
    let mut contents = Vec::with_capacity(els.len());
    let mut overall_gv = GroupVars::new();
    let mut dropped_gv = GroupVars::new();

    // We will edit this later if it turns out it has content & isn't discarded
    let self_node = arena.new_node((IR::Rendered(None), GroupVars::Plain));
    if els.is_empty() {
        return self_node;
    }

    for el in els {
        let child = el.intermediate(db, state, ctx, arena);
        let ch = arena.get(child).unwrap().get();
        match ch.0 {
            IR::Rendered(None) => {
                dropped_gv = dropped_gv.neighbour(gv);
                overall_gv = overall_gv.neighbour(gv);
            }
            _ => {
                self_node.append(child, arena);
                overall_gv = overall_gv.neighbour(gv)
            }
        }
    }

    let ir = if contents.is_empty() {
        IR::Rendered(None)
    } else {
        IR::Seq(IrSeq {
            formatting,
            affixes: affixes.cloned(),
            delimiter,
            display: if ctx.in_bibliography { display } else { None },
            quotes,
            text_case,
            dropped_gv: if is_group { Some(dropped_gv) } else { None },
        })
    };

    let (set_ir, set_gv) = if is_group {
        overall_gv.implicit_conditional(ir)
    } else {
        (ir, overall_gv)
    };

    let (self_ir, self_gv) = arena.get_mut(self_node).unwrap().get_mut();
    *self_ir = set_ir;
    *self_gv = set_gv;
    self_node
}

pub fn ref_sequence_basic<'c>(
    db: &dyn IrDatabase,
    state: &mut IrState,
    ctx: &RefContext<'c>,
    els: &[Element],
    stack: Formatting,
) -> (RefIR, GroupVars) {
    ref_sequence(
        db,
        state,
        ctx,
        els,
        "".into(),
        Some(stack),
        None,
        None,
        None,
        TextCase::None,
    )
}

pub fn ref_sequence<'c>(
    db: &dyn IrDatabase,
    state: &mut IrState,
    ctx: &RefContext<'c, Markup>,
    els: &[Element],
    delimiter: Atom,
    formatting: Option<Formatting>,
    affixes: Option<&Affixes>,
    _display: Option<DisplayMode>,
    quotes: Option<LocalizedQuotes>,
    text_case: TextCase,
) -> (RefIR, GroupVars) {
    let _fmt = &ctx.format;

    let mut contents = Vec::with_capacity(els.len());
    let mut overall_gv = GroupVars::new();
    // let mut dropped_gv = GroupVars::new();

    for el in els {
        let (ir, gv) =
            Disambiguation::<Markup>::ref_ir(el, db, ctx, state, formatting.unwrap_or_default());
        match ir {
            RefIR::Edge(None) => {
                // dropped_gv = dropped_gv.neighbour(gv);
                overall_gv = overall_gv.neighbour(gv);
            }
            _ => {
                contents.push(ir);
                overall_gv = overall_gv.neighbour(gv)
            }
        }
    }

    if !contents.iter().any(|x| *x != RefIR::Edge(None)) {
        (RefIR::Edge(None), overall_gv)
    } else {
        (
            RefIR::Seq(RefIrSeq {
                contents,
                formatting,
                affixes: affixes.cloned(),
                delimiter,
                quotes,
                text_case,
                // dropped_gv,
            }),
            overall_gv,
        )
    }
}

use fnv::FnvHashSet;
pub fn fnv_set_with_cap<T: std::hash::Hash + std::cmp::Eq>(cap: usize) -> FnvHashSet<T> {
    FnvHashSet::with_capacity_and_hasher(cap, fnv::FnvBuildHasher::default())
}

use csl::{StandardVariable, TextCase, TextElement, TextSource, Variable, VariableForm};
pub fn plain_text_element(v: Variable) -> TextElement {
    TextElement {
        source: TextSource::Variable(StandardVariable::Ordinary(v), VariableForm::Long),
        formatting: None,
        affixes: None,
        quotes: false,
        strip_periods: false,
        text_case: TextCase::None,
        display: None,
    }
}
