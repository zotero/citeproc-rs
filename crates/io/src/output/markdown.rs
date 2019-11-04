// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::OutputFormat;
use crate::utils::{IntercalateExact};

use csl::{FontStyle, FontWeight, Formatting};

// use typed_arena::Arena;
use serde::{Serialize, Serializer};

#[derive(Debug, Clone, PartialEq)]
pub enum MarkdownFormatting {
    Bold,
    Italic,
    BoldItalic,
    None,
}
impl MarkdownFormatting {
    fn surround(&self, s: &str) -> String {
        let start = match *self {
            MarkdownFormatting::None => "",
            MarkdownFormatting::Bold => "**",
            MarkdownFormatting::Italic => "_",
            MarkdownFormatting::BoldItalic => "_**",
        };
        let end = match *self {
            MarkdownFormatting::None => "",
            MarkdownFormatting::Bold => "**",
            MarkdownFormatting::Italic => "_",
            MarkdownFormatting::BoldItalic => "**_",
        };
        start.to_owned() + &s + end
    }
}

#[derive(Debug, Clone)]
pub enum MarkdownNode {
    Text(String, MarkdownFormatting),
    Group(Vec<MarkdownNode>, MarkdownFormatting),
}

// struct FlipFlopState {
//     in_emph: bool,
//     in_string: bool,
//     in_small_caps: bool,
//     in_outer_quotes: bool,
// }

impl MarkdownNode {
    // Not great, but a start.
    fn to_string(&self) -> String {
        match *self {
            MarkdownNode::Text(ref t, ref f) => f.surround(t),
            MarkdownNode::Group(ref cs, ref f) => {
                let mut s = "".to_owned();
                for c in cs {
                    s += &c.to_string()
                }
                f.surround(&s)
            }
        }
    }
}

impl Serialize for MarkdownNode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub struct Markdown {
}
impl Markdown {
    pub fn new() -> Self {
        Markdown { }
    }
}
impl<'a> From<&'a Formatting> for MarkdownFormatting {
    fn from(f: &'a Formatting) -> Self {
        match (&f.font_style, &f.font_weight) {
            (FontStyle::Italic, FontWeight::Bold) => MarkdownFormatting::BoldItalic,
            (FontStyle::Italic, _) => MarkdownFormatting::Italic,
            (_, FontWeight::Bold) => MarkdownFormatting::Bold,
            _ => MarkdownFormatting::None,
        }
    }
}

impl OutputFormat<MarkdownNode, String> for Markdown {
    fn plain(&self, s: &str) -> MarkdownNode {
        MarkdownNode::Text(s.to_owned(), MarkdownFormatting::None)
    }

    fn text_node(&self, s: String, formatting: Formatting) -> MarkdownNode {
        MarkdownNode::Text(s, formatting.into())
    }

    fn group(
        &self,
        nodes: &[MarkdownNode],
        delimiter: &str,
        formatting: Formatting,
    ) -> MarkdownNode {
        let delim = self.plain(delimiter);
        MarkdownNode::Group(nodes.iter().intercalate_exact(&delim), formatting.into())
    }

    fn output(&self, intermediate: MarkdownNode) -> String {
        intermediate.to_string()
    }
}
