// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::prelude::*;
use citeproc_io::output::markup::Markup;
use citeproc_io::output::LocalizedQuotes;
use core::fmt;
use csl::{Affixes, Choose, DateVariable, Formatting, GivenNameDisambiguationRule, TextElement};
use csl::{NumberVariable, StandardVariable, Variable};

use std::sync::Arc;

pub mod transforms;

pub type IrSum<O> = (IR<O>, GroupVars);

// Intermediate Representation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IR<O: OutputFormat = Markup> {
    // no (further) disambiguation possible
    Rendered(Option<CiteEdgeData<O>>),
    // the name block,
    Name(NameIR<O>),

    Substitute,

    /// a single <if disambiguate="true"> being tested once means the whole <choose> is re-rendered in step 4
    /// or <choose><if><conditions><condition>
    /// Should also include `if variable="year-suffix"` because that could change.
    ConditionalDisamb(ConditionalDisambIR),
    YearSuffix(YearSuffix),

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
    Seq(IrSeq),

    /// Only exists to aggregate the counts of names
    NameCounter(IrNameCounter<O>),
}

/// Simplified output that's more readable
impl<O: OutputFormat> std::fmt::Display for IR<O> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IR::Name(_) => write!(f, "Name"),
            IR::Seq(seq) => {
                let mut dbg = f.debug_struct("Seq");
                if let Some(pre) = seq.affixes.as_ref().map(|x| x.prefix.as_str()) {
                    dbg.field("prefix", &pre);
                }
                if let Some(delim) = seq.delimiter.as_ref() {
                    dbg.field("delimiter", &delim);
                }
                if let Some(suf) = seq.affixes.as_ref().map(|x| x.suffix.as_str()) {
                    dbg.field("suffix", &suf);
                }
                if let Some(gv) = seq.dropped_gv {
                    dbg.field("dropped_gv", &gv);
                }
                dbg.finish()
            }
            IR::Substitute => write!(f, "Substitute"),
            IR::ConditionalDisamb(_) => write!(f, "ConditionalDisamb"),
            IR::NameCounter(_) => write!(f, "NameCounter"),
            IR::Rendered(None) => write!(f, "<empty>"),
            IR::Rendered(Some(data)) => write!(f, "{:?}", data.build()),
            IR::YearSuffix(_) => write!(f, "YearSuffix"),
        }
    }
}

/// # Disambiguation and group_vars
///
/// IrSeq needs to hold things
#[derive(Default, PartialEq, Eq, Clone)]
pub struct IrSeq {
    pub formatting: Option<Formatting>,
    pub affixes: Option<Affixes>,
    pub delimiter: Option<SmartString>,
    pub display: Option<DisplayMode>,
    pub quotes: Option<LocalizedQuotes>,
    pub text_case: TextCase,
    /// GroupVars of the dropped nodes in an implicit conditional group.
    ///
    /// If this is Some, it contains the neighbour()-sum of the groupvars of any child nodes that
    /// were thrown out during tree construction or modification. If nothing was thrown out, or we
    /// only threw out empty plain text, then it's just the initial value (Plain); if something
    /// *was* thrown out, then it will typically be `Missing`.
    ///
    /// In this fashion, you do not need to carry around a bunch of dead nodes just to recompute
    /// the group vars of the seq. A variable that was called but empty will continue to impact the
    /// rendering of implicit conditionals.
    ///
    /// If this is None, this sequence is simply not an implicit conditional. It might be, for
    /// instance, a `<layout>` node.
    pub dropped_gv: Option<GroupVars>,
    /// This is for `<if>` (etc) branches, which each behave as a seq, but do not clear the
    /// delimiter from a surrounding group or layout seq; instead, they inherit it.
    pub should_inherit_delim: bool,
    /// Useful for identifying each top-of-cite `<layout>` element, especially when two or more
    /// cites have already been combined into one tree.
    pub is_layout: bool,
}

