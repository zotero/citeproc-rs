// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::{FormatOptions, InlineElement, MarkupWriter, MaybeTrimStart};
use crate::output::micro_html::MicroNode;
use crate::output::FormatCmd;
use crate::String;
use csl::Formatting;

#[derive(Debug)]
pub struct RtfWriter<'a> {
    dest: &'a mut String,
    options: FormatOptions,
}

impl<'a> RtfWriter<'a> {
    pub fn new(dest: &'a mut String, options: FormatOptions) -> Self {
        RtfWriter {
            dest,
            options,
        }
    }
}

impl<'a> MarkupWriter for RtfWriter<'a> {
    fn write_escaped(&mut self, text: &str) {
        rtf_escape_into(text, self.dest);
    }
    fn stack_preorder(&mut self, stack: &[FormatCmd]) {
        for cmd in stack.iter() {
            let tag = cmd.rtf_tag();
            self.dest.push('{');
            self.dest.push_str(tag);
        }
    }

    fn stack_postorder(&mut self, stack: &[FormatCmd]) {
        for cmd in stack.iter() {
            if *cmd == FormatCmd::DisplayRightInline {
                let tlen = self.dest.trim_end_matches(' ').len();
                self.dest.truncate(tlen);
            }
            self.dest.push('}');
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
                let tag = cmd.rtf_tag();
                self.dest.push('{');
                self.dest.push_str(tag);
                self.write_micros(nodes, trim_start);
                self.dest.push('}');
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
                rtf_escape_into(text.trim_start_if(trim_start), self.dest);
            }
            Div(display, inlines) => {
                self.stack_formats(inlines, Formatting::default(), Some(*display))
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
                self.write_escaped(localized.opening(*is_inner).trim_start_if(trim_start));
                self.write_inlines(inlines, false);
                self.write_escaped(localized.closing(*is_inner));
            }
            Anchor { url, content, .. } => {
                // TODO: quoted-escape the url?
                self.dest.push_str(r#"{\field{\*\fldinst HYPERLINK \""#);
                self.dest.push_str(&url);
                self.dest.push_str(r#""}{\fldrslt "#);
                self.write_inlines(content, true);
                self.dest.push_str("}}");
            }
        }
    }
}

impl FormatCmd {
    fn rtf_tag(self) -> &'static str {
        use super::FormatCmd::*;
        match self {
            // TODO: RTF display commands
            DisplayBlock => "",
            DisplayIndent => "",
            DisplayLeftMargin => "",
            DisplayRightInline => "",

            FontStyleItalic => "\\i ",
            FontStyleOblique => "\\i ",
            FontStyleNormal => "\\i0 ",

            FontWeightBold => "\\b ",
            FontWeightNormal => "\\b0 ",

            // Not supported?
            FontWeightLight => "\\b0 ",

            FontVariantSmallCaps => "\\scaps ",
            FontVariantNormal => "\\scaps0 ",

            TextDecorationUnderline => "\\ul ",
            TextDecorationNone => "\\ul0 ",

            VerticalAlignmentSuperscript => "\\super ",
            VerticalAlignmentSubscript => "\\sub ",
            VerticalAlignmentBaseline => "\\nosupersub ",
        }
    }
}

fn rtf_escape_into(s: &str, buf: &mut String) {
    let mut utf16_buffer = [0; 2];
    for c in s.chars() {
        match c {
            '\\' | '{' | '}' => {
                buf.push('\\');
                buf.push(c);
            }
            '\t' => buf.push_str("\\tab "),
            '\n' => buf.push_str("\\line "),
            '\x20'..='\x7e' => buf.push(c),
            _unicode => {
                let slice = c.encode_utf16(&mut utf16_buffer);
                for &u16c in slice.iter() {
                    use std::fmt::Write;
                    // The spec says 'most control words' accept signed 16-bit, but Word and
                    // TextEdit both produce unsigned 16-bit, and even convert signed to unsigned
                    // when saving. So we'll do that here. (citeproc-js does this too.)
                    //
                    // Terminates the \uN keyword with a space, where citeproc-js uses \uN{}
                    let _result = write!(buf, "\\uc0\\u{} ", u16c);
                }
            }
        }
    }
}

#[cfg(test)]
fn rtf_escape(s: &str) -> String {
    let mut buf = String::new();
    rtf_escape_into(s, &mut buf);
    buf
}

#[test]
fn test_rtf_escape_unicode() {
    let tab = "Hello \t";
    assert_eq!(&rtf_escape(tab), r"Hello \tab ");

    let heart = "Hello \u{2764}";
    assert_eq!(&rtf_escape(heart), r"Hello \uc0\u10084 ");

    let poop = "Hello 💩";
    assert_eq!(&rtf_escape(poop), r"Hello \uc0\u55357 \uc0\u56489 ");
}
