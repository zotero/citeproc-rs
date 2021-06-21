use std::collections::HashMap;
use std::sync::Arc;

use citeproc_db::ClusterId;
use citeproc_io::{Cite, CiteMode, ClusterMode};
use csl::Collapse;

use crate::db::IrGen;
use crate::ir::transforms;
use crate::prelude::*;

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
    if let Some(mode) = cluster_mode {
        transforms::apply_cluster_mode(db, &fmt, mode, &mut irs);
    }

    // Cite capitalization
    // TODO: allow clients to pass a flag to prevent this (on ix==0) when a cluster is in the
    // middle of an existing footnote, and isn't preceded by a period (or however else a client
    // wants to judge that).
    // We capitalize all cites whose prefixes end with full stops.
    for (
        ix,
        CiteInCluster {
            gen4,
            prefix_parsed,
            ..
        },
    ) in irs.iter_mut().enumerate()
    {
        if style.class != csl::StyleClass::InText
            && prefix_parsed
                .as_ref()
                .map_or(ix == 0, |pre| fmt.ends_with_full_stop(pre))
        {
            // dbg!(ix, prefix_parsed);
            let gen_mut = Arc::make_mut(gen4);
            gen_mut.tree_mut().capitalize_first_term_of_cluster(&fmt);
        }
    }

    // csl_test_suite::affix_WithCommas.txt
    let suppress_delimiter = |cites: &[CiteInCluster<Markup>], ix: usize| -> bool {
        let this_suffix = match cites.get(ix) {
            Some(x) => x.cite.suffix.as_ref().map(AsRef::as_ref).unwrap_or(""),
            None => "",
        };
        let next_prefix = match cites.get(ix + 1) {
            Some(x) => x.cite.prefix.as_ref().map(AsRef::as_ref).unwrap_or(""),
            None => "",
        };
        let ends_punc = |string: &str| {
            string
                .chars()
                .rev()
                .nth(0)
                .map_or(false, |x| x == ',' || x == '.' || x == '?' || x == '!')
        };
        let starts_punc = |string: &str| {
            string
                .chars()
                .nth(0)
                .map_or(false, |x| x == ',' || x == '.' || x == '?' || x == '!')
        };

        // "2000 is one source,; David Jones" => "2000 is one source, David Jones"
        // "2000;, and David Jones" => "2000, and David Jones"
        ends_punc(this_suffix) || starts_punc(next_prefix)
    };

    let flatten_affix_unnamed =
        |unnamed: &CiteInCluster<Markup>, cite_is_last: bool| -> MarkupBuild {
            let CiteInCluster { cite, gen4, .. } = unnamed;
            use std::borrow::Cow;
            let flattened = gen4.tree_ref().flatten_or_plain(&fmt, CSL_STYLE_ERROR);
            let mut pre = Cow::from(cite.prefix.as_ref().map(AsRef::as_ref).unwrap_or(""));
            let mut suf = Cow::from(cite.suffix.as_ref().map(AsRef::as_ref).unwrap_or(""));
            if !pre.is_empty() && !pre.ends_with(' ') {
                let pre_mut = pre.to_mut();
                pre_mut.push(' ');
            }
            let suf_first = suf.chars().nth(0);
            if suf_first.map_or(false, |x| {
                x != ' ' && !citeproc_io::output::markup::is_punc(x)
            }) {
                let suf_mut = suf.to_mut();
                suf_mut.insert_str(0, " ");
            }
            let suf_last_punc = suf.chars().rev().nth(0).map_or(false, |x| {
                x == ',' || x == '.' || x == '!' || x == '?' || x == ':'
            });
            if suf_last_punc && !cite_is_last {
                let suf_mut = suf.to_mut();
                suf_mut.push(' ');
            }
            let opts = IngestOptions {
                is_external: true,
                ..Default::default()
            };
            let prefix_parsed = fmt.ingest(&pre, &opts);
            let suffix_parsed = fmt.ingest(&suf, &opts);
            // TODO: custom procedure for joining user-supplied cite affixes, which should interact
            // with terminal punctuation by overriding rather than joining in the usual way.
            use std::iter::once;
            fmt.seq(
                once(prefix_parsed)
                    .chain(once(flattened))
                    .chain(once(suffix_parsed)),
            )
        };
    let flatten_affix_cite = |cites: &[CiteInCluster<Markup>], ix: usize| -> Option<MarkupBuild> {
        Some(flatten_affix_unnamed(cites.get(ix)?, ix == cites.len() - 1))
    };

    let cgroup_delim = style
        .citation
        .cite_group_delimiter
        .as_opt_str()
        .unwrap_or(", ");
    let ysuf_delim = style
        .citation
        .year_suffix_delimiter
        .as_opt_str()
        .or(style.citation.layout.delimiter.as_opt_str())
        .unwrap_or("");
    let acol_delim = style
        .citation
        .after_collapse_delimiter
        .as_opt_str()
        .or(style.citation.layout.delimiter.as_opt_str())
        .unwrap_or("");
    let layout_delim = style.citation.layout.delimiter.as_ref();

    let intext_el = style.intext.as_ref();
    let intext_delim = intext_el.map_or("", |x| x.layout.delimiter.as_opt_str().unwrap_or(""));
    let intext_pre = intext_el.map_or("", |x| {
        x.layout
            .affixes
            .as_ref()
            .map_or("", |af| af.prefix.as_str())
    });
    let intext_suf = intext_el.map_or("", |x| {
        x.layout
            .affixes
            .as_ref()
            .map_or("", |af| af.suffix.as_str())
    });

    enum OutputChannels {
        CitationLayout(MarkupBuild),
        SplitIntextCitation(MarkupBuild, MarkupBuild),
        IntextLayout(MarkupBuild),
    }

    // returned usize is advance len
    let render_range =
        |ranges: &[RangePiece], group_delim: &str, outer_delim: &str| -> (MarkupBuild, usize) {
            let mut advance_to = 0usize;
            let mut group: Vec<MarkupBuild> = Vec::with_capacity(ranges.len());
            for (ix, piece) in ranges.iter().enumerate() {
                let is_last = ix == ranges.len() - 1;
                match *piece {
                    RangePiece::Single(Collapsible {
                        ix, force_single, ..
                    }) => {
                        advance_to = ix;
                        if let Some(one) = flatten_affix_cite(&irs, ix) {
                            group.push(one);
                            if !is_last && !suppress_delimiter(&irs, ix) {
                                group.push(fmt.plain(if force_single {
                                    outer_delim
                                } else {
                                    group_delim
                                }));
                            }
                        }
                    }
                    RangePiece::Range(start, end) => {
                        advance_to = end.ix;
                        let mut delim = "\u{2013}";
                        if start.number == end.number - 1 {
                            // Not represented as a 1-2, just two sequential numbers 1,2
                            delim = group_delim;
                        }
                        let mut g = vec![];
                        if let Some(start) = flatten_affix_cite(&irs, start.ix) {
                            g.push(start);
                        }
                        if let Some(end) = flatten_affix_cite(&irs, end.ix) {
                            g.push(end);
                        }
                        // Delimiters here are never suppressed by build_cite, as they wouldn't be part
                        // of the range if they had affixes on the inside
                        group.push(fmt.group(g, delim, None));
                        if !is_last && !suppress_delimiter(&irs, end.ix) {
                            group.push(fmt.plain(group_delim));
                        }
                    }
                }
            }
            (fmt.group(group, "", None), advance_to)
        };

    let mut intext_authors = Vec::with_capacity(irs.len() * 2);
    let mut built_cites = Vec::with_capacity(irs.len() * 2);

    let mut push_built_cite = |output: OutputChannels, ix: usize| {
        match output {
            // <intext><layout>; joined later with the intext layout's delimiter/affixes
            OutputChannels::IntextLayout(intext) => intext_authors.push(intext),
            // the normal one, <citation><layout>, joined later witht the citation layout's
            // delimiter/affixes
            OutputChannels::CitationLayout(cite) => built_cites.push(cite),
            // one into each
            OutputChannels::SplitIntextCitation(intext, cite) => {
                intext_authors.push(intext);
                built_cites.push(cite);
            }
        }
    };

    // render the intext stream
    for CiteInCluster {
        gen4,
        layout_destination,
        ..
    } in irs.iter()
    {
        match layout_destination {
            LayoutDestination::Nowhere => {}
            LayoutDestination::MainToCitation => {}
            LayoutDestination::MainToIntext => {}
            LayoutDestination::MainToCitationPlusIntext(intext_node) => {}
        }
    }

    let mut ix = 0;

    while ix < irs.len() {
        let CiteInCluster {
            vanished,
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
            let (built, advance_to) = render_range(
                collapsed_ranges,
                layout_delim.as_opt_str().unwrap_or(""),
                acol_delim,
            );
            built_cites.push(built);
            if !suppress_delimiter(&irs, ix) {
                built_cites.push(fmt.plain(acol_delim));
            } else {
                built_cites.push(fmt.plain(""));
            }
            ix = advance_to + 1;
        } else if *first_of_name {
            let mut group = Vec::with_capacity(4);
            let mut rix = ix;
            while rix < irs.len() {
                let r = &irs[rix];
                if rix != ix && !r.subsequent_same_name {
                    break;
                }
                if !r.collapsed_year_suffixes.is_empty() {
                    let (built, advance_to) =
                        render_range(&r.collapsed_year_suffixes, ysuf_delim, acol_delim);
                    group.push(built);
                    if !suppress_delimiter(&irs, ix) {
                        group.push(fmt.plain(cgroup_delim));
                    } else {
                        group.push(fmt.plain(""));
                    }
                    rix = advance_to;
                } else {
                    if let Some(b) = flatten_affix_cite(&irs, rix) {
                        group.push(b);
                        if !suppress_delimiter(&irs, ix) {
                            group.push(fmt.plain(if irs[rix].has_locator {
                                acol_delim
                            } else {
                                cgroup_delim
                            }));
                        } else {
                            group.push(fmt.plain(""));
                        }
                    }
                }
                rix += 1;
            }
            group.pop();
            built_cites.push(fmt.group(group, "", None));
            if !suppress_delimiter(&irs, ix) {
                built_cites.push(fmt.plain(acol_delim));
            } else {
                built_cites.push(fmt.plain(""));
            }
            ix = rix;
        } else {
            if let Some(built) = flatten_affix_cite(&irs, ix) {
                built_cites.push(built);
                if !suppress_delimiter(&irs, ix) {
                    built_cites.push(fmt.plain(layout_delim.as_opt_str().unwrap_or("")));
                } else {
                    built_cites.push(fmt.plain(""));
                }
            }
            ix += 1;
        }
    }
    built_cites.pop();

    fmt.with_format(
        fmt.affixed(fmt.group(built_cites, "", None), layout.affixes.as_ref()),
        layout.formatting,
    )
}

