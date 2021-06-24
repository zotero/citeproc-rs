// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2021 Corporation for Digital Scholarship

use citeproc_db::ClusterId;
use citeproc_io::{Cite, CiteMode, ClusterMode};
use csl::Collapse;
use std::borrow::Cow;

use crate::ir::transforms;
use crate::prelude::*;

use super::CiteInCluster;

pub(crate) struct Positional<T>(pub T, pub Option<T>);
pub(crate) fn iter_peek_is_last<'a, T>(
    slice: &'a [T],
) -> impl Iterator<Item = Positional<&'a T>> + 'a {
    let len = slice.len();
    slice
        .iter()
        .enumerate()
        .map(move |(ix, this)| Positional(this, slice.get(ix + 1)))
}

pub(crate) struct LayoutStream<'a> {
    chunks: Vec<Chunk<'a>>,
    delimiters: LayoutDelimiters<'a>,
    fmt: &'a Markup,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Affix {
    Prefix,
    Suffix,
}
use Affix::*;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Chunk<'a> {
    Cite {
        built: MarkupBuild,
        suppress_delim: bool,
    },
    Prefix(SmartCow<'a>),
    Suffix(SmartCow<'a>),
    Delim(DelimKind),
}

impl<'a> LayoutStream<'a> {
    pub(crate) fn new(cap: usize, delimiters: LayoutDelimiters<'a>, fmt: &'a Markup) -> Self {
        Self {
            chunks: Vec::with_capacity(cap),
            delimiters,
            fmt,
        }
    }
    pub(crate) fn write_interspersed(
        &mut self,
        iter: impl IntoIterator<Item = MarkupBuild>,
        delim_kind: DelimKind,
    ) {
        use itertools::Itertools;
        self.chunks.extend(Itertools::intersperse(
            iter.into_iter().map(|built| Chunk::Cite {
                built,
                suppress_delim: false,
            }),
            Chunk::Delim(delim_kind),
        ))
    }

    /// Replaces an existing delimiter, which means you can write delimiters unconditionally and
    /// replace them with more appropriate ones later
    pub(crate) fn write_cite(
        &mut self,
        prefix: Option<SmartCow<'a>>,
        built: MarkupBuild,
        suffix: Option<SmartCow<'a>>,
        suppress_delim: bool,
    ) {
        if let Some(pre) = prefix {
            if pre.as_ref() == "" {

            }
            // todo: pop_delim if prefix starts with punc
            self.chunks.push(Chunk::Prefix(pre))
        }
        self.chunks.push(Chunk::Cite {
            built,
            suppress_delim,
        });
        if let Some(suf) = suffix {
            self.chunks.push(Chunk::Suffix(suf))
        }
    }

    pub(crate) fn close_year_suffix_ranges(&mut self) {
        self.write_delim(Some(DelimKind::CollapseYearSuffixLast));
    }

    pub(crate) fn close_year_suffix_run(&mut self) {
        self.write_delim(Some(DelimKind::CollapseYearSuffixLast));
    }

    pub(crate) fn close_name_run(&mut self) {
        self.write_delim(Some(DelimKind::AfterCollapsedGroup));
    }

    // If you pass None, this calls pop_delim.
    pub(crate) fn write_delim(&mut self, delim_kind: Option<DelimKind>) {
        let delim_kind = if let Some(dnew) = delim_kind {
            dnew
        } else {
            self.pop_delim();
            return;
        };
        let push_chunk = match self.chunks.last_mut() {
            Some(Chunk::Prefix(a)) => !ends_punc(&a),
            Some(Chunk::Suffix(a)) => true,
            Some(Chunk::Cite {
                built,
                suppress_delim,
            }) => !*suppress_delim,
            Some(Chunk::Delim(d)) => {
                *d = delim_kind;
                return;
            }
            None => false,
        };
        if push_chunk {
            self.chunks.push(Chunk::Delim(delim_kind))
        }
    }

    pub(crate) fn pop_delim(&mut self) -> Option<DelimKind> {
        match self.chunks.last() {
            Some(Chunk::Delim(d)) => {
                // we know it's a delim now, so don't bother matching again
                let d = *d;
                self.chunks.pop().map(|_| d)
            }
            _ => None,
        }
    }

    pub(crate) fn finish(mut self) -> Option<MarkupBuild> {
        self.pop_delim();
        if self.chunks.is_empty() {
            return None;
        }
        let fmt = self.fmt;
        let delimiters = self.delimiters;
        let seq = self.chunks.into_iter().filter_map(|x| match x {
            Chunk::Cite { built, .. } => Some(built),
            Chunk::Prefix(s) if !s.is_empty() => Some(fmt.plain(&s)),
            Chunk::Suffix(s) if !s.is_empty() => Some(fmt.plain(&s)),
            Chunk::Delim(d) => delimiters.delim(d).map(|x| fmt.plain(x)),
            _ => None,
        });
        Some(fmt.with_format(
            fmt.affixed(fmt.seq(seq), delimiters.affixes),
            delimiters.formatting,
        ))
        .filter(|x| !x.is_empty())
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

#[derive(Debug, Clone)]
pub(crate) struct LayoutDelimiters<'a> {
    pub cite_group: &'a str,
    pub year_suffix: &'a str,
    pub after_collapse: &'a str,
    pub layout_delim: &'a str,
    pub affixes: Option<&'a Affixes>,
    pub formatting: Option<Formatting>,
}

pub struct CiteOpensRuns {
    name: bool,
    multiple_years: bool,
    year_suffixes: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum DelimKind {
    Layout,
    CiteGroup,
    AfterCollapsedGroup,
    CollapseCitationNumbersMid,
    CollapseCitationNumbersLast,
    CollapseYearSuffixMid,
    CollapseYearSuffixLast,
}

impl<'a> LayoutDelimiters<'a> {
    pub(crate) fn delim(&self, post: DelimKind) -> Option<&'a str> {
        Some(match post {
            DelimKind::CiteGroup => self.cite_group,
            DelimKind::AfterCollapsedGroup => self.after_collapse,
            DelimKind::CollapseCitationNumbersMid => self.layout_delim,
            DelimKind::CollapseCitationNumbersLast => self.after_collapse,
            DelimKind::CollapseYearSuffixMid => self.year_suffix,
            DelimKind::CollapseYearSuffixLast => self.cite_group,
            DelimKind::Layout => self.layout_delim,
        })
        .filter(|x| !x.is_empty())
    }
    pub(crate) fn from_citation(citation: &'a csl::Citation) -> Self {
        let layout_opt = citation.layout.delimiter.as_opt_str();
        let cite_group = citation.cite_group_delimiter.as_opt_str().unwrap_or(", ");
        let year_suffix = citation
            .year_suffix_delimiter
            .as_opt_str()
            .or(layout_opt)
            .unwrap_or("");
        let after_collapse = citation
            .after_collapse_delimiter
            .as_opt_str()
            .or(layout_opt)
            .unwrap_or("");
        let layout_delim = layout_opt.unwrap_or("");
        let affixes = citation.layout.affixes.as_ref();
        let formatting = citation.layout.formatting.clone();
        Self {
            cite_group,
            year_suffix,
            after_collapse,
            layout_delim,
            affixes,
            formatting,
        }
    }
    pub(crate) fn from_intext(
        intext_el: Option<&'a csl::InText>,
        citation: &'a csl::Citation,
    ) -> Self {
        let mut citation = LayoutDelimiters::from_citation(citation);
        citation.formatting = None;
        citation.affixes = None;
        if let Some(intext_el) = intext_el {
            let layout_opt = intext_el.layout.delimiter.as_opt_str();
            let cite_group = intext_el
                .cite_group_delimiter
                .as_opt_str()
                .unwrap_or(citation.cite_group);
            let year_suffix = intext_el
                .year_suffix_delimiter
                .as_opt_str()
                .unwrap_or(citation.cite_group);
            let after_collapse = intext_el
                .after_collapse_delimiter
                .as_opt_str()
                .unwrap_or(citation.after_collapse);
            let layout_delim = layout_opt.unwrap_or(citation.layout_delim);
            let affixes = intext_el.layout.affixes.as_ref();
            let formatting = intext_el.layout.formatting.clone();
            citation = Self {
                cite_group,
                year_suffix,
                after_collapse,
                layout_delim,
                affixes,
                formatting,
            }
        }
        citation
    }
}

fn is_no_delim_punc(c: char) -> bool {
    c == ',' || c == '.' || c == '?' || c == '!'
}
fn ends_punc(string: &str) -> bool {
    string.chars().rev().nth(0).map_or(false, is_no_delim_punc)
}
fn starts_punc(string: &str) -> bool {
    string.chars().nth(0).map_or(false, is_no_delim_punc)
}

pub(crate) fn suppress_delimiter_between(
    this: &CiteInCluster<Markup>,
    next: &CiteInCluster<Markup>,
) -> bool {
    let this_suffix = this.suffix_str().unwrap_or("");
    let next_prefix = next.prefix_str().unwrap_or("");
    // "2000 is one source,; David Jones" => "2000 is one source, David Jones"
    // "2000;, and David Jones" => "2000, and David Jones"
    ends_punc(this_suffix) || starts_punc(next_prefix)
}

pub(crate) fn flatten_with_affixes(
    cite_in_cluster: &CiteInCluster<Markup>,
    cite_is_final_in_cluster: bool,
    fmt: &Markup,
) -> MarkupBuild {
    let CiteInCluster { cite, gen4, .. } = cite_in_cluster;
    let flattened = gen4.tree_ref().flatten_or_plain(&fmt, CSL_STYLE_ERROR);

    // we treat the None cases as empty strings because we would otherwise need a case
    // explosion for fmt.seq below. When they're empty they stay empty and don't allocate.
    //
    let mut pre = Cow::from(cite_in_cluster.prefix_str().unwrap_or(""));
    let mut suf = Cow::from(cite_in_cluster.suffix_str().unwrap_or(""));
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
    if suf_last_punc && !cite_is_final_in_cluster {
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
}
