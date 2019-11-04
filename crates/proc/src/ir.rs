// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::disamb::Nfa;
use crate::prelude::*;
use citeproc_io::output::markup::Markup;
use csl::Atom;
use csl::{Affixes, BodyDate, Choose, Element, Formatting, GivenNameDisambiguationRule};
use csl::{NumberVariable, StandardVariable, Variable};

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
    Explicit(Element),
    Plain,
}

impl Eq for RefIR {}
impl PartialEq for RefIR {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

use crate::disamb::names::RefNameIR;

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
    /// The Nfa represents all the edge streams that a Names block can output for one of its
    /// variables.
    Name(RefNameIR, Nfa),

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
    pub display: Option<DisplayMode>,
}

impl RefIR {
    pub fn debug(&self, db: &impl IrDatabase) -> String {
        match self {
            RefIR::Edge(Some(e)) => format!("{:?}", db.lookup_edge(*e)),
            RefIR::Edge(None) => "None".into(),
            RefIR::Seq(seq) => {
                let mut s = String::new();
                s.push_str("[");
                let mut seen = false;
                for x in &seq.contents {
                    if seen {
                        s.push_str(",");
                    }
                    seen = true;
                    s.push_str(&x.debug(db));
                }
                s.push_str("]");
                s
            }
            RefIR::Name(rnir, _nfa) => format!("NameVariable::{:?}", rnir.variable),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            RefIR::Edge(None) => true,
            RefIR::Seq(seq) => seq.contents.is_empty(),
            RefIR::Name(_rnir, nfa) => nfa.is_empty(),
            _ => false,
        }
    }

    pub(crate) fn keep_first_ysh(&mut self, ysh_explicit_edge: Edge, ysh_edge: Edge) {
        let found = &mut false;
        self.visit_ysh(ysh_explicit_edge, &mut |opt_e| {
            if !*found {
                // first time
                *found = true;
                *opt_e = Some(ysh_edge);
            } else {
                // subsequent ones are extraneous, so make them disappear
                *opt_e = None;
            }
            false
        });
        self.visit_ysh(ysh_edge, &mut |opt_e| {
            if !*found {
                *found = true;
            } else {
                *opt_e = None;
            }
            false
        });
    }

