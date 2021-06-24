// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2021 Corporation for Digital Scholarship

use std::collections::HashMap;
use std::sync::Arc;

use citeproc_db::ClusterId;
use citeproc_io::{Cite, CiteMode, ClusterMode};
use csl::Collapse;

use crate::helpers::{
    collapse_ranges::{collapse_ranges, Segment},
    slice_group_by::{group_by, group_by_mut},
};

use crate::db::IrGen;
use crate::ir::transforms;
use crate::prelude::*;

mod layout;
use layout::DelimKind;
pub(crate) use layout::LayoutDestination;

pub fn built_cluster_before_output(
    db: &dyn IrDatabase,
    cluster_id: ClusterId,
    fmt: &Markup,
) -> MarkupBuild {
    let cite_ids = if let Some(x) = db.cluster_cites_sorted(cluster_id) {
        x
    } else {
        return fmt.plain("");
    };
    let style = db.style();
    let layout = &style.citation.layout;
    let sorted_refs_arc = db.sorted_refs();
    let mut irs: Vec<_> = cite_ids
        .iter()
        .map(|&id| {
            let gen4 = db.ir_fully_disambiguated(id);
            let cite = id.lookup(db);
            let (_keys, citation_numbers_by_id) = &*sorted_refs_arc;
            let cnum = citation_numbers_by_id.get(&cite.ref_id).cloned();
            CiteInCluster::new(id, cite, cnum.map(|x| x.get()), gen4, &fmt)
        })
        .collect();

    if let Some((_cgd, collapse)) = style.citation.group_collapsing() {
        group_and_collapse(&fmt, collapse, &mut irs);
    }

    let cluster_mode = db.cluster_mode(cluster_id);
    if let Some(mode) = &cluster_mode {
        transforms::apply_cluster_mode(db, &fmt, mode, &mut irs);
    }

    // Cite capitalization
    // TODO: allow clients to pass a flag to prevent this (on ix==0) when a cluster is in the
    // middle of an existing footnote, and isn't preceded by a period (or however else a client
    // wants to judge that).
    // We capitalize all cites whose prefixes end with full stops.
    if style.class != csl::StyleClass::InText {
        for (ix, cite) in irs.iter_mut().enumerate() {
            if cite
                .prefix_parsed
                .as_ref()
                .map_or(ix == 0, |pre| fmt.ends_with_full_stop(pre))
            {
                // dbg!(ix, prefix_parsed);
                let gen_mut = Arc::make_mut(&mut cite.gen4);
                gen_mut.tree_mut().capitalize_first_term_of_cluster(&fmt);
            }
        }
    }

    // csl_test_suite::affix_WithCommas.txt
    let should_suppress_delimiter = |cites: &[CiteInCluster<Markup>], ix: usize| -> bool {
        if let (Some(a), Some(b)) = (cites.get(ix), cites.get(ix + 1)) {
            layout::suppress_delimiter_between(a, b)
        } else {
            false
        }
    };

    fn flatten_affix_cite(
        cites: &[CiteInCluster<Markup>],
        ix: usize,
        fmt: &Markup,
    ) -> Option<(Option<SmartString>, MarkupBuild, Option<SmartString>)> {
        Some(layout::flatten_with_affixes(
            cites.get(ix)?,
            ix + 1 == cites.len(),
            fmt,
        ))
    }

    enum OutputChannels {
        CitationLayout(MarkupBuild),
        SplitIntextCitation(MarkupBuild, MarkupBuild),
        IntextLayout(MarkupBuild),
    }

    // returned usize is advance len
    let render_range = |stream: &mut layout::LayoutStream,
                        ranges: &[RangePiece],
                        group_delim: DelimKind,
                        outer_delim: DelimKind|
     -> usize {
        let mut advance_to = 0usize;
        let mut group: Vec<MarkupBuild> = Vec::with_capacity(ranges.len());
        for (ix, piece) in ranges.iter().enumerate() {
            let is_last = ix + 1 == ranges.len();
            match *piece {
                RangePiece::Single(Collapsible {
                    ix, force_single, ..
                }) => {
                    advance_to = ix;
                    if let Some((pre, one, suf)) = flatten_affix_cite(&irs, ix, fmt) {
                        stream.write_cite(pre, one, suf);
                        let delim_kind = if force_single {
                            outer_delim
                        } else {
                            group_delim
                        };
                        stream.write_delim(Some(delim_kind));
                    }
                }
                RangePiece::Range(range_start, range_end) => {
                    advance_to = range_end.ix;
                    let mut range_delimiter = DelimKind::Range;
                    if range_start.number + 1 == range_end.number {
                        // Not represented as a 1-2, just two sequential numbers 1,2
                        range_delimiter = group_delim;
                    }
                    // XXX: need to guarantee before designating it a RangePiece that the start's
                    // suffix is None and the end's prefix is also None
                    let start = flatten_affix_cite(&irs, range_start.ix, fmt);
                    let end = flatten_affix_cite(&irs, range_end.ix, fmt);
                    // Delimiters here are never suppressed by build_cite, as they wouldn't be part
                    // of the range if they had affixes on the inside
                    match (start, end) {
                        (Some((pre, start, _)), Some((_, end, suf))) => {
                            stream.write_cite(pre, start, None);
                            stream.write_delim(Some(range_delimiter));
                            stream.write_cite(None, end, suf);
                        }
                        (Some((a, b, c)), None) | ((None, Some((a, b, c)))) => {
                            stream.write_cite(a, b, c);
                        }
                        _ => {}
                    }
                    stream.write_delim(Some(group_delim));
                }
            }
        }
        stream.write_delim(Some(outer_delim));
        advance_to
    };

    let citation_el = &style.citation;
    let intext_el = style.intext.as_ref();

    let citation_delims = layout::LayoutDelimiters::from_citation(&style.citation);
    let intext_delimiters = layout::LayoutDelimiters::from_intext(intext_el, citation_el);

    let mut citation_stream = layout::LayoutStream::new(irs.len() * 2, citation_delims, fmt);
    let mut intext_stream = layout::LayoutStream::new(0, intext_delimiters, fmt);

    // render the intext stream
    let intext_authors = group_by(irs.as_slice(), |a, b| {
        a.unique_name_number == b.unique_name_number
    })
    // only need the first one of each group, the rest should have identical names
    //
    //
    //
    // TODO: technically the intext name can differ from the first name element.
    // do we factor that in by replacing with intext where possible in group_and_collapse?
    //
    //
    //
    //
    .map(|run| &run[0])
    .filter_map(|cite| match cite.layout_destination {
        LayoutDestination::MainToIntext => Some((cite, cite.gen4.tree_ref().node)),
        LayoutDestination::MainToCitationPlusIntext(node) => Some((cite, node)),
        _ => None,
    })
    .map(|(cite, node)| {
        cite.gen4
            .tree_ref()
            .with_node(node)
            .flatten_or_plain(fmt, "[NO_PRINTED_FORM]")
    });

    intext_stream.write_interspersed(intext_authors, DelimKind::Layout);

    let mut ix = 0;
    while ix < irs.len() {
        let CiteInCluster {
            trailing_only: vanished,
            collapsed_ranges,
            first_of_name,
            layout_destination,
            ..
        } = &irs[ix];
        if *layout_destination == LayoutDestination::MainToIntext || *vanished {
            ix += 1;
            continue;
        }
        if !collapsed_ranges.is_empty() {
            let advance_to = render_range(
                &mut citation_stream,
                collapsed_ranges,
                DelimKind::CollapseCitationNumbersMid,
                DelimKind::CollapseCitationNumbersLast,
            );
            ix = advance_to + 1;
        } else if *first_of_name {
            let mut rix = ix;
            while rix < irs.len() {
                let r = &irs[rix];
                if rix != ix && !r.subsequent_same_name {
                    break;
                }
                if !r.collapsed_year_suffixes.is_empty() {
                    // rix is the start of a run of 1999a[rix],b,c
                    let advance_to = render_range(
                        &mut citation_stream,
                        &r.collapsed_year_suffixes,
                        DelimKind::CollapseYearSuffixMid,
                        // weird
                        DelimKind::CollapseYearSuffixLast,
                    );
                    rix = advance_to;
                } else {
                    // rix is actually just a single cite with a suppressed name
                    // Jones 1999, 2000[rix]
                    if let Some((pre, built, suf)) = flatten_affix_cite(&irs, rix, fmt) {
                        citation_stream.write_cite(pre, built, suf);
                        let delim_kind = if irs[rix].has_locator {
                            Some(DelimKind::AfterCollapsedGroup)
                        } else {
                            Some(DelimKind::CiteGroup)
                        };
                        citation_stream.write_delim(
                            // changed ix to advance_to here
                            delim_kind,
                        );
                    }
                }
                rix += 1;
            }
            citation_stream.write_delim(Some(DelimKind::AfterCollapsedGroup));
            ix = rix;
        } else {
            if let Some((pre, built, suf)) = flatten_affix_cite(&irs, ix, fmt) {
                citation_stream.write_cite(pre, built, suf);
                citation_stream.write_delim(Some(DelimKind::Layout));
            }
            ix += 1;
        }
    }

    let citation_final = citation_stream.finish();
    let intext_final = intext_stream.finish();
    if intext_final.is_none() {
        return fmt.seq(citation_final.into_iter());
    }
    let infix = render_composite_infix(
        match &cluster_mode {
            Some(ClusterMode::Composite { infix }) => Some(infix.as_opt_str()),
            _ => None,
        },
        fmt,
    );
    use core::iter::once;
    let seq = intext_final.into_iter().chain(infix).chain(citation_final);
    fmt.seq(seq)
}

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(transparent)]
pub(crate) struct NameNumber(u32);

