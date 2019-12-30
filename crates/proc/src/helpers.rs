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
    db: &impl IrDatabase,
    state: &mut IrState,
    ctx: &CiteContext<'c, O, I>,
    els: &[Element],
) -> IrSum<O>
where
    O: OutputFormat,
    I: OutputFormat,
{
    sequence(db, state, ctx, els, "".into(), None, None, None, None, TextCase::None, false)
}


pub fn sequence<'c, O, I>(
    db: &impl IrDatabase,
    state: &mut IrState,
    ctx: &CiteContext<'c, O, I>,
    els: &[Element],
    delimiter: Atom,
    formatting: Option<Formatting>,
    affixes: Option<&Affixes>,
    display: Option<DisplayMode>,
    // Only because <text macro="xxx" /> supports quotes.
    quotes: Option<LocalizedQuotes>,
    text_case: TextCase,
    is_group: bool,
) -> IrSum<O>
where
    O: OutputFormat,
    I: OutputFormat,
{
    let mut contents = Vec::with_capacity(els.len());
    let mut overall_gv = GroupVars::new();
    let mut dropped_gv = GroupVars::new();

    for el in els {
        let (ir, gv) = el.intermediate(db, state, ctx);
        match ir {
            IR::Rendered(None) => {
                dropped_gv = dropped_gv.neighbour(gv);
                overall_gv = overall_gv.neighbour(gv);
            }
            _ => {
                contents.push((ir, gv));
                overall_gv = overall_gv.neighbour(gv)
            }
        }
    }

    let ir = if contents.is_empty() {
        IR::Rendered(None)
    } else {
        IR::Seq(IrSeq {
            contents,
            formatting,
            affixes: affixes.cloned(),
            delimiter,
            display: if ctx.in_bibliography { display } else { None },
            quotes,
            text_case,
            dropped_gv: if is_group {
                Some(dropped_gv)
            } else {
                None
            },
        })
    };
    if is_group {
        overall_gv.implicit_conditional(ir)
    } else {
        (ir, overall_gv)
    }
}

pub fn ref_sequence_basic<'c>(
    db: &impl IrDatabase,
    state: &mut IrState,
    ctx: &RefContext<'c>,
    els: &[Element],
    stack: Formatting,
) -> (RefIR, GroupVars) {
    ref_sequence(db, state, ctx, els, "".into(), Some(stack), None, None, None, TextCase::None)
}


pub fn ref_sequence<'c>(
    db: &impl IrDatabase,
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
        let (ir, gv) = Disambiguation::<Markup>::ref_ir(el, db, ctx, state, formatting.unwrap_or_default());
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
