// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::prelude::*;
use citeproc_io::output::markup::Markup;
use citeproc_io::output::LocalizedQuotes;
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

/// # Disambiguation and group_vars
///
/// IrSeq needs to hold things
#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct IrSeq {
    pub formatting: Option<Formatting>,
    pub affixes: Option<Affixes>,
    pub delimiter: Option<SmartString>,
    pub display: Option<DisplayMode>,
    pub quotes: Option<LocalizedQuotes>,
    pub text_case: TextCase,
    /// If this is None, this sequence is simply an implicit conditional
    pub dropped_gv: Option<GroupVars>,
    pub should_inherit_delim: bool,
    pub is_layout: bool,
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
            GroupVars::Unresolved,
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

impl<O> IR<O>
where
    O: OutputFormat,
{
    /// Rendered(None), empty YearSuffix or empty seq
    pub fn is_empty(node: NodeId, arena: &IrArena<O>) -> bool {
        let me = match arena.get(node) {
            Some(x) => x.get(),
            None => return false,
        };
        match &me.0 {
            IR::Rendered(None) => true,
            IR::Seq(_) | IR::Name(_) | IR::ConditionalDisamb(_) | IR::YearSuffix(_) => {
                node.children(arena).next().is_none()
            }
            IR::NameCounter(_nc) => false,
            _ => false,
        }
    }
}

// impl<O> Eq for IR<O> where O: OutputFormat + PartialEq + Eq {}
impl<O> IR<O>
where
    O: OutputFormat + PartialEq,
{
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

/// Currently, flattening into EdgeData(String) only works when the Output type is String
/// So Pandoc isn't ready yet; maybe you can flatten Pandoc structure into a string.
impl<O: OutputFormat<Output = SmartString>> IR<O> {
    /// Assumes any group vars have been resolved, so every item touched by flatten should in fact
    /// be rendered
    pub fn flatten(
        node: NodeId,
        arena: &IrArena<O>,
        fmt: &O,
        override_delim: Option<&str>,
    ) -> Option<O::Build> {
        // must clone
        match arena.get(node)?.get().0 {
            IR::Rendered(None) => None,
            IR::Rendered(Some(ref x)) => Some(x.inner()),
            IR::ConditionalDisamb(_) => IR::flatten_children(node, arena, fmt, override_delim),
            IR::YearSuffix(_) | IR::NameCounter(_) | IR::Name(_) => {
                IR::flatten_children(node, arena, fmt, None)
            }
            IR::Seq(ref seq) => seq.flatten_seq(node, arena, fmt, override_delim),
        }
    }

    pub fn flatten_children(
        self_id: NodeId,
        arena: &IrArena<O>,
        fmt: &O,
        override_delim: Option<&str>,
    ) -> Option<O::Build> {
        let mut group = Vec::new();
        for child in self_id
            .children(arena)
            .filter_map(|child| IR::flatten(child, arena, fmt, override_delim))
        {
            group.push(child)
        }
        if group.is_empty() {
            return None;
        }
        Some(fmt.group(group, "", None))
    }
}

impl<O: OutputFormat<Output = SmartString>> CiteEdgeData<O> {
    pub(crate) fn to_edge_data(&self, fmt: &O, formatting: Formatting) -> EdgeData {
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

impl<O: OutputFormat> IR<O> {
    pub(crate) fn list_year_suffix_hooks(root: NodeId, arena: &IrArena<O>) -> Vec<NodeId> {
        fn list_ysh_inner<O: OutputFormat>(
            node: NodeId,
            arena: &IrArena<O>,
            vec: &mut Vec<NodeId>,
        ) {
            let me = match arena.get(node) {
                Some(x) => x.get(),
                None => return,
            };
            match &me.0 {
                IR::YearSuffix(..) => vec.push(node),
                IR::NameCounter(_) | IR::Rendered(_) | IR::Name(_) => {}
                IR::ConditionalDisamb(_) | IR::Seq(_) => {
                    node.children(arena)
                        .for_each(|child| list_ysh_inner(child, arena, vec));
                }
            }
        }
        let mut vec = Vec::new();
        list_ysh_inner(root, arena, &mut vec);
        vec
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
        let me = match arena.get(node) {
            Some(x) => x.get(),
            None => return,
        };
        match &me.0 {
            IR::Rendered(None) => {}
            IR::Rendered(Some(ed)) => edges.push(ed.to_edge_data(fmt, formatting)),
            IR::YearSuffix(_ys) => {
                if !IR::is_empty(node, arena) {
                    edges.push(EdgeData::YearSuffix);
                }
            }
            IR::Name(_) | IR::NameCounter(_) => {
                IR::append_child_edges(node, arena, edges, fmt, formatting, None)
            }
            // Inherit the delimiter here.
            IR::ConditionalDisamb(_) => {
                IR::append_child_edges(node, arena, edges, fmt, formatting, inherit_delim)
            }
            IR::Seq(seq) => {
                if IrSeq::overall_group_vars(seq.dropped_gv, node, arena)
                    .map_or(true, |x| x.should_render_tree())
                {
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

    pub fn to_edge_stream(root: NodeId, arena: &IrArena<Markup>, fmt: &Markup) -> Vec<EdgeData> {
        let mut edges = Vec::new();
        IR::append_edges(root, arena, &mut edges, fmt, Formatting::default(), None);
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

impl<O: OutputFormat> IR<O> {
    pub(crate) fn recompute_group_vars(node: NodeId, arena: &mut IrArena<O>) {
        let _me = match arena.get(node) {
            Some(x) => x.get(),
            None => return,
        };
        let mut queue = Vec::new();
        for node in node.descendants(arena) {
            match &arena.get(node).unwrap().get().0 {
                IR::Seq(seq) => {
                    queue.push((node, seq.dropped_gv));
                }
                _ => {}
            }
        }
        // Reverse, such that descendants are recalculated first
        for (seq_node, dropped_gv) in queue.into_iter().rev() {
            // let data = arena.get_mut(node).unwrap().get_mut();
            if let Some(force) = IrSeq::overall_group_vars(dropped_gv, seq_node, arena) {
                arena.get_mut(seq_node).unwrap().get_mut().1 = force;
            }
        }
    }
}

impl IrSeq {
    pub(crate) fn overall_group_vars<O: OutputFormat>(
        dropped_gv: Option<GroupVars>,
        self_id: NodeId,
        arena: &IrArena<O>,
    ) -> Option<GroupVars> {
        dropped_gv.map(|dropped| {
            let acc = self_id.children(arena).fold(dropped, |acc, child| {
                let gv = arena.get(child).unwrap().get().1;
                acc.neighbour(gv)
            });
            // Replicate GroupVars::implicit_conditional
            if acc != GroupVars::Missing {
                GroupVars::Important
            } else {
                GroupVars::Plain
            }
        })
    }
}

impl IrSeq {
    // TODO: Groupvars
    fn flatten_seq<O: OutputFormat<Output = SmartString>>(
        &self,
        id: NodeId,
        arena: &IrArena<O>,
        fmt: &O,
        override_delim: Option<&str>,
    ) -> Option<O::Build> {
        // Do this where it won't require mut access
        // self.recompute_group_vars();
        if !IrSeq::overall_group_vars(self.dropped_gv, id, arena)
            .map_or(true, |x| x.should_render_tree())
        {
            return None;
        }
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
        let xs: Vec<_> = id
            .children(arena)
            .filter_map(|child| IR::flatten(child, arena, fmt, delimiter.as_opt_str()))
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
