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
    use super::ir::IR::*;
    let fmt = &ctx.format;

    let fold_seq = |(va, gva): (&mut Vec<IR<O>>, GroupVars), (other, gvb): IrSum<O>| {
        match other {
            // this seq is another group with its own delimiter (possibly)
            b @ Seq(_) => {
                va.push(b);
            }
            // say b was TextSource::Variable; not rendering it makes (Rendered(None), OnlyEmpty)
            // so you do have to handle it. We do (below).
            // You do have to make sure that if it was a group that did not
            // end up producing output, it has a correct gv = NoneSeen.
            Rendered(None) => {}
            Rendered(Some(CiteEdgeData::Output(bb))) => {
                if let Some(last) = va.pop() {
                    if let Rendered(None) = last {
                        va.push(Rendered(Some(CiteEdgeData::Output(bb))));
                    } else if let Rendered(Some(CiteEdgeData::Output(aa))) = last {
                        va.push(Rendered(Some(CiteEdgeData::Output(
                            fmt.join_delim(aa, &delimiter, bb),
                        ))))
                    } else {
                        va.push(last);
                        va.push(Rendered(Some(CiteEdgeData::Output(bb))));
                    }
                } else {
                    va.push(Rendered(Some(CiteEdgeData::Output(bb))));
                }
            }
            o => {
                va.push(o);
            }
        }
        gva.neighbour(gvb)
    };

    // This reduction has to be associative, because Rayon's `reduce` does not run in-order.
    // i.e. not:
    //      folder(0, folder(1, folder(2, folder(3, folder(4, 5)))));
    // it might be instead:
    //      folder(folder(0, folder(1, 2)), folder(0, folder(3, folder(4, 5)))).
    //
    // Note that our monoid zero is Rendered(None). We only start building a Seq if one of the
    // child elements is a disambiguation-participant IR node like Names, Seq, Choose. But we
    // prefer to stay with Rendered as long as possible, so the smallest output is mzero, then
    // Rendered(Some(xxx)). If there is only a single item in the sequence, it should end up as
    // the only output.
    //
    // <group><names>...</names></group> matches `(Rendered(None), b) => b` == Names(...)

    let folder = |left: IrSum<O>, right: IrSum<O>| {
        match (left, right) {
            ((a, gva), (Rendered(None), gvb)) => (a, gva.neighbour(gvb)),
            ((Rendered(None), gva), (b, gvb)) => (b, gva.neighbour(gvb)),
            // aa,bb
            (
                (Rendered(Some(CiteEdgeData::Output(aa))), gva),
                (Rendered(Some(CiteEdgeData::Output(bb))), gvb),
            ) => (
                Rendered(Some(CiteEdgeData::Output(
                    fmt.join_delim(aa, &delimiter, bb),
                ))),
                gva.neighbour(gvb),
            ),
            ((Seq(mut s), gva), b) => {
                let gvc = fold_seq((&mut s.contents, gva), b);
                (Seq(s), gvc)
            }
            ((a, gva), (b, gvb)) => (
                Seq(IrSeq {
                    contents: vec![a, b],
                    formatting,
                    affixes: affixes.clone(),
                    delimiter: delimiter.clone(),
                }),
                gva.neighbour(gvb),
            ),
        }
    };

    // #[cfg(feature="rayon")] {
    //     use rayon::prelude::*;
    //     els.par_iter()
    //         .map(|el| el.intermediate(db, state, ctx))
    //         .reduce(|| IR::Rendered(None), folder)
    // }
    // #[cfg(not(feature = "rayon"))] {
    // }

    let (inner, gv) = els
        .iter()
        .map(|el| el.intermediate(state, ctx))
        .fold((IR::Rendered(None), GroupVars::new()), folder);

    if let Rendered(None) = inner {
        (inner, gv)
    } else if let Rendered(Some(CiteEdgeData::Output(x))) = inner {
        (
            Rendered(Some(CiteEdgeData::Output(
                fmt.affixed(fmt.with_format(x, formatting), &affixes),
            ))),
            gv,
        )
    } else if let Seq(_) = inner {
        // no formatting necessary, Seq has it embedded
        (inner, gv)
    } else {
        (
            Seq(IrSeq {
                contents: vec![inner],
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

pub fn ref_sequence<'c>(
    db: &impl IrDatabase,
    ctx: &RefContext<'c, Html>,
    els: &[Element],
    delimiter: Atom,
    stack: Option<Formatting>,
    affixes: Affixes,
) -> (RefIR, GroupVars) {
    let fmt = &ctx.format;

    let e = fmt.output_in_context(fmt.plain(&delimiter), stack.unwrap_or_default());
    let delim = RefIR::Edge(Some(db.edge(EdgeData::Output(e))));

    // This reduction has to be associative, because Rayon's `reduce` does not run in-order.
    // i.e. not:
    //      folder(0, folder(1, folder(2, folder(3, folder(4, 5)))));
    // it might be instead:
    //      folder(folder(0, folder(1, 2)), folder(0, folder(3, folder(4, 5)))).
    //
    // Note that our monoid zero is Rendered(None). We only start building a Seq if one of the
    // child elements is a disambiguation-participant IR node like Names, Seq, Choose. But we
    // prefer to stay with Rendered as long as possible, so the smallest output is mzero, then
    // Rendered(Some(xxx)). If there is only a single item in the sequence, it should end up as
    // the only output.
    //
    // <group><names>...</names></group> matches `(Rendered(None), b) => b` == Names(...)

    let mut seen_one = false;
    let (mut inner, mut gv) = els
        .iter()
        .map(|el| Disambiguation::<Html>::ref_ir(el, db, ctx, stack.unwrap_or_default()))
        .fold(
            (Vec::new(), GroupVars::new()),
            |(mut acc, acc_gv), (ir, gv)| match ir {
                RefIR::Edge(None) => (acc, acc_gv),
                RefIR::Edge(_) | RefIR::Names(..) => {
                    if seen_one {
                        acc.push(delim.clone());
                    }
                    acc.push(ir);
                    seen_one = true;
                    (acc, acc_gv.neighbour(gv))
                }
                RefIR::Seq(inner) => {
                    if seen_one {
                        acc.push(delim.clone());
                    }
                    acc.extend(inner);
                    seen_one = true;
                    (acc, acc_gv.neighbour(gv))
                }
            },
        );

    if inner.is_empty() {
        (RefIR::Edge(None), gv)
    } else {
        if affixes.prefix.len() > 0 {
            let e = fmt.output_in_context(fmt.plain(&affixes.prefix), Default::default());
            let edge = RefIR::Edge(Some(db.edge(EdgeData::Output(e))));
            inner.insert(0, edge);
        }
        if affixes.suffix.len() > 0 {
            let e = fmt.output_in_context(fmt.plain(&affixes.suffix), Default::default());
            let edge = RefIR::Edge(Some(db.edge(EdgeData::Output(e))));
            inner.push(edge);
        }
        (RefIR::Seq(inner), gv)
    }
}
