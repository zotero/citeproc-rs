// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::disamb::Nfa;
use crate::prelude::*;
use citeproc_io::output::markup::Markup;
use citeproc_io::output::LocalizedQuotes;
use csl::Atom;
use csl::{Affixes, Choose, Element, Formatting, GivenNameDisambiguationRule, DateVariable, TextElement};
use csl::{NumberVariable, StandardVariable, Variable};
use crate::disamb::names::RefNameIR;

use std::sync::Arc;

pub mod transforms;

pub type IrSum<O> = (IR<O>, GroupVars);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DisambPass {
    AddNames,
    AddGivenName(GivenNameDisambiguationRule),
    AddYearSuffix(u32),
    Conditionals,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct YearSuffix<O: OutputFormat> {
    // Clone element into here, because we already know it's a <text variable="" />
    pub(crate) hook: YearSuffixHook,
    pub(crate) ir: Box<IR<O>>,
    pub(crate) group_vars: GroupVars,
    pub(crate) suffix_num: Option<u32>,
}

impl<O: OutputFormat> IR<O> {
    pub(crate) fn year_suffix(hook: YearSuffixHook) -> IrSum<O> {
        (
            IR::YearSuffix(YearSuffix {
                hook,
                group_vars: GroupVars::Unresolved,
                suffix_num: None,
                ir: Box::new(IR::Rendered(None)),
            }),
            GroupVars::Unresolved
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum YearSuffixHook {
    // Clone element into here, because we already know it's a <text variable="" />
    Explicit(TextElement),
    Plain,
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

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct RefIrSeq {
    pub contents: Vec<RefIR>,
    pub formatting: Option<Formatting>,
    pub affixes: Option<Affixes>,
    pub delimiter: Atom,
    pub quotes: Option<LocalizedQuotes>,
    pub text_case: TextCase,
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

    pub(crate) fn keep_first_ysh(&mut self, ysh_explicit_edge: Edge, ysh_plain_edge: Edge, ysh_edge: Edge) {
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
        self.visit_ysh(ysh_plain_edge, &mut |opt_e| {
            if !*found {
                *found = true;
                *opt_e = Some(ysh_edge);
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
            RefIR::Edge(ref mut opt_e) if *opt_e == Some(ysh_edge) => callback(opt_e),
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
    /// Accessed isn't really part of a reference -- it doesn't help disambiguating one from
    /// another. So we will ignore it. Works for, e.g., date_YearSuffixImplicitWithNoDate.txt
    Accessed(O::Build),
    Year(O::Build),
    Term(O::Build),
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
    pub fn from_date_variable(var: DateVariable) -> fn(O::Build) -> Self {
        match var {
            DateVariable::Accessed => CiteEdgeData::Accessed,
            _ => CiteEdgeData::Output,
        }
    }
}

use crate::disamb::names::NameIR;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalDisambIR<O: OutputFormat> {
    pub choose: Arc<Choose>,
    pub group_vars: GroupVars,
    pub done: bool,
    pub ir: Box<IR<O>>,
}

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
    ConditionalDisamb(Arc<Mutex<ConditionalDisambIR<O>>>),
    YearSuffix(YearSuffix<O>),

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

    /// Only exists to aggregate the counts of names
    NameCounter(IrNameCounter<O>),
}

#[derive(Debug, Clone)]
pub struct IrNameCounter<O: OutputFormat> {
    pub name_irs: Vec<NameIR<O>>,
    pub ir: Box<IR<O>>,
    pub group_vars: GroupVars,
}

#[derive(Debug, Clone)]
pub struct RefIrNameCounter {
    name_irs: Vec<RefNameIR>,
}

impl<O: OutputFormat> IrNameCounter<O> {
    pub fn count<I: OutputFormat>(&self, ctx: &CiteContext<'_, O, I>) -> u32 {
        self.name_irs
            .iter()
            .map(|nir| nir.count(ctx))
            .sum()
    }
    pub fn render_cite<I: OutputFormat>(&self, ctx: &CiteContext<'_, O, I>) -> IrSum<O> {
        let fmt = &ctx.format;
        let count = self.count(ctx);
        let built = if ctx.sort_key.is_some() {
            fmt.affixed_text(
                format!("{:08}", count),
                None,
                Some(&crate::sort::natural_sort::num_affixes()),
            )
        } else {
            // This isn't sort-mode, you can render NameForm::Count as text.
            fmt.text_node(format!("{}", count), None)
        };
        (IR::Rendered(Some(CiteEdgeData::Output(built))), GroupVars::Important)
    }
}

impl RefIrNameCounter {
    fn count(&self) -> u32 {
        500
    }
    pub fn render_ref(&self, db: &impl IrDatabase, ctx: &RefContext<'_, Markup>, stack: Formatting, piq: Option<bool>) -> (RefIR, GroupVars) {
        let count = self.count();
        let fmt = ctx.format;
        let out = fmt.output_in_context(fmt.text_node(format!("{}", count), None), stack, piq);
        let edge = db.edge(EdgeData::<Markup>::Output(out));
        (RefIR::Edge(Some(edge)), GroupVars::Important)
    }
}

impl<O> IR<O>
where
    O: OutputFormat,
{
    /// Rendered(None), empty YearSuffix or empty seq
    pub fn is_empty(&self) -> bool {
        match self {
            IR::Rendered(None) => true,
            IR::YearSuffix(ys) => ys.ir.is_empty(),
            IR::Seq(seq) if seq.contents.is_empty() => true,
            IR::ConditionalDisamb(c) => c.lock().unwrap().ir.is_empty(),
            IR::Name(nir) => nir.lock().unwrap().ir.is_empty(),
            IR::NameCounter(nc) => false,
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
            (IR::YearSuffix(s), IR::YearSuffix(o)) if s == o => true,
            (IR::ConditionalDisamb(a), IR::ConditionalDisamb(b)) => {
                let aa = a.lock().unwrap();
                let bb = b.lock().unwrap();
                *aa == *bb
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

impl Default for RefIR {
    fn default() -> Self {
        RefIR::Edge(None)
    }
}

/// Currently, flattening into EdgeData(String) only works when the Output type is String
/// So Pandoc isn't ready yet; maybe you can flatten Pandoc structure into a string.
impl<O: OutputFormat<Output = String>> IR<O> {
    /// Assumes any group vars have been resolved, so every item touched by flatten should in fact
    /// be rendered
    pub fn flatten(&self, fmt: &O) -> Option<O::Build> {
        // must clone
        match self {
            IR::Rendered(None) => None,
            IR::Rendered(Some(ref x)) => Some(x.inner()),
            IR::Name(nir) => nir.lock().unwrap().ir.flatten(fmt),
            IR::ConditionalDisamb(c) => c.lock().unwrap().ir.flatten(fmt),
            IR::YearSuffix(YearSuffix { ir, .. }) => ir.flatten(fmt),
            IR::Seq(ref seq) => seq.flatten_seq(fmt),
            IR::NameCounter(nc) => nc.ir.flatten(fmt),
        }
    }
}

impl<O: OutputFormat<Output = String>> CiteEdgeData<O> {
    pub(crate) fn to_edge_data(
        &self,
        fmt: &O,
        formatting: Formatting,
    ) -> EdgeData {
        match self {
            CiteEdgeData::Output(x) | CiteEdgeData::Year(x) | CiteEdgeData::Term(x) => {
                EdgeData::Output(fmt.output_in_context(x.clone(), formatting, None))
            }
            CiteEdgeData::YearSuffix(_) => EdgeData::YearSuffix,
            CiteEdgeData::Frnn(_) => EdgeData::Frnn,
            CiteEdgeData::FrnnLabel(_) => EdgeData::FrnnLabel,
            CiteEdgeData::Locator(_) => EdgeData::Locator,
            CiteEdgeData::LocatorLabel(_) => EdgeData::LocatorLabel,
            CiteEdgeData::CitationNumber(_) => EdgeData::CitationNumber,
            CiteEdgeData::CitationNumberLabel(_) => EdgeData::CitationNumberLabel,
            CiteEdgeData::Accessed(_) => EdgeData::Accessed,
        }
    }
    fn inner(&self) -> O::Build {
        match self {
            CiteEdgeData::Output(x)
            | CiteEdgeData::Term(x)
            | CiteEdgeData::Year(x)
            | CiteEdgeData::YearSuffix(x)
            | CiteEdgeData::Frnn(x)
            | CiteEdgeData::FrnnLabel(x)
            | CiteEdgeData::Locator(x)
            | CiteEdgeData::LocatorLabel(x)
            | CiteEdgeData::CitationNumber(x)
            | CiteEdgeData::Accessed(x)
            | CiteEdgeData::CitationNumberLabel(x) => x.clone(),
        }
    }
}

impl IR<Markup> {
    pub(crate) fn visit_year_suffix_hooks<F>(&mut self, callback: &mut F) -> bool
    where
        F: (FnMut(&mut YearSuffix<Markup>) -> bool),
    {
        match self {
            IR::YearSuffix(ys) => callback(ys),
            IR::ConditionalDisamb(c) => {
                // XXX(check this): boxed has already been rendered, so the `if` was with
                // disambiguate=false, probably. So you can visit it.
                c.lock().unwrap().ir.visit_year_suffix_hooks(callback)
            }
            IR::Seq(seq) => {
                for (ir, gv) in seq.contents.iter_mut() {
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

    fn append_edges(
        &self,
        edges: &mut Vec<EdgeData>,
        fmt: &Markup,
        formatting: Formatting,
    ) {
        match self {
            IR::Rendered(None) => {}
            IR::Rendered(Some(ed)) => {
                edges.push(ed.to_edge_data(fmt, formatting))
            }
            IR::YearSuffix(ys) => {
                if !ys.ir.is_empty() {
                    edges.push(EdgeData::YearSuffix);
                }
            }
            IR::ConditionalDisamb(c) => c.lock().unwrap().ir.append_edges(edges, fmt, formatting),
            IR::Seq(seq) => {
                if seq.overall_group_vars().map_or(true, |x| x.should_render_tree()) {
                    seq.append_edges(edges, fmt, formatting)
                }
            },
            IR::Name(nir) => nir.lock().unwrap().ir.append_edges(edges, fmt, formatting),
            IR::NameCounter(nc) => nc.ir.append_edges(edges, fmt, formatting),
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

/// # Disambiguation and group_vars
///
/// IrSeq needs to hold things 
#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct IrSeq<O: OutputFormat> {
    pub contents: Vec<IrSum<O>>,
    pub formatting: Option<Formatting>,
    pub affixes: Option<Affixes>,
    pub delimiter: Atom,
    pub display: Option<DisplayMode>,
    pub quotes: Option<LocalizedQuotes>,
    pub text_case: TextCase,
    /// If this is None, this sequence is simply an implicit conditional
    pub dropped_gv: Option<GroupVars>,
}


impl<O: OutputFormat> IR<O> {
    pub(crate) fn recompute_group_vars(&mut self) {
        match self {
            IR::Seq(seq) => seq.recompute_group_vars(),
            _ => {},
        }
    }
}

impl<O: OutputFormat> IrSeq<O> {
    pub(crate) fn overall_group_vars(&self) -> Option<GroupVars> {
        self.dropped_gv
            .map(|dropped| {
                let acc = self.contents.iter().fold(dropped, |acc, (_, gv)| acc.neighbour(*gv));
                // Replicate GroupVars::implicit_conditional
                if acc != GroupVars::Missing {
                    GroupVars::Important
                } else {
                    GroupVars::Plain
                }
            })
    }
    /// GVs are stored outside of individual child IRs, so we need a way to update those if the
    /// children have mutated themselves.
    pub(crate) fn recompute_group_vars(&mut self) {
        for (ir, gv) in self.contents.iter_mut() {
            if let Some(force_gv) = ir.force_gv() {
                *gv = force_gv;
            }
        }
    }
}

impl<O> IR<O>
where
    O: OutputFormat,
{
    pub(crate) fn force_gv(&mut self) -> Option<GroupVars> {
        match self {
            IR::Rendered(_) => None,
            IR::YearSuffix(ys) => Some(ys.group_vars),
            IR::Seq(seq) => {
                seq.recompute_group_vars();
                seq.overall_group_vars()
            },
            IR::ConditionalDisamb(c) => Some(c.lock().unwrap().group_vars),
            IR::Name(_) => None,
            IR::NameCounter(nc) => Some(nc.group_vars),
        }
    }
}

impl<O: OutputFormat<Output = String>> IrSeq<O> {
    // TODO: Groupvars
    fn flatten_seq(&self, fmt: &O) -> Option<O::Build> {
        // Do this where it won't require mut access
        // self.recompute_group_vars();
        if !self.overall_group_vars().map_or(true, |x| x.should_render_tree()) {
            return None;
        }
        let IrSeq {
            formatting,
            ref delimiter,
            ref affixes,
            ref quotes,
            display,
            text_case,
            ref contents,
            dropped_gv: _,
        } = *self;
        let xs: Vec<_> = contents.iter().filter_map(|(ir, gv)| ir.flatten(fmt)).collect();
        if xs.is_empty() {
            return None;
        }
        let grp = fmt.group(xs, delimiter, formatting);
        let grp = fmt.affixed_quoted(grp, affixes.as_ref(), quotes.clone());
        // TODO: pass in_bibliography from ctx
        let mut grp = fmt.with_display(grp, display, true);
        fmt.apply_text_case(
            &mut grp,
            &IngestOptions {
                text_case,
                ..Default::default()
            },
        );
        Some(grp)
    }
}

impl IrSeq<Markup> {
    fn append_edges(&self, edges: &mut Vec<EdgeData>, fmt: &Markup, format_context: Formatting) {
        // Currently recreates the whole markup-formatting infrastructure, but keeps the same
        // granularity of edges that RefIR will produce.

        if self.contents.is_empty() {
            return;
        }
        let IrSeq {
            ref contents,
            ref affixes,
            ref delimiter,
            // TODO: use these
            quotes: _,
            formatting,
            display,
            text_case,
            dropped_gv: _,
        } = *self;
        let affixes = affixes.as_ref();

        // TODO: move display out of tag_stack, so that quotes can go inside it.
        // Damn those macros.
        let stack = fmt.tag_stack(formatting.unwrap_or_else(Default::default), display);
        let sub_formatting = formatting
            .map(|mine| format_context.override_with(mine))
            .unwrap_or(format_context);
        let mut open_tags = String::new();
        let mut close_tags = String::new();
        fmt.stack_preorder(&mut open_tags, &stack);
        fmt.stack_postorder(&mut close_tags, &stack);

        if !affixes.map_or(true, |a| a.prefix.is_empty()) {
            edges.push(EdgeData::Output(affixes.unwrap().prefix.to_string()));
        }

        if !open_tags.is_empty() {
            edges.push(EdgeData::Output(open_tags));
        }

        // push the innards
        let _len = contents.len();
        let mut seen = false;
        let mut sub = Vec::new();
        for (_n, (ir, _gv)) in contents.iter().enumerate() {
            ir.append_edges(&mut sub, fmt, sub_formatting);
            if !sub.is_empty() {
                if seen {
                    if !delimiter.is_empty() {
                        edges.push(EdgeData::Output(
                            fmt.output_in_context(fmt.plain(delimiter.as_ref()), sub_formatting, None),
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

        if !affixes.map_or(true, |a| a.suffix.is_empty()) {
            edges.push(EdgeData::Output(affixes.unwrap().suffix.to_string()));
        }
    }
}
