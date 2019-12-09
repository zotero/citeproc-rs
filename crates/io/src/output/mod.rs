// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::IngestOptions;
use csl::Atom;

#[cfg(feature = "markup")]
pub mod markup;
pub mod micro_html;
#[cfg(feature = "pandoc")]
pub mod pandoc;
#[cfg(feature = "plain")]
pub mod plain;
mod superscript;

use std::marker::{Send, Sync};

// pub use self::pandoc::Pandoc;
// pub use self::plain::PlainText;
// pub use self::markup::Markup;

use csl::{Affixes, DisplayMode, Formatting};
use serde::{de::DeserializeOwned, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalizedQuotes {
    Single(Atom, Atom),
    Double(Atom, Atom),
    // Would this be better?
    // /// When the locale supplied single quotes that were just unicode curly quotes, you can use
    // /// optimized HTML/Pandoc objects that do the flip-flopping for you. Otherwise, flip flopping
    // /// is not supported.
    // SystemSingle,
    // // See SystemSingle
    // SystemDouble,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

pub trait OutputFormat: Send + Sync + Clone + Default + std::fmt::Debug {
    type Input: std::fmt::Debug + DeserializeOwned + Default + Clone + Send + Sync + Eq + Hash;
    type Build: std::fmt::Debug + Default + Clone + Send + Sync + Eq;
    type Output: Default + Clone + Send + Sync + Eq + Serialize;
    type BibMeta: Serialize;

    fn meta(&self) -> Self::BibMeta;

    fn ingest(&self, input: &str, options: IngestOptions) -> Self::Build;

    /// Affixes are not included in the formatting on a text node. They are converted into text
    /// nodes themselves, with no formatting except whatever is applied by a parent group.
    ///
    /// [Spec](https://docs.citationstyles.org/en/stable/specification.html#affixes)

    // TODO: make formatting an Option<Formatting>
    fn text_node(&self, s: String, formatting: Option<Formatting>) -> Self::Build;

    /// Group some text nodes. You might want to optimise for the case where delimiter is empty.
    fn group(
        &self,
        nodes: Vec<Self::Build>,
        delimiter: &str,
        formatting: Option<Formatting>,
    ) -> Self::Build;

    fn seq(&self, nodes: impl Iterator<Item = Self::Build>) -> Self::Build;

    fn join_delim(&self, a: Self::Build, delim: &str, b: Self::Build) -> Self::Build;

    fn is_empty(&self, a: &Self::Build) -> bool;
    fn output(&self, intermediate: Self::Build) -> Self::Output;
    fn output_in_context(
        &self,
        intermediate: Self::Build,
        _format_stacked: Formatting,
    ) -> Self::Output {
        // XXX: unnecessary, just to skip rewriting a bunch of formats
        self.output(intermediate)
    }

    fn plain(&self, s: &str) -> Self::Build;

    fn affixed_text_quoted(
        &self,
        s: String,
        format_inner: Option<Formatting>,
        affixes: Option<&Affixes>,
        quotes: Option<&LocalizedQuotes>,
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

    fn quoted(&self, b: Self::Build, quotes: &LocalizedQuotes) -> Self::Build;

    #[inline]
    fn affixed(&self, b: Self::Build, affixes: Option<&Affixes>) -> Self::Build {
        self.affixed_quoted(b, affixes, None)
    }

    fn affixed_quoted(
        &self,
        b: Self::Build,
        affixes: Option<&Affixes>,
        quotes: Option<&LocalizedQuotes>,
    ) -> Self::Build {
        use std::iter::once;
        let pre = affixes.map_or(true, |x| x.prefix.is_empty());
        let suf = affixes.map_or(true, |x| x.suffix.is_empty());
        let b = if let Some(lq) = quotes {
            self.quoted(b, lq)
        } else {
            b
        };
        match (pre, suf) {
            (true, true) => b,

            (false, true) => self.seq(once(self.plain(&affixes.unwrap().prefix)).chain(once(b))),

            (true, false) => self.seq(once(b).chain(once(self.plain(&affixes.unwrap().suffix)))),

            (false, false) => self.seq(
                once(self.plain(&affixes.unwrap().prefix))
                    .chain(once(b))
                    .chain(once(self.plain(&affixes.unwrap().suffix))),
            ),
        }
    }

    fn with_format(&self, a: Self::Build, f: Option<Formatting>) -> Self::Build;
    fn with_display(
        &self,
        a: Self::Build,
        display: Option<DisplayMode>,
        in_bibliography: bool,
    ) -> Self::Build;

    fn hyperlinked(&self, a: Self::Build, target: Option<&str>) -> Self::Build;

    fn stack_preorder(&self, s: &mut String, stack: &[FormatCmd]);
    fn stack_postorder(&self, s: &mut String, stack: &[FormatCmd]);
    fn tag_stack(&self, formatting: Formatting, display: Option<DisplayMode>) -> Vec<FormatCmd>;
}
