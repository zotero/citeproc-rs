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

pub(crate) struct LayoutStream<'a> {
    chunks: Vec<Chunk<'a>>,
    layout_delim: Option<&'a str>,
    default_delim: Option<&'a str>,
    affixes: Option<&'a Affixes>,
    fmt: &'a Markup,
    formatting: Option<Formatting>,
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
    Delim(&'a str),
}

impl<'a> LayoutStream<'a> {
    pub(crate) fn new(
        cap: usize,
        layout_delim: Option<&'a str>,
        default_delim: Option<&'a str>,
        affixes: Option<&'a Affixes>,
        fmt: &'a Markup,
        formatting: Option<Formatting>,
    ) -> Self {
        Self {
            chunks: Vec::with_capacity(cap),
            layout_delim,
            default_delim,
            affixes,
            fmt,
            formatting,
        }
    }
    pub(crate) fn write_interspersed(
        &mut self,
        iter: impl IntoIterator<Item = MarkupBuild>,
        delim: &'a str,
    ) {
        use itertools::Itertools;
        self.chunks.extend(Itertools::intersperse(
            iter.into_iter().map(|built| Chunk::Cite {
                built,
                suppress_delim: false,
            }),
            Chunk::Delim(delim),
        ))
    }

    /// Replaces an existing delimiter, which means you can write delimiters unconditionally and
    /// replace them with more appropriate ones later
    pub(crate) fn write_delim(&mut self, delim: &'a str) {
        match self.chunks.last_mut() {
            Some(Chunk::Prefix(a)) if ends_punc(&a) => return,
            Some(Chunk::Suffix(a)) => return,
            Some(Chunk::Cite {
                built,
                suppress_delim,
            }) if *suppress_delim => return,
            Some(Chunk::Delim(d)) => {
                *d = delim;
                return;
            }
            _ => {}
        }
        self.chunks.push(Chunk::Delim(delim))
    }
    pub(crate) fn write_cite(&mut self, built: MarkupBuild, suppress_delim: bool) {
        self.chunks.push(Chunk::Cite {
            built,
            suppress_delim,
        })
    }

    pub(crate) fn finish(self) -> Option<MarkupBuild> {
        let Self {
            mut chunks,
            layout_delim,
            affixes,
            formatting,
            fmt,
            ..
        } = self;
        // we maintain either a delimiter or an extra element at the end all the way through, just for this
        match chunks.last() {
            Some(Chunk::Delim(_)) => {
                chunks.pop();
            }
            _ => {}
        }
        let seq = chunks.into_iter().map(|x| match x {
            Chunk::Cite { built, .. } => built,
            Chunk::Prefix(s) => fmt.plain(&s),
            Chunk::Suffix(s) => fmt.plain(&s),
            Chunk::Delim(s) => fmt.plain(s),
        });
        if seq.len() == 0 {
            return None
        }
        Some(fmt.with_format(fmt.affixed(fmt.seq(seq), affixes), formatting))
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

pub(crate) struct LayoutDelimiters<'a> {
    pub cite_group: &'a str,
    pub year_suffix: &'a str,
    pub after_collapse: &'a str,
    pub layout_delim: &'a str,
}

pub struct CiteOpensRuns {
    name: bool,
    multiple_years: bool,
    year_suffixes: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum CitePostDelimiter {
    GroupMid,
    GroupLast,
    YearSuffixMid,
    YearSuffixLast,
    SuppressDelimiter,
}

impl<'a> LayoutDelimiters<'a> {
    pub(crate) fn for_cite_post(&self, post: CitePostDelimiter) -> &'a str {
        use CitePostDelimiter as CPD;
        match post {
            CPD::GroupMid => self.cite_group,
            CPD::GroupLast => self.after_collapse,
            CPD::YearSuffixMid => self.after_collapse,
            CPD::YearSuffixLast => self.after_collapse,
            CPD::SuppressDelimiter => "",
        }
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
        let between_cites = layout_opt.unwrap_or("");
        Self {
            cite_group,
            year_suffix,
            after_collapse,
            layout_delim: between_cites,
        }
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
