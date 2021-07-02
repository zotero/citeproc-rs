// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2021 Corporation for Digital Scholarship

use super::CiteInCluster;
use crate::prelude::*;
use citeproc_io::TrimInPlace;

#[derive(Debug)]
pub(crate) struct LayoutStream<'a> {
    chunks: Vec<Chunk>,
    delimiters: LayoutDelimiters<'a>,
    fmt: &'a Markup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Chunk {
    Cite { built: MarkupBuild },
    Prefix(SmartString),
    Suffix(SmartString),
    Delim(DelimKind),
}

impl Chunk {
    fn as_delim_mut(&mut self) -> Option<&mut DelimKind> {
        match self {
            Chunk::Delim(d) => Some(d),
            _ => None,
        }
    }
    fn as_prefix_mut(&mut self) -> Option<&mut SmartString> {
        match self {
            Chunk::Prefix(d) => Some(d),
            _ => None,
        }
    }
    fn as_suffix_mut(&mut self) -> Option<&mut SmartString> {
        match self {
            Chunk::Suffix(d) => Some(d),
            _ => None,
        }
    }
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
            iter.into_iter().map(|built| Chunk::Cite { built }),
            Chunk::Delim(delim_kind),
        ))
    }

    pub(crate) fn overwrite_and_position(&mut self) {
        if let Some(_) = self.delimiters.and_last_delimiter {
            if let Some(last_delim) = self.chunks.iter_mut().rev().find_map(Chunk::as_delim_mut) {
                *last_delim = DelimKind::And;
            }
        }
    }

    pub(crate) fn trim_first_last_affixes(&mut self) {
        if let Some(suffix_in_final_pos) = self
            .chunks
            .iter_mut()
            .nth_back(0)
            .and_then(Chunk::as_suffix_mut)
        {
            suffix_in_final_pos.trim_end_in_place();
        }
        if let Some(prefix_initial) = self.chunks.iter_mut().nth(0).and_then(Chunk::as_prefix_mut) {
            prefix_initial.trim_start_in_place();
        }
    }

    pub(crate) fn write_flat(
        &mut self,
        single: &CiteInCluster<Markup>,
        override_delim_kind: Option<DelimKind>,
    ) {
        let (pre, built, suf) = flatten_with_affixes(single, self.fmt);
        self.write_cite(pre, built, suf);
        self.write_delim(override_delim_kind.or(single.own_delimiter));
    }

    /// Replaces an existing delimiter, which means you can write delimiters unconditionally and
    /// replace them with more appropriate ones later
    pub(crate) fn write_cite(
        &mut self,
        prefix: Option<SmartString>,
        built: MarkupBuild,
        suffix: Option<SmartString>,
    ) {
        if let Some(pre) = prefix {
            // XXX: should also maybe rewrite a delimiter to be AfterCollapseDelimiter if it's CiteGroup
            if starts_punc(pre.as_ref()) {
                self.pop_delim();
            }
            self.chunks.push(Chunk::Prefix(pre))
        }
        self.chunks.push(Chunk::Cite { built });
        if let Some(suf) = suffix {
            self.chunks.push(Chunk::Suffix(suf))
        }
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
            Some(Chunk::Suffix(a)) => !ends_punc(&a),
            Some(Chunk::Prefix(_)) => true,
            Some(Chunk::Cite { .. }) => true,
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
        self.overwrite_and_position();
        self.trim_first_last_affixes();
        if self.chunks.is_empty() {
            return None;
        }
        let fmt = self.fmt;
        let delimiters = self.delimiters;
        let external = IngestOptions {
            is_external: true,
            ..Default::default()
        };
        let seq = self.chunks.into_iter().filter_map(|x| match x {
            Chunk::Cite { built, .. } => Some(built),
            Chunk::Prefix(s) if !s.is_empty() => Some(fmt.ingest(&s, &external)),
            Chunk::Suffix(s) if !s.is_empty() => Some(fmt.ingest(&s, &external)),
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
pub(crate) enum WhichStream {
    /// Do not render this cite anywhere, it is in the middle of a collapsed range.
    Nowhere,
    /// Take gen4's tree and put it in the `<citation><layout>` stream
    MainToCitation,
    /// Take gen4's tree and put it in the `<intext><layout>` stream
    MainToIntext { success: bool },
    /// Take gen4's tree and put it in the `<citation><layout>` stream, and then put this detached node
    /// into the `<intext><layout>` stream
    ///
    /// None gives [NO_PRINTED_FORM] instead of a rendered intext.
    MainToCitationPlusIntext(Option<NodeId>),
}

impl Default for WhichStream {
    fn default() -> Self {
        WhichStream::MainToCitation
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
    pub and_last_delimiter: Option<SmartString>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum DelimKind {
    Layout,
    CiteGroup,
    AfterCollapse,
    YearSuffix,
    Range,
    And,
}

impl<'a> LayoutDelimiters<'a> {
    pub(crate) fn delim(&'a self, post: DelimKind) -> Option<&'a str> {
        Some(match post {
            DelimKind::CiteGroup => self.cite_group,
            DelimKind::AfterCollapse => self.after_collapse,
            DelimKind::YearSuffix => self.year_suffix,
            DelimKind::Layout => self.layout_delim,
            DelimKind::Range => "\u{2013}",
            // should not have to observe None here, simply don't write any Ands until you are sure
            // you have and_last_delimiter
            DelimKind::And => return self.and_last_delimiter.as_opt_str(),
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
            and_last_delimiter: None,
        }
    }
    pub(crate) fn from_intext(
        intext_el: Option<&'a csl::InText>,
        citation: &'a csl::Citation,
        merged_locale: &'a csl::Locale,
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
            let after_collapse = intext_el
                .after_collapse_delimiter
                .as_opt_str()
                .unwrap_or(citation.after_collapse);
            let layout_delim = layout_opt.unwrap_or(citation.layout_delim);
            let affixes = intext_el.layout.affixes.as_ref();
            let and_last_delimiter = intext_el.and.map(|x| match x {
                csl::NameAnd::Symbol => SmartString::from(" & "),
                csl::NameAnd::Text => {
                    let mut string = SmartString::new();
                    string.push(' ');
                    string.push_str(merged_locale.and_term(None).unwrap_or("and").trim());
                    string.push(' ');
                    string
                }
            });
            let formatting = intext_el.layout.formatting.clone();
            citation = Self {
                cite_group,
                year_suffix: citation.year_suffix,
                after_collapse,
                layout_delim,
                affixes,
                formatting,
                and_last_delimiter,
            }
        }
        citation
    }
}

fn is_no_delim_punc(c: char) -> bool {
    c == ',' || c == '.' || c == '?' || c == '!'
}
fn ends_punc(string: &str) -> bool {
    // got to trim spaces first, people might input a suffix like "hello; "
    string
        .trim_end()
        .chars()
        .rev()
        .nth(0)
        .map_or(false, is_no_delim_punc)
}
fn starts_punc(string: &str) -> bool {
    string
        .trim_start()
        .chars()
        .nth(0)
        .map_or(false, is_no_delim_punc)
}

pub(crate) fn flatten_with_affixes(
    cite_in_cluster: &CiteInCluster<Markup>,
    fmt: &Markup,
) -> (Option<SmartString>, MarkupBuild, Option<SmartString>) {
    let CiteInCluster { gen4, .. } = cite_in_cluster;
    let flattened = gen4.tree_ref().flatten_or_plain(&fmt, CSL_STYLE_ERROR);

    // we treat the None cases as empty strings because we would otherwise need a case
    // explosion for fmt.seq below. When they're empty they stay empty and don't allocate.
    //
    let mut pre = cite_in_cluster.prefix_str().map(SmartString::from);
    let mut suf = cite_in_cluster.suffix_str().map(SmartString::from);
    if let Some(pre) = pre.as_mut() {
        if !pre.is_empty() && !pre.ends_with(' ') {
            pre.push(' ');
        }
    }
    if let Some(suf) = suf.as_mut() {
        let suf_first = suf.chars().nth(0);
        if suf_first.map_or(false, |x| {
            x != ' ' && !citeproc_io::output::markup::is_punc(x)
        }) {
            suf.insert_str(0, " ");
        }
        let suf_last_punc = suf.chars().rev().nth(0).map_or(false, |x| {
            x == ',' || x == '.' || x == '!' || x == '?' || x == ':'
        });
        // for a final position suffix, we clean up trailing whitespace later (trim_first_last_affixes)
        if suf_last_punc {
            suf.push(' ');
        }
    }
    (pre, flattened, suf)
}
