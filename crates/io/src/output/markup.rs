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

// Basically, affixes go outside Quoted elements. So we can just look for text elements that come
// right after quoted ones.
fn move_punctuation(slice: &mut [InlineElement]) {
    fn is_punc(c: char) -> bool {
        c == '.' || c == ',' || c == '!'
    }

    if slice.len() >= 2 {
        let len = slice.len();
        for i in 0..len - 1 {
            if let Some((first, rest)) = (&mut slice[i..]).split_first_mut() {
                let next = rest
                    .first_mut()
                    .expect("only iterated to len-1, so infallible");

                // Quoted elements are less common, so search for it first
                let quoted = if let Some(x) = find_right_quote(first) {
                    x
                } else {
                    continue;
                };

                fn find_string_micro(m: &mut MicroNode) -> Option<&mut String> {
                    match m {
                        MicroNode::Text(string) => Some(string),
                        MicroNode::NoCase(nodes) | MicroNode::Formatted(nodes, _) => {
                            nodes.first_mut().and_then(find_string_micro)
                        }
                        _ => None,
                    }
                }

                // Must be followed by some text
                let string = match next {
                    InlineElement::Text(ref mut string) => string,
                    InlineElement::Micro(ref mut micros) => {
                        if let Some(string) = micros.first_mut().and_then(find_string_micro) {
                            string
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                };

                // That text must be is_punc
                if !string.chars().nth(0).map_or(false, is_punc) {
                    continue;
                }

                // O(n), but n tends to be 2, like with ", " so this is ok
                let c = string.remove(0);
                let mut s = String::new();
                s.push(c);

                match quoted {
                    Quoted::Inline(inlines) => {
                        inlines.push(InlineElement::Text(s));
                    }
                    Quoted::Micro(children) => {
                        children.push(MicroNode::Text(s));
                    }
                }
            }
        }
    } else {
        // recurse manually over the 0 or 1 items in it, and their children
        for inl in slice.iter_mut() {
            match inl {
                InlineElement::Quoted { inlines, .. }
                | InlineElement::Div(_, inlines)
                | InlineElement::Formatted(inlines, _) => move_punctuation(inlines),
                _ => {}
            }
        }
    }

    enum Quoted<'a> {
        Inline(&'a mut Vec<InlineElement>),
        Micro(&'a mut Vec<MicroNode>),
    }

    fn find_right_quote_micro<'b>(micro: &'b mut MicroNode) -> Option<Quoted<'b>> {
        match micro {
            MicroNode::Quoted {
                localized,
                children,
                ..
            } => {
                if localized.punctuation_in_quote {
                    // prefer to dive deeper, and catch "'inner quotes,'" too.

                    // This is a limitation of NLL borrowck analysis at the moment, but will be
                    // solved with Polonius: https://users.rust-lang.org/t/solved-borrow-doesnt-drop-returning-this-value-requires-that/24182
                    //
                    // The unsafe is casting a vec to itself; it's safe.
                    //
                    // let deeper = children.last_mut().and_then(find_right_quote_micro);
                    // if deeper.is_some() {
                    //     return deeper;
                    // }

                    if !children.is_empty() {
                        let len = children.len();
                        let last_mut =
                            unsafe { &mut (*((children) as *mut Vec<MicroNode>))[len - 1] };
                        let deeper = find_right_quote_micro(last_mut);
                        if deeper.is_some() {
                            return deeper;
                        }
                    }

                    Some(Quoted::Micro(children))
                } else {
                    None
                }
            }
            // Dive into formatted bits
            MicroNode::NoCase(nodes) | MicroNode::Formatted(nodes, _) => {
                nodes.last_mut().and_then(find_right_quote_micro)
            }
            _ => None,
        }
    }

    fn find_right_quote<'a>(el: &'a mut InlineElement) -> Option<Quoted<'a>> {
        match el {
            InlineElement::Quoted {
                localized, inlines, ..
            } => {
                if localized.punctuation_in_quote {
                    // prefer to dive deeper, and catch "'inner quotes,'" too.

                    // See above re unsafe
                    if !inlines.is_empty() {
                        let len = inlines.len();
                        let last_mut =
                            unsafe { &mut (*((inlines) as *mut Vec<InlineElement>))[len - 1] };
                        let deeper = find_right_quote(last_mut);
                        if deeper.is_some() {
                            return deeper;
                        }
                    }
                    Some(Quoted::Inline(inlines))
                } else {
                    None
                }
            }
            InlineElement::Micro(micros) => micros.last_mut().and_then(find_right_quote_micro),
            InlineElement::Div(_, inlines) | InlineElement::Formatted(inlines, _) => {
                inlines.last_mut().and_then(find_right_quote)
            }
            _ => None,
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
