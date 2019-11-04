// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::prelude::*;

use citeproc_io::output::markup::Markup;
use csl::Atom;
use csl::{Affixes, Element, Formatting, DisplayMode};

pub fn sequence<'c, O, I>(
    db: &impl IrDatabase,
    state: &mut IrState,
    ctx: &CiteContext<'c, O, I>,
    els: &[Element],
    delimiter: Atom,
    formatting: Option<Formatting>,
    affixes: Affixes,
    display: Option<DisplayMode>,
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
                acc.push(ir);
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
                affixes,
                delimiter,
                display,
            }),
            gv,
        )
    }
}

pub fn ref_sequence<'c>(
    db: &impl IrDatabase,
    ctx: &RefContext<'c, Markup>,
    state: &mut IrState,
    els: &[Element],
    delimiter: Atom,
    formatting: Option<Formatting>,
    affixes: Affixes,
    display: Option<DisplayMode>
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
                affixes,
                delimiter,
                display,
            }),
            gv,
        )
    }
}

use fnv::FnvHashSet;
pub fn fnv_set_with_cap<T: std::hash::Hash + std::cmp::Eq>(cap: usize) -> FnvHashSet<T> {
    FnvHashSet::with_capacity_and_hasher(cap, fnv::FnvBuildHasher::default())
}