#[derive(Debug, Copy, Clone)]
enum RangeCollapseKeyTopLevel {
    ForceSingle,
    SameName(NameNumber),
    CitationNumber(u32),
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum RangeCollapseKey {
    ForceSingle,
    SameNameSameYear(NameNumber, i32),
    CitationNumber(u32),
}

impl RangeCollapseKey {
    fn top_level(self) -> RangeCollapseKeyTopLevel {
        match self {
            RangeCollapseKey::CitationNumber(c) => RangeCollapseKeyTopLevel::CitationNumber(c),
            RangeCollapseKey::SameNameSameYear(n, _) => RangeCollapseKeyTopLevel::SameName(n),
            RangeCollapseKey::ForceSingle => RangeCollapseKeyTopLevel::ForceSingle,
        }
    }
    fn inc(&self) -> Self {
        match *self {
            RangeCollapseKey::ForceSingle => RangeCollapseKey::ForceSingle,
            // don't increment year
            RangeCollapseKey::SameNameSameYear(n, y) => RangeCollapseKey::SameNameSameYear(n, y),
            RangeCollapseKey::CitationNumber(n) => RangeCollapseKey::CitationNumber(n + 1),
        }
    }
    fn year(self) -> Option<i32> {
        match self {
            RangeCollapseKey::SameNameSameYear(_, y) => Some(y),
            _ => None,
        }
    }
}

impl PartialEq for RangeCollapseKeyTopLevel {
    fn eq(&self, other: &Self) -> bool {
        match (*self, *other) {
            // ForceSingle doesn't equate with anything, not even itself. NOT reflexive (a == a
            // doesn't always hold) and hence not Eq.
            //
            // For PartialEq only need symmetric `a==b implies b==a` and transitive `a==b and b==c
            // implies a==c`
            (RangeCollapseKeyTopLevel::ForceSingle, _)
            | (_, RangeCollapseKeyTopLevel::ForceSingle) => false,
            (
                RangeCollapseKeyTopLevel::SameName(aname),
                RangeCollapseKeyTopLevel::SameName(bname),
            ) => aname == bname,
            (
                RangeCollapseKeyTopLevel::CitationNumber(_a),
                RangeCollapseKeyTopLevel::CitationNumber(_b),
            ) => true,
            _ => false,
        }
    }
}

impl PartialEq for RangeCollapseKey {
    fn eq(&self, other: &Self) -> bool {
        match (*self, *other) {
            // ForceSingle doesn't equate with anything, not even itself. NOT reflexive (a == a
            // doesn't always hold) and hence not Eq.
            //
            // For PartialEq only need symmetric `a==b implies b==a` and transitive `a==b and b==c
            // implies a==c`
            (RangeCollapseKey::ForceSingle, _) | (_, RangeCollapseKey::ForceSingle) => false,
            (
                RangeCollapseKey::SameNameSameYear(aname, ayear),
                RangeCollapseKey::SameNameSameYear(bname, byear),
            ) => aname == bname && ayear == byear,
            (RangeCollapseKey::CitationNumber(a), RangeCollapseKey::CitationNumber(b)) => a == b,
            _ => false,
        }
    }
}

#[test]
fn test_range_collapse_key() {
    let nameyear = |n: u32, y: i32| RangeCollapseKey::SameNameSameYear(NameNumber(n), y);
    let solo = RangeCollapseKey::ForceSingle;
    let cnum = RangeCollapseKey::CitationNumber;
    struct OneCite(&'static str, RangeCollapseKey);
    let vec = vec![
        OneCite("a", nameyear(1, 1965)),
        OneCite("b", nameyear(1, 1965)),
        OneCite("c", nameyear(1, 1997)),
        OneCite("d", nameyear(2, 1997)),
        OneCite("e", nameyear(3, 1998)),
        OneCite("f", solo),
        OneCite("g", cnum(1)),
        OneCite("h", cnum(2)),
        OneCite("i", cnum(3)),
        OneCite("j", cnum(4)),
        OneCite("k", cnum(6)),
        OneCite("l", solo),
        OneCite("m", cnum(8)),
    ];

    let iter = vec.iter();
    use crate::helpers::{collapse_ranges, slice_group_by};
    use collapse_ranges::Segment;
    // groups adjacent items only.
    let grouped = slice_group_by::group_by(&vec, |a, b| a.1.top_level() == b.1.top_level());

    let groups: Vec<String> = grouped
        .map(|elems| {
            use itertools::Itertools;
            // this is a noop (one group per top level group) except for SameNameSameYear groups,
            // which are now stratified by year.
            // let cloned = stratified.clone();
            collapse_ranges::collapse_ranges(elems.iter(), |a, b| a.1.inc() == b.1)
                .map(|x| match x.as_ref().map(|x| x.0) {
                    Segment::Single(a) => format!("{}", a),
                    Segment::RangeInclusive(a, b) => format!("{}..={}", a, b),
                })
                .intersperse(",".to_owned())
                // stratified.iter().map(|x| x.0).intersperse(",")
                .collect::<String>()
        })
        .collect();
    let expected = vec![
        "a..=b,c".to_owned(),
        "d".to_owned(),
        "e".to_owned(),
        "f".to_owned(),
        // these four are all cnum, so they are all in one group.
        "g..=j,k".to_owned(),
        "l".to_owned(),
        "m".to_owned(),
    ];
    assert_eq!(groups, expected);
}

/// A wrapper for Option where `a == b` evaluates to false if either is empty
///
/// Implements PartialEq, but does not implement Eq, of course.
#[derive(Debug, Copy, Clone)]
pub(crate) enum Partial<T> {
    Incomparable,
    Filled(T),
}

use Partial::{Filled, Incomparable};

impl<T> Default for Partial<T> {
    fn default() -> Self {
        Self::Incomparable
    }
}

impl<T> From<Option<T>> for Partial<T> {
    fn from(o: Option<T>) -> Self {
        o.map_or(Partial::Incomparable, Partial::Filled)
    }
}

impl<T> Partial<T> {
    pub(crate) fn map<R>(self, mut f: impl FnMut(T) -> R) -> Partial<R> {
        match self {
            Self::Incomparable => Partial::Incomparable,
            Self::Filled(x) => Partial::Filled(f(x)),
        }
    }
    pub(crate) fn and<T2>(self, other: Partial<T2>) -> Partial<(T, T2)> {
        match (self, other) {
            (Self::Filled(a), Partial::Filled(b)) => Partial::Filled((a, b)),
            _ => Partial::Incomparable,
        }
    }
    pub(crate) fn option(self) -> Option<T> {
        match self {
            Self::Incomparable => None,
            Self::Filled(t) => Some(t),
        }
    }
    pub(crate) fn as_ref(&self) -> Partial<&T> {
        match self {
            Self::Incomparable => Partial::Incomparable,
            Self::Filled(t) => Partial::Filled(t),
        }
    }
    pub(crate) fn filter(self, enable: bool) -> Partial<T> {
        match (self, enable) {
            (Self::Filled(t), true) => Partial::Filled(t),
            _ => Partial::Incomparable,
        }
    }
}

impl<T> PartialEq for Partial<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Filled(a), Self::Filled(b)) => a.eq(b),
            _ => false,
        }
    }
}

