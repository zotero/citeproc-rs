// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::InlineElement;
use super::MarkupWriter;
use crate::output::micro_html::MicroNode;
use crate::output::FormatCmd;

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct PlainWriter {}

impl MarkupWriter for PlainWriter {
    fn stack_preorder(&self, _s: &mut String, _stack: &[FormatCmd]) {}

    fn stack_postorder(&self, _s: &mut String, _stack: &[FormatCmd]) {}

    fn write_inline(&self, s: &mut String, inline: &InlineElement) {
        inline.to_plain_inner(s, self);
    }
}

impl MicroNode {
    fn to_plain_inner(&self, s: &mut String, options: &PlainWriter) {
        use MicroNode::*;
        match self {
            Text(text) => {
                s.push_str(&text);
            }
            Formatted(nodes, _cmd) => {
                for node in nodes {
                    node.to_plain_inner(s, options);
                }
            }
            NoCase(inners) => {
                for i in inners {
                    i.to_plain_inner(s, options);
                }
            }
        }
    }
}

impl InlineElement {
    fn to_plain_inner(&self, s: &mut String, options: &PlainWriter) {
        use super::InlineElement::*;
        match self {
            Text(text) => {
                use v_htmlescape::escape;
                s.push_str(&escape(text).to_string());
            }
            Div(_display, inlines) => {
                for i in inlines {
                    i.to_plain_inner(s, options);
                }
            }
            Micro(micros) => {
                for micro in micros {
                    micro.to_plain_inner(s, options);
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
                    i.to_plain_inner(s, options);
                }
                s.push('”');
            }
            Anchor {
                title: _,
                url: _,
                content,
            } => {
                for i in content {
                    i.to_plain_inner(s, options);
                }
            }
        }
    }
}
