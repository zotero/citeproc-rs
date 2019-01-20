use super::cite_context::*;
use super::ir::IR::*;
use super::{IrSeq, Proc, IR};
use crate::output::OutputFormat;
use crate::style::element::{Affixes, Element, Formatting};

pub fn sequence<'c, 's: 'c, O>(
    ctx: &CiteContext<'c, O>,
    els: &'s [Element],
    delimiter: &'c str,
    formatting: Option<&'c Formatting>,
    affixes: Affixes,
) -> IR<'c, O>
where
    O: OutputFormat,
{
    let fmt = &ctx.format;

    let fold_seq = |va: &mut Vec<IR<'c, O>>, other: IR<'c, O>| {
        match other {
            // this seq is another group with its own delimiter (possibly)
            b @ Seq(_) => {
                va.push(b);
            }
            Rendered(None) => {}
            Rendered(Some(bb)) => {
                if let Some(last) = va.pop() {
                    if let Rendered(None) = last {
                        va.push(Rendered(Some(bb)))
                    } else if let Rendered(Some(aa)) = last {
                        va.push(Rendered(Some(fmt.join_delim(aa, delimiter, bb))))
                    } else {
                        va.push(last);
                        va.push(Rendered(Some(bb)));
                    }
                } else {
                    va.push(Rendered(Some(bb)));
                }
            }
            o => {
                va.push(o);
            }
        }
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

    let folder = |left: IR<'c, O>, right: IR<'c, O>| {
        match (left, right) {
            (a, Rendered(None)) => a,
            (Rendered(None), b) => b,
            // aa,bb
            (Rendered(Some(aa)), Rendered(Some(bb))) => {
                Rendered(Some(fmt.join_delim(aa, delimiter, bb)))
            }
            (Seq(mut s), b) => {
                fold_seq(&mut s.contents, b);
                Seq(s)
            }
            (a, b) => Seq(IrSeq {
                contents: vec![a, b],
                formatting,
                affixes: affixes.clone(),
                delimiter,
            }),
        }
    };

    // #[cfg(feature="rayon")] {
    //     use rayon::prelude::*;
    //     els.par_iter()
    //         .map(|el| el.intermediate(ctx))
    //         .reduce(|| IR::Rendered(None), folder)
    // }
    // #[cfg(not(feature = "rayon"))] {
    // }

    let inner = els
        .iter()
        .map(|el| el.intermediate(ctx))
        .fold(IR::Rendered(None), folder);

    if let Rendered(None) = inner {
        inner
    } else if let Rendered(Some(x)) = inner {
        Rendered(Some(fmt.affixed(fmt.with_format(x, formatting), &affixes)))
    } else if let Seq(_) = inner {
        // no formatting necessary, Seq has it embedded
        inner
    } else {
        Seq(IrSeq {
            contents: vec![inner],
            formatting,
            affixes,
            delimiter,
        })
    }
}