pub(crate) struct CiteInCluster<O: OutputFormat> {
    pub cite_id: CiteId,
    pub cite: Arc<Cite<O>>,
    pub cnum: Partial<u32>,
    pub gen4: Arc<IrGen>,
    pub layout_destination: LayoutDestination,
    /// So we can look for punctuation at the end and use the format's quoting abilities
    pub prefix_parsed: Option<MarkupBuild>,

    /// First of a group of cites with the same name
    pub first_of_name: bool,
    /// Subsequent in a group of cites with the same name
    pub subsequent_same_name: bool,
    /// equivalent to using first_of_name + subsequent_same_name, except works with group_by
    pub unique_name_number: Partial<u32>,

    /// First of a group of cites with the same year, all with suffixes
    /// (same name implied)
    pub first_of_same_year: bool,
    /// Subsequent in a group of cites with the same year, all with suffixes
    /// (same name implied)
    pub subsequent_same_year: bool,
    /// equivalent to using first_of_same_year + subsequent_same_year, except works with group_by
    /// (within a unique_name_number group_by)
    pub range_collapse_key: RangeCollapseKey,

    pub year: Partial<SmartString>,
    pub year_suffix: Partial<u32>,

    /// Ranges of year suffixes (not alphabetic, in its base u32 form)
    /// (only applicable if first_of_ys == true)
    pub collapsed_year_suffixes: Vec<RangePiece>,

