use crate::output::OutputFormat;
use crate::style::element::{Choose as ChooseEl, Date as DateEl, Formatting, Names as NamesEl};

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
    ConditionalDisamb(&'s ChooseEl, Box<IR<'s, O>>),
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
            IR::Rendered(None) => fmt.plain(""),
            IR::Rendered(Some(ref x)) => x.clone(),
            IR::Names(_, ref x) => x.clone(),
            IR::ConditionalDisamb(_, ref xs) => xs.clone().flatten(fmt),
            IR::YearSuffix(_, ref x) => x.clone(),
            IR::Seq(ref xs) => seq(xs),
        }
    }
}