    pub(crate) fn visit_ysh<F>(&mut self, ysh_edge: Edge, callback: &mut F) -> bool
    where
        F: (FnMut(&mut Option<Edge>) -> bool),
    {
        match self {
            RefIR::Edge(ref mut opt_e) if opt_e == &Some(ysh_edge) => callback(opt_e),
            RefIR::Seq(seq) => {
                for ir in seq.contents.iter_mut() {
                    let done = ir.visit_ysh(ysh_edge, callback);
                    if done {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CiteEdgeData<O: OutputFormat = Markup> {
    Output(O::Build),
    Locator(O::Build),
    LocatorLabel(O::Build),
    /// Used for representing a YearSuffix that has actually been rendered during disambiguation.
    YearSuffix(O::Build),
    CitationNumber(O::Build),
    CitationNumberLabel(O::Build),
    Frnn(O::Build),
    FrnnLabel(O::Build),
}

impl<O: OutputFormat> CiteEdgeData<O> {
    pub fn from_number_variable(var: NumberVariable, label: bool) -> fn(O::Build) -> Self {
        match (var, label) {
            (NumberVariable::Locator, false) => CiteEdgeData::Locator,
            (NumberVariable::Locator, true) => CiteEdgeData::LocatorLabel,
            (NumberVariable::FirstReferenceNoteNumber, false) => CiteEdgeData::Frnn,
            (NumberVariable::FirstReferenceNoteNumber, true) => CiteEdgeData::FrnnLabel,
            (NumberVariable::CitationNumber, false) => CiteEdgeData::CitationNumber,
            (NumberVariable::CitationNumber, true) => CiteEdgeData::CitationNumberLabel,
            _ => CiteEdgeData::Output,
        }
    }
    pub fn from_ordinary_variable(var: Variable) -> fn(O::Build) -> Self {
        match var {
            Variable::YearSuffix => CiteEdgeData::YearSuffix,
            _ => CiteEdgeData::Output,
        }
    }
    pub fn from_standard_variable(var: StandardVariable, label: bool) -> fn(O::Build) -> Self {
        match var {
            StandardVariable::Number(nv) => CiteEdgeData::from_number_variable(nv, label),
            StandardVariable::Ordinary(v) => CiteEdgeData::from_ordinary_variable(v),
        }
    }
}

use crate::disamb::names::NameIR;
use std::sync::Mutex;

// Intermediate Representation
#[derive(Debug, Clone)]
pub enum IR<O: OutputFormat = Markup> {
    // no (further) disambiguation possible
    Rendered(Option<CiteEdgeData<O>>),
    // the name block,
    Name(Arc<Mutex<NameIR<O>>>),

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

impl<O> IR<O>
where
    O: OutputFormat,
{
    pub fn is_empty(&self) -> bool {
        match self {
            IR::Rendered(None) | IR::YearSuffix(_, None) => true,
            IR::Seq(seq) if seq.contents.is_empty() => true,
            IR::ConditionalDisamb(_c, boxed) => boxed.is_empty(),
            IR::Name(nir) => nir.lock().unwrap().ir.is_empty(),
            _ => false,
        }
    }
}

impl<O> Eq for IR<O> where O: OutputFormat + PartialEq + Eq {}
impl<O> PartialEq for IR<O>
where
    O: OutputFormat + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (IR::Rendered(s), IR::Rendered(o)) if s == o => true,
            (IR::Seq(s), IR::Seq(o)) if s == o => true,
            (IR::YearSuffix(s1, s2), IR::YearSuffix(o1, o2)) if s1 == o1 && s2 == o2 => true,
            (IR::ConditionalDisamb(s1, s2), IR::ConditionalDisamb(o1, o2))
                if s1 == o1 && s2 == o2 =>
            {
                true
            }
            (IR::Name(self_nir), IR::Name(other_nir)) => {
                let s = self_nir.lock().unwrap();
                let o = other_nir.lock().unwrap();
                *s == *o
            }
            _ => false,
        }
    }
}

impl<O: OutputFormat> Default for IR<O> {
    fn default() -> Self {
        IR::Rendered(None)
    }
}

/// Currently, flattening into EdgeData(String) only works when the Output type is String
/// So Pandoc isn't ready yet; maybe you can flatten Pandoc structure into a string.
impl<O: OutputFormat<Output = String>> IR<O> {
    pub fn flatten(&self, fmt: &O) -> Option<O::Build> {
        // must clone
        match self {
            IR::Rendered(None) => None,
            IR::Rendered(Some(ref x)) => Some(x.inner()),
            IR::Name(nir) => nir.lock().unwrap().ir.flatten(fmt),
            IR::ConditionalDisamb(_, ref xs) => (*xs).flatten(fmt),
            IR::YearSuffix(_, ref x) => x.clone(),
            IR::Seq(ref seq) => seq.flatten_seq(fmt),
        }
    }
}

impl<O: OutputFormat<Output = String>> IrSeq<O> {
    // TODO: Groupvars
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

impl<O: OutputFormat<Output = String>> CiteEdgeData<O> {
    pub(crate) fn to_edge_data(&self, fmt: &O, formatting: Formatting) -> EdgeData {
        match self {
            CiteEdgeData::Output(x) => {
                EdgeData::Output(fmt.output_in_context(x.clone(), formatting))
            }
            CiteEdgeData::YearSuffix(_) => EdgeData::YearSuffix,
            CiteEdgeData::Frnn(_) => EdgeData::Frnn,
            CiteEdgeData::FrnnLabel(_) => EdgeData::FrnnLabel,
            CiteEdgeData::Locator(_) => EdgeData::Locator,
            CiteEdgeData::LocatorLabel(_) => EdgeData::LocatorLabel,
            CiteEdgeData::CitationNumber(_) => EdgeData::CitationNumber,
            CiteEdgeData::CitationNumberLabel(_) => EdgeData::CitationNumberLabel,
        }
    }
    fn inner(&self) -> O::Build {
        match self {
            CiteEdgeData::Output(x)
            | CiteEdgeData::YearSuffix(x)
            | CiteEdgeData::Frnn(x)
            | CiteEdgeData::FrnnLabel(x)
            | CiteEdgeData::Locator(x)
            | CiteEdgeData::LocatorLabel(x)
            | CiteEdgeData::CitationNumber(x)
            | CiteEdgeData::CitationNumberLabel(x) => x.clone(),
        }
    }
}

impl IR<Markup> {
    pub(crate) fn visit_year_suffix_hooks<F>(&mut self, callback: &mut F) -> bool
    where
        F: (FnMut(&mut IR<Markup>) -> bool),
    {
        match self {
            IR::YearSuffix(..) => callback(self),
            IR::ConditionalDisamb(_, ref mut boxed) => {
                // XXX(check this): boxed has already been rendered, so the `if` was with
                // disambiguate=false, probably. So you can visit it.
                boxed.visit_year_suffix_hooks(callback)
            }
            IR::Seq(seq) => {
                for ir in seq.contents.iter_mut() {
                    let done = ir.visit_year_suffix_hooks(callback);
                    if done {
                        return true;
                    }
                }
                false
            }
            _ => false,
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
                    if !o.is_empty() {
                        edges.push(EdgeData::Output(o))
                    }
                }
            }
            IR::ConditionalDisamb(_, xs) => (*xs).append_edges(edges, fmt, formatting),
            IR::Seq(seq) => seq.append_edges(edges, fmt, formatting),
            IR::Name(nir) => nir.lock().unwrap().ir.append_edges(edges, fmt, formatting),
        }
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

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct IrSeq<O: OutputFormat> {
    pub contents: Vec<IR<O>>,
    pub formatting: Option<Formatting>,
    pub affixes: Affixes,
    pub delimiter: Atom,
    pub display: Option<DisplayMode>,
}

impl IrSeq<Markup> {
    fn append_edges(&self, edges: &mut Vec<EdgeData>, fmt: &Markup, format_context: Formatting) {
        if self.contents.is_empty() {
            return;
        }
        let IrSeq {
            ref contents,
            ref affixes,
            ref delimiter,
            formatting,
            display,
        } = *self;

        let stack = fmt.tag_stack(formatting.unwrap_or_else(Default::default), display);
        let sub_formatting = formatting
            .map(|mine| format_context.override_with(mine))
            .unwrap_or(format_context);
        let mut open_tags = String::new();
        let mut close_tags = String::new();
        fmt.stack_preorder(&mut open_tags, &stack);
        fmt.stack_postorder(&mut close_tags, &stack);

        if !affixes.prefix.is_empty() {
            edges.push(EdgeData::Output(affixes.prefix.to_string()));
        }

        if !open_tags.is_empty() {
            edges.push(EdgeData::Output(open_tags));
        }

        // push the innards
        let _len = contents.len();
        let mut seen = false;
        let mut sub = Vec::new();
        for (_n, ir) in contents.iter().enumerate() {
            ir.append_edges(&mut sub, fmt, sub_formatting);
            if !sub.is_empty() {
                if seen {
                    if !delimiter.is_empty() {
                        edges.push(EdgeData::Output(
                            fmt.output_in_context(fmt.plain(delimiter.as_ref()), sub_formatting),
                        ));
                    }
                } else {
                    seen = true;
                }
                edges.extend(sub.drain(..));
            }
        }
        if !close_tags.is_empty() {
            edges.push(EdgeData::Output(close_tags));
        }

        if !affixes.suffix.is_empty() {
            edges.push(EdgeData::Output(affixes.suffix.to_string()));
        }
    }
}
