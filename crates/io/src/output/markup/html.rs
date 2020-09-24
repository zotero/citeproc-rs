// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use smartstring::alias::String;
use super::InlineElement;
use super::MarkupWriter;
use crate::output::micro_html::MicroNode;
use crate::output::FormatCmd;
use csl::Formatting;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct HtmlOptions {
    // TODO: is it enough to have one set of localized quotes for the entire style?
    // quotes: LocalizedQuotes,
    use_b_for_strong: bool,
    link_anchors: bool,
}

impl Default for HtmlOptions {
    fn default() -> Self {
        HtmlOptions {
            use_b_for_strong: false,
            link_anchors: true,
        }
    }
}

impl HtmlOptions {
    pub fn test_suite() -> Self {
        HtmlOptions {
            use_b_for_strong: true,
            link_anchors: false,
        }
    }
}

#[derive(Debug)]
pub struct HtmlWriter<'a> {
    dest: &'a mut String,
    options: HtmlOptions,
}

impl<'a> HtmlWriter<'a> {
    pub fn new(dest: &'a mut String, options: HtmlOptions) -> Self {
        HtmlWriter { dest, options }
    }
}

impl<'a> MarkupWriter for HtmlWriter<'a> {
    fn write_escaped(&mut self, text: &str) {
        use v_htmlescape::escape;
        self.dest.push_str(&escape(text).to_string());
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
            let tag = cmd.html_tag(&self.options);
            self.dest.push_str("</");
            self.dest.push_str(tag.0);
            self.dest.push_str(">");
        }
    }

    fn write_micro(&mut self, micro: &MicroNode) {
        use MicroNode::*;
        match micro {
            Text(text) => {
                self.write_escaped(text);
            }
            Quoted {
                is_inner,
                localized,
                children,
            } => {
                self.write_escaped(localized.opening(*is_inner));
                self.write_micros(children);
                self.write_escaped(localized.closing(*is_inner));
            }
            Formatted(nodes, cmd) => {
                self.stack_preorder(&[*cmd][..]);
                self.write_micros(nodes);
                self.stack_postorder(&[*cmd][..]);
            }
            NoCase(inners) => {
                self.write_micros(inners);
            }
            NoDecor(inners) => {
                self.write_micros(inners);
            }
        }
    }

    fn write_inline(&mut self, inline: &InlineElement) {
        use super::InlineElement::*;
        match inline {
            Text(text) => {
                self.write_escaped(text);
            }
            Div(display, inlines) => {
                self.stack_formats(inlines, Formatting::default(), Some(*display));
            }
            Micro(micros) => {
                self.write_micros(micros);
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
                self.write_escaped(localized.opening(*is_inner));
                self.write_inlines(inlines);
                self.write_escaped(localized.closing(*is_inner));
            }
            Anchor { url, content, .. } => {
                if self.options.link_anchors {
                    self.dest.push_str(r#"<a href=""#);
                    // TODO: HTML-quoted-escape? the url?
                    self.dest.push_str(&url.trim());
                    self.dest.push_str(r#"">"#);
                    self.write_inlines(content);
                    self.dest.push_str("</a>");
                } else {
                    self.dest.push_str(&url.trim());
                }
            }
        }
    }
}

impl FormatCmd {
    fn html_tag(self, options: &HtmlOptions) -> (&'static str, &'static str) {
        match self {
            FormatCmd::DisplayBlock => ("div", r#" class="csl-block""#),
            FormatCmd::DisplayIndent => ("div", r#" class="csl-indent""#),
            FormatCmd::DisplayLeftMargin => ("div", r#" class="csl-left-margin""#),
            FormatCmd::DisplayRightInline => ("div", r#" class="csl-right-inline""#),

            FormatCmd::FontStyleItalic => ("i", ""),
            FormatCmd::FontStyleOblique => ("span", r#" style="font-style:oblique;""#),
            FormatCmd::FontStyleNormal => ("span", r#" style="font-style:normal;""#),

            FormatCmd::FontWeightBold => {
                if options.use_b_for_strong {
                    ("b", "")
                } else {
                    ("strong", "")
                }
            }
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

// impl InlineElement {
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
// }
