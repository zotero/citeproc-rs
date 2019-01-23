use super::group::GroupVars;
use crate::output::OutputFormat;
use crate::style::element::{Affixes, BodyDate, Choose, Formatting, Names as NamesEl};

// /// Just exists to make it easier to add other tree-folded summary data.
// /// Even if it's only `GroupVars` for now.
// #[derive(Debug)]
// pub struct Summary(GroupVars);

pub type IrSum<'c, O> = (IR<'c, O>, GroupVars);

#[derive(Debug)]
pub enum YearSuffixHook<'c> {
    Date(&'c BodyDate),
    Explicit(),
}

#[derive(Debug)]
pub struct IrSeq<'c, O: OutputFormat> {
    pub contents: Vec<IR<'c, O>>,
    pub formatting: Option<&'c Formatting>,
    pub affixes: Affixes,
    pub delimiter: &'c str,
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
    // Should also include `if variable="year-suffix"` because that could change.
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
    Seq(IrSeq<'c, O>),
}

impl<'c, O: OutputFormat> IR<'c, O> {
    pub fn flatten(&self, fmt: &O) -> O::Build {
        let flatten_seq = |seq: &IrSeq<'c, O>| {
            let xs: Vec<_> = seq.contents.iter().map(|i| i.flatten(fmt)).collect();
            let grp = fmt.group(xs, seq.delimiter, seq.formatting);
            fmt.affixed(grp, &seq.affixes)
        };
        // must clone
        match self {
            IR::Rendered(None) => fmt.plain(""),
            IR::Rendered(Some(ref x)) => x.clone(),
            IR::Names(_, ref x) => x.clone(),
            IR::ConditionalDisamb(_, ref xs) => (*xs).flatten(fmt),
            IR::YearSuffix(_, ref x) => x.clone(),
            IR::Seq(seq) => flatten_seq(seq),
        }
    }
}
