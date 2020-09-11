// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::micro_html::micro_html_to_string;
use super::{FormatCmd, LocalizedQuotes, OutputFormat};
use crate::IngestOptions;

use csl::{DisplayMode, Formatting};

#[derive(Debug, Clone, PartialEq)]
pub struct PlainText;

impl Default for PlainText {
    fn default() -> Self {
        PlainText {}
    }
}

impl OutputFormat for PlainText {
    type Input = String;
    type Build = String;
    type Output = String;
    type BibMeta = ();

    fn meta(&self) -> Self::BibMeta {}

    #[inline]
    fn ingest(&self, input: &str, options: &IngestOptions) -> Self::Build {
        micro_html_to_string(input, options).replace(|c| c == '\'' || c == '"' || c == ',', "")
    }

    #[inline]
    fn plain(&self, s: &str) -> Self::Build {
        s.to_owned()
    }

    #[inline]
    fn text_node(&self, s: String, _: Option<Formatting>) -> Self::Build {
        s
    }

    fn join_delim(&self, mut a: Self::Build, delim: &str, b: Self::Build) -> Self::Build {
        a.push_str(&delim);
        a.push_str(&b);
        a
    }

    fn seq(&self, mut nodes: impl Iterator<Item = Self::Build>) -> Self::Build {
        if let Some(first) = nodes.next() {
            nodes.fold(first, |mut a, b| {
                a.push_str(&b);
                a
            })
        } else {
            String::new()
        }
    }

    fn group(
        &self,
        nodes: Vec<Self::Build>,
        delimiter: &str,
        _f: Option<Formatting>,
    ) -> Self::Build {
        nodes.join(delimiter)
    }

    fn quoted(&self, b: Self::Build, _quotes: LocalizedQuotes) -> Self::Build {
        // We don't want quotes because sorting macros should ignore them
        // quotes.opening(false).to_owned() + &b + quotes.closing(false)
        b
    }

    #[inline]
    fn with_format(&self, a: Self::Build, _f: Option<Formatting>) -> Self::Build {
        a
    }

    #[inline]
    fn with_display(&self, a: Self::Build, _d: Option<DisplayMode>, _in_bib: bool) -> Self::Build {
        a
    }

    #[inline]
    fn hyperlinked(&self, a: Self::Build, _target: Option<&str>) -> Self::Build {
        a
    }

    #[inline]
    fn is_empty(&self, a: &Self::Build) -> bool {
        a.is_empty()
    }

    #[inline]
    fn output_in_context(
        &self,
        intermediate: Self::Build,
        _formatting: Formatting,
        _punctuation_in_quote: Option<bool>,
    ) -> Self::Output {
        intermediate
    }

    #[inline]
    fn stack_preorder(&self, _s: &mut String, _stack: &[FormatCmd]) {}
    #[inline]
    fn stack_postorder(&self, _s: &mut String, _stack: &[FormatCmd]) {}
    #[inline]
    fn tag_stack(&self, _formatting: Formatting, _: Option<DisplayMode>) -> Vec<FormatCmd> {
        Vec::new()
    }

    #[inline]
    fn append_suffix(&self, pre_and_content: &mut Self::Build, suffix: &str) {
        // TODO: do moving punctuation here as well
        pre_and_content.push_str(suffix)
    }

    #[inline]
    fn apply_text_case(&self, build: &mut Self::Build, options: &IngestOptions) {
        let is_uppercase = !build.chars().any(|c| c.is_lowercase());
        let string = std::mem::replace(build, String::new());
        *build = options.transform_case(string, false, true, is_uppercase);
    }
}
