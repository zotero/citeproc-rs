// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::disamb::{Edge, EdgeData};
use crate::prelude::*;
use citeproc_io::output::html::Html;
use csl::style::{
    Affixes, BodyDate, Choose, Conditions, Element, Formatting, GivenNameDisambiguationRule,
    Names as NamesEl,
};
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

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefIR<O: OutputFormat = Html> {
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
    Seq(Vec<RefIR<O>>),

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
    Edge(Edge),

    /// We use this to apply a FreeCond set to a reference to create a path through the
    /// constructed NFA.
    /// See the module level documentation for `disamb`.
    Branch(Arc<Conditions>, Box<IR<O>>),

    /// When constructing RefIR, we know whether the names variables exist or not.
    /// So we don't have to handle 'substitute' any special way -- just drill down into the
    /// names element, apply its formatting, and end up with
    ///
    /// ```txt
    /// Seq [
    ///     Edge("<whatever formatting>"),
    ///     // whatever the substitute element outputted
    ///     Edge("</whatever>")
    /// ]
    /// ```
    Names(Arc<NamesEl>, Box<RefIR<O>>),
}

/// A version of [`EdgeData`] that has a piece of output for every
pub enum CiteEdgeData<O: OutputFormat = Html> {
    Output(O::Output),
    Locator(O::Output),
    LocatorLabel(O::Output),
    YearSuffix(O::Output),
    CitationNumber(O::Output),
    BibNumber(O::Output),
    Frnn(O::Output),
}

// Intermediate Representation
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum IR<O: OutputFormat> {
    // no (further) disambiguation possible
    Rendered(Option<CiteEdgeData<O>>),
    // the name block,
    // the current render
    Names(Arc<NamesEl>, O::Build),

    /// a single <if disambiguate="true"> being tested once means the whole <choose> is re-rendered in step 4
    /// or <choose><if><conditions><condition>
    /// Should also include `if variable="year-suffix"` because that could change.
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
    Seq(IrSeq<O>),
}

impl<O: OutputFormat> IR<O> {
    fn is_rendered(&self) -> bool {
        match self {
            IR::Rendered(_) => true,
            _ => false,
        }
    }

    pub fn disambiguate<'c>(
        &mut self,
        db: &impl IrDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O>,
        is_unambig: &impl Fn(&IrState) -> bool,
    ) {
        *self = match self {
            IR::Rendered(_) => {
                return;
            }
            IR::Names(ref el, ref _x) => {
                // TODO: re-eval again until names are exhausted
                let (new_ir, _) = el.intermediate(state, ctx);
                new_ir
            }
            IR::ConditionalDisamb(ref el, ref _xs) => {
                let (new_ir, _) = el.intermediate(state, ctx);
                new_ir
            }
            IR::YearSuffix(ref ysh, ref _x) => {
                // TODO: save GroupVars state in IrSeq so a Group with a year-suffix in
                // it can do normal group suppression
                if let YearSuffixHook::Explicit(ref el) = ysh {
                    let (new_ir, _) = el.intermediate(state, ctx);
                    new_ir
                } else {
                    // not implemented
                    return;
                }
            }
            IR::Seq(ref mut seq) => {
                for ir in seq.contents.iter_mut() {
                    ir.disambiguate(db, state, ctx, is_unambig);
                }
                if seq.contents.iter().all(|ir| ir.is_rendered()) {
                    IR::Rendered(seq.flatten_seq(&ctx.format))
                } else {
                    return;
                }
            }
        }
    }

    pub fn flatten(&self, fmt: &O) -> Option<O::Build> {
        // must clone
        match self {
            IR::Rendered(None) => None,
            IR::Rendered(Some(ref x)) => Some(x.clone()),
            IR::Names(_, ref x) => Some(x.clone()),
            IR::ConditionalDisamb(_, ref xs) => (*xs).flatten(fmt),
            IR::YearSuffix(_, ref x) => Some(x.clone()),
            IR::Seq(ref seq) => seq.flatten_seq(fmt),
        }
    }

    fn append_edges(&self, edges: &mut Vec<EdgeData>, fmt: &O, formatting: Formatting) {
        match self {
            IR::Rendered(None) => {}
            IR::Rendered(Some(ed)) => edges.push(ed.into()),
            // TODO: reshape year suffixes to contain IR with maybe a CiteEdgeData::YearSuffix
            // inside
            IR::YearSuffix(_hook, x) => edges.push(EdgeData::Output(x.clone())),
            IR::ConditionalDisamb(_, xs) => (*xs).append_edges(edges, fmt, formatting),
            IR::Seq(seq) => seq.append_edges(edges, fmt, formatting),
            IR::Names(_names, r) => edges.push(EdgeData::Output(r.clone())),
        }
    }

    pub fn to_edge_stream(&self, fmt: &O) -> Vec<EdgeData> {
        let mut edges = Vec::new();
    }
}

impl<'a> From<&'a CiteEdgeData> for EdgeData {
    fn from(cite_edge: &CiteEdgeData) -> Self {
        match cite_edge {
            CiteEdgeData::Output(x) => EdgeData::Output(x.clone()),
            CiteEdgeData::YearSuffix(_) => EdgeData::YearSuffix,
            CiteEdgeData::Frnn(_) => EdgeData::Frnn,
            CiteEdgeData::Locator(_) => EdgeData::Locator,
            CiteEdgeData::BibNumber(_) => EdgeData::BibNumber,
            CiteEdgeData::CitationNumber(_) => EdgeData::CitationNumber,
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

use citeproc_io::output::FormatCmd;

impl<O: OutputFormat> IrSeq<O> {
    fn append_edges(&self, edges: &mut EdgeData, fmt: &O, formatting: Formatting) -> Vec<EdgeData> {
        let stack = fmt.tag_stack();
        let sub_formatting = self
            .formatting
            .map(|mine| formatting.override_with(mine))
            .unwrap_or(formatting);
        let mut open_tags = String::new();
        let mut close_tags = String::new();
        fmt.stack_preorder(&mut open_tags, stack);
        fmt.stack_postorder(&mut close_tags, stack);
        edges.push(EdgeData::Output(open_tags));
        // push the innards
        let len = self.contents.len();
        for (n, ir) in self.contents.iter().enumerate() {
            ir.append_edges(&mut edges, fmt, sub_formatting);
            if n != len {
                edges.push(EdgeData::Output(fmt.plain()))
            }
        }
        edges.push(EdgeData::Output(close_tags));
        edges
    }

    fn flatten_seq(&self, fmt: &O) -> Option<O::Build> {
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