struct LayoutStream<'a> {
    group: Vec<MarkupBuild>,
    layout_delim: Option<&'a str>,
    default_delim: Option<&'a str>,
    affixes: Option<&'a Affixes>,
    fmt: &'a Markup,
    formatting: Option<Formatting>,
}

impl<'a> LayoutStream<'a> {
    fn write(&mut self, built: MarkupBuild) {}
    fn finish(self) -> MarkupBuild {
        let Self {
            group,
            layout_delim,
            affixes,
            formatting,
            fmt,
            default_delim: _,
        } = self;
        // we maintain either a delimiter or an extra element at the end all the way through, just for this
        fmt.with_format(fmt.affixed(fmt.group(group, "", None), affixes), formatting)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayoutDestination {
    /// Do not render this cite anywhere, it is in the middle of a collapsed range.
    Nowhere,
    /// Take gen4's tree and put it in the `<citation><layout>` stream
    MainToCitation,
    /// Take gen4's tree and put it in the `<intext><layout>` stream
    MainToIntext,
    /// Take gen4's tree and put it in the `<citation><layout>` stream, and then put this detached node
    /// into the `<intext><layout>` stream
    MainToCitationPlusIntext(NodeId),
}

impl Default for LayoutDestination {
    fn default() -> Self {
        LayoutDestination::MainToCitation
    }
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

pub(crate) struct CiteInCluster<O: OutputFormat> {
    pub cite_id: CiteId,
    pub cite: Arc<Cite<O>>,
    pub cnum: Option<u32>,
    pub gen4: Arc<IrGen>,
    pub layout_destination: LayoutDestination,
    /// So we can look for punctuation at the end and use the format's quoting abilities
    pub prefix_parsed: Option<MarkupBuild>,

    /// First of a group of cites with the same name
    pub first_of_name: bool,
    /// Subsequent in a group of cites with the same name
    pub subsequent_same_name: bool,
    /// equivalent to using first_of_name + subsequent_same_name, except works with group_by
    pub unique_name_number: u32,

    /// First of a group of cites with the same year, all with suffixes
    /// (same name implied)
    pub first_of_same_year: bool,
    /// Subsequent in a group of cites with the same year, all with suffixes
    /// (same name implied)
    pub subsequent_same_year: bool,
    /// equivalent to using first_of_same_year + subsequent_same_year, except works with group_by
    /// (within a unique_name_number group_by)
    pub range_collapse_key: RangeCollapseKey,

    pub year_suffix: Option<u32>,

    /// Ranges of year suffixes (not alphabetic, in its base u32 form)
    /// (only applicable if first_of_ys == true)
    pub collapsed_year_suffixes: Vec<RangePiece>,

    /// Ranges of citation numbers
    /// (only applicable if first_of_ys == true)
    pub collapsed_ranges: Vec<RangePiece>,

    /// Tagging removed cites is cheaper than memmoving the rest of the Vec
    pub vanished: bool,

    pub has_locator: bool,
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
            .field("first_of_name", &self.first_of_name)
            .field("subsequent_same_name", &self.subsequent_same_name)
            .field("unique_name_number", &self.unique_name_number)
            .field("first_of_same_year", &self.first_of_same_year)
            .field("subsequent_same_year", &self.subsequent_same_year)
            .field("year", &self.range_collapse_key)
            .field("year_suffix", &self.year_suffix)
            .field("collapsed_year_suffixes", &self.collapsed_year_suffixes)
            .field("collapsed_ranges", &self.collapsed_ranges)
            .field("vanished", &self.vanished)
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
            cnum,
            first_of_name: false,
            subsequent_same_name: false,
            unique_name_number: 0,
            first_of_same_year: false,
            range_collapse_key: RangeCollapseKey::ForceSingle,
            subsequent_same_year: false,
            year_suffix: None,
            collapsed_year_suffixes: Vec::new(),
            collapsed_ranges: Vec::new(),
            vanished: false,
        }
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

pub fn collapse_ranges(nums: &[Collapsible]) -> Vec<RangePiece> {
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
        collapse_ranges(&[s(1), s(2), s(3)]),
        vec![RangePiece::Range(s(1), s(3))]
    );
    assert_eq!(
        collapse_ranges(&[s(1), s(2), Collapsible::new(4, 3)]),
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
    let mut same_years: HashMap<SmartString, (usize, bool)> = HashMap::new();

    // First, group cites with the same name
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
                    }
                    *seen_once = true;
                    cites[ix].subsequent_same_name = true;
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
        let mut top_ix = 0;
        while top_ix < cites.len() {
            if cites[top_ix].first_of_name {
                let mut moved = 0;
                let mut ix = top_ix;
                while ix < cites.len() {
                    if ix != top_ix && !cites[ix].subsequent_same_name {
                        break;
                    }
                    moved += 1;
                    let tree = cites[ix].gen4.tree_ref();
                    let year_and_suf =
                        tree.find_first_year_and_suffix()
                            .and_then(|(ys_node, suf)| {
                                let ys_tree = tree.with_node(ys_node);
                                let flat = ys_tree.flatten(fmt, None)?;
                                Some((fmt.output(flat, false), suf))
                            });
                    if let Some((y, suf)) = year_and_suf {
                        cites[ix].year_suffix = Some(suf);
                        same_years
                            .entry(y)
                            .and_modify(|(oix, seen_once)| {
                                if *oix == ix - 1 {
                                    if !*seen_once {
                                        cites[*oix].first_of_same_year = true;
                                    }
                                    cites[ix].subsequent_same_year = true;
                                    *seen_once = true;
                                } else {
                                    *seen_once = false;
                                }
                                *oix = ix;
                            })
                            .or_insert((ix, false));
                    }
                    ix += 1;
                }
                top_ix += moved;
            }
            top_ix += 1;
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

    if let Some(collapse) = collapse {
        match collapse {
            Collapse::CitationNumber => {
                let mut ix = 0;
                while ix < cites.len() {
                    let slice = &mut cites[ix..];
                    if let Some((u, rest)) = slice.split_first_mut() {
                        if u.first_of_name {
                            let following = rest.iter_mut().take_while(|u| u.subsequent_same_name);

                            let mut cnums = Vec::new();
                            if let Some(cnum) = u.cnum {
                                cnums.push(Collapsible::new(cnum, ix));
                            }
                            let mut count = 0;
                            for (nix, cite) in following.enumerate() {
                                if let Some(cnum) = cite.cnum {
                                    cnums.push(Collapsible {
                                        number: cnum,
                                        ix: ix + nix + 1,
                                        force_single: cite.has_locator
                                            || cite.cite.mode == Some(CiteMode::AuthorOnly),
                                    })
                                }
                                cite.vanished = true;
                                count += 1;
                            }
                            ix += count;
                            u.collapsed_ranges = collapse_ranges(&cnums);
                        }
                    }
                    ix += 1;
                }
            }
            Collapse::Year => {
                let mut ix = 0;
                while ix < cites.len() {
                    let slice = &mut cites[ix..];
                    if let Some((u, rest)) = slice.split_first_mut() {
                        if u.first_of_name {
                            let following = rest.iter_mut().take_while(|u| u.subsequent_same_name);
                            let mut count = 0;
                            for cite in following {
                                let gen4 = Arc::make_mut(&mut cite.gen4);
                                gen4.tree_mut().suppress_names();
                                count += 1;
                            }
                            ix += count;
                        }
                    }
                    ix += 1;
                }
            }
            Collapse::YearSuffixRanged | Collapse::YearSuffix => {
                let mut ix = 0;
                while ix < cites.len() {
                    let slice = &mut cites[ix..];
                    if let Some((u, rest)) = slice.split_first_mut() {
                        if u.first_of_name {
                            let following = rest.iter_mut().take_while(|u| u.subsequent_same_name);
                            for cite in following {
                                let gen4 = Arc::make_mut(&mut cite.gen4);
                                gen4.tree_mut().suppress_names()
                            }
                        }
                        if u.first_of_same_year {
                            let following = rest.iter_mut().take_while(|u| u.subsequent_same_year);

                            if collapse == Collapse::YearSuffixRanged {
                                // Potentially confusing; 'cnums' here are year suffixes in u32 form.
                                let mut cnums = Vec::new();
                                if let Some(cnum) = u.year_suffix {
                                    cnums.push(Collapsible::new(cnum, ix));
                                }
                                for (nix, cite) in following.enumerate() {
                                    if let Some(cnum) = cite.year_suffix {
                                        cnums.push(Collapsible {
                                            number: cnum,
                                            ix: ix + nix + 1,
                                            force_single: cite.has_locator,
                                        });
                                    }
                                    cite.vanished = true;
                                    if !cite.has_locator {
                                        let gen4 = Arc::make_mut(&mut cite.gen4);
                                        gen4.tree_mut().suppress_year();
                                    }
                                }
                                u.collapsed_year_suffixes = collapse_ranges(&cnums);
                            } else {
                                if let Some(cnum) = u.year_suffix {
                                    u.collapsed_year_suffixes
                                        .push(RangePiece::Single(Collapsible::new(cnum, ix)));
                                }
                                for (nix, cite) in following.enumerate() {
                                    if let Some(cnum) = cite.year_suffix {
                                        u.collapsed_year_suffixes.push(RangePiece::Single(
                                            Collapsible {
                                                number: cnum,
                                                ix: ix + nix + 1,
                                                force_single: cite.has_locator,
                                            },
                                        ));
                                    }
                                    cite.vanished = true;
                                    let gen4 = Arc::make_mut(&mut cite.gen4);
                                    gen4.tree_mut().suppress_year();
                                }
                            }
                        }
                    }
                    ix += 1;
                }
            }
        }
    }
}

////////////////////////////////
// Cluster Modes & Cite Modes //
////////////////////////////////

fn render_cluster_infix<O: OutputFormat>(infix: Option<&str>, fmt: &O) -> O::Build {
    let mut infix: SmartString = infix.unwrap_or(" ").into();
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
    fmt.ingest(
        &infix,
        &IngestOptions {
            is_external: true,
            ..Default::default()
        },
    )
}
