// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2021 Corporation for Digital Scholarship

use std::collections::HashMap;
use std::sync::Arc;

use citeproc_db::ClusterId;
use citeproc_io::{Cite, ClusterMode};
use csl::Collapse;

use crate::helpers::slice_group_by::{group_by, group_by_mut};

use crate::db::IrGen;
use crate::ir::transforms;
use crate::prelude::*;

mod layout;
use layout::DelimKind;
pub(crate) use layout::WhichStream;

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
    let sorted_refs_arc = db.sorted_refs();
    let mut irs: Vec<_> = cite_ids
        .iter()
        .map(|&id| {
            let gen4 = db.ir_fully_disambiguated(id);
            let position = db.cite_position(id).0;
            let cite = id.lookup(db);
            let (_keys, citation_numbers_by_id) = &*sorted_refs_arc;
            let cnum = citation_numbers_by_id.get(&cite.ref_id).cloned();
            CiteInCluster::new(id, cite, position, cnum.map(|x| x.get()), gen4, &fmt)
        })
        .collect();

    if let Some(maybe_collapse) = style.citation.group_collapsing() {
        group_by_name(&fmt, maybe_collapse, &mut irs);
    }

    // cluster mode has to be applied before group_and_collapse because it would otherwise be
    // working on names blocks that have already been suppressed.
    // intext_Composite_Multiple.yml
    let cluster_mode = db.cluster_mode(cluster_id);
    log::trace!(
        "built_cluster_before_output: cluster_id = {:?}, cluster_mode = {:?}",
        cluster_id,
        cluster_mode
    );
    if let Some(mode) = &cluster_mode {
        transforms::apply_cluster_mode(db, mode, &mut irs, style.class, fmt);
    } else {
        transforms::apply_cite_modes(db, &mut irs, fmt);
    }

    if let Some(Some(collapse)) = style.citation.group_collapsing() {
        collapse_cites(&fmt, collapse, &mut irs);
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

    let default_locale = db.default_locale();
    let citation_delims = layout::LayoutDelimiters::from_citation(&style.citation);
    let intext_delimiters = layout::LayoutDelimiters::from_intext(
        style.intext.as_ref(),
        &style.citation,
        &default_locale,
    );

    let mut citation_stream = layout::LayoutStream::new(irs.len() * 2, citation_delims, fmt);
    let mut intext_stream = layout::LayoutStream::new(0, intext_delimiters, fmt);

    // render the intext stream
    let intext_authors = group_by(&irs, |a, b| a.by_name() == b.by_name())
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
        .filter_map(|cite| match cite.destination {
            WhichStream::MainToIntext { success } => {
                Some((cite, Some(cite.gen4.tree_ref().node).filter(|_| success)))
            }
            WhichStream::MainToCitationPlusIntext(opt_node) => Some((cite, opt_node)),
            _ => None,
        })
        .map(|(cite, opt_node)| {
            opt_node
                .and_then(|node| {
                    cite.gen4
                        .tree_ref()
                        .with_node(node)
                        // this is something @fbennett made up specifically for author-only / clusters.
                        .flatten(fmt, None)
                })
                .unwrap_or_else(|| fmt.plain(CLUSTER_NO_PRINTED_FORM))
        });

    intext_stream.write_interspersed(intext_authors, DelimKind::Layout);

    for cite in &irs {
        match cite.destination {
            WhichStream::Nowhere | WhichStream::MainToIntext { .. } => {
                continue;
            }
            _ => {
                citation_stream.write_flat(cite, None);
            }
        }
    }

    let citation_final = citation_stream.finish();
    let intext_final = intext_stream.finish();
    if intext_final.is_none() {
        if citation_final.is_none() {
            return fmt.plain(CLUSTER_NO_PRINTED_FORM);
        } else {
            return fmt.seq(citation_final.into_iter());
        }
    }
    let infix = render_composite_infix(
        match &cluster_mode {
            Some(ClusterMode::Composite { infix, .. }) => Some(infix.as_opt_str()),
            // humans::intext_Mixed.yml
            // This is to separate any author-only cites from any others (suppress-author, normal)
            // in there.
            None => Some(Some(" ")).filter(|_| citation_final.is_some()),
            _ => None,
        },
        fmt,
    );
    let seq = intext_final.into_iter().chain(infix).chain(citation_final);
    fmt.seq(seq)
}

