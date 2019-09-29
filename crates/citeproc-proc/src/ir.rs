// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::disamb::Nfa;
use crate::prelude::*;
use citeproc_io::output::markup::Markup;
use csl::style::{Affixes, BodyDate, Choose, Element, Formatting, GivenNameDisambiguationRule};
use csl::Atom;

use std::sync::Arc;

pub type IrSum<O> = (IR<O>, GroupVars);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DisambPass {
    AddNames,
    AddGivenName(GivenNameDisambiguationRule),
    AddYearSuffix(u32),
    Conditionals,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum YearSuffixHook {
    Date(Arc<BodyDate>),
    // Clone element into here, because we already know it's a <text variable="" />
    // And it's cheap to clone those
    Explicit(Element),
}

impl Eq for RefIR {}
impl PartialEq for RefIR {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum RefIR {
    /// A piece of output that a cite can match in the final DFA.
    /// e.g.
    ///
    /// ```txt
    /// EdgeData::Output(r#"<span style="font-weight: bold;">"#)
    /// EdgeData::Output("Some title, <i>23/4/1969</i>")
    /// EdgeData::Locator
    /// ```
    ///
    /// Each is interned into an `Edge` newtype referencing the salsa database.
    Edge(Option<Edge>),

    /// When constructing RefIR, we know whether the names variables exist or not.
    /// So we don't have to handle 'substitute' any special way -- just drill down into the
    /// names element, apply its formatting, and end up with
    ///
    /// ```txt
    /// [
    ///     Edge("<whatever formatting>"),
    ///     // whatever the substitute element outputted
    ///     Edge("</whatever>")
    /// ]
    /// ```
    ///
    /// The Nfa represents all the token streams that the Names block can output.
    Names(Nfa, Box<RefIR>),

    /// A non-string EdgeData can be surrounded by a Seq with other strings to apply its
    /// formatting. This will use `OutputFormat::stack_preorder() / ::stack_postorder()`.
    ///
    /// ```txt
    /// RefIR::Seq(vec![
    ///     EdgeData::Output("<i>"),
    ///     EdgeData::Locator,
    ///     EdgeData::Output("</i>"),
    /// ])
    /// ```
    Seq(RefIrSeq),
    // Could use this to apply a FreeCond set to a reference to create a path through the
    // constructed NFA.
    // See the module level documentation for `disamb`.
    // Branch(Arc<Conditions>, Box<IR<O>>),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RefIrSeq {
    pub contents: Vec<RefIR>,
    pub formatting: Option<Formatting>,
    pub affixes: Affixes,
    pub delimiter: Atom,
}

impl RefIR {
    pub fn debug(&self, db: &impl IrDatabase) -> String {
        match self {
            RefIR::Edge(Some(e)) => format!("{:?}", db.lookup_edge(*e)),
            RefIR::Edge(None) => "None".into(),
            RefIR::Seq(seq) => {
                let mut s = String::new();
                for x in &seq.contents {
                    s.push_str(&x.debug(db));
                }
                s
            }
            RefIR::Names(_nfa, ir) => ir.debug(db),
        }
    }
}

/// A version of [`EdgeData`] that has a piece of output for every
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CiteEdgeData<O: OutputFormat = Markup> {
    Output(O::Build),
    Locator(O::Build),
    LocatorLabel(O::Build),
    YearSuffix(O::Build),
    CitationNumber(O::Build),
    Frnn(O::Build),
}

use csl::variables::{NumberVariable, StandardVariable, Variable};
impl<O: OutputFormat> CiteEdgeData<O> {
    pub fn from_number_variable(var: NumberVariable) -> fn(O::Build) -> Self {
        match var {
            NumberVariable::Locator => CiteEdgeData::Locator,
            NumberVariable::FirstReferenceNoteNumber => CiteEdgeData::Frnn,
            NumberVariable::CitationNumber => CiteEdgeData::CitationNumber,
            _ => CiteEdgeData::Output,
        }
    }
    pub fn from_ordinary_variable(var: Variable) -> fn(O::Build) -> Self {
        match var {
            Variable::YearSuffix => CiteEdgeData::YearSuffix,
            _ => CiteEdgeData::Output,
        }
    }
    pub fn from_standard_variable(var: StandardVariable) -> fn(O::Build) -> Self {
        match var {
            StandardVariable::Number(nv) => CiteEdgeData::from_number_variable(nv),
            StandardVariable::Ordinary(v) => CiteEdgeData::from_ordinary_variable(v),
        }
    }
}

use crate::disamb::names::NamesIR;

// Intermediate Representation
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum IR<O: OutputFormat = Markup> {
    // no (further) disambiguation possible
    Rendered(Option<CiteEdgeData<O>>),
    // the name block,
    // the current render
    Names(NamesIR<O::Build>, Box<IR<O>>),

    /// a single <if disambiguate="true"> being tested once means the whole <choose> is re-rendered in step 4
    /// or <choose><if><conditions><condition>
    /// Should also include `if variable="year-suffix"` because that could change.
    ConditionalDisamb(Arc<Choose>, Box<IR<O>>),
    YearSuffix(YearSuffixHook, Option<O::Build>),

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
    Seq(IrSeq<O>),
}

use std::mem;

impl IR<Markup> {
    pub fn disambiguate<'c>(
        &mut self,
        db: &impl IrDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, Markup>,
    ) -> bool {
        info!(
            "attempting to disambiguate {:?} with {:?}",
            ctx.cite_id, ctx.disamb_pass
        );
        let mut ret = false;
        *self = match self {
            IR::Rendered(_) => {
                return ret;
            }
            IR::Names(ref mut names_ir, ref _x) => {
                // TODO: re-eval again until names are exhausted
                // i.e. return true until then
                names_ir.crank(ctx.disamb_pass);
                let (new_ir, _) = names_ir.intermediate(db, state, ctx);
                IR::Names(mem::replace(names_ir, NamesIR::default()), Box::new(new_ir))
            }
            IR::ConditionalDisamb(ref el, ref _xs) => {
                if let Some(DisambPass::Conditionals) = ctx.disamb_pass {
                    let (new_ir, _) = el.intermediate(db, state, ctx);
                    new_ir
                } else {
                    return ret;
                }
            }
            IR::YearSuffix(ref ysh, ref _x) => {
                // TODO: save GroupVars state in IrSeq so a Group with a year-suffix in
                // it can do normal group suppression
                if let Some(DisambPass::AddYearSuffix(_)) = ctx.disamb_pass {
                    if let YearSuffixHook::Explicit(ref el) = ysh {
                        let (new_ir, _gv) = el.intermediate(db, state, ctx);
                        new_ir
                    } else {
                        warn!("YearSuffixHook::Date not implemented");
                        return ret;
                    }
                } else {
                    return ret;
                }
            }
            IR::Seq(ref mut seq) => {
                ret = seq
                    .contents
                    .iter_mut()
                    .any(|ir| ir.disambiguate(db, state, ctx));
                return ret;
            }
        };
        ret
    }

    pub fn flatten(&self, fmt: &Markup) -> Option<<Markup as OutputFormat>::Build> {
        // must clone
        match self {
            IR::Rendered(None) => None,
            IR::Rendered(Some(ref x)) => Some(x.inner()),
            IR::Names(_, ref x) => (*x).flatten(fmt),
            IR::ConditionalDisamb(_, ref xs) => (*xs).flatten(fmt),
            IR::YearSuffix(_, ref x) => x.clone(),
            IR::Seq(ref seq) => seq.flatten_seq(fmt),
        }
    }

    fn append_edges(&self, edges: &mut Vec<EdgeData>, fmt: &Markup, formatting: Formatting) {
        match self {
            IR::Rendered(None) => {}
            IR::Rendered(Some(ed)) => edges.push(ed.to_edge_data(fmt, formatting)),
            // TODO: reshape year suffixes to contain IR with maybe a CiteEdgeData::YearSuffix
            // inside
            IR::YearSuffix(_hook, x) => {
                let out = x
                    .as_ref()
                    .map(|x| fmt.output_in_context(x.clone(), formatting));
                if let Some(o) = out {
                    if o.len() > 0 {
                        edges.push(EdgeData::Output(o))
                    }
                }
            }
            IR::ConditionalDisamb(_, xs) => (*xs).append_edges(edges, fmt, formatting),
            IR::Seq(seq) => seq.append_edges(edges, fmt, formatting),
            IR::Names(_names_ir, r) => r.append_edges(edges, fmt, formatting),
        }
        ()
    }

    pub fn to_edge_stream(&self, fmt: &Markup) -> Vec<EdgeData> {
        let mut edges = Vec::new();
        self.append_edges(&mut edges, fmt, Formatting::default());
        edges
    }
}

// impl<'a> From<&'a CiteEdgeData> for EdgeData {
//     fn from(cite_edge: &CiteEdgeData) -> Self {
//         match cite_edge {
//             CiteEdgeData::Output(x) => EdgeData::Output(x.clone()),
//             CiteEdgeData::YearSuffix(_) => EdgeData::YearSuffix,
//             CiteEdgeData::Frnn(_) => EdgeData::Frnn,
//             CiteEdgeData::Locator(_) => EdgeData::Locator,
//             CiteEdgeData::CitationNumber(_) => EdgeData::CitationNumber,
//         }
//     }
// }

impl CiteEdgeData<Markup> {
    fn to_edge_data(&self, fmt: &Markup, formatting: Formatting) -> EdgeData {
        match self {
            CiteEdgeData::Output(x) => {
                EdgeData::Output(fmt.output_in_context(x.clone(), formatting))
            }
            CiteEdgeData::YearSuffix(_) => EdgeData::YearSuffix,
            CiteEdgeData::Frnn(_) => EdgeData::Frnn,
            CiteEdgeData::Locator(_) => EdgeData::Locator,
            CiteEdgeData::LocatorLabel(_) => EdgeData::LocatorLabel,
            CiteEdgeData::CitationNumber(_) => EdgeData::CitationNumber,
        }
    }
    fn inner(&self) -> <Markup as OutputFormat>::Build {
        match self {
            CiteEdgeData::Output(x)
            | CiteEdgeData::YearSuffix(x)
            | CiteEdgeData::Frnn(x)
            | CiteEdgeData::Locator(x)
            | CiteEdgeData::LocatorLabel(x)
            | CiteEdgeData::CitationNumber(x) => x.clone(),
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

impl IrSeq<Markup> {
    fn append_edges(&self, edges: &mut Vec<EdgeData>, fmt: &Markup, formatting: Formatting) {
        if self.contents.len() == 0 {
            return;
        }
        let stack = fmt.tag_stack(self.formatting.unwrap_or_else(Default::default));
        let sub_formatting = self
            .formatting
            .map(|mine| formatting.override_with(mine))
            .unwrap_or(formatting);
        let mut open_tags = String::new();
        let mut close_tags = String::new();
        fmt.stack_preorder(&mut open_tags, &stack);
        fmt.stack_postorder(&mut close_tags, &stack);
        if open_tags.len() > 0 {
            edges.push(EdgeData::Output(open_tags));
        }
        // push the innards
        let _len = self.contents.len();
        let mut seen = false;
        let mut sub = Vec::new();
        for (_n, ir) in self.contents.iter().enumerate() {
            ir.append_edges(&mut sub, fmt, sub_formatting);
            if sub.len() > 0 {
                if seen {
                    if !self.delimiter.is_empty() {
                        edges.push(EdgeData::Output(fmt.output_in_context(
                            fmt.plain(self.delimiter.as_ref()),
                            sub_formatting,
                        )));
                    }
                } else {
                    seen = true;
                }
                edges.extend(sub.drain(..));
            }
        }
        if close_tags.len() > 0 {
            edges.push(EdgeData::Output(close_tags));
        }
    }

    fn flatten_seq(&self, fmt: &Markup) -> Option<<Markup as OutputFormat>::Build> {
        let xs: Vec<_> = self
            .contents
            .iter()
            .filter_map(|i| i.flatten(fmt))
            .collect();
        if xs.is_empty() {
            return None;
        }
        let grp = fmt.group(xs, &self.delimiter, self.formatting);
        Some(fmt.affixed(grp, &self.affixes))
    }
}
