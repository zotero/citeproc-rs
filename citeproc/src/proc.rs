use crate::input::DateOrRange;
use crate::input::Reference;
use crate::output::OutputFormat;
use crate::style::element::Delimiter;
use crate::style::element::{
    Choose as ChooseEl, Date as DateEl, Element, Formatting, Layout as LayoutEl, Names as NamesEl,
    Style,
};

#[derive(Debug)]
pub enum YearSuffixHook<'s> {
    Date(&'s DateEl),
    Explicit(),
}

// Intermediate Representation
#[derive(Debug)]
pub enum IR<'s, O: OutputFormat> {
    // no (further) disambiguation possible
    Rendered(Option<O::Build>),
    // the name block,
    // the current render
    Names(&'s NamesEl, O::Build),

    // a single <if disambiguate="true"> being tested once means the whole <choose> is re-rendered in step 4
    // or <choose><if><conditions><condition>
    ConditionalDisamb(&'s ChooseEl, Vec<IR<'s, O>>),
    YearSuffix(YearSuffixHook<'s>, O::Build),

    // Think:
    // <if disambiguate="true" ...>
    //     <text macro="..." />
    //     <text macro="..." />
    //     <text variable="year-suffix" />
    //     <text macro="..." />
    // </if>
    // = Seq[
    //     Rendered(...), // collapsed multiple nodes into one rendered
    //     YearSuffix(Explicit(Text(Variable::YearSuffix), T)),
    //     Rendered(..)
    // ]
    // // TODO: store delimiter and affixes for later
    Seq(Vec<IR<'s, O>>),
}

impl<'s, O: OutputFormat> IR<'s, O> {
    pub fn flatten<'r>(&'s self, fmt: &O) -> O::Build {
        // TODO: change fmt.group to accept iterators instead
        let seq = |xs: &[IR<'s, O>]| {
            let v: Vec<O::Build> = xs.iter().map(|i| i.flatten(fmt)).collect();
            fmt.group(&v, "", &Formatting::default())
        };
        // must clone
        match self {
            Rendered(None) => fmt.plain(""),
            Rendered(Some(ref x)) => x.clone(),
            Names(_, ref x) => x.clone(),
            ConditionalDisamb(_, ref xs) => seq(xs),
            YearSuffix(_, ref x) => x.clone(),
            Seq(ref xs) => seq(xs),
        }
    }
}

use self::IR::*;

// TODO: function to walk the entire tree for a <text variable="year-suffix"> to work out which
// nodes are possibly disambiguate-able in year suffix mode and if such a node should be inserted
// at the end of the layout block before the suffix.
// TODO: also to figure out which macros are needed
// TODO: juris-m module loading in advance? probably in advance.

// Levels 1-3 will also have to update the ConditionalDisamb's current render

// 's: style
// 'r: reference
pub trait Proc<'s> {
    // TODO: include settings and reference and macro map
    fn intermediate<'r, O>(&'s self, fmt: &O, refr: &Reference<'r>) -> IR<'s, O>
    where
        O: OutputFormat;
}

#[cfg_attr(feature = "flame_it", flame)]
impl<'s> Proc<'s> for Style<'s> {
    fn intermediate<'r, O>(&'s self, fmt: &O, refr: &Reference<'r>) -> IR<'s, O>
    where
        O: OutputFormat,
    {
        let citation = &self.citation;
        let layout = &citation.layout;
        layout.intermediate(fmt, refr)
    }
}

// TODO: insert affixes into group before processing as a group
impl<'s> Proc<'s> for LayoutEl<'s> {
    #[cfg_attr(feature = "flame_it", flame)]
    fn intermediate<'r, O>(&'s self, fmt: &O, refr: &Reference<'r>) -> IR<'s, O>
    where
        O: OutputFormat,
    {
        sequence(fmt, refr, &self.formatting, &self.delimiter, &self.elements)
    }
}

impl<'s> Proc<'s> for Element {
    #[cfg_attr(feature = "flame_it", flame)]
    fn intermediate<'r, O>(&'s self, fmt: &O, refr: &Reference<'r>) -> IR<'s, O>
    where
        O: OutputFormat,
    {
        let null_f = Formatting::default();
        match *self {
            Element::Choose(ref _ch) => {
                // TODO: work out if disambiguate appears on the conditions
                Rendered(Some(fmt.plain("choose")))
            }
            Element::Macro(ref name, ref f, ref _af, ref _quo) => {
                Rendered(Some(fmt.text_node(&format!("(macro {})", name), &f)))
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

            Element::Variable(ref var, ref f, ref af, ref _form, ref _del, ref _quo) => {
                let content = if let Some(val) = refr.ordinary.get(var) {
                    Some(fmt.group(
                        &[
                            fmt.plain(&af.prefix),
                            fmt.text_node(val, &f),
                            fmt.plain(&af.suffix),
                        ],
                        "",
                        &null_f,
                    ))
                } else {
                    None
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
                let content = if let Some(val) = refr.number.get(var) {
                    Some(fmt.group(
                        &[
                            fmt.plain(&af.prefix),
                            fmt.text_node(&format!("{}", val), &f),
                            fmt.plain(&af.suffix),
                        ],
                        "",
                        &null_f,
                    ))
                } else {
                    None
                };
                IR::Rendered(content)
            }
            Element::Names(ref ns) => IR::Names(ns, fmt.plain("names first-pass")),
            Element::Group(ref f, ref d, ref els) => sequence(fmt, refr, f, d, els.as_ref()),
            Element::Date(ref dt) => {
                dt.intermediate(fmt, refr)
                // IR::YearSuffix(YearSuffixHook::Date(dt.clone()), fmt.plain("date"))
            }
        }
    }
}

fn sequence<'s, 'r, O>(
    fmt: &O,
    refr: &Reference<'r>,
    f: &Formatting,
    delim: &Delimiter,
    els: &'s [Element],
) -> IR<'s, O>
where
    O: OutputFormat,
{
    let mut dedup = vec![];
    let mut dups = vec![];
    for el in els.iter() {
        let pr = el.intermediate(fmt, refr);
        if let Rendered(Some(r)) = pr {
            dups.push(r);
        } else if let Rendered(None) = pr {
        } else {
            if !dups.is_empty() {
                let r = Rendered(Some(fmt.group(&dups, &delim.0, &f)));
                dedup.push(r);
                dups.clear();
            }
            dedup.push(pr);
        }
    }
    if !dups.is_empty() {
        let r = Rendered(Some(fmt.group(&dups, &delim.0, &f)));
        dedup.push(r);
        dups.clear();
    }
    if dedup.len() == 1 {
        return dedup.into_iter().nth(0).unwrap();
    }
    Seq(dedup)
}

impl<'s> Proc<'s> for DateEl {
    #[cfg_attr(feature = "flame_it", flame)]
    fn intermediate<'r, O>(&'s self, fmt: &O, refr: &Reference<'r>) -> IR<'s, O>
    where
        O: OutputFormat,
    {
        let content = refr
            .date
            .get(&self.variable)
            .and_then(|val| {
                if let DateOrRange::Single(d) = val {
                    Some(d)
                } else {
                    None
                }
            })
            .map(|val| {
                let string = format!("{}-{}-{}", val.year, val.month, val.day);
                fmt.group(
                    &[
                        fmt.plain(&self.affixes.prefix),
                        fmt.text_node(&string, &self.formatting),
                        fmt.plain(&self.affixes.suffix),
                    ],
                    "",
                    &Formatting::default(),
                )
            });
        IR::Rendered(content)
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