/// A wrapper for Option where `a == b` evaluates to false if either is empty
///
/// Implements PartialEq, but does not implement Eq, of course.
#[derive(Debug, Copy, Clone)]
pub(crate) enum Partial<T> {
    Incomparable,
    Filled(T),
}

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

pub(crate) struct CiteInCluster<O: OutputFormat = Markup> {
    pub cite_id: CiteId,
    pub cite: Arc<Cite<O>>,
    pub position: csl::Position,
    pub cnum: Partial<u32>,
    pub gen4: Arc<IrGen>,
    /// Tagging removed cites is cheaper than memmoving the rest of the Vec
    pub destination: WhichStream,
    /// So we can look for punctuation at the end and use the format's quoting abilities
    pub prefix_parsed: Option<MarkupBuild>,
    /// A key to group_by cites in order to collapse runs of the same **name**.
    pub unique_name_number: Partial<u32>,
    /// A key to group_by cites in order to collapse runs of the same **year**.
    pub year: Partial<SmartString>,
    /// A key to group_by cites in order to collapse runs of the same **year-suffix**.
    pub year_suffix: Partial<u32>,
    pub has_locator: bool,
    pub has_locator_or_affixes: bool,
    pub own_delimiter: Option<DelimKind>,
}

impl<O: OutputFormat> CiteInCluster<O> {
    pub(crate) fn by_year(&self) -> Partial<&SmartString> {
        self.year.as_ref()
    }
    pub(crate) fn by_name(&self) -> Partial<u32> {
        self.unique_name_number
    }
    pub(crate) fn isolate_loc_affix(&self) -> Partial<()> {
        Partial::Filled(()).filter(!self.has_locator_or_affixes)
    }
    pub(crate) fn by_year_suffix(&self) -> Partial<u32> {
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
            .field("extra_node", &self.destination)
            .field("prefix_parsed", &self.prefix_parsed)
            .field("has_locator", &self.has_locator)
            .field("has_locator_or_affixes", &self.has_locator_or_affixes)
            .field("own_delimiter", &self.own_delimiter)
            .field("unique_name_number", &self.unique_name_number)
            .field("year_suffix", &self.year_suffix)
            .field("gen4_full", &self.gen4)
            .finish()
    }
}

