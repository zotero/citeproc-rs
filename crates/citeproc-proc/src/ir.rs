// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::disamb::{Edge, EdgeData, Nfa};
use crate::prelude::*;
use citeproc_io::output::html::Html;
use csl::style::{
    Affixes, BodyDate, Choose, Conditions, Element, Formatting, GivenNameDisambiguationRule,
    Names as NamesEl,
};
use csl::Atom;
use petgraph::graph::NodeIndex;
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
    Names(Nfa, NodeIndex, NodeIndex, Box<RefIR>),

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
    Seq(Vec<RefIR>),
    // Could use this to apply a FreeCond set to a reference to create a path through the
    // constructed NFA.
    // See the module level documentation for `disamb`.
    // Branch(Arc<Conditions>, Box<IR<O>>),
}

use std::fmt::{self, Debug, Formatter};

impl RefIR {
    pub fn debug(&self, db: &impl IrDatabase) -> String {
        match self {
            RefIR::Edge(Some(e)) => format!("{:?}", db.lookup_edge(*e)),
            RefIR::Edge(None) => "None".into(),
            RefIR::Seq(seq) => {
                let mut s = String::new();
                for x in seq {
                    s.push_str(&x.debug(db));
                }
                s
            }
            RefIR::Names(_nfa, _start, _end, ir) => ir.debug(db),
        }
    }
}

/// A version of [`EdgeData`] that has a piece of output for every
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CiteEdgeData<O: OutputFormat = Html> {
    Output(O::Build),
    Locator(O::Build),
    LocatorLabel(O::Build),
    YearSuffix(O::Build),
    CitationNumber(O::Build),
    BibNumber(O::Build),
    Frnn(O::Build),
}

// Intermediate Representation
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum IR<O: OutputFormat = Html> {
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

impl IR<Html> {
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
        ctx: &CiteContext<'c, Html>,
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
                    IR::Rendered(seq.flatten_seq(&ctx.format).map(CiteEdgeData::Output))
                } else {
                    return;
                }
            }
        }
    }

    pub fn flatten(&self, fmt: &Html) -> Option<<Html as OutputFormat>::Build> {
        // must clone
        match self {
            IR::Rendered(None) => None,
            IR::Rendered(Some(ref x)) => Some(x.inner()),
            IR::Names(_, ref x) => Some(x.clone()),
            IR::ConditionalDisamb(_, ref xs) => (*xs).flatten(fmt),
            IR::YearSuffix(_, ref x) => Some(x.clone()),
            IR::Seq(ref seq) => seq.flatten_seq(fmt),
        }
    }

    fn append_edges(&self, edges: &mut Vec<EdgeData>, fmt: &Html, formatting: Formatting) {
        match self {
            IR::Rendered(None) => {}
            IR::Rendered(Some(ed)) => edges.push(ed.to_edge_data(fmt, formatting)),
            // TODO: reshape year suffixes to contain IR with maybe a CiteEdgeData::YearSuffix
            // inside
            IR::YearSuffix(_hook, x) => edges.push(EdgeData::Output(
                fmt.output_in_context(x.clone(), formatting),
            )),
            IR::ConditionalDisamb(_, xs) => (*xs).append_edges(edges, fmt, formatting),
            IR::Seq(seq) => seq.append_edges(edges, fmt, formatting),
            IR::Names(_names, r) => edges.push(EdgeData::Output(
                fmt.output_in_context(r.clone(), formatting),
            )),
        }
        ()
    }

    pub fn to_edge_stream(&self, fmt: &Html) -> Vec<EdgeData> {
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
//             CiteEdgeData::BibNumber(_) => EdgeData::BibNumber,
//             CiteEdgeData::CitationNumber(_) => EdgeData::CitationNumber,
//         }
//     }
// }

impl CiteEdgeData<Html> {
    fn to_edge_data(&self, fmt: &Html, formatting: Formatting) -> EdgeData {
        match self {
            CiteEdgeData::Output(x) => {
                EdgeData::Output(fmt.output_in_context(x.clone(), formatting))
            }
            CiteEdgeData::YearSuffix(_) => EdgeData::YearSuffix,
            CiteEdgeData::Frnn(_) => EdgeData::Frnn,
            CiteEdgeData::Locator(_) => EdgeData::Locator,
            CiteEdgeData::LocatorLabel(_) => EdgeData::LocatorLabel,
            CiteEdgeData::BibNumber(_) => EdgeData::BibNumber,
            CiteEdgeData::CitationNumber(_) => EdgeData::CitationNumber,
        }
    }
    fn inner(&self) -> <Html as OutputFormat>::Build {
        match self {
            CiteEdgeData::Output(x)
            | CiteEdgeData::YearSuffix(x)
            | CiteEdgeData::Frnn(x)
            | CiteEdgeData::Locator(x)
            | CiteEdgeData::LocatorLabel(x)
            | CiteEdgeData::BibNumber(x)
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

impl IrSeq<Html> {
    fn append_edges(&self, edges: &mut Vec<EdgeData>, fmt: &Html, formatting: Formatting) {
        let stack = fmt.tag_stack(self.formatting.unwrap_or_else(Default::default));
        let sub_formatting = self
            .formatting
            .map(|mine| formatting.override_with(mine))
            .unwrap_or(formatting);
        let mut open_tags = String::new();
        let mut close_tags = String::new();
        fmt.stack_preorder(&mut open_tags, &stack);
        fmt.stack_postorder(&mut close_tags, &stack);
        edges.push(EdgeData::Output(open_tags));
        // push the innards
        let len = self.contents.len();
        for (n, ir) in self.contents.iter().enumerate() {
            ir.append_edges(edges, fmt, sub_formatting);
            if n != len {
                edges.push(EdgeData::Output(fmt.output_in_context(
                    fmt.plain(self.delimiter.as_ref()),
                    sub_formatting,
                )))
            }
        }
        edges.push(EdgeData::Output(close_tags));
    }

    fn flatten_seq(&self, fmt: &Html) -> Option<<Html as OutputFormat>::Build> {
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
