// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::InlineElement;
use super::MarkupWriter;
use crate::output::micro_html::MicroNode;
use crate::output::FormatCmd;

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct RtfWriter {}

impl MarkupWriter for RtfWriter {
    fn stack_preorder(&self, s: &mut String, stack: &[FormatCmd]) {
        for cmd in stack.iter() {
            let tag = cmd.rtf_tag(self);
            s.push('{');
            s.push_str(tag);
        }
    }

    fn stack_postorder(&self, s: &mut String, stack: &[FormatCmd]) {
        for _cmd in stack.iter() {
            s.push('}');
        }
    }

    fn write_inline(&self, s: &mut String, inline: &InlineElement) {
        inline.to_rtf_inner(s, self);
    }
}

impl FormatCmd {
    fn rtf_tag(&self, _options: &RtfWriter) -> &'static str {
        use super::FormatCmd::*;
        match self {
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

impl MicroNode {
    fn to_rtf_inner(&self, s: &mut String, options: &RtfWriter) {
        use MicroNode::*;
        match self {
            Text(text) => {
                rtf_escape_into(text, s);
            }
            Formatted(nodes, cmd) => {
                let tag = cmd.rtf_tag(options);
                *s += "{";
                *s += tag;
                for node in nodes {
                    node.to_rtf_inner(s, options);
                }
                *s += "}";
            }
            NoCase(inners) => {
                for i in inners {
                    i.to_rtf_inner(s, options);
                }
            }
        }
    }
}

impl InlineElement {
    fn to_rtf(inlines: &[InlineElement], options: &RtfWriter) -> String {
        let mut s = String::new();
        for i in inlines {
            i.to_rtf_inner(&mut s, options);
        }
        s
    }

    fn to_rtf_inner(&self, s: &mut String, options: &RtfWriter) {
        use super::InlineElement::*;
        match self {
            Text(text) => {
                rtf_escape_into(text, s);
            }
            Micro(micros) => {
                for micro in micros {
                    micro.to_rtf_inner(s, options);
                }
            }
            Formatted(inlines, formatting) => {
                options.stack_formats(s, inlines, *formatting);
            }
            Quoted(_qt, inners) => {
                s.push('"');
                for i in inners {
                    i.to_rtf_inner(s, options);
                }
                s.push('"');
            }
            Anchor {
                title: _,
                url,
                content,
            } => {
                // TODO: {\field{\*\fldinst{HYPERLINK "https://google.com"}}{\fldrslt whatever}}
                // TODO: HTML-quoted-escape? the url?
                s.push_str(r#"<a href=""#);
                s.push_str(&url);
                s.push_str(r#"">"#);
                for i in content {
                    i.to_rtf_inner(s, options);
                }
                s.push_str("</a>");
            }
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
    let mut buf = String::with_capacity(s.len());
    rtf_escape_into(s, &mut buf);
    buf
}

#[test]
fn test_rtf_escape_unicode() {
    let tab = "Hello \t";
    assert_eq!(rtf_escape(tab), r"Hello \tab ");

    let heart = "Hello \u{2764}";
    assert_eq!(rtf_escape(heart), r"Hello \uc0\u10084 ");

    let poop = "Hello 💩";
    assert_eq!(rtf_escape(poop), r"Hello \uc0\u55357 \uc0\u56489 ");
}
