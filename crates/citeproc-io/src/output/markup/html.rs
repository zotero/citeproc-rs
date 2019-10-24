// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::InlineElement;
use super::MarkupWriter;
use crate::output::micro_html::MicroNode;
use crate::output::FormatCmd;
use csl::DisplayMode;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HtmlWriter {
    // TODO: is it enough to have one set of localized quotes for the entire style?
    // quotes: LocalizedQuotes,
    use_b_for_strong: bool,
    link_anchors: bool,
}

impl Default for HtmlWriter {
    fn default() -> Self {
        HtmlWriter {
            use_b_for_strong: false,
            link_anchors: true,
        }
    }
}

impl HtmlWriter {
    pub fn test_suite() -> Self {
        HtmlWriter {
            use_b_for_strong: true,
            link_anchors: false,
        }
    }
}

impl MarkupWriter for HtmlWriter {
    fn stack_preorder(&self, s: &mut String, stack: &[FormatCmd]) {
        for cmd in stack.iter() {
            let tag = cmd.html_tag(self);
            s.push_str("<");
            s.push_str(tag.0);
            s.push_str(tag.1);
            s.push_str(">");
        }
    }

    fn stack_postorder(&self, s: &mut String, stack: &[FormatCmd]) {
        for cmd in stack.iter().rev() {
            let tag = cmd.html_tag(self);
            s.push_str("</");
            s.push_str(tag.0);
            s.push_str(">");
        }
    }

    fn write_inline(&self, s: &mut String, inline: &InlineElement) {
        inline.to_html_inner(s, self);
    }
}

impl MicroNode {
    fn to_html_inner(&self, s: &mut String, options: &HtmlWriter) {
        use MicroNode::*;
        match self {
            Text(text) => {
                use v_htmlescape::escape;
                s.push_str(&escape(text).to_string());
            }
            Formatted(nodes, cmd) => {
                let tag = cmd.html_tag(options);
                *s += "<";
                *s += tag.0;
                *s += tag.1;
                *s += ">";
                for node in nodes {
                    node.to_html_inner(s, options);
                }
                *s += "</";
                *s += tag.0;
                *s += ">";
            }
            NoCase(inners) => {
                for i in inners {
                    i.to_html_inner(s, options);
                }
            }
        }
    }
}

impl FormatCmd {
    fn html_tag(&self, options: &HtmlWriter) -> (&'static str, &'static str) {
        use super::FormatCmd::*;
        match self {
            FontStyleItalic => ("i", ""),
            FontStyleOblique => ("span", r#" style="font-style:oblique;""#),
            FontStyleNormal => ("span", r#" style="font-style:normal;""#),

            FontWeightBold => {
                if options.use_b_for_strong {
                    ("b", "")
                } else {
                    ("strong", "")
                }
            }
            FontWeightNormal => ("span", r#" style="font-weight:normal;""#),
            FontWeightLight => ("span", r#" style="font-weight:light;""#),

            FontVariantSmallCaps => ("span", r#" style="font-variant:small-caps;""#),
            FontVariantNormal => ("span", r#" style="font-variant:normal;""#),

            TextDecorationUnderline => ("span", r#" style="text-decoration:underline;""#),
            TextDecorationNone => ("span", r#" style="text-decoration:none;""#),

            VerticalAlignmentSuperscript => ("sup", ""),
            VerticalAlignmentSubscript => ("sub", ""),
            VerticalAlignmentBaseline => ("span", r#" style="vertical-alignment:baseline;"#),
        }
    }
}

impl InlineElement {
    fn to_html(inlines: &[InlineElement], options: &HtmlWriter) -> String {
        let mut s = String::new();
        for i in inlines {
            i.to_html_inner(&mut s, options);
        }
        s
    }
    // fn is_disp(&self, disp: DisplayMode) -> bool {
    //     match *self {
    //         Div(display, _) => disp == display,
    //         _ => false,
    //     }
    // }
    // fn collapsing_left_margin(inlines: &[InlineElement], s: &mut s) {
    //     use super::InlineElement::*;
    //     let mut iter = inlines.iter().peekable();
    //     while let Some(i) = iter.next() {
    //         let peek = iter.peek();
    //         match i {
    //             Div(display, inlines) => {
    //                 if display == DisplayMode::LeftMargin {
    //                     if let Some(peek) = iter.peek() {
    //                         if !peek.is_disp(DisplayMode::RightInline) {
    //                             Div(DisplayMode::Block)
    //                             continue;
    //                         }
    //                     }
    //                 }
    //             }
    //         }
    //         i.to_html_inner(&mut s, options);
    //     }
    // }
    fn to_html_inner(&self, s: &mut String, options: &HtmlWriter) {
        use super::InlineElement::*;
        match self {
            Text(text) => {
                use v_htmlescape::escape;
                s.push_str(&escape(text).to_string());
            }
            Div(display, inlines) => {
                use std::fmt::Write;
                let disp: &str = display.as_ref();
                write!(s, "<div class=\"csl-{}\">", disp).unwrap();
                for i in inlines {
                    i.to_html_inner(s, options);
                }
                s.push_str("</div>");
            }
            Micro(micros) => {
                for micro in micros {
                    micro.to_html_inner(s, options);
                }
            }
            Formatted(inlines, formatting) => {
                options.stack_formats(s, inlines, *formatting);
            }
            Quoted(_qt, inners) => {
                // TODO: use localized quotes
                // TODO: move punctuation
                s.push('“');
                for i in inners {
                    i.to_html_inner(s, options);
                }
                s.push('”');
            }
            Anchor {
                title: _,
                url,
                content,
            } => {
                if options.link_anchors {
                    s.push_str(r#"<a href=""#);
                    // TODO: HTML-quoted-escape? the url?
                    s.push_str(&url.trim());
                    s.push_str(r#"">"#);
                    for i in content {
                        i.to_html_inner(s, options);
                    }
                    s.push_str("</a>");
                } else {
                    s.push_str(&url.trim());
                }
            }
        }
    }
}
