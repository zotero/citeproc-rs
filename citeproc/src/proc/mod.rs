use crate::input::Reference;
use crate::output::OutputFormat;
use crate::style::element::{Element, Formatting, Layout as LayoutEl, Style};
use crate::style::variables::*;

mod choose;
mod date;
mod helpers;
mod cite_context;
mod ir;
use self::helpers::sequence;
pub use self::cite_context::*;
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
    fn intermediate<'r, O>(&'s self, ctx: &CiteContext<'c, 'r, O>) -> IR<'c, O>
    where
        O: OutputFormat;
}

#[cfg_attr(feature = "flame_it", flame)]
impl<'c, 's: 'c> Proc<'c, 's> for Style {
    fn intermediate<'r, O>(&'s self, ctx: &CiteContext<'c, 'r, O>) -> IR<'c, O>
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
    #[cfg_attr(feature = "flame_it", flame)]
    fn intermediate<'r, O>(&'s self, ctx: &CiteContext<'c, 'r, O>) -> IR<'c, O>
    where
        O: OutputFormat,
    {
        sequence(ctx, &self.formatting, &self.delimiter.0, &self.elements)
    }
}

impl<'c, 's: 'c> Proc<'c, 's> for Element {
    #[cfg_attr(feature = "flame_it", flame)]
    fn intermediate<'r, O>(&'s self, ctx: &CiteContext<'c, 'r, O>) -> IR<'c, O>
    where
        O: OutputFormat,
    {
        let fmt = ctx.format;
        let null_f = Formatting::default();
        match *self {
            Element::Choose(ref ch) => ch.intermediate(ctx),

            Element::Macro(ref name, ref f, ref _af, ref _quo) => {
                // TODO: be able to return errors
                let macro_unsafe = ctx.style.macros.get(name).expect("macro errors unimplemented!");
                sequence(ctx, &f, "", &macro_unsafe)
                // IR::Rendered(Some(fmt.text_node(&format!("(macro {})", name), &f)))
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
                    StandardVariable::Ordinary(ref v) => ctx.reference.ordinary
                        .get(v)
                        .map(|val| fmt.affixed(&format!("{}", val), &f, &af)),
                    StandardVariable::Number(ref v) => ctx.reference.number.get(v).map(|val| match *val {
                        Ok(int) => fmt.affixed(&format!("{}", int), &f, &af),
                        Err(st) => fmt.affixed(&format!("{}", st), &f, &af),
                    }),
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
                IR::Rendered(ctx.reference.number.get(&var).map(|val| match *val {
                    Ok(int) => fmt.affixed(&format!("{}", int), &f, &af),
                    Err(st) => fmt.affixed(&format!("{}", st), &f, &af),
                }))
            }

            Element::Names(ref ns) => IR::Names(ns, fmt.plain("names first-pass")),
            Element::Group(ref f, ref d, ref els) => sequence(ctx, f, &d.0, els.as_ref()),
            Element::Date(ref dt) => {
                dt.intermediate(ctx)
                // IR::YearSuffix(YearSuffixHook::Date(dt.clone()), fmt.plain("date"))
            }
        }
    }
}

#[cfg(all(test, feature = "flame_it"))]
mod test {
    use super::Proc;
    use crate::input::*;
    use crate::output::PlainText;
    use crate::style::build_style;
    use crate::style::element::{CslType, Style};
    use crate::style::variables::*;
    use crate::test::Bencher;
    use std::fs::File;
    use std::io::prelude::*;
    use std::str::FromStr;

    #[bench]
    fn bench_intermediate(b: &mut Bencher) {
        let path = "/Users/cormac/git/citeproc-rs/example.csl";
        let mut f = File::open(path).expect("no file at path");
        let mut contents = String::new();
        f.read_to_string(&mut contents)
            .expect("something went wrong reading the file");
        let s = build_style(&contents);
        let fmt = PlainText::new();
        let mut refr = Reference::empty("id", CslType::LegalCase);
        refr.ordinary.insert(Variable::ContainerTitle, "TASCC");
        refr.number.insert(NumberVariable::Number, 55);
        refr.date.insert(
            DateVariable::Issued,
            DateOrRange::from_str("1998-01-04").unwrap(),
        );
        if let Ok(style) = s {
            b.iter(|| {
                style.intermediate(&fmt, &refr);
            });
        }
    }

}
