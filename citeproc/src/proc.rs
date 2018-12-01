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
    Rendered(T),
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

#[cfg_attr(feature = "flame_it", flame)]
pub fn proc_intermediate<T: Debug, O: Serialize>(
    style: &Style,
    fmt: &impl OutputFormat<T, O>,
) -> Intermediate<T> {
    let citation = &style.citation;
    let layout = &citation.layout;
    layout.proc_intermediate(fmt)
}

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

trait Proc {
    // TODO: include settings and reference and macro map
    fn proc_intermediate<T: Debug, O: Serialize>(&self, fmt: &impl OutputFormat<T, O>)
        -> Intermediate<T>;
}

// TODO: insert affixes into group before processing as a group
impl Proc for LayoutEl {
    #[cfg_attr(feature = "flame_it", flame)]
    fn proc_intermediate<T: Debug, O: Serialize>(
        &self,
        fmt: &impl OutputFormat<T, O>,
    ) -> Intermediate<T> {
        let f = &self.formatting;
        let _af = &self.affixes;
        let d = &self.delimiter;
        let els = &self.elements;
        let mut dedup = vec![];
        let mut dups = vec![];
        for el in els.into_iter() {
            let pr = el.proc_intermediate(fmt);
            if let Rendered(r) = pr {
                dups.push(r);
            } else {
                if !dups.is_empty() {
                    let r = Rendered(fmt.group(&dups, &d.0, &f));
                    dedup.push(r);
                    dups.clear();
                }
                dedup.push(pr);
            }
        }
        if !dups.is_empty() {
            let r = Rendered(fmt.group(&dups, &d.0, &f));
            dedup.push(r);
            dups.clear();
        }
        if dedup.len() == 1 {
            return dedup.into_iter().nth(0).unwrap();
        }
        Seq(dedup)
    }
}

impl Proc for Element {
    #[cfg_attr(feature = "flame_it", flame)]
    fn proc_intermediate<T: Debug, O: Serialize>(
        &self,
        fmt: &impl OutputFormat<T, O>,
    ) -> Intermediate<T> {
        let null_f = Formatting::default();
        match *self {
            Element::Choose(ref _ch) => {
                // TODO: work out if disambiguate appears on the conditions
                Rendered(fmt.plain("choose"))
            }
            Element::Macro(ref name, ref f, ref _af, ref _quo) => {
                Rendered(fmt.text_node(&format!("(macro {})", name), &f))
            }
            Element::Const(ref val, ref f, ref af, ref _quo) => Intermediate::Rendered(fmt.group(
                &[
                    fmt.plain(&af.prefix),
                    fmt.text_node(&val, &f),
                    fmt.plain(&af.suffix),
                ],
                "",
                &null_f,
            )),
            Element::Variable(ref var, ref f, ref af, ref _form, ref _del, ref _quo) => {
                Intermediate::Rendered(fmt.group(
                    &[
                        fmt.plain(&af.prefix),
                        fmt.text_node(&format!("(var {})", var.as_ref()), &f),
                        fmt.plain(&af.suffix),
                    ],
                    "",
                    &null_f,
                ))
            }
            Element::Term(ref term, ref _form, ref f, ref af, ref _pl) => {
                Intermediate::Rendered(fmt.group(
                    &[
                        fmt.plain(&af.prefix),
                        fmt.text_node(&format!("(term {})", term), &f),
                        fmt.plain(&af.suffix),
                    ],
                    "",
                    &null_f,
                ))
            }
            Element::Label(ref var, ref _form, ref f, ref af, ref _pl) => {
                Intermediate::Rendered(fmt.group(
                    &[
                        fmt.plain(&af.prefix),
                        fmt.text_node(&format!("(label {})", var.as_ref()), &f),
                        fmt.plain(&af.suffix),
                    ],
                    "",
                    &null_f,
                ))
            }
            Element::Number(ref var, ref _form, ref f, ref af, ref _pl) => {
                Intermediate::Rendered(fmt.group(
                    &[
                        fmt.plain(&af.prefix),
                        fmt.text_node(&format!("(num {})", var.as_ref()), &f),
                        fmt.plain(&af.suffix),
                    ],
                    "",
                    &null_f,
                ))
            }
            Element::Names(ref ns) => {
                Intermediate::Names(ns.clone(), fmt.plain("names first-pass"))
            }
            Element::Group(ref f, ref d, ref els) => {
                let mut dedup = vec![];
                let mut dups = vec![];
                for el in els.into_iter() {
                    let pr = el.proc_intermediate(fmt);
                    if let Rendered(r) = pr {
                        dups.push(r);
                    } else {
                        if !dups.is_empty() {
                            let r = Rendered(fmt.group(&dups, &d.0, &f));
                            dedup.push(r);
                            dups.clear();
                        }
                        dedup.push(pr);
                    }
                }
                if !dups.is_empty() {
                    let r = Rendered(fmt.group(&dups, &d.0, &f));
                    dedup.push(r);
                    dups.clear();
                }
                if dedup.len() == 1 {
                    return dedup.into_iter().nth(0).unwrap();
                }
                Seq(dedup)
            }
            Element::Date(ref dt) => {
                Intermediate::YearSuffix(YearSuffixHook::Date(dt.clone()), fmt.plain("date"))
            }
        }
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
