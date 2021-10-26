// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::{FormatOptions, InlineElement, MarkupWriter, MaybeTrimStart};
use crate::output::micro_html::MicroNode;
use crate::output::FormatCmd;
use crate::String;
use core::fmt::{self, Write};
use csl::Formatting;
use url::Url;

#[derive(Debug)]
pub struct HtmlWriter<'a> {
    dest: &'a mut String,
    options: FormatOptions,
}

impl<'a> HtmlWriter<'a> {
    pub fn new(dest: &'a mut String, options: FormatOptions) -> Self {
        HtmlWriter { dest, options }
    }
}

impl<'a> MarkupWriter for HtmlWriter<'a> {
    fn buf(&mut self) -> &mut String {
        self.dest
    }
    fn write_escaped(&mut self, text: &str) {
        write!(self.dest, "{}", escape_html(text)).unwrap();
    }
    fn stack_preorder(&mut self, stack: &[FormatCmd]) {
        for cmd in stack.iter() {
            let tag = cmd.html_tag(&self.options);
            self.dest.push_str("<");
            self.dest.push_str(tag.0);
            self.dest.push_str(tag.1);
            self.dest.push_str(">");
        }
    }

    fn stack_postorder(&mut self, stack: &[FormatCmd]) {
        for cmd in stack.iter().rev() {
            if *cmd == FormatCmd::DisplayRightInline {
                let tlen = self.dest.trim_end_matches(' ').len();
                self.dest.truncate(tlen)
            }
            let tag = cmd.html_tag(&self.options);
            self.dest.push_str("</");
            self.dest.push_str(tag.0);
            self.dest.push_str(">");
        }
    }

    fn write_micro(&mut self, micro: &MicroNode, trim_start: bool) {
        use MicroNode::*;
        match micro {
            Text(text) => {
                self.write_escaped(text.trim_start_if(trim_start));
            }
            Quoted {
                is_inner,
                localized,
                children,
            } => {
                self.write_escaped(localized.opening(*is_inner).trim_start_if(trim_start));
                self.write_micros(children, false);
                self.write_escaped(localized.closing(*is_inner));
            }
            Formatted(nodes, cmd) => {
                self.stack_preorder(&[*cmd][..]);
                self.write_micros(nodes, trim_start);
                self.stack_postorder(&[*cmd][..]);
            }
            NoCase(inners) => {
                self.write_micros(inners, trim_start);
            }
            NoDecor(inners) => {
                self.write_micros(inners, trim_start);
            }
        }
    }

