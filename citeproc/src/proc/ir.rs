use super::group::GroupVars;
use super::{CiteContext, IrState, Proc};
use crate::db::ReferenceDatabase;
use crate::output::OutputFormat;
use crate::style::element::{
    Affixes, BodyDate, Choose, Formatting, GivenNameDisambiguationRule, Names as NamesEl,
};
use crate::Atom;
use std::sync::Arc;

// /// Just exists to make it easier to add other tree-folded summary data.
// /// Even if it's only `GroupVars` for now.
// #[derive(Debug)]
// pub struct Summary(GroupVars);

pub type IrSum<O> = (IR<O>, GroupVars);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReEvaluation {
    AddNames,
    AddGivenName(GivenNameDisambiguationRule),
    AddYearSuffix(u32),
    Conditionals,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum YearSuffixHook {
    Date(Arc<BodyDate>),
    Explicit(/* XXX: clone a text node into here */),
}

// Intermediate Representation
#[derive(Debug, PartialEq, Eq, Clone)]
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
    fn is_rendered(&self) -> bool {
        match self {
            IR::Rendered(_) => true,
            _ => false,
        }
    }

    pub fn flatten(&self, fmt: &O) -> O::Build {
        // must clone
        match self {
            IR::Rendered(None) => fmt.plain(""),
            IR::Rendered(Some(ref x)) => x.clone(),
            IR::Names(_, ref x) => x.clone(),
            IR::ConditionalDisamb(_, ref xs) => (*xs).flatten(fmt),
            IR::YearSuffix(_, ref x) => x.clone(),
            IR::Seq(ref seq) => seq.flatten_seq(fmt),
        }
    }

    pub fn re_evaluate<'c>(
        &mut self,
        db: &impl ReferenceDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O>,
        is_unambig: &impl Fn(&IrState) -> bool,
    ) {
        use std::mem;
        *self = match self {
            IR::Rendered(_) => {
                return;
            }
            IR::Names(ref el, ref _x) => {
                // TODO: re-eval again until names are exhausted
                let (new_ir, _) = el.intermediate(db, state, ctx);
                mem::replace(self, new_ir)
            }
            IR::ConditionalDisamb(ref el, ref _xs) => {
                let (new_ir, _) = el.intermediate(db, state, ctx);
                mem::replace(self, new_ir)
            }
            IR::YearSuffix(ref _el, ref _x) => {
                // XXX: implement
                return;
                // let (new_ir, _) = el.intermediate(db, state, ctx);
                // mem::replace(self, new_ir);
            }
            IR::Seq(ref mut seq) => {
                for ir in seq.contents.iter_mut() {
                    ir.re_evaluate(db, state, ctx, is_unambig);
                }
                if seq.contents.iter().all(|ir| ir.is_rendered()) {
                    let new_ir = IR::Rendered(Some(seq.flatten_seq(&ctx.format)));
                    mem::replace(self, new_ir)
                } else {
                    return;
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct IrSeq<O: OutputFormat> {
    pub contents: Vec<IR<O>>,
    pub formatting: Option<Formatting>,
    pub affixes: Affixes,
    pub delimiter: Atom,
}

impl<O: OutputFormat> IrSeq<O> {
    fn flatten_seq(&self, fmt: &O) -> O::Build {
        let xs: Vec<_> = self.contents.iter().map(|i| i.flatten(fmt)).collect();
        let grp = fmt.group(xs, &self.delimiter, self.formatting);
        fmt.affixed(grp, &self.affixes)
    }
}