    /// Ranges of citation numbers
    /// (only applicable if first_of_ys == true)
    pub collapsed_ranges: Vec<RangePiece>,

    /// Tagging removed cites is cheaper than memmoving the rest of the Vec
    pub trailing_only: bool,

    pub has_locator: bool,
    pub suppress_delimiter: bool,
}

use std::fmt::{Debug, Formatter};

impl<O: OutputFormat<Output = SmartString>> Debug for CiteInCluster<O> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let fmt = &Markup::default();
        f.debug_struct("CiteInCluster")
            .field("cite_id", &self.cite_id)
            .field("cite", &self.cite)
            .field("cnum", &self.cnum)
            .field(
                "gen4",
                &self
                    .gen4
                    .tree_ref()
                    .flatten(fmt, None)
                    .map(|x| fmt.output(x, false)),
            )
            .field("extra_node", &self.layout_destination)
            .field("prefix_parsed", &self.prefix_parsed)
            .field("has_locator", &self.has_locator)
            .field("suppress_delimiter", &self.suppress_delimiter)
            .field("first_of_name", &self.first_of_name)
            .field("subsequent_same_name", &self.subsequent_same_name)
            .field("unique_name_number", &self.unique_name_number)
            .field("first_of_same_year", &self.first_of_same_year)
            .field("subsequent_same_year", &self.subsequent_same_year)
            .field("year", &self.range_collapse_key)
            .field("year_suffix", &self.year_suffix)
            .field("collapsed_year_suffixes", &self.collapsed_year_suffixes)
            .field("collapsed_ranges", &self.collapsed_ranges)
            .field("vanished", &self.trailing_only)
            .field("gen4_full", &self.gen4)
            .finish()
    }
}

