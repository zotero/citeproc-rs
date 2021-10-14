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
    fn write_escaped(&mut self, text: &str) {
        write!(self.dest, "{}", v_htmlescape::escape(text)).unwrap();
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
            Anchor { url, content, .. } => {
                if self.options.link_anchors {
                    self.dest.push_str(r#"<a href=""#);
                    escape_attribute(self.dest, url.trim()).unwrap();
                    self.dest.push_str(r#"">"#);
                    self.write_inlines(content, false);
                    self.dest.push_str("</a>");
                } else {
                    self.dest.push_str(&url.trim());
                }
            }
        }
    }
}

impl FormatCmd {
    fn html_tag(self, options: &FormatOptions) -> (&'static str, &'static str) {
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
///
/// Works for attributes surrounded by double quotes, not single.
fn scan_encodable_a<'a>(remain: &'a str) -> IResult<&'a str, Encodable<'a>> {
    nbc::take_till1(|x| matches!(x, '<' | '&' | '"'))
        .map(Encodable::Chunk)
        .or(nbc::tag("<").map(|_| Encodable::Esc("&lt;")))
        .or(nbc::tag("&").map(|_| Encodable::Esc("&amp;")))
        .or(nbc::tag("\"").map(|_| Encodable::Esc("&quot;")))
        .parse(remain)
}

fn escape_attribute<F: fmt::Write>(f: &mut F, attr_inner: &str) -> fmt::Result {
    let mut remain = attr_inner;
    while let Ok((rest, chunk)) = scan_encodable_a(remain) {
        remain = rest;
        match chunk {
            Encodable::Chunk(s) => f.write_str(s)?,
            Encodable::Esc(s) => f.write_str(s)?,
        }
    }
    Ok(())
}
