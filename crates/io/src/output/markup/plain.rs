// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::InlineElement;
use super::MarkupWriter;
use crate::output::micro_html::MicroNode;
use crate::output::FormatCmd;
use csl::Formatting;

#[derive(Debug)]
pub struct PlainWriter<'a> {
    dest: &'a mut String,
}

impl<'a> PlainWriter<'a> {
    pub fn new(dest: &'a mut String) -> Self {
        PlainWriter { dest }
    }
}

impl<'a> MarkupWriter for PlainWriter<'a> {
    fn write_escaped(&mut self, text: &str) {
        self.dest.push_str(text);
    }
    fn stack_preorder(&mut self, _stack: &[FormatCmd]) {}

    fn stack_postorder(&mut self, _stack: &[FormatCmd]) {}

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
                self.dest.push_str(localized.opening(*is_inner));
                self.write_micros(children);
                self.dest.push_str(localized.closing(*is_inner));
            }
            Formatted(nodes, _cmd) => {
                self.write_micros(nodes);
            }
            NoCase(inners) => {
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
            Anchor { content, .. } => {
                self.write_inlines(content);
            }
        }
    }
}
