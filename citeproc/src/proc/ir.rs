use crate::output::OutputFormat;
use crate::style::element::{Choose, IndependentDate, Names as NamesEl};

#[derive(Debug)]
pub enum YearSuffixHook<'c> {
    Date(&'c IndependentDate),
    Explicit(),
}

// Intermediate Representation
#[derive(Debug)]
pub enum IR<'c, O: OutputFormat> {
    // no (further) disambiguation possible
    Rendered(Option<O::Build>),
    // the name block,
    // the current render
    Names(&'c NamesEl, O::Build),

    // a single <if disambiguate="true"> being tested once means the whole <choose> is re-rendered in step 4
    // or <choose><if><conditions><condition>
    ConditionalDisamb(&'c Choose, Box<IR<'c, O>>),
    YearSuffix(YearSuffixHook<'c>, O::Build),

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
    Seq(Vec<IR<'c, O>>),
}

impl<'c, O: OutputFormat> IR<'c, O> {
    pub fn flatten(&self, fmt: &O) -> O::Build {
        // TODO: change fmt.group to accept iterators instead
        let seq = |xs: &[IR<'c, O>]| {
            let v: Vec<O::Build> = xs.iter().map(|i| i.flatten(fmt)).collect();
            fmt.group(&v, "", None)
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
