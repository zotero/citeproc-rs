// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::prelude::*;

use citeproc_io::output::html::Html;
use csl::style::{Affixes, Element, Formatting};
use csl::Atom;

pub fn sequence<'c, O>(
    state: &mut IrState,
    ctx: &CiteContext<'c, O>,
    els: &[Element],
    delimiter: Atom,
    formatting: Option<Formatting>,
    affixes: Affixes,
) -> IrSum<O>
where
    O: OutputFormat,
{
    let fmt = &ctx.format;

    let (inner, gv) = els.iter().map(|el| el.intermediate(state, ctx)).fold(
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
            }),
            gv,
        )
    }
}

pub fn ref_sequence<'c>(
    db: &impl IrDatabase,
    ctx: &RefContext<'c, Html>,
    els: &[Element],
    delimiter: Atom,
    formatting: Option<Formatting>,
    affixes: Affixes,
) -> (RefIR, GroupVars) {
    let fmt = &ctx.format;

    let (inner, gv) = els
        .iter()
        .map(|el| Disambiguation::<Html>::ref_ir(el, db, ctx, formatting.unwrap_or_default()))
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
            }),
            gv,
        )
    }
}

pub fn to_bijective_base_26(int: u32) -> String {
    let mut n = int;
    let mut s = String::new();
    while n > 0 {
        n -= 1;
        s.push(char::from((65 + 32 + (n % 26)) as u8));
        n /= 26;
    }
    s
}

use fnv::FnvHashSet;
pub fn fnv_set_with_cap<T: std::hash::Hash + std::cmp::Eq>(cap: usize) -> FnvHashSet<T> {
    FnvHashSet::with_capacity_and_hasher(cap, fnv::FnvBuildHasher::default())
}
