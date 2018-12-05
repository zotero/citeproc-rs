use crate::output::OutputFormat;
use crate::style::element::{Element, Formatting, Layout as LayoutEl, Style};
use crate::style::variables::*;

mod choose;
mod cite_context;
mod date;
mod helpers;
mod ir;
pub use self::cite_context::*;
use self::helpers::sequence;
pub use self::ir::*;

// TODO: function to walk the entire tree for a <text variable="year-suffix"> to work out which
// nodes are possibly disambiguate-able in year suffix mode and if such a node should be inserted
// at the end of the layout block before the suffix.
// TODO: also to figure out which macros are needed
// TODO: juris-m module loading in advance? probably in advance.

// Levels 1-3 will also have to update the ConditionalDisamb's current render

// 's: style
// 'r: reference
pub trait Proc<'c, 's: 'c> {
    // TODO: include settings and reference and macro map
    fn intermediate<'r, O>(&'s self, ctx: &mut CiteContext<'c, 'r, O>) -> IR<'c, O>
    where
        O: OutputFormat;
}

#[cfg_attr(feature = "flame_it", flame("Style"))]
impl<'c, 's: 'c> Proc<'c, 's> for Style {
    fn intermediate<'r, O>(&'s self, ctx: &mut CiteContext<'c, 'r, O>) -> IR<'c, O>
    where
        O: OutputFormat,
    {
        let citation = &self.citation;
        let layout = &citation.layout;
        layout.intermediate(ctx)
    }
}

// TODO: insert affixes into group before processing as a group
impl<'c, 's: 'c> Proc<'c, 's> for LayoutEl {
    #[cfg_attr(feature = "flame_it", flame("Layout"))]
    fn intermediate<'r, O>(&'s self, ctx: &mut CiteContext<'c, 'r, O>) -> IR<'c, O>
    where
        O: OutputFormat,
    {
        sequence(ctx, &self.formatting, &self.delimiter.0, &self.elements)
    }
}

impl<'c, 's: 'c> Proc<'c, 's> for Element {
    #[cfg_attr(feature = "flame_it", flame("Element"))]
    fn intermediate<'r, O>(&'s self, ctx: &mut CiteContext<'c, 'r, O>) -> IR<'c, O>
    where
        O: OutputFormat,
    {
        let fmt = ctx.format;
        let null_f = Formatting::default();
        match *self {
            Element::Choose(ref ch) => ch.intermediate(ctx),

            Element::Macro(ref name, ref f, ref _af, ref _quo) => {
                // TODO: be able to return errors
                let macro_unsafe = ctx
                    .style
                    .macros
                    .get(name)
                    .expect("macro errors unimplemented!");
                sequence(ctx, &f, "", &macro_unsafe)
            }

            Element::Const(ref val, ref f, ref af, ref _quo) => IR::Rendered(Some(fmt.group(
                &[
                    fmt.plain(&af.prefix),
                    fmt.text_node(&val, &f),
                    fmt.plain(&af.suffix),
                ],
                "",
                &null_f,
            ))),

            Element::Variable(ref var, ref f, ref af, ref _form, ref _quo) => {
                let content = match *var {
                    StandardVariable::Ordinary(ref v) => ctx
                        .reference
                        .ordinary
                        .get(v)
                        .map(|val| fmt.affixed(&format!("{}", val), &f, &af)),
                    StandardVariable::Number(ref v) => {
                        ctx.reference.number.get(v).map(|val| fmt.affixed(&val.to_string(), &f, &af) /*match *val {
                            Ok(int) => fmt.affixed(&format!("{}", int), &f, &af),
                            Err(st) => fmt.affixed(&format!("{}", st), &f, &af),
                        }*/)
                    }
                };
                IR::Rendered(content)
            }

            Element::Term(ref term, ref _form, ref f, ref af, ref _pl) => {
                IR::Rendered(Some(fmt.group(
                    &[
                        fmt.plain(&af.prefix),
                        fmt.text_node(&format!("(term {})", term), &f),
                        fmt.plain(&af.suffix),
                    ],
                    "",
                    &null_f,
                )))
            }

            Element::Label(ref var, ref _form, ref f, ref af, ref _pl) => {
                IR::Rendered(Some(fmt.group(
                    &[
                        fmt.plain(&af.prefix),
                        fmt.text_node(&format!("(label {})", var.as_ref()), &f),
                        fmt.plain(&af.suffix),
                    ],
                    "",
                    &null_f,
                )))
            }

            Element::Number(ref var, ref _form, ref f, ref af, ref _pl) => {
                IR::Rendered(ctx.reference.number.get(&var).map(|val| fmt.affixed(&val.to_string(), &f, &af) /*match *val {
                    Ok(int) => fmt.affixed(&format!("{}", int), &f, &af),
                    Err(st) => fmt.affixed(&format!("{}", st), &f, &af),
                }*/))
            }

            Element::Names(ref ns) => IR::Names(ns, fmt.plain("names first-pass")),

            // TODO: cs:group implicitly acts as a conditional: cs:group and its child elements
            // are suppressed if a) at least one rendering element in cs:group calls a variable
            // (either directly or via a macro), and b) all variables that are called are
            // empty. This accommodates descriptive cs:text elements.
            //
            // You're going to have to replace sequence() with something more complicated.
            // And pass up information about .any(|v| used variables).
            Element::Group(ref f, ref d, ref els) => sequence(ctx, f, &d.0, els.as_ref()),
            Element::Date(ref dt) => {
                dt.intermediate(ctx)
                // IR::YearSuffix(YearSuffixHook::Date(dt.clone()), fmt.plain("date"))
            }
        }
    }
}