impl fmt::Debug for IrSeq {
    #[rustfmt::skip]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            formatting, affixes, delimiter, display, quotes,
            text_case, dropped_gv, should_inherit_delim, is_layout,
        } = self;
        let mut f = f.debug_struct("IrSeq");
        if formatting.is_some() { f.field("formatting", &formatting); }
        if affixes.is_some() { f.field("affixes", &affixes); }
        if delimiter.is_some() { f.field("delimiter", &delimiter); }
        if display.is_some() { f.field("display", &display); }
        if quotes.is_some() { f.field("quotes", &quotes); }
        if *text_case != TextCase::None { f.field("text_case", &text_case); }
        if dropped_gv.is_some() { f.field("dropped_gv", &dropped_gv); }
        if *should_inherit_delim { f.field("should_inherit_delim", &should_inherit_delim); }
        if *is_layout { f.field("is_layout", &is_layout); }
        f.finish()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DisambPass {
    AddNames,
    AddGivenName(GivenNameDisambiguationRule),
    AddYearSuffix(u32),
    Conditionals,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct YearSuffix {
    // Has IR child.

    // Clone element into here, because we already know it's a <text variable="" />
    pub(crate) hook: YearSuffixHook,
    pub(crate) suffix_num: Option<u32>,
}

impl<O: OutputFormat> IR<O> {
    pub(crate) fn year_suffix(hook: YearSuffixHook) -> IrSum<O> {
        (
            IR::YearSuffix(YearSuffix {
                hook,
                suffix_num: None,
            }),
            // A year suffix is initially missing.
            // It may later gain a value (UnresolvedImportant)
            GroupVars::UnresolvedMissing,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum YearSuffixHook {
    // Clone element into here, because we already know it's a <text variable="" />
    Explicit(TextElement),
    Plain,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CiteEdgeData<O: OutputFormat = Markup> {
    /// Used for [IR::leading_names_block_or_title]
    Title(O::Build),
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
            Variable::Title => CiteEdgeData::Title,
            Variable::TitleShort => CiteEdgeData::Title,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalDisambIR {
    // Has IR children
    pub choose: Arc<Choose>,
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrNameCounter<O: OutputFormat> {
    pub name_irs: Vec<NameIR<O>>,
    pub group_vars: GroupVars,
}

impl<O: OutputFormat> IrNameCounter<O> {
    pub fn count<I: OutputFormat>(&self, ctx: &CiteContext<'_, O, I>) -> u32 {
        self.name_irs.iter().map(|nir| nir.count(ctx)).sum()
    }
    pub fn render_cite<I: OutputFormat>(&self, ctx: &CiteContext<'_, O, I>) -> IrSum<O> {
        let fmt = &ctx.format;
        let count = self.count(ctx);
        let built = if ctx.sort_key.is_some() {
            fmt.affixed_text(
                smart_format!("{:08}", count),
                None,
                Some(&crate::sort::natural_sort::num_affixes()),
            )
        } else {
            // This isn't sort-mode, you can render NameForm::Count as text.
            fmt.text_node(smart_format!("{}", count), None)
        };
        (
            IR::Rendered(Some(CiteEdgeData::Output(built))),
            GroupVars::Important,
        )
    }
}

// impl<O> Eq for IR<O> where O: OutputFormat + PartialEq + Eq {}
impl<O> IR<O>
where
    O: OutputFormat + PartialEq,
{
    #[allow(dead_code)]
    fn deep_equals(
        &self,
        _self_gv: GroupVars,
        self_id: NodeId,
        self_arena: &IrArena<O>,
        other: &Self,
        _other_gv: GroupVars,
        other_id: NodeId,
        other_arena: &IrArena<O>,
    ) -> bool {
        match (self, other) {
            (IR::Rendered(a), IR::Rendered(b)) if a == b => return true,
            (IR::Seq(a), IR::Seq(b)) if a == b => {}
            (IR::YearSuffix(a), IR::YearSuffix(b)) if a == b => {}
            (IR::ConditionalDisamb(a), IR::ConditionalDisamb(b)) if a == b => {}
            (IR::Name(a), IR::Name(b)) if a == b => {}
            (IR::Substitute, IR::Substitute) => {}
            _ => return false,
        }
        self_id
            .children(self_arena)
            .zip(other_id.children(other_arena))
            .all(|(a, b)| {
                let (ai, ag) = self_arena.get(a).unwrap().get();
                let (bi, bg) = other_arena.get(b).unwrap().get();
                ai.deep_equals(*ag, a, self_arena, bi, *bg, b, other_arena)
            })
    }
}

impl<O: OutputFormat> Default for IR<O> {
    fn default() -> Self {
        IR::Rendered(None)
    }
}

impl<O: OutputFormat<Output = SmartString>> CiteEdgeData<O> {
    pub(crate) fn to_edge_data(&self, fmt: &O, formatting: Formatting) -> EdgeData {
        match self {
            CiteEdgeData::Output(x)
            | CiteEdgeData::Title(x)
            | CiteEdgeData::Year(x)
            | CiteEdgeData::Term(x) => {
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
}
impl<O: OutputFormat> CiteEdgeData<O> {
    pub(crate) fn inner(&self) -> O::Build {
        self.build().clone()
    }
    fn build(&self) -> &O::Build {
        match self {
            Self::Title(b)
            | Self::Output(b)
            | Self::Locator(b)
            | Self::LocatorLabel(b)
            | Self::YearSuffix(b)
            | Self::CitationNumber(b)
            | Self::CitationNumberLabel(b)
            | Self::Frnn(b)
            | Self::FrnnLabel(b)
            | Self::Accessed(b)
            | Self::Year(b)
            | Self::Term(b) => b,
        }
    }
}

impl IR<Markup> {
    fn append_edges(
        node: NodeId,
        arena: &IrArena<Markup>,
        edges: &mut Vec<EdgeData>,
        fmt: &Markup,
        formatting: Formatting,
        inherit_delim: Option<&str>,
    ) {
        let (me, gv) = match arena.get(node) {
            Some(x) => x.get(),
            None => return,
        };
        let tree = IrTreeRef { node, arena };
        match me {
            IR::Rendered(None) => {}
            IR::Rendered(Some(ed)) => edges.push(ed.to_edge_data(fmt, formatting)),
            IR::YearSuffix(_ys) => {
                if !tree.is_empty() {
                    edges.push(EdgeData::YearSuffix);
                }
            }
            IR::Name(_) | IR::NameCounter(_) | IR::Substitute => {
                IR::append_child_edges(node, arena, edges, fmt, formatting, None)
            }
            // Inherit the delimiter here.
            IR::ConditionalDisamb(_) => {
                IR::append_child_edges(node, arena, edges, fmt, formatting, inherit_delim)
            }
            IR::Seq(seq) => {
                if gv.should_render_tree(seq.is_implicit_conditional()) {
                    seq.append_edges(node, arena, edges, fmt, formatting, inherit_delim)
                }
            }
        }
    }

    fn append_child_edges(
        node: NodeId,
        arena: &IrArena<Markup>,
        edges: &mut Vec<EdgeData>,
        fmt: &Markup,
        formatting: Formatting,
        inherit_delim: Option<&str>,
    ) {
        for child in node.children(arena) {
            IR::append_edges(child, arena, edges, fmt, formatting, inherit_delim);
        }
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

impl IrSeq {
    pub(crate) fn is_implicit_conditional(&self) -> bool {
        self.dropped_gv.is_some()
    }
    pub(crate) fn unconditional_child_gv_sum<O: OutputFormat>(tree: IrTreeRef<O>) -> GroupVars {
        tree.children()
            .fold(GroupVars::Plain, |acc, child| {
                let gv = child.get_node().unwrap().get().1;
                acc.neighbour(gv)
            })
            .unconditional()
    }
    pub(crate) fn overall_group_vars<O: OutputFormat>(
        dropped_gv: Option<GroupVars>,
        tree: IrTreeRef<O>,
    ) -> GroupVars {
        dropped_gv.map_or_else(
            // Not an implicit conditional
            || IrSeq::unconditional_child_gv_sum(tree),
            // An implicit conditional
            |dropped| {
                let acc = tree.children().fold(dropped, |acc, child| {
                    let gv = child.get_node().unwrap().get().1;
                    acc.neighbour(gv)
                });
                // Replicate GroupVars::implicit_conditional
                acc.promote_plain()
            },
        )
    }
}

/// Currently, flattening into EdgeData(String) only works when the Output type is String
/// So Pandoc isn't ready yet; maybe you can flatten Pandoc structure into a string.
impl<'a, O: OutputFormat<Output = SmartString>> IrTreeRef<'a, O> {
    pub(crate) fn flatten_or_plain(&self, fmt: &O, if_empty: &str) -> O::Build {
        self.flatten(fmt, None)
            .unwrap_or_else(|| fmt.plain(if_empty))
    }

    /// Some group vars may be in an unresolved state.
    /// Anything that's unresolved, do not render it. It might come back later.
    pub(crate) fn flatten(&self, fmt: &O, override_delim: Option<&str>) -> Option<O::Build> {
        // must clone
        let (ref ir, gv) = *self.arena.get(self.node)?.get();
        match ir {
            IR::Rendered(None) => None,
            IR::Rendered(Some(ref x)) => Some(x.inner()),
            IR::ConditionalDisamb(_) => self.flatten_children(fmt, override_delim),
            IR::YearSuffix(_) | IR::NameCounter(_) | IR::Name(_) | IR::Substitute => {
                self.flatten_children(fmt, None)
            }
            IR::Seq(seq) if gv.should_render_tree(seq.is_implicit_conditional()) => {
                seq.flatten_seq(*self, fmt, override_delim)
            }
            _ => None,
        }
    }
    pub(crate) fn flatten_children(
        &self,
        fmt: &O,
        override_delim: Option<&str>,
    ) -> Option<O::Build> {
        let mut group = Vec::new();
        for child in self
            .children()
            .filter_map(|child| child.flatten(fmt, override_delim))
        {
            group.push(child)
        }
        if group.is_empty() {
            return None;
        }
        Some(fmt.group(group, "", None))
    }
}

impl<'a> IrTreeRef<'a, Markup> {
    pub fn to_edge_stream(&self, fmt: &Markup) -> Vec<EdgeData> {
        let mut edges = Vec::new();
        IR::append_edges(
            self.node,
            self.arena,
            &mut edges,
            fmt,
            Formatting::default(),
            None,
        );
        edges
    }
}

impl IrSeq {
    // TODO: Groupvars
    fn flatten_seq<O: OutputFormat<Output = SmartString>>(
        &self,
        tree: IrTreeRef<O>,
        fmt: &O,
        override_delim: Option<&str>,
    ) -> Option<O::Build> {
        let IrSeq {
            formatting,
            ref delimiter,
            ref affixes,
            ref quotes,
            display,
            text_case,
            dropped_gv: _,
            should_inherit_delim,
            is_layout: _,
        } = *self;
        let xs: Vec<_> = tree
            .children()
            .filter_map(|child| child.flatten(fmt, delimiter.as_opt_str()))
            .collect();
        if xs.is_empty() {
            return None;
        }
        let delim = override_delim
            .filter(|_| should_inherit_delim)
            .or(delimiter.as_opt_str())
            .unwrap_or("");
        let grp = fmt.group(xs, delim, formatting);
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

    fn append_edges(
        &self,
        node: NodeId,
        arena: &IrArena<Markup>,
        edges: &mut Vec<EdgeData>,
        fmt: &Markup,
        format_context: Formatting,
        override_delim: Option<&str>,
    ) {
        // Currently recreates the whole markup-formatting infrastructure, but keeps the same
        // granularity of edges that RefIR will produce.

        if node.children(arena).next().is_none() {
            return;
        }
        let IrSeq {
            ref affixes,
            ref delimiter,
            formatting,
            display,
            // TODO: use these
            quotes: _,
            text_case: _,
            dropped_gv: _,
            should_inherit_delim,
            is_layout: _,
        } = *self;
        let delimiter = override_delim
            .filter(|_| should_inherit_delim)
            .or(delimiter.as_opt_str());
        let affixes = affixes.as_ref();

        // TODO: move display out of tag_stack, so that quotes can go inside it.
        // Damn those macros.
        let stack = fmt.tag_stack(formatting.unwrap_or_else(Default::default), display);
        let sub_formatting = formatting
            .map(|mine| format_context.override_with(mine))
            .unwrap_or(format_context);
        let mut open_tags = SmartString::new();
        let mut close_tags = SmartString::new();
        fmt.stack_preorder(&mut open_tags, &stack);
        fmt.stack_postorder(&mut close_tags, &stack);

        if !affixes.map_or(true, |a| a.prefix.is_empty()) {
            edges.push(EdgeData::Output(affixes.unwrap().prefix.as_str().into()));
        }

        if !open_tags.is_empty() {
            edges.push(EdgeData::Output(open_tags));
        }

        // push the innards
        let mut seen = false;
        let mut sub = Vec::new();
        for child in node.children(arena) {
            IR::append_edges(child, arena, &mut sub, fmt, sub_formatting, delimiter);
            if !sub.is_empty() {
                if seen {
                    if let Some(delimiter) = delimiter {
                        edges.push(EdgeData::Output(fmt.output_in_context(
                            fmt.plain(delimiter),
                            sub_formatting,
                            None,
                        )));
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
            edges.push(EdgeData::Output(affixes.unwrap().suffix.as_str().into()));
        }
    }
}
