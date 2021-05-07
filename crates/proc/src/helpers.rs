// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::prelude::*;

use citeproc_io::output::markup::Markup;

pub fn sequence<'c, O, I>(
    db: &dyn IrDatabase,
    state: &mut IrState,
    ctx: &CiteContext<'c, O, I>,
    arena: &mut IrArena<O>,
    els: &[Element],
    implicit_conditional: bool,
    seq_template: Option<&dyn Fn() -> IrSeq>,
) -> NodeId
where
    O: OutputFormat,
    I: OutputFormat,
{
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
        let gv = ch.1;
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

    let ir = if self_node.children(arena).next().is_none() {
        IR::Rendered(None)
    } else {
        let mut seq = IrSeq {
            dropped_gv: if implicit_conditional {
                Some(dropped_gv)
            } else {
                None
            },
            ..if let Some(tmpl) = seq_template {
                tmpl()
            } else {
                Default::default()
            }
        };
        if !ctx.in_bibliography {
            seq.display = None;
        }
        IR::Seq(seq)
    };

    let (set_ir, set_gv) = if implicit_conditional {
        overall_gv.implicit_conditional(ir)
    } else {
        (ir, overall_gv)
    };

    let (self_ir, self_gv) = arena.get_mut(self_node).unwrap().get_mut();
    *self_ir = set_ir;
    *self_gv = set_gv;
    self_node
}

pub fn ref_sequence<'c>(
    db: &dyn IrDatabase,
    state: &mut IrState,
    ctx: &RefContext<'c, Markup>,
    els: &[Element],
    _implicit_conditional: bool,
    formatting: Option<Formatting>,
    seq_template: Option<&dyn Fn() -> RefIrSeq>,
) -> (RefIR, GroupVars) {
    let _fmt = &ctx.format;

    let mut contents = Vec::with_capacity(els.len());
    let mut overall_gv = GroupVars::new();
    let fmting = formatting.unwrap_or_default();

    for el in els {
        let (got_ir, gv) = el.ref_ir(db, ctx, state, fmting);
        match got_ir {
            RefIR::Edge(None) => {
                overall_gv = overall_gv.neighbour(gv);
            }
            _ => {
                contents.push(got_ir);
                overall_gv = overall_gv.neighbour(gv)
            }
        }
    }

    if !contents.iter().any(|x| !matches!(x, RefIR::Edge(None))) {
        (RefIR::Edge(None), overall_gv)
    } else {
        let mut seq = if let Some(tmpl) = seq_template {
            tmpl()
        } else {
            Default::default()
        };
        seq.contents = contents;
        seq.formatting = formatting;
        (RefIR::Seq(seq), overall_gv)
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
