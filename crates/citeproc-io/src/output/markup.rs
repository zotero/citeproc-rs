// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::micro_html::MicroNode;
use super::{FormatCmd, LocalizedQuotes, OutputFormat};
use crate::utils::JoinMany;
use crate::IngestOptions;
use csl::style::{
    FontStyle, FontVariant, FontWeight, Formatting, TextDecoration, VerticalAlignment,
};
use self::InlineElement::*;

mod rtf;
use self::rtf::RtfWriter;

mod html;
use self::html::HtmlWriter;

mod flip_flop;
use self::flip_flop::FlipFlopState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Markup {
    Html(HtmlWriter),
    Rtf(RtfWriter),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum QuoteType {
    SingleQuote,
    DoubleQuote,
}

/// TODO: serialize and deserialize using an HTML parser?
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InlineElement {
    /// This is how we can flip-flop only user-supplied styling.
    /// Inside this is parsed micro html
    Micro(Vec<MicroNode>),

    Formatted(Vec<InlineElement>, Formatting),
    Quoted(QuoteType, Vec<InlineElement>),
    Text(String),
    Anchor {
        title: String,
        url: String,
        content: Vec<InlineElement>,
    },
}

impl Markup {
    pub fn html() -> Self {
        Markup::Html(HtmlWriter::default())
    }
    pub fn test_html() -> Self {
        Markup::Html(HtmlWriter::test_suite())
    }
    pub fn rtf() -> Self {
        Markup::Rtf(RtfWriter::default())
    }
}

impl Default for Markup {
    fn default() -> Self {
        Markup::Html(HtmlWriter::default())
    }
}

impl OutputFormat for Markup {
    type Input = String;
    type Build = Vec<InlineElement>;
    type Output = String;

    #[inline]
    fn ingest(&self, input: &str, options: IngestOptions) -> Self::Build {
        vec![InlineElement::Micro(MicroNode::parse(input, options))]
    }

    #[inline]
    fn plain(&self, s: &str) -> Self::Build {
        self.text_node(s.to_owned(), None)
    }

    #[inline]
    fn text_node(&self, text: String, f: Option<Formatting>) -> Vec<InlineElement> {
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
    fn quoted(&self, b: Self::Build, quotes: &LocalizedQuotes) -> Self::Build {
        let qt = match quotes {
            LocalizedQuotes::Single(..) => QuoteType::SingleQuote,
            LocalizedQuotes::Double(..) => QuoteType::SingleQuote,
            // Would this be better? Only allow
            // LocalizedQuotes::Double(open, close) |
            // LocalizedQuotes::Single(open, close) => {
            //     return self.affixed(b, Affixes { prefix: open.clone(), suffix: close.clone() });
            // }
        };
        vec![InlineElement::Quoted(qt, b)]
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
    fn stack_preorder(&self, s: &mut String, stack: &[FormatCmd]) {
        match self {
            Markup::Html(ref writer) => writer.stack_preorder(s, stack),
            Markup::Rtf(ref writer) => writer.stack_preorder(s, stack),
        }
    }

    #[inline]
    fn stack_postorder(&self, s: &mut String, stack: &[FormatCmd]) {
        match self {
            Markup::Html(ref writer) => writer.stack_postorder(s, stack),
            Markup::Rtf(ref writer) => writer.stack_postorder(s, stack),
        }
    }

    #[inline]
    fn tag_stack(&self, formatting: Formatting) -> Vec<FormatCmd> {
        tag_stack(formatting)
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
        let flipped = initial_state.flip_flop_inlines(&intermediate);
        let mut string = String::new();
        match self {
            Markup::Html(ref writer) => writer.write_inlines(&mut string, &flipped),
            Markup::Rtf(ref writer) => writer.write_inlines(&mut string, &flipped),
        }
        string
    }
}

pub trait MarkupWriter {
    fn stack_preorder(&self, s: &mut String, stack: &[FormatCmd]);
    fn stack_postorder(&self, s: &mut String, stack: &[FormatCmd]);
    fn write_inline(&self, s: &mut String, inline: &InlineElement);
    fn write_inlines(&self, s: &mut String, inlines: &[InlineElement]) {
        for i in inlines {
            self.write_inline(s, i);
        }
    }

    /// Use this to write an InlineElement::Formatted
    fn stack_formats(
        &self,
        s: &mut String,
        inlines: &[InlineElement],
        formatting: Formatting,
    ) {
        let stack = tag_stack(formatting);
        self.stack_preorder(s, &stack);
        for inner in inlines {
            self.write_inline(s, inner);
        }
        self.stack_postorder(s, &stack);
    }
}

fn tag_stack(formatting: Formatting) -> Vec<FormatCmd> {
    use super::FormatCmd::*;
    let mut stack = Vec::new();
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

