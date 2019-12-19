// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use self::InlineElement::*;
use super::micro_html::MicroNode;
use super::{FormatCmd, LocalizedQuotes, OutputFormat};
use crate::utils::JoinMany;
use crate::IngestOptions;
use csl::{
    DisplayMode, FontStyle, FontVariant, FontWeight, Formatting, TextDecoration, VerticalAlignment,
};

mod rtf;
use self::rtf::RtfWriter;

mod html;
use self::html::{HtmlOptions, HtmlWriter};

mod plain;
use self::plain::PlainWriter;

mod flip_flop;
use self::flip_flop::FlipFlopState;
mod move_punctuation;
mod parse_quotes;
use self::move_punctuation::move_punctuation;
pub use self::parse_quotes::parse_quotes;
pub(self) mod puncttable;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Markup {
    Html(HtmlOptions),
    Rtf,
    Plain,
}

/// TODO: serialize and deserialize using an HTML parser?
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum InlineElement {
    /// This is how we can flip-flop only user-supplied styling.
    /// Inside this is parsed micro html
    Micro(Vec<MicroNode>),

    Formatted(Vec<InlineElement>, Formatting),
    /// Bool is "is_inner"
    Quoted {
        is_inner: bool,
        localized: LocalizedQuotes,
        inlines: Vec<InlineElement>,
    },
    Text(String),
    Anchor {
        title: String,
        url: String,
        content: Vec<InlineElement>,
    },
    Div(DisplayMode, Vec<InlineElement>),
}

impl Markup {
    pub fn html() -> Self {
        Markup::Html(HtmlOptions::default())
    }
    pub fn test_html() -> Self {
        Markup::Html(HtmlOptions::test_suite())
    }
    pub fn rtf() -> Self {
        Markup::Rtf
    }
    pub fn plain() -> Self {
        Markup::Plain
    }
}