impl CiteInCluster<Markup> {
    pub fn new(
        cite_id: CiteId,
        cite: Arc<Cite<Markup>>,
        position: csl::Position,
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
            position,
            cite,
            gen4,
            destination: WhichStream::default(),
            prefix_parsed,
            cnum: Partial::from(cnum),
            // by default, no names are in groups.
            unique_name_number: Partial::Incomparable,
            year: Partial::Incomparable,
            year_suffix: Partial::Incomparable,
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

pub(crate) fn group_by_name<O: OutputFormat<Output = SmartString>>(
    fmt: &Markup,
    collapse: Option<Collapse>,
    cites: &mut Vec<CiteInCluster<O>>,
) {
    // Neat trick: same_names[None] tracks cites without a cs:names block, which helps with styles
    // that only include a year. (What kind of style is that?
    // magic_ImplicitYearSuffixExplicitDelimiter.txt, I guess that's the only possible reason, but
    // ok.)
    let mut same_names: HashMap<Option<SmartString>, (usize, bool, Partial<u32>)> = HashMap::new();

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
                        *seen_local = true;
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
            [_single] => {
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
}

pub(crate) fn collapse_cites<O: OutputFormat<Output = SmartString>>(
    fmt: &Markup,
    collapse: Collapse,
    cites: &mut Vec<CiteInCluster<O>>,
) {
    log::debug!("collapse = {:?}", collapse);
    if collapse == Collapse::YearSuffixRanged || collapse == Collapse::YearSuffix {
        let name_runs = group_by_mut(cites.as_mut(), |a, b| a.by_name() == b.by_name());
        for run in name_runs {
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
        }
    }

    fn suppress_names<O: OutputFormat>(cite: &mut CiteInCluster<O>) {
        let gen4 = Arc::make_mut(&mut cite.gen4);
        gen4.tree_mut().suppress_names()
    }

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
                            ignored.destination = WhichStream::Nowhere;
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
                    name_run
                        .iter()
                        .map(|x| (x.unique_name_number, x.own_delimiter))
                        .collect::<Vec<_>>()
                );
                let delim_for_cite = |cite: &CiteInCluster<O>, next_affixed: bool| {
                    if cite.has_locator_or_affixes || next_affixed {
                        Some(DelimKind::AfterCollapse)
                    } else {
                        Some(DelimKind::CiteGroup)
                    }
                };
                match name_run {
                    [] => log::warn!("run of same name should never be empty"),
                    [_single] => {}
                    [head, middle @ .., last] => {
                        head.own_delimiter = delim_for_cite(
                            head,
                            middle.get(0).map_or(false, |x| x.has_locator_or_affixes),
                        );
                        let mut middle_iter = middle.iter_mut().peekable();
                        while let Some(cite) = middle_iter.next() {
                            suppress_names(cite);
                            let next_affixed = middle_iter
                                .peek()
                                .map(|x| &**x)
                                .or(by_name.peek().and_then(|x| x.first()))
                                .map_or(false, |x| x.has_locator_or_affixes);
                            cite.own_delimiter = delim_for_cite(cite, next_affixed);
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
                    let mut prev_affixed = false;
                    let mut ysuf_runs = monotonic_nonaffixed_ysufs.enumerate().peekable();
                    while let Some((ysuf_ix, ysuf_run)) = ysuf_runs.next() {
                        let next_affixed = ysuf_runs
                            .peek()
                            .and_then(|x| x.1.first())
                            .or(by_name.peek().and_then(|x| x.first()))
                            .map_or(false, |x| x.has_locator_or_affixes);
                        collapse_year_suffix_run(
                            ysuf_run,
                            ysuf_ix == 0,
                            ranged,
                            &mut prev_affixed,
                            next_affixed,
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

fn collapse_year_suffix_run<O: OutputFormat>(
    ysuf_run: &mut [CiteInCluster<O>],
    is_first_ysuf_run: bool,
    ranged: bool,
    prev_affixed: &mut bool,
    next_affixed: bool,
) {
    fn suppress_year<O: OutputFormat>(cite: &mut CiteInCluster<O>) {
        let gen4 = Arc::make_mut(&mut cite.gen4);
        gen4.tree_mut().suppress_year()
    }

    let trailing_delim = |cite: &CiteInCluster<O>, d: DelimKind| {
        if cite.has_locator_or_affixes || next_affixed {
            Some(DelimKind::CiteGroup)
        } else {
            Some(d)
        }
    };
    match ysuf_run {
        [] => log::warn!("run of year suffixes should never be empty"),
        [single] => {
            if ranged && !single.has_locator_or_affixes && !is_first_ysuf_run && !*prev_affixed {
                log::debug!("suppressing year on a single.");
                suppress_year(single);
            }
            *prev_affixed = single.has_locator_or_affixes;
            single.own_delimiter = trailing_delim(single, DelimKind::YearSuffix);
        }
        [head, middle @ .., last] if ranged => {
            if !is_first_ysuf_run && !*prev_affixed {
                suppress_year(head);
            }
            head.own_delimiter = Some(if !middle.is_empty() {
                DelimKind::Range
            } else {
                DelimKind::YearSuffix
            });
            for ignored in middle {
                ignored.destination = WhichStream::Nowhere;
            }
            suppress_year(last);
            last.own_delimiter = trailing_delim(last, DelimKind::YearSuffix);
            *prev_affixed = false;
        }
        [head, middle @ .., last] => {
            if !is_first_ysuf_run && !*prev_affixed {
                suppress_year(head);
            }
            head.own_delimiter = Some(DelimKind::YearSuffix);
            for cite in middle {
                suppress_year(cite);
                cite.own_delimiter = Some(DelimKind::YearSuffix);
            }
            suppress_year(last);
            last.own_delimiter = trailing_delim(last, DelimKind::YearSuffix);
            *prev_affixed = false;
        }
    }
}

////////////////////////////////
// Cluster Modes & Cite Modes //
////////////////////////////////

/// If infix is `None`, returns None.
/// If Infix is `Some(None)`, returns a single space.
/// If Infix is `Some(Some(x))`, adjusts puncuated ends.
fn render_composite_infix<O: OutputFormat>(
    infix: Option<Option<&str>>,
    fmt: &O,
) -> Option<O::Build> {
    let mut infix: SmartString = infix?.unwrap_or(" ").into();
    if !infix.ends_with(" ") {
        infix.push(' ');
    }
    if infix.starts_with('\'') {
        infix.replace_range(0..1, "’");
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
