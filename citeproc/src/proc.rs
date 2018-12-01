use crate::input::Reference;
use crate::input::DateOrRange;
use crate::style::element::Delimiter;
use crate::output::OutputFormat;
use crate::style::element::{
    Choose as ChooseEl, Date as DateEl, Element, Formatting, Layout as LayoutEl, Names as NamesEl,
    Style,
};
use serde::Serialize;
use std::fmt::Debug;
use std::rc::Rc;

#[derive(Debug)]
pub enum YearSuffixHook {
    Date(Rc<DateEl>),
    Explicit(),
}

#[derive(Debug)]
pub enum Intermediate<T>
where
    T: Debug,
{
    // no (further) disambiguation possible
    Rendered(Option<T>),
    // the name block,
    // the current render
    Names(Rc<NamesEl>, T),
    // a single <if disambiguate="true"> means the whole <choose> is re-rendered in step 4
    // or <choose><if><conditions><condition>
    // the current render
    ConditionalDisamb(Rc<ChooseEl>, Vec<Intermediate<T>>),
    YearSuffix(YearSuffixHook, T),

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
    Seq(Vec<Intermediate<T>>),
}

use self::Intermediate::*;

// TODO: function to walk the entire tree for a <text variable="year-suffix"> to work out which
// nodes are possibly disambiguate-able in year suffix mode and if such a node should be inserted
// at the end of the layout block before the suffix.
// TODO: also to figure out which macros are needed
// TODO: juris-m module loading in advance? probably in advance.

// Levels 1-3 will also have to update the ConditionalDisamb's current render

fn _disamb_1() {
    unimplemented!()
}

fn _disamb_2() {
    unimplemented!()
}

fn _disamb_3() {
    unimplemented!()
}

fn _disamb_4() {
    unimplemented!()
}

pub trait Proc {
    // TODO: include settings and reference and macro map
    fn proc_intermediate<'r, T: Debug, O: Serialize>(
        &self,
        fmt: &impl OutputFormat<T, O>,
        refr: &Reference<'r>
    ) -> Intermediate<T>;
}

#[cfg_attr(feature = "flame_it", flame)]
impl Proc for Style {
    fn proc_intermediate<'r, T: Debug, O: Serialize>(
        &self,
        fmt: &impl OutputFormat<T, O>,
        refr: &Reference<'r>,
        ) -> Intermediate<T> {
        let citation = &self.citation;
        let layout = &citation.layout;
        layout.proc_intermediate(fmt, refr)
    }
}

// TODO: insert affixes into group before processing as a group
impl Proc for LayoutEl {
    #[cfg_attr(feature = "flame_it", flame)]
    fn proc_intermediate<'r, T: Debug, O: Serialize>(
        &self,
        fmt: &impl OutputFormat<T, O>,
        refr: &Reference<'r>
    ) -> Intermediate<T> {
        sequence(fmt, refr, &self.formatting, &self.delimiter, self.elements.as_ref())
    }
}

impl Proc for Element {
    #[cfg_attr(feature = "flame_it", flame)]
    fn proc_intermediate<'r, T: Debug, O: Serialize>(
        &self,
        fmt: &impl OutputFormat<T, O>,
        refr: &Reference<'r>,
    ) -> Intermediate<T> {
        let null_f = Formatting::default();
        match *self {
            Element::Choose(ref _ch) => {
                // TODO: work out if disambiguate appears on the conditions
                Rendered(Some(fmt.plain("choose")))
            }
            Element::Macro(ref name, ref f, ref _af, ref _quo) => {
                Rendered(Some(fmt.text_node(&format!("(macro {})", name), &f)))
            }
            Element::Const(ref val, ref f, ref af, ref _quo) =>
                Intermediate::Rendered(Some(fmt.group(
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
                } else { None };
                Intermediate::Rendered(content)
            }

            Element::Term(ref term, ref _form, ref f, ref af, ref _pl) => {
                Intermediate::Rendered(Some(fmt.group(
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
                Intermediate::Rendered(Some(fmt.group(
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
                } else { None };
                Intermediate::Rendered(content)
            }
            Element::Names(ref ns) => {
                Intermediate::Names(ns.clone(), fmt.plain("names first-pass"))
            }
            Element::Group(ref f, ref d, ref els) => {
                sequence(fmt, refr, f, d, els.as_ref())
            }
            Element::Date(ref dt) => {
                dt.proc_intermediate(fmt, refr)
                // Intermediate::YearSuffix(YearSuffixHook::Date(dt.clone()), fmt.plain("date"))
            }
        }
    }
}

fn sequence<'r, T: Debug, O: Serialize>(
    fmt: &impl OutputFormat<T, O>,
    refr: &Reference<'r>,
    f: &Formatting,
    delim: &Delimiter,
    els: &[Rc<Element>]
) -> Intermediate<T> {

    let mut dedup = vec![];
    let mut dups = vec![];
    for el in els.iter() {
        let pr = el.proc_intermediate(fmt, refr);
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

impl Proc for DateEl {
    #[cfg_attr(feature = "flame_it", flame)]
    fn proc_intermediate<'r, T: Debug, O: Serialize>(
        &self,
        fmt: &impl OutputFormat<T, O>,
        refr: &Reference<'r>,
    ) -> Intermediate<T> {
        let content = refr.date
            .get(&self.variable)
            .and_then(|val| {
                if let DateOrRange::Single(d) = val {
                    Some(d)
                } else { None }
            })
            .map(|val| {
                let string = format!("{}-{}-{}",  val.year, val.month, val.day);
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
        Intermediate::Rendered(content)
    }
}

#[cfg(all(test, feature = "flame_it"))]
mod test {
    use super::proc_intermediate;
    use crate::output::PlainText;
    use crate::style::build_style;
    use crate::test::Bencher;
    use std::fs::File;
    use std::io::prelude::*;

    #[bench]
    fn bench_intermediate(b: &mut Bencher) {
        let path = "/Users/cormac/git/citeproc-rs/example.csl";
        let mut f = File::open(path).expect("no file at path");
        let mut contents = String::new();
        f.read_to_string(&mut contents)
            .expect("something went wrong reading the file");
        let s = build_style(&contents);
        let fmt = PlainText::new();
        if let Ok(style) = s {
            b.iter(|| {
                proc_intermediate(&style, &fmt);
            });
        }
    }

}