impl Default for Markup {
    fn default() -> Self {
        Markup::Html(HtmlOptions::default())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MarkupBibMeta {
    #[serde(rename = "markupPre")]
    markup_pre: String,
    #[serde(rename = "markupPost")]
    markup_post: String,
}

impl OutputFormat for Markup {
    type Input = String;
    type Build = Vec<InlineElement>;
    type Output = String;
    type BibMeta = MarkupBibMeta;

    fn meta(&self) -> Self::BibMeta {
        let (pre, post) = match self {
            Markup::Html(_) => ("<div class=\"csl-bib-body\">", "</div>"),
            Markup::Rtf => ("", ""),
            Markup::Plain => ("", ""),
        };
        MarkupBibMeta {
            markup_pre: pre.to_string(),
            markup_post: post.to_string(),
        }
    }

    #[inline]
    fn ingest(&self, input: &str, options: &IngestOptions) -> Self::Build {
        let mut nodes = MicroNode::parse(input, options);
        options.apply_text_case_micro(&mut nodes);
        vec![InlineElement::Micro(nodes)]
    }

    #[inline]
    fn plain(&self, s: &str) -> Self::Build {
        self.text_node(s.to_owned(), None)
    }

    #[inline]
    fn text_node(&self, text: String, f: Option<Formatting>) -> Vec<InlineElement> {
        if text.is_empty() {
            return vec![];
        }
        let v = vec![Text(text)];
        self.fmt_vec(v, f)
    }

    #[inline]
    fn seq(&self, nodes: impl Iterator<Item = Self::Build>) -> Self::Build {
        itertools::concat(nodes)
    }

    #[inline]
    fn join_delim(&self, a: Self::Build, delim: &str, b: Self::Build) -> Self::Build {
        [a, b].join_many(&self.plain(delim))
    }

    #[inline]
    fn group(
        &self,
        nodes: Vec<Self::Build>,
        delimiter: &str,
        formatting: Option<Formatting>,
    ) -> Self::Build {
        if nodes.len() == 1 {
            self.fmt_vec(nodes.into_iter().nth(0).unwrap(), formatting)
        } else {
            let delim = self.plain(delimiter);
            self.fmt_vec(nodes.join_many(&delim), formatting)
        }
    }

    #[inline]
    fn with_format(&self, a: Self::Build, f: Option<Formatting>) -> Self::Build {
        self.fmt_vec(a, f)
    }

    #[inline]
    fn with_display(
        &self,
        a: Self::Build,
        display: Option<DisplayMode>,
        in_bib: bool,
    ) -> Self::Build {
        if in_bib {
            if let Some(d) = display {
                return vec![InlineElement::Div(d, a)];
            }
        }
        a
    }

    #[inline]
    fn quoted(&self, b: Self::Build, quotes: LocalizedQuotes) -> Self::Build {
        // Default not is_inner; figure out which ones are inner/outer when doing flip-flop later.
        vec![InlineElement::Quoted {
            is_inner: false,
            localized: quotes,
            inlines: b,
        }]
    }

    #[inline]
    fn hyperlinked(&self, a: Self::Build, target: Option<&str>) -> Self::Build {
        // TODO: allow internal linking using the Attr parameter (e.g.
        // first-reference-note-number)
        if let Some(target) = target {
            vec![InlineElement::Anchor {
                title: "".into(),
                url: target.into(),
                content: a,
            }]
        } else {
            a
        }
    }

    #[inline]
    fn is_empty(&self, a: &Self::Build) -> bool {
        a.is_empty()
    }

    #[inline]
    fn output(&self, intermediate: Self::Build) -> Self::Output {
        let null = FlipFlopState::default();
        self.output_with_state(intermediate, null)
    }

    #[inline]
    fn output_in_context(
        &self,
        intermediate: Self::Build,
        format_stacked: Formatting,
    ) -> Self::Output {
        let state = FlipFlopState::from_formatting(format_stacked);
        self.output_with_state(intermediate, state)
    }

    #[inline]
    fn stack_preorder(&self, dest: &mut String, stack: &[FormatCmd]) {
        match *self {
            Markup::Html(options) => HtmlWriter::new(dest, options).stack_preorder(stack),
            Markup::Rtf => PlainWriter::new(dest).stack_preorder(stack),
            Markup::Plain => PlainWriter::new(dest).stack_preorder(stack),
        }
    }

    #[inline]
    fn stack_postorder(&self, dest: &mut String, stack: &[FormatCmd]) {
        match *self {
            Markup::Html(options) => HtmlWriter::new(dest, options).stack_postorder(stack),
            Markup::Rtf => PlainWriter::new(dest).stack_postorder(stack),
            Markup::Plain => PlainWriter::new(dest).stack_postorder(stack),
        }
    }

    #[inline]
    fn tag_stack(&self, formatting: Formatting, display: Option<DisplayMode>) -> Vec<FormatCmd> {
        tag_stack(formatting, display)
    }

    #[inline]
    fn append_suffix(&self, pre_and_content: &mut Self::Build, suffix: &str) {
        let suffix = MicroNode::parse(suffix, &IngestOptions::default());
        use self::move_punctuation::append_suffix;
        append_suffix(pre_and_content, suffix);
    }
}

impl Markup {
    fn fmt_vec(
        &self,
        inlines: Vec<InlineElement>,
        formatting: Option<Formatting>,
    ) -> Vec<InlineElement> {
        if let Some(f) = formatting {
            vec![Formatted(inlines, f)]
        } else {
            inlines
        }
    }

    fn output_with_state(
        &self,
        intermediate: <Self as OutputFormat>::Build,
        initial_state: FlipFlopState,
    ) -> <Self as OutputFormat>::Output {
        let mut flipped = initial_state.flip_flop_inlines(&intermediate);
        move_punctuation(&mut flipped);
        let mut dest = String::new();
        match *self {
            Markup::Html(options) => HtmlWriter::new(&mut dest, options).write_inlines(&flipped),
            Markup::Rtf => RtfWriter::new(&mut dest).write_inlines(&flipped),
            Markup::Plain => PlainWriter::new(&mut dest).write_inlines(&flipped),
        }
        dest
    }
}

pub trait MarkupWriter {
    fn write_escaped(&mut self, text: &str);
    fn stack_preorder(&mut self, stack: &[FormatCmd]);
    fn stack_postorder(&mut self, stack: &[FormatCmd]);

    /// Use this to write an InlineElement::Formatted
    fn stack_formats(
        &mut self,
        inlines: &[InlineElement],
        formatting: Formatting,
        display: Option<DisplayMode>,
    ) {
        let stack = tag_stack(formatting, display);
        self.stack_preorder(&stack);
        self.write_inlines(inlines);
        self.stack_postorder(&stack);
    }

    fn write_micro(&mut self, micro: &MicroNode);
    /// Returned boolean = true if it used the peeked element to move some punctuation inside, and
    /// hence should skip it.
    fn write_micros(&mut self, micros: &[MicroNode]) {
        for micro in micros {
            self.write_micro(micro);
        }
    }
    fn write_inline(&mut self, inline: &InlineElement);
    fn write_inlines(&mut self, inlines: &[InlineElement]) {
        for inline in inlines {
            self.write_inline(inline);
        }
    }
}

fn tag_stack(formatting: Formatting, display: Option<DisplayMode>) -> Vec<FormatCmd> {
    use super::FormatCmd::*;
    let mut stack = Vec::new();
    match display {
        Some(DisplayMode::Block) => stack.push(DisplayBlock),
        Some(DisplayMode::Indent) => stack.push(DisplayIndent),
        Some(DisplayMode::LeftMargin) => stack.push(DisplayLeftMargin),
        Some(DisplayMode::RightInline) => stack.push(DisplayRightInline),
        _ => {}
    }
    match formatting.font_style {
        Some(FontStyle::Italic) => stack.push(FontStyleItalic),
        Some(FontStyle::Oblique) => stack.push(FontStyleOblique),
        Some(FontStyle::Normal) => stack.push(FontStyleNormal),
        _ => {}
    }
    match formatting.font_weight {
        Some(FontWeight::Bold) => stack.push(FontWeightBold),
        Some(FontWeight::Light) => stack.push(FontWeightLight),
        Some(FontWeight::Normal) => stack.push(FontWeightNormal),
        _ => {}
    }
    match formatting.font_variant {
        Some(FontVariant::SmallCaps) => stack.push(FontVariantSmallCaps),
        Some(FontVariant::Normal) => stack.push(FontVariantNormal),
        _ => {}
    };
    match formatting.text_decoration {
        Some(TextDecoration::Underline) => stack.push(TextDecorationUnderline),
        Some(TextDecoration::None) => stack.push(TextDecorationNone),
        _ => {}
    }
    match formatting.vertical_alignment {
        Some(VerticalAlignment::Superscript) => stack.push(VerticalAlignmentSuperscript),
        Some(VerticalAlignment::Subscript) => stack.push(VerticalAlignmentSubscript),
        Some(VerticalAlignment::Baseline) => stack.push(VerticalAlignmentBaseline),
        _ => {}
    }
    stack
}
