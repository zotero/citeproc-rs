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

    if let Some(maybe_collapse) = style.citation.group_collapsing() {
        group_and_maybe_collapse(&fmt, maybe_collapse, &mut irs);
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

    fn flatten_affix_cite(
        cites: &[CiteInCluster<Markup>],
        ix: usize,
        fmt: &Markup,
    ) -> Option<(Option<SmartString>, MarkupBuild, Option<SmartString>)> {
        Some(layout::flatten_with_affixes(cites.get(ix)?, fmt))
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
                    stream.write_flat(&irs[ix], None);
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

    let merged_locale = db.default_locale();
    let citation_delims = layout::LayoutDelimiters::from_citation(&style.citation);
    let intext_delimiters =
        layout::LayoutDelimiters::from_intext(intext_el, citation_el, &merged_locale);

    log::debug!("citation_delims: {:?}", citation_delims);

    let mut citation_stream = layout::LayoutStream::new(irs.len() * 2, citation_delims, fmt);
    let mut intext_stream = layout::LayoutStream::new(0, intext_delimiters, fmt);

    // render the intext stream
    let intext_authors = group_by(irs.as_slice(), |a, b| a.by_name() == b.by_name())
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
                // this is something @fbennett made up specifically for author-only.
                .flatten_or_plain(fmt, "[NO_PRINTED_FORM]")
        });

    intext_stream.write_interspersed(intext_authors, DelimKind::Layout);

    for cite in &irs {
        if cite.trailing_only || cite.layout_destination == LayoutDestination::MainToIntext {
            continue;
        }
        citation_stream.write_flat(cite, None);
    }

    log::debug!("citation_stream: {:#?}", &citation_stream);
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
    pub collapsed_citation_numbers: Vec<RangePiece>,

    /// Tagging removed cites is cheaper than memmoving the rest of the Vec
    pub trailing_only: bool,

    pub has_locator: bool,
    pub has_locator_or_affixes: bool,
    pub own_delimiter: Option<DelimKind>,
    pub suppress_delimiter: bool,
}

