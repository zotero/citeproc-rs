use crate::output::OutputFormat;
use crate::style::element::{Affixes, Element, Layout as LayoutEl, Style};
use crate::style::terms::{GenderedTermSelector, TextTermSelector};
use crate::style::variables::*;

mod choose;
mod cite_context;
mod date;
mod helpers;
mod ir;
mod names;
pub use self::cite_context::*;
use self::helpers::sequence;
pub use self::ir::*;

// TODO: function to walk the entire tree for a <text variable="year-suffix"> to work out which
// nodes are possibly disambiguate-able in year suffix mode and if such a node should be inserted
// at the end of the layout block before the suffix. (You would only insert an IR node, not in the
// actual style, to keep it immutable and plain-&borrow-thread-shareable).
// TODO: also to figure out which macros are needed
// TODO: juris-m module loading in advance? probably in advance.

// Levels 1-3 will also have to update the ConditionalDisamb's current render

//
// * `'c`: [Cite]
// * `'ci`: [Cite]
// * `'r`: [Reference][]
//
// [Style]: ../style/element/struct.Style.html
// [Reference]: ../input/struct.Reference.html
pub trait Proc<'c, 'r: 'c, 'ci: 'c, O>
where
    O: OutputFormat,
{
    /// `'s` (the self lifetime) must live longer than the IR it generates, because the IR will
    /// often borrow from self to be recomputed during disambiguation.
    fn intermediate<'s: 'c>(&'s self, ctx: &CiteContext<'c, 'r, 'ci, O>) -> IR<'c, O>;
}

impl<'c, 'r: 'c, 'ci: 'c, O> Proc<'c, 'r, 'ci, O> for Style
where
    O: OutputFormat,
{
    fn intermediate<'s: 'c>(&'s self, ctx: &CiteContext<'c, 'r, 'ci, O>) -> IR<'c, O> {
        let citation = &self.citation;
        let layout = &citation.layout;
        layout.intermediate(ctx)
    }
}

impl<'c, 'r: 'c, 'ci: 'c, O> Proc<'c, 'r, 'ci, O> for LayoutEl
where
    O: OutputFormat,
{
    /// Layout's delimiter and affixes are going to be applied later, when we join a cluster.
    fn intermediate<'s: 'c>(&'s self, ctx: &CiteContext<'c, 'r, 'ci, O>) -> IR<'c, O> {
        sequence(ctx, &self.elements, "", None, Affixes::default())
    }
}

impl<'c, 'r: 'c, 'ci: 'c, O> Proc<'c, 'r, 'ci, O> for Element
where
    O: OutputFormat,
{
    fn intermediate<'s: 'c>(&'s self, ctx: &CiteContext<'c, 'r, 'ci, O>) -> IR<'c, O> {
        let fmt = ctx.format;
        match *self {
            Element::Choose(ref ch) => ch.intermediate(ctx),

            Element::Macro(ref name, ref f, ref af, ref _quo) => {
                // TODO: be able to return errors
                let macro_unsafe = ctx
                    .style
                    .macros
                    .get(name)
                    .expect("macro errors unimplemented!");
                sequence(ctx, &macro_unsafe, "", f.as_ref(), af.clone())
            }

            Element::Const(ref val, ref f, ref af, ref _quo) => {
                IR::Rendered(Some(fmt.affixed_text(val.clone(), f.as_ref(), &af)))
            }

            Element::Variable(ref var, ref f, ref af, ref _form, ref _quo) => {
                let content = match *var {
                    StandardVariable::Ordinary(ref v) => ctx.reference.ordinary.get(v).map(|val| {
                        let s = if v.should_replace_hyphens() {
                            val.replace('-', "\u{2013}")
                        } else {
                            val.clone().into_owned()
                        };
                        fmt.affixed_text(s, f.as_ref(), &af)
                    }),
                    StandardVariable::Number(ref v) => ctx.reference.number.get(v).map(|val| {
                        fmt.affixed_text(val.verbatim(v.should_replace_hyphens()), f.as_ref(), &af)
                    }),
                };
                IR::Rendered(content)
            }

            Element::Term(ref term_selector, ref f, ref af, pl) => {
                let content = ctx
                    .style
                    .locale_overrides
                    // TODO: support multiple locales!
                    .get(&None)
                    .unwrap()
                    .get_text_term(term_selector, pl)
                    .map(|val| fmt.affixed_text(val.to_owned(), f.as_ref(), &af));
                IR::Rendered(content)
            }

            Element::Label(ref var, ref form, ref f, ref af, ref pl) => {
                use crate::style::element::Plural;
                let selector =
                    GenderedTermSelector::from_number_variable(&ctx.cite.locator_type, var, form);
                let num_val = ctx.get_number(var);
                let plural = match (num_val, pl) {
                    (None, _) => None,
                    (Some(ref val), Plural::Contextual) => Some(val.is_multiple()),
                    (Some(_), Plural::Always) => Some(true),
                    (Some(_), Plural::Never) => Some(false),
                };
                let content = plural.and_then(|p| {
                    selector.and_then(|sel| {
                        ctx.style
                            .locale_overrides
                            // TODO: support multiple locales!
                            .get(&None)
                            .unwrap()
                            .get_text_term(&TextTermSelector::Gendered(sel), p)
                            .map(|val| fmt.affixed_text(val.to_owned(), f.as_ref(), &af))
                    })
                });
                IR::Rendered(content)
            }

            Element::Number(ref var, ref _form, ref f, ref af, ref _pl) => {
                IR::Rendered(ctx.get_number(var).map(|val| {
                    fmt.affixed_text(val.as_number(var.should_replace_hyphens()), f.as_ref(), &af)
                }))
            }

            Element::Names(ref ns) => ns.intermediate(ctx),

            //
            // You're going to have to replace sequence() with something more complicated.
            // And pass up information about .any(|v| used variables).
            Element::Group(ref g) => sequence(
                ctx,
                g.elements.as_ref(),
                &g.delimiter.0,
                g.formatting.as_ref(),
                g.affixes.clone(),
            ),
            Element::Date(ref dt) => {
                dt.intermediate(ctx)
                // IR::YearSuffix(YearSuffixHook::Date(dt.clone()), fmt.plain("date"))
            }
        }
    }
}
