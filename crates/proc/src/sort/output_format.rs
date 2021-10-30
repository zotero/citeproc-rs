// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use citeproc_io::output::links::Link;
use citeproc_io::output::{
    micro_html::micro_html_to_string, FormatCmd, LocalizedQuotes, OutputFormat,
};
use citeproc_io::{lazy, IngestOptions, SmartString};

use csl::{DisplayMode, Formatting};

#[derive(Debug, Clone, PartialEq)]
pub struct SortStringFormat;

impl Default for SortStringFormat {
    fn default() -> Self {
        SortStringFormat
    }
}

// We don't want these characters in a sort string
fn remove_quotes(s: SmartString) -> SmartString {
    lazy::lazy_char_transform_owned(s, |c: char| {
        if c == '\'' || c == '"' || c == ',' {
            None
        } else {
            Some(c)
        }
        .into_iter()
    })
}

impl OutputFormat for SortStringFormat {
    type Input = SmartString;
    type Build = SmartString;
    type Output = SmartString;
    type BibMeta = ();

    fn meta(&self) -> Self::BibMeta {}

    #[inline]
    fn ingest(&self, input: &str, options: &IngestOptions) -> Self::Build {
        remove_quotes(micro_html_to_string(input, options))
    }

    #[inline]
    fn plain(&self, s: &str) -> Self::Build {
        s.into()
    }

    #[inline]
    fn text_node(&self, s: SmartString, _: Option<Formatting>) -> Self::Build {
        s
    }

    fn join_delim(&self, mut a: Self::Build, delim: &str, b: Self::Build) -> Self::Build {
        a.push_str(&delim);
        a.push_str(&b);
        a
    }

    fn seq(&self, nodes: impl IntoIterator<Item = Self::Build>) -> Self::Build {
        let mut iter = nodes.into_iter();
        if let Some(first) = iter.next() {
            iter.fold(first, |mut a, b| {
                a.push_str(&b);
                a
            })
        } else {
            SmartString::new()
        }
    }

    fn group(
        &self,
        nodes: Vec<Self::Build>,
        delimiter: &str,
        _f: Option<Formatting>,
    ) -> Self::Build {
        let std_string = nodes.join(delimiter);
        SmartString::from(std_string)
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
    fn stack_preorder(&self, _s: &mut SmartString, _stack: &[FormatCmd]) {}
    #[inline]
    fn stack_postorder(&self, _s: &mut SmartString, _stack: &[FormatCmd]) {}
    #[inline]
    fn tag_stack(&self, _formatting: Formatting, _: Option<DisplayMode>) -> Vec<FormatCmd> {
        Vec::new()
    }

    #[inline]
    fn append_suffix(&self, pre_and_content: &mut Self::Build, suffix: &str) {
        // TODO: do moving punctuation here as well
        pre_and_content.push_str(suffix)
    }

    fn ends_with_full_stop(&self, _build: &Self::Build) -> bool {
        // not needed
        false
    }

    #[inline]
    fn apply_text_case(&self, build: &mut Self::Build, options: &IngestOptions) {
        let is_uppercase = !build.chars().any(|c| c.is_lowercase());
        let string = std::mem::replace(build, SmartString::new());
        *build = options.transform_case(string, false, true, is_uppercase);
    }

    fn link(&self, link: Link) -> Self::Build {
        match link {
            Link::Url { url, .. } | Link::Id { url, .. } => {
                smart_format!("{}", url)
            }
        }
    }
}
