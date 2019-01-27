use super::group::GroupVars;
use crate::output::OutputFormat;
use crate::style::element::{Affixes, BodyDate, Choose, Formatting, Names as NamesEl};
use crate::Atom;
use std::sync::Arc;

// /// Just exists to make it easier to add other tree-folded summary data.
// /// Even if it's only `GroupVars` for now.
// #[derive(Debug)]
// pub struct Summary(GroupVars);

pub type IrSum<O> = (IR<O>, GroupVars);

#[derive(Debug, PartialEq, Eq)]
pub enum YearSuffixHook {
    Date(Arc<BodyDate>),
    Explicit(),
}

#[derive(Debug, PartialEq, Eq)]
pub struct IrSeq<O: OutputFormat> {
    pub contents: Vec<IR<O>>,
    pub formatting: Option<Formatting>,
    pub affixes: Affixes,
    pub delimiter: Atom,
}

// Intermediate Representation
#[derive(Debug, PartialEq, Eq)]
pub enum IR<O: OutputFormat> {
    // no (further) disambiguation possible
    Rendered(Option<O::Build>),
    // the name block,
    // the current render
    Names(Arc<NamesEl>, O::Build),

    // a single <if disambiguate="true"> being tested once means the whole <choose> is re-rendered in step 4
    // or <choose><if><conditions><condition>
    // Should also include `if variable="year-suffix"` because that could change.
    ConditionalDisamb(Arc<Choose>, Box<IR<O>>),
    YearSuffix(YearSuffixHook, O::Build),

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
    Seq(IrSeq<O>),
}

impl<O: OutputFormat> IR<O> {
    pub fn flatten(&self, fmt: &O) -> O::Build {
        let flatten_seq = |seq: &IrSeq<O>| {
            let xs: Vec<_> = seq.contents.iter().map(|i| i.flatten(fmt)).collect();
            let grp = fmt.group(xs, &seq.delimiter, seq.formatting);
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