    fn write_inline(&mut self, inline: &InlineElement, trim_start: bool) {
        use super::InlineElement::*;
        match inline {
            Text(text) => {
                self.write_escaped(text.trim_start_if(trim_start));
            }
            Div(display, inlines) => {
                self.stack_formats(inlines, Formatting::default(), Some(*display));
            }
            Micro(micros) => {
                self.write_micros(micros, trim_start);
            }
            Formatted(inlines, formatting) => {
                self.stack_formats(inlines, *formatting, None);
            }
            Quoted {
                is_inner,
                localized,
                inlines,
            } => {
                // TODO: move punctuation
                self.write_escaped(localized.opening(*is_inner).trim_start_if(trim_start));
                self.write_inlines(inlines, false);
                self.write_escaped(localized.closing(*is_inner));
            }
            Linked(link) => self.write_link(r#"<a href=""#, link, r#"">"#, "</a>", self.options),
        }
    }

    fn write_url(&mut self, url: &Url, trailing_slash: bool, in_attr: bool) {
        super::write_url(
            self.dest,
            url,
            trailing_slash,
            in_attr,
            |b, s| write!(b, "{}", escape_html_attribute(s)),
            |b, s| write!(b, "{}", escape_html(s)),
        )
        .unwrap()
    }
}

impl FormatCmd {
    fn html_tag(self, _options: &FormatOptions) -> (&'static str, &'static str) {
        match self {
            FormatCmd::DisplayBlock => ("div", r#" class="csl-block""#),
            FormatCmd::DisplayIndent => ("div", r#" class="csl-indent""#),
            FormatCmd::DisplayLeftMargin => ("div", r#" class="csl-left-margin""#),
            FormatCmd::DisplayRightInline => ("div", r#" class="csl-right-inline""#),

            FormatCmd::FontStyleItalic => ("i", ""),
            FormatCmd::FontStyleOblique => ("span", r#" style="font-style:oblique;""#),
            FormatCmd::FontStyleNormal => ("span", r#" style="font-style:normal;""#),

            FormatCmd::FontWeightBold => ("b", ""),
            FormatCmd::FontWeightNormal => ("span", r#" style="font-weight:normal;""#),
            FormatCmd::FontWeightLight => ("span", r#" style="font-weight:light;""#),

            FormatCmd::FontVariantSmallCaps => ("span", r#" style="font-variant:small-caps;""#),
            FormatCmd::FontVariantNormal => ("span", r#" style="font-variant:normal;""#),

            FormatCmd::TextDecorationUnderline => {
                ("span", r#" style="text-decoration:underline;""#)
            }
            FormatCmd::TextDecorationNone => ("span", r#" style="text-decoration:none;""#),

            FormatCmd::VerticalAlignmentSuperscript => ("sup", ""),
            FormatCmd::VerticalAlignmentSubscript => ("sub", ""),
            FormatCmd::VerticalAlignmentBaseline => {
                ("span", r#" style="vertical-alignment:baseline;""#)
            }
        }
    }
}

use nom::{bytes::complete as nbc, IResult, Parser};

enum Encodable<'a> {
    Chunk(&'a str),
    Esc(&'static str),
}

/// Try to gobble up as many non-escaping characters as possible.
fn scan_encodable_attr<'a>(remain: &'a str) -> IResult<&'a str, Encodable<'a>> {
    nbc::take_till1(|x| matches!(x, '"' | '\''))
        .map(Encodable::Chunk)
        .or(nbc::tag("\"").map(|_| Encodable::Esc("&quot;")))
        .or(nbc::tag("'").map(|_| Encodable::Esc("&#x27;")))
        .parse(remain)
}

/// Try to gobble up as many non-escaping characters as possible.
fn scan_encodable<'a>(remain: &'a str) -> IResult<&'a str, Encodable<'a>> {
    nbc::take_till1(|x| matches!(x, '<' | '>' | '&' | '"' | '\''))
        .map(Encodable::Chunk)
        .or(nbc::tag("<").map(|_| Encodable::Esc("&lt;")))
        .or(nbc::tag(">").map(|_| Encodable::Esc("&gt;")))
        .or(nbc::tag("&").map(|_| Encodable::Esc("&amp;")))
        .or(nbc::tag("\"").map(|_| Encodable::Esc("&quot;")))
        .or(nbc::tag("'").map(|_| Encodable::Esc("&#x27;")))
        .parse(remain)
}

struct HtmlEscaper<'a> {
    text: &'a str,
}

impl fmt::Display for HtmlEscaper<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut remain = self.text;
        while let Ok((rest, chunk)) = scan_encodable(remain) {
            remain = rest;
            match chunk {
                Encodable::Chunk(s) => f.write_str(s)?,
                Encodable::Esc(s) => f.write_str(s)?,
            }
        }
        Ok(())
    }
}

struct HtmlAttrEscaper<'a> {
    attr_inner: &'a str,
}

impl fmt::Display for HtmlAttrEscaper<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut remain = self.attr_inner;
        while let Ok((rest, chunk)) = scan_encodable_attr(remain) {
            remain = rest;
            match chunk {
                Encodable::Chunk(s) => f.write_str(s)?,
                Encodable::Esc(s) => f.write_str(s)?,
            }
        }
        Ok(())
    }
}

fn escape_html_attribute(attr_inner: &str) -> HtmlAttrEscaper {
    HtmlAttrEscaper { attr_inner }
}

fn escape_html(text: &str) -> HtmlEscaper {
    HtmlEscaper { text }
}