impl CiteInCluster<Markup> {
    pub fn new(
        cite_id: CiteId,
        cite: Arc<Cite<Markup>>,
        cnum: Option<u32>,
        gen4: Arc<IrGen>,
        fmt: &Markup,
    ) -> Self {
        let prefix_parsed = cite.prefix.as_opt_str().map(|p| {
            fmt.ingest(
                p,
                &IngestOptions {
                    is_external: true,
                    ..Default::default()
                },
            )
        });
        CiteInCluster {
            cite_id,
            has_locator: cite.locators.is_some() && gen4.tree_ref().find_locator().is_some(),
            cite,
            gen4,
            layout_destination: LayoutDestination::default(),
            prefix_parsed,
            cnum: Partial::from(cnum),
            first_of_name: false,
            subsequent_same_name: false,
            unique_name_number: Partial::Incomparable,
            first_of_same_year: false,
            range_collapse_key: RangeCollapseKey::ForceSingle,
            subsequent_same_year: false,
            year: Partial::Incomparable,
            year_suffix: Partial::Incomparable,
            collapsed_year_suffixes: Vec::new(),
            collapsed_ranges: Vec::new(),
            trailing_only: false,
            suppress_delimiter: false,
        }
    }

    fn prefix_str(&self) -> Option<&str> {
        self.cite.prefix.as_ref().map(AsRef::as_ref)
    }
    fn suffix_str(&self) -> Option<&str> {
        self.cite.suffix.as_ref().map(AsRef::as_ref)
    }
}

