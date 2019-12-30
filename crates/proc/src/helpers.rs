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
    sequence(db, state, ctx, els, "".into(), None, None, None, None, TextCase::None)
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
) -> IrSum<O>
where
    O: OutputFormat,
    I: OutputFormat,
{
    let _fmt = &ctx.format;

    let (inner, gv) = els.iter().map(|el| el.intermediate(db, state, ctx)).fold(
        (Vec::new(), GroupVars::new()),
        |(mut acc, acc_gv), (ir, gv)| match ir {
            IR::Rendered(None) => (acc, acc_gv.neighbour(gv)),
            _ => {
                acc.push((ir, gv));
                (acc, acc_gv.neighbour(gv))
            }
        },
    );

    if inner.is_empty() {
        (IR::Rendered(None), gv)
    } else {
        (
            IR::Seq(IrSeq {
                contents: inner,
                formatting,
                affixes: affixes.cloned(),
                delimiter,
                display: if ctx.in_bibliography { display } else { None },
                quotes,
                text_case,
            }),
            gv,
        )
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

    let (inner, gv) = els
        .iter()
        .map(|el| {
            Disambiguation::<Markup>::ref_ir(el, db, ctx, state, formatting.unwrap_or_default())
        })
        .fold(
            (Vec::new(), GroupVars::new()),
            |(mut acc, acc_gv), (ir, gv)| match ir {
                RefIR::Edge(None) => (acc, acc_gv.neighbour(gv)),
                _ => {
                    acc.push(ir);
                    (acc, acc_gv.neighbour(gv))
                }
            },
        );

    if inner.is_empty() {
        (RefIR::Edge(None), gv)
    } else {
        (
            RefIR::Seq(RefIrSeq {
                contents: inner,
                formatting,
                affixes: affixes.cloned(),
                delimiter,
                quotes,
                text_case,
            }),
            gv,
        )
    }
}

use fnv::FnvHashSet;
pub fn fnv_set_with_cap<T: std::hash::Hash + std::cmp::Eq>(cap: usize) -> FnvHashSet<T> {
    FnvHashSet::with_capacity_and_hasher(cap, fnv::FnvBuildHasher::default())
}