impl<O: OutputFormat> CiteInCluster<O> {
    fn by_year(&self) -> Partial<&SmartString> {
        self.year.as_ref() //.filter(!self.has_locator)
    }
    fn by_name(&self) -> Partial<u32> {
        self.unique_name_number
        // .filter(!self.has_locator)
        // .filter(self.cite.mode != Some(CiteMode::AuthorOnly))
    }
    fn isolate_loc_affix(&self) -> Partial<()> {
        Partial::Filled(()).filter(!self.has_locator_or_affixes)
    }
    fn by_year_suffix(&self) -> Partial<u32> {
        self.year_suffix
    }
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
            .field("has_locator_or_affixes", &self.has_locator_or_affixes)
            .field("own_delimiter", &self.own_delimiter)
            .field("suppress_delimiter", &self.suppress_delimiter)
            .field("first_of_name", &self.first_of_name)
            .field("subsequent_same_name", &self.subsequent_same_name)
            .field("unique_name_number", &self.unique_name_number)
            .field("first_of_same_year", &self.first_of_same_year)
            .field("subsequent_same_year", &self.subsequent_same_year)
            .field("year", &self.range_collapse_key)
            .field("year_suffix", &self.year_suffix)
            .field("collapsed_year_suffixes", &self.collapsed_year_suffixes)
            .field("collapsed_ranges", &self.collapsed_citation_numbers)
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
        let has_locator = cite.locators.is_some() && gen4.tree_ref().find_locator().is_some();
        CiteInCluster {
            cite_id,
            has_locator,
            has_locator_or_affixes: has_locator || cite.has_affix(),
            own_delimiter: Some(DelimKind::Layout),
            cite,
            gen4,
            layout_destination: LayoutDestination::default(),
            prefix_parsed,
            cnum: Partial::from(cnum),
            first_of_name: false,
            subsequent_same_name: false,
            // by default, no names are in groups.
            unique_name_number: Partial::Filled(0),
            first_of_same_year: false,
            range_collapse_key: RangeCollapseKey::ForceSingle,
            subsequent_same_year: false,
            year: Partial::Incomparable,
            year_suffix: Partial::Incomparable,
            collapsed_year_suffixes: Vec::new(),
            collapsed_citation_numbers: Vec::new(),
            trailing_only: false,
            suppress_delimiter: false,
        }
    }

    pub(crate) fn prefix_str(&self) -> Option<&str> {
        self.cite.prefix.as_ref().map(AsRef::as_ref)
    }
    pub(crate) fn suffix_str(&self) -> Option<&str> {
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

pub(crate) fn group_and_maybe_collapse<O: OutputFormat<Output = SmartString>>(
    fmt: &Markup,
    collapse: Option<Collapse>,
    cites: &mut Vec<CiteInCluster<O>>,
) {
    // Neat trick: same_names[None] tracks cites without a cs:names block, which helps with styles
    // that only include a year. (What kind of style is that?
    // magic_ImplicitYearSuffixExplicitDelimiter.txt, I guess that's the only possible reason, but
    // ok.)
    let mut same_names: HashMap<Option<SmartString>, (usize, bool, Partial<u32>)> = HashMap::new();
    // let mut same_years: HashMap<SmartString, (usize, bool)> = HashMap::new();

    // First, group cites with the same name
    if matches!(
        collapse,
        None | Some(Collapse::Year) | Some(Collapse::YearSuffix) | Some(Collapse::YearSuffixRanged)
    ) {
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
                .and_modify(|(oix, seen_local, name_number)| {
                    // set the name number on all of them
                    cites[ix].unique_name_number = *name_number;

                    // Keep cites separated by affixes together
                    // seen_local tracks whether we're the first to see this name since we reset
                    if cites
                        .get(oix.saturating_sub(1))
                        .map_or(false, |u| u.cite.has_suffix())
                        || cites.get(*oix).map_or(false, |u| u.cite.has_affix())
                        || cites.get(*oix + 1).map_or(false, |u| u.cite.has_prefix())
                        || cites
                            .get(ix.saturating_sub(1))
                            .map_or(false, |u| u.cite.has_suffix())
                        || cites.get(ix).map_or(false, |u| u.cite.has_affix())
                        || cites.get(ix + 1).map_or(false, |u| u.cite.has_prefix())
                    {
                        *oix = ix;
                        *seen_local = false;
                        return;
                    }
                    if *oix < ix {
                        if !*seen_local {
                            cites[*oix].first_of_name = true;
                        }
                        *seen_local = true;
                        cites[ix].subsequent_same_name = true;
                        let rotation = &mut cites[*oix + 1..ix + 1];
                        rotation.rotate_right(1);
                        *oix += 1;
                    }
                })
                .or_insert_with(|| {
                    let seen_local = true;
                    let name_number = Partial::Filled(unique_name);
                    cites[ix].unique_name_number = name_number;
                    unique_name += 1;
                    (ix, seen_local, name_number)
                });
        }
    }

    // Unconditional; cover the group only, no collapse case
    let name_runs = group_by_mut(cites.as_mut(), |a, b| a.by_name() == b.by_name());
    for run in name_runs {
        log::debug!(
            "group only name_run: {:?}",
            run.iter().map(|x| x.unique_name_number).collect::<Vec<_>>()
        );
        let set_delim = |cite: &mut CiteInCluster<O>| {
            if !cite.has_locator_or_affixes {
                cite.own_delimiter = Some(DelimKind::CiteGroup);
            }
        };
        match run {
            [] => log::warn!("run of same name should never be empty"),
            [single] => {
                // this is the default
                // single.own_delimiter = Some(DelimKind::Layout);
            }
            [head, middle @ .., last] => {
                set_delim(head);
                for cite in middle {
                    // XXX: we kinda need to know if the following cite is going to
                    // have a prefix... unless we fix it up in LayoutStream::finish
                    set_delim(cite);
                }
                last.own_delimiter = if collapse.is_some() {
                    Some(DelimKind::AfterCollapse)
                } else {
                    Some(DelimKind::Layout)
                };
            }
        }
    }

    if collapse.map_or(false, |c| {
        c == Collapse::YearSuffixRanged || c == Collapse::YearSuffix
    }) {
        let name_runs = group_by_mut(cites.as_mut(), |a, b| a.by_name() == b.by_name());
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

    fn suppress_names<O: OutputFormat>(cite: &mut CiteInCluster<O>) {
        let gen4 = Arc::make_mut(&mut cite.gen4);
        gen4.tree_mut().suppress_names()
    }

    if let Some(collapse) = collapse {
        log::debug!("collapse = {:?}", collapse);
        match collapse {
            Collapse::CitationNumber => {
                let monotonic_runs = group_by_mut(cites, |a, b| {
                    a.cnum.map(|x| x + 1) == b.cnum && a.isolate_loc_affix() == b.isolate_loc_affix()
                });
                for run in monotonic_runs {
                    match run {
                        [] => log::warn!("run of citation numbers should never be empty"),
                        [single] => {
                            single.own_delimiter = Some(DelimKind::Layout);
                        }
                        [head, middle @ .., last] => {
                            head.own_delimiter = Some(if !middle.is_empty() {
                                DelimKind::Range
                            } else {
                                DelimKind::Layout
                            });
                            for ignored in middle {
                                ignored.trailing_only = true;
                            }
                            last.own_delimiter = Some(DelimKind::AfterCollapse);
                        }
                    }
                }
            }
            Collapse::Year => {
                let mut by_name =
                    group_by_mut(cites.as_mut(), |a, b| a.by_name() == b.by_name()).peekable();
                while let Some(name_run) = by_name.next() {
                    log::debug!(
                        "name_run: {:?}",
                        name_run.iter()
                            .map(|x| (x.unique_name_number, x.own_delimiter))
                            .collect::<Vec<_>>()
                    );
                    let delim_for_cite = |cite: &CiteInCluster<O>| {
                        if !cite.has_locator_or_affixes {
                            Some(DelimKind::CiteGroup)
                        } else {
                            Some(DelimKind::AfterCollapse)
                        }
                    };
                    match name_run {
                        [] => log::warn!("run of same name should never be empty"),
                        [single] => {}
                        [head, middle @ .., last] => {
                            head.own_delimiter = delim_for_cite(head);
                            for cite in middle {
                                suppress_names(cite);
                                // XXX: we kinda need to know if the following cite is going to
                                // have a prefix...
                                cite.own_delimiter = delim_for_cite(cite);
                            }
                            suppress_names(last);
                        }
                    }
                }
            }
            Collapse::YearSuffixRanged | Collapse::YearSuffix => {
                let ranged = collapse == Collapse::YearSuffixRanged;
                let mut by_name =
                    group_by_mut(cites.as_mut(), |a, b| a.by_name() == b.by_name()).peekable();
                while let Some(name_run) = by_name.next() {
                    let name_run_end_delim = name_run.last().and_then(|l| l.own_delimiter);
                    // suppress names in the tail.
                    for cite in &mut name_run[1..] {
                        let gen4 = Arc::make_mut(&mut cite.gen4);
                        gen4.tree_mut().suppress_names();
                    }
                    let by_year = group_by_mut(name_run, |a, b| a.by_year() == b.by_year());
                    for year_run in by_year {
                        let monotonic_nonaffixed_ysufs = group_by_mut(year_run, |a, b| {
                            a.by_year_suffix().map(|ysuf| ysuf + 1) == b.by_year_suffix()
                                && a.isolate_loc_affix() == b.isolate_loc_affix()
                        });
                        for (ysuf_ix, ysuf_run) in monotonic_nonaffixed_ysufs.enumerate() {
                            collapse_year_suffix_run(ysuf_run, ysuf_ix == 0, ranged);
                            log::debug!(
                                "ysuf_run: {:?}",
                                ysuf_run
                                    .iter()
                                    .map(|x| (x.year.clone(), x.year_suffix))
                                    .collect::<Vec<_>>()
                            );
                        }
                        year_run.last_mut().unwrap().own_delimiter = Some(DelimKind::CiteGroup);
                    }
                    log::debug!(
                        "name_run: {:?}",
                        name_run
                            .iter()
                            .map(|x| x.unique_name_number)
                            .collect::<Vec<_>>()
                    );
                    name_run.last_mut().unwrap().own_delimiter = name_run_end_delim;
                }
            }
        }
    }
}

fn collapse_year_suffix_run<O: OutputFormat>(
    ysuf_run: &mut [CiteInCluster<O>],
    is_first_ysuf_run: bool,
    ranged: bool,
) {
    fn suppress_year<O: OutputFormat>(cite: &mut CiteInCluster<O>) {
        let gen4 = Arc::make_mut(&mut cite.gen4);
        gen4.tree_mut().suppress_year()
    }

    let delim_for_cite = |cite: &CiteInCluster<O>, d: DelimKind| {
        if !cite.has_locator_or_affixes {
            Some(d)
        } else {
            Some(DelimKind::AfterCollapse)
        }
    };
    match ysuf_run {
        [] => log::warn!("run of year suffixes should never be empty"),
        [single] => {
            if ranged && !single.has_locator_or_affixes && !is_first_ysuf_run {
                suppress_year(single);
            }
            single.own_delimiter = delim_for_cite(single, DelimKind::YearSuffix);
        }
        [head, middle @ .., last] if ranged => {
            if !is_first_ysuf_run {
                suppress_year(head);
            }
            head.own_delimiter = Some(if !middle.is_empty() {
                DelimKind::Range
            } else {
                DelimKind::YearSuffix
            });
            for ignored in middle {
                ignored.trailing_only = true;
            }
            suppress_year(last);
            last.own_delimiter = delim_for_cite(last, DelimKind::YearSuffix);
        }
        [head, middle @ .., last] => {
            if !is_first_ysuf_run {
                suppress_year(head);
            }
            head.own_delimiter = Some(DelimKind::YearSuffix);
            for cite in middle {
                suppress_year(cite);
                cite.own_delimiter = Some(DelimKind::YearSuffix);
            }
            suppress_year(last);
            last.own_delimiter = delim_for_cite(last, DelimKind::YearSuffix);
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