////////////////////////////////
// Cite Grouping & Collapsing //
////////////////////////////////

/// For styles which refer to a citation number and want ranges of them collapsed.
///
/// > [1, 2-4, 5]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Collapsible {
    /// Cnum is sometimes "citation number", sometimes year suffix, whatever's being collapsed.
    pub number: u32,
    /// The index of a citation number in a sequence of them
    pub ix: usize,
    /// Don't include this in a [RangePiece::Range]
    pub force_single: bool,
}

impl Collapsible {
    fn new(c: u32, ix: usize) -> Self {
        Collapsible {
            number: c,
            ix,
            force_single: false,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RangePiece {
    /// If the length of the range is only two, it should be rendered with a comma anyway
    Range(Collapsible, Collapsible),
    Single(Collapsible),
}

impl RangePiece {
    /// Return value is the previous value, to be emitted, if the next item couldn't be appended
    fn try_append(&mut self, nxt: Collapsible) -> Option<RangePiece> {
        *self = match self {
            _ if nxt.force_single => return Some(std::mem::replace(self, RangePiece::Single(nxt))),
            RangePiece::Single(prv) if prv.number == nxt.number - 1 => RangePiece::Range(*prv, nxt),
            RangePiece::Range(_, end) if end.number == nxt.number - 1 => {
                *end = nxt;
                return None;
            }
            _ => return Some(std::mem::replace(self, RangePiece::Single(nxt))),
        };
        return None;
    }
}

#[test]
fn range_append() {
    let mut range = RangePiece::Single(Collapsible::new(1, 1));
    let emit = range.try_append(Collapsible::new(2, 2));
    assert_eq!(
        (range, emit),
        (
            RangePiece::Range(Collapsible::new(1, 1), Collapsible::new(2, 2)),
            None
        )
    );
    let mut range = RangePiece::Single(Collapsible::new(1, 1));
    let emit = range.try_append(Collapsible::new(3, 2));
    assert_eq!(
        (range, emit),
        (
            RangePiece::Single(Collapsible::new(3, 2)),
            Some(RangePiece::Single(Collapsible::new(1, 1)))
        )
    );
}

pub fn collapse_collapsible_ranges(nums: &[Collapsible]) -> Vec<RangePiece> {
    let mut pieces = Vec::new();
    if let Some(init) = nums.first() {
        let mut wip = RangePiece::Single(*init);
        for &num in &nums[1..] {
            if let Some(emit) = wip.try_append(num) {
                pieces.push(emit);
            }
        }
        pieces.push(wip);
    }
    pieces
}

#[test]
fn range_collapse() {
    let s = |cnum: u32| Collapsible::new(cnum, cnum as usize);
    assert_eq!(
        collapse_collapsible_ranges(&[s(1), s(2), s(3)]),
        vec![RangePiece::Range(s(1), s(3))]
    );
    assert_eq!(
        collapse_collapsible_ranges(&[s(1), s(2), Collapsible::new(4, 3)]),
        vec![
            RangePiece::Range(s(1), s(2)),
            RangePiece::Single(Collapsible::new(4, 3))
        ]
    );
}

pub(crate) fn group_and_collapse<O: OutputFormat<Output = SmartString>>(
    fmt: &Markup,
    collapse: Option<Collapse>,
    cites: &mut Vec<CiteInCluster<O>>,
) {
    // Neat trick: same_names[None] tracks cites without a cs:names block, which helps with styles
    // that only include a year. (What kind of style is that?
    // magic_ImplicitYearSuffixExplicitDelimiter.txt, I guess that's the only possible reason, but
    // ok.)
    let mut same_names: HashMap<Option<SmartString>, (usize, bool)> = HashMap::new();
    // let mut same_years: HashMap<SmartString, (usize, bool)> = HashMap::new();

    // First, group cites with the same name
    let mut unique_name = 1;
    for ix in 0..cites.len() {
        let gen4 = &cites[ix].gen4;
        let tree = gen4.tree_ref();
        let rendered = tree
            .first_names_block()
            .and_then(|node| tree.with_node(node).flatten(fmt, None))
            .map(|flat| fmt.output(flat, false));
        same_names
            .entry(rendered)
            .and_modify(|(oix, seen_once)| {
                // Keep cites separated by affixes together
                if cites.get(*oix).map_or(false, |u| u.cite.has_suffix())
                    || cites.get(*oix + 1).map_or(false, |u| u.cite.has_prefix())
                    || cites.get(ix - 1).map_or(false, |u| u.cite.has_suffix())
                    || cites.get(ix).map_or(false, |u| u.cite.has_affix())
                {
                    *oix = ix;
                    *seen_once = false;
                    return;
                }
                if *oix < ix {
                    if !*seen_once {
                        cites[*oix].first_of_name = true;
                        cites[*oix].unique_name_number = Partial::Filled(unique_name);
                        unique_name += 1;
                    }
                    *seen_once = true;
                    cites[ix].subsequent_same_name = true;
                    cites[ix].unique_name_number = cites[*oix].unique_name_number;
                    let rotation = &mut cites[*oix + 1..ix + 1];
                    rotation.rotate_right(1);
                    *oix += 1;
                }
            })
            .or_insert((ix, false));
    }

    if collapse.map_or(false, |c| {
        c == Collapse::YearSuffixRanged || c == Collapse::YearSuffix
    }) {
        let name_runs = group_by_mut(cites.as_mut(), |a, b| {
            a.unique_name_number == b.unique_name_number
        });
        for run in name_runs {
            let mut ix = 0;
            for cite in run.iter_mut() {
                let tree = cite.gen4.tree_ref();
                let year_and_suf = tree
                    .find_first_year_and_suffix()
                    .and_then(|(ys_node, suf)| {
                        let ys_tree = tree.with_node(ys_node);
                        let flat = ys_tree.flatten(fmt, None)?;
                        Some((fmt.output(flat, false), suf))
                    });
                if let Some((y, suf)) = year_and_suf {
                    cite.year = Partial::Filled(y);
                    cite.year_suffix = Partial::Filled(suf);
                }
            }
            for run in group_by_mut(run, |a, b| a.year.as_ref() == b.year.as_ref()) {
                if run.len() > 1 {
                    run[0].first_of_same_year = true;
                    for cite in &mut run[1..] {
                        cite.subsequent_same_year = true;
                    }
                }
            }
        }
    }

    if collapse == Some(Collapse::CitationNumber) {
        // XXX: Gotta factor in that some might have prefixes and suffixes
        if let Some((first, rest)) = cites.split_first_mut() {
            first.first_of_name = true;
            for r in rest {
                r.subsequent_same_name = true;
            }
        }
    }

    impl<O: OutputFormat> CiteInCluster<O> {
        fn by_year(&self) -> Partial<&SmartString> {
            self.year.as_ref().filter(!self.has_locator)
        }
        fn by_name(&self) -> Partial<u32> {
            self.unique_name_number
                .filter(!self.has_locator)
                .filter(self.cite.mode != Some(CiteMode::AuthorOnly))
        }
    }

    if let Some(collapse) = collapse {
        match collapse {
            Collapse::CitationNumber => {
                let by_name = group_by_mut(cites.as_mut(), |a, b| a.by_name() == b.by_name());
                let mut ix = 0;
                for name_run in by_name {
                    let mut cnums = Vec::new();
                    if let Filled(cnum) = name_run[0].cnum {
                        let mut c = Collapsible::new(cnum, ix);
                        c.force_single = name_run.len() == 1;
                        cnums.push(c);
                    }
                    for (run_ix, cite) in name_run[1..].iter_mut().enumerate() {
                        let run_ix = run_ix + 1;
                        if let Filled(cnum) = cite.cnum {
                            cnums.push(Collapsible {
                                number: cnum,
                                ix: ix + run_ix,
                                force_single: false,
                                // force_single: cite.has_locator
                                //     || cite.cite.mode == Some(CiteMode::AuthorOnly),
                            })
                        }
                        cite.trailing_only = true;
                    }
                    name_run[0].collapsed_ranges = collapse_collapsible_ranges(&cnums);
                    ix += name_run.len();
                }
            }
            Collapse::Year => {
                let by_name = group_by_mut(cites.as_mut(), |a, b| {
                    a.unique_name_number == b.unique_name_number
                });
                for run in by_name {
                    // runs are non-empty
                    for cite in &mut run[1..] {
                        let gen4 = Arc::make_mut(&mut cite.gen4);
                        gen4.tree_mut().suppress_names();
                    }
                }
            }
            Collapse::YearSuffixRanged | Collapse::YearSuffix => {
                let by_name = group_by_mut(cites.as_mut(), |a, b| a.by_name() == b.by_name());
                let mut ix = 0;
                for name_run in by_name {
                    for cite in &mut name_run[1..] {
                        let gen4 = Arc::make_mut(&mut cite.gen4);
                        gen4.tree_mut().suppress_names()
                    }
                    let by_year = group_by_mut(name_run, |a, b| a.by_year() == b.by_year());
                    let mut nix = 0;
                    for year_run in by_year {
                        if collapse == Collapse::YearSuffixRanged {
                            // Potentially confusing: cnums here are year suffixes in u32 form
                            let mut cnums = Vec::new();
                            if let Filled(suf) = year_run[0].year_suffix {
                                let mut c = Collapsible::new(suf, ix + nix);
                                c.force_single = year_run.len() == 1;
                                cnums.push(c);
                            }
                            for (yix, cite) in year_run[1..].iter_mut().enumerate() {
                                let yix = yix + 1;
                                if let Filled(cnum) = cite.year_suffix {
                                    cnums.push(Collapsible {
                                        number: cnum,
                                        ix: ix + nix + yix,
                                        force_single: cite.has_locator,
                                    });
                                }
                                cite.trailing_only = true;
                                if !cite.has_locator {
                                    let gen4 = Arc::make_mut(&mut cite.gen4);
                                    gen4.tree_mut().suppress_year();
                                }
                            }
                            year_run[0].collapsed_year_suffixes =
                                collapse_collapsible_ranges(&cnums);
                        } else {
                            let mut range_pieces = Vec::new();
                            if let Some(cnum) = year_run[0].year_suffix.option() {
                                range_pieces
                                    .push(RangePiece::Single(Collapsible::new(cnum, ix + nix)));
                            }
                            for (yix, cite) in year_run[1..].iter_mut().enumerate() {
                                let yix = yix + 1;
                                if let Some(cnum) = cite.year_suffix.option() {
                                    range_pieces.push(RangePiece::Single(Collapsible {
                                        number: cnum,
                                        ix: ix + nix + yix,
                                        force_single: cite.has_locator,
                                    }));
                                }
                                cite.trailing_only = true;
                                let gen4 = Arc::make_mut(&mut cite.gen4);
                                gen4.tree_mut().suppress_year();
                            }
                            year_run[0].collapsed_year_suffixes = range_pieces;
                        }
                        nix += year_run.len();
                    }
                    ix += name_run.len();
                }
            }
        }
    }
}

////////////////////////////////
// Cluster Modes & Cite Modes //
////////////////////////////////

fn render_composite_infix<O: OutputFormat>(
    infix: Option<Option<&str>>,
    fmt: &O,
) -> Option<O::Build> {
    let mut infix: SmartString = infix?.unwrap_or(" ").into();
    if !infix.ends_with(" ") {
        infix.push(' ');
    }
    let is_punc = |c| unic_ucd_category::GeneralCategory::of(c).is_punctuation();
    if !infix
        .chars()
        .nth(0)
        .map_or(true, |x| x == ' ' || is_punc(x))
    {
        infix.insert(0, ' ');
    }
    Some(fmt.ingest(
        &infix,
        &IngestOptions {
            is_external: true,
            ..Default::default()
        },
    ))
}
