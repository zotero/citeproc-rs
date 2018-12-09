use super::cite_context::*;
use super::{Proc, IR};
use crate::output::OutputFormat;
use crate::style::element::{Element, Formatting};

#[cfg(feature = "rayon")]
use rayon::prelude::*;

#[cfg_attr(feature = "flame_it", flame)]
pub fn sequence<'c, 's: 'c, 'r, 'ci, O>(
    ctx: &CiteContext<'c, 'r, 'ci, O>,
    f: &Formatting,
    delim: &str,
    els: &'s [Element],
) -> IR<'c, O>
where
    O: OutputFormat,
{
    //TODO: add delimiters to deferred IR::Seq

    let fold_seq = |va: &mut Vec<IR<'c, O>>, other: IR<'c, O>| {
        use super::ir::IR::*;
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
                        va.push(Rendered(Some(ctx.format.group(&[aa, bb], delim, &f))))
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
    // the only output, so
    //
    // <group><names>...</names></group> matches `(Rendered(None), b) => b` == Names(...)

    let folder = |left: IR<'c, O>, right: IR<'c, O>| {
        use super::ir::IR::*;
        match (left, right) {
            (a, Rendered(None)) => a,
            (Rendered(None), b) => b,
            // aa,bb
            (Rendered(Some(aa)), Rendered(Some(bb))) => {
                Rendered(Some(ctx.format.group(&[aa, bb], delim, &f)))
            }
            (Seq(mut va), b) => {
                fold_seq(&mut va, b);
                Seq(va)
            }
            (a, b) => Seq(vec![a, b]),
        }
    };

    // #[cfg(feature="rayon")] {
    //     els.par_iter()
    //         .map(|el| el.intermediate(ctx))
    //         .reduce(|| IR::Rendered(None), folder)
    // }
    // #[cfg(not(feature = "rayon"))] {
    els.iter()
        .map(|el| el.intermediate(ctx))
        .fold(IR::Rendered(None), folder)
    // }
}

#[cfg(test)]
mod test {
    use super::super::ir::IR::*;
    use super::super::Proc;
    use super::*;

    #[test]
    fn associative() {}
}
