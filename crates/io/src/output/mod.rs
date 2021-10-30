// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::String;
use std::marker::{Send, Sync};

use crate::IngestOptions;
use csl::{Atom, Locale, QuoteTerm, SimpleTermSelector};

#[cfg(feature = "markup")]
pub mod markup;

// #[cfg(feature = "pandoc")]
// pub mod pandoc;

pub mod links;
pub mod micro_html;
mod parse_quotes;
mod puncttable;
mod superscript;

use csl::{Affixes, DisplayMode, Formatting};
use serde::{de::DeserializeOwned, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalizedQuotes {
    pub outer: (Atom, Atom),
    pub inner: (Atom, Atom),
    /// Default false, pulled from LocaleOptions
    pub punctuation_in_quote: bool,
}

impl LocalizedQuotes {
    pub fn closing(&self, is_inner: bool) -> &str {
        if is_inner {
            self.outer.1.as_ref()
        } else {
            self.inner.1.as_ref()
        }
    }
    pub fn opening(&self, is_inner: bool) -> &str {
        if is_inner {
            self.outer.0.as_ref()
        } else {
            self.inner.0.as_ref()
        }
    }

    pub fn simple() -> Self {
        LocalizedQuotes {
            outer: (Atom::from("\u{201C}"), Atom::from("\u{201D}")),
            inner: (Atom::from("\u{2018}"), Atom::from("\u{2019}")),
            punctuation_in_quote: false,
        }
    }

    pub fn from_locale(locale: &Locale) -> Self {
        let getter = |qt: QuoteTerm| {
            locale
                .simple_terms
                .get(&SimpleTermSelector::Quote(qt))
                .unwrap()
                .singular()
        };
        let open_outer = getter(QuoteTerm::OpenQuote);
        let close_outer = getter(QuoteTerm::CloseQuote);
        let open_inner = getter(QuoteTerm::OpenInnerQuote);
        let close_inner = getter(QuoteTerm::CloseInnerQuote);
        LocalizedQuotes {
            outer: (Atom::from(open_outer), Atom::from(close_outer)),
            inner: (Atom::from(open_inner), Atom::from(close_inner)),
            punctuation_in_quote: locale.options_node.punctuation_in_quote.unwrap_or(false),
        }
    }
}

impl Default for LocalizedQuotes {
    fn default() -> Self {
        LocalizedQuotes::simple()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum FormatCmd {
    FontStyleItalic,
    FontStyleOblique,
    FontStyleNormal,
    FontWeightBold,
    FontWeightNormal,
    FontWeightLight,
    FontVariantSmallCaps,
    FontVariantNormal,
    TextDecorationUnderline,
    TextDecorationNone,
    VerticalAlignmentSuperscript,
    VerticalAlignmentSubscript,
    VerticalAlignmentBaseline,
    DisplayBlock,
    DisplayIndent,
    DisplayLeftMargin,
    DisplayRightInline,
}

use std::hash::Hash;

use self::links::Link;

pub trait OutputFormat: Send + Sync + Clone + Default + PartialEq + std::fmt::Debug {
    type Input: std::fmt::Debug + DeserializeOwned + Default + Clone + Send + Sync + Eq + Hash;
    type Build: std::fmt::Debug + Default + Clone + Send + Sync + Eq;
    type Output: Default + Clone + Send + Sync + Eq + Serialize;
    type BibMeta: Serialize;

    fn meta(&self) -> Self::BibMeta;

    fn ingest(&self, input: &str, options: &IngestOptions) -> Self::Build;

    /// Affixes are not included in the formatting on a text node. They are converted into text
    /// nodes themselves, with no formatting except whatever is applied by a parent group.
    ///
    /// [Spec](https://docs.citationstyles.org/en/stable/specification.html#affixes)

    fn text_node(&self, s: String, formatting: Option<Formatting>) -> Self::Build;

    /// Group some text nodes. You might want to optimise for the case where delimiter is empty.
    fn group(
        &self,
        nodes: Vec<Self::Build>,
        delimiter: &str,
        formatting: Option<Formatting>,
    ) -> Self::Build;

    fn seq(&self, nodes: impl IntoIterator<Item = Self::Build>) -> Self::Build;

    fn join_delim(&self, a: Self::Build, delim: &str, b: Self::Build) -> Self::Build;

    fn is_empty(&self, a: &Self::Build) -> bool;
    fn output(&self, intermediate: Self::Build, punctuation_in_quote: bool) -> Self::Output {
        self.output_in_context(
            intermediate,
            Formatting::default(),
            Some(punctuation_in_quote),
        )
    }

    fn output_in_context(
        &self,
        intermediate: Self::Build,
        _format_stacked: Formatting,
        punctuation_in_quote: Option<bool>,
    ) -> Self::Output;

    fn plain(&self, s: &str) -> Self::Build;

    fn affixed_text_quoted(
        &self,
        s: String,
        format_inner: Option<Formatting>,
        affixes: Option<&Affixes>,
        quotes: Option<LocalizedQuotes>,
    ) -> Self::Build {
        self.affixed_quoted(self.text_node(s, format_inner), affixes, quotes)
    }

    fn affixed_text(
        &self,
        s: String,
        format_inner: Option<Formatting>,
        affixes: Option<&Affixes>,
    ) -> Self::Build {
        self.affixed(self.text_node(s, format_inner), affixes)
    }

    fn quoted(&self, b: Self::Build, quotes: LocalizedQuotes) -> Self::Build;

    #[inline]
    fn affixed(&self, b: Self::Build, affixes: Option<&Affixes>) -> Self::Build {
        self.affixed_quoted(b, affixes, None)
    }

    fn affixed_quoted(
        &self,
        b: Self::Build,
        affixes: Option<&Affixes>,
        quotes: Option<LocalizedQuotes>,
    ) -> Self::Build {
        use std::iter::once;
        let b = if let Some(lq) = quotes {
            self.quoted(b, lq)
        } else {
            b
        };
        let mut pre_and_content = if let Some(prefix) = affixes.as_ref().map(|a| &a.prefix) {
            if !prefix.is_empty() {
                self.seq(once(self.ingest(prefix, &IngestOptions::for_affixes())).chain(once(b)))
            } else {
                b
            }
        } else {
            b
        };
        if let Some(suffix) = affixes.as_ref().map(|a| &a.suffix) {
            if !suffix.is_empty() {
                self.append_suffix(&mut pre_and_content, suffix);
            }
        }
        pre_and_content
    }

    fn append_suffix(&self, pre_and_content: &mut Self::Build, suffix: &str);
    fn ends_with_full_stop(&self, build: &Self::Build) -> bool;

    fn apply_text_case(&self, mutable: &mut Self::Build, options: &IngestOptions);

    fn with_format(&self, a: Self::Build, f: Option<Formatting>) -> Self::Build;
    fn with_display(
        &self,
        a: Self::Build,
        display: Option<DisplayMode>,
        in_bibliography: bool,
    ) -> Self::Build;

    fn link(&self, link: Link) -> Self::Build;

    fn stack_preorder(&self, s: &mut String, stack: &[FormatCmd]);
    fn stack_postorder(&self, s: &mut String, stack: &[FormatCmd]);
    fn tag_stack(&self, formatting: Formatting, display: Option<DisplayMode>) -> Vec<FormatCmd>;
}
