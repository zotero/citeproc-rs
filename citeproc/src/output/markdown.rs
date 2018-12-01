use super::OutputFormat;
use crate::utils::Intercalate;

use crate::style::element::{FontStyle, FontWeight, Formatting};

// use typed_arena::Arena;
use serde::{Serialize, Serializer};

#[derive(Clone, PartialEq)]
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

#[derive(Clone)]
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

impl OutputFormat<MarkdownNode, MarkdownNode> for Markdown {
    fn text_node(&self, s: &str, formatting: &Formatting) -> MarkdownNode {
        MarkdownNode::Text(s.to_owned(), formatting.into())
    }

    fn group(
        &self,
        nodes: &[MarkdownNode],
        delimiter: &str,
        formatting: &Formatting,
    ) -> MarkdownNode {
        let delim = self.text_node(delimiter, &Formatting::default());
        MarkdownNode::Group(nodes.intercalate(&delim), formatting.into())
    }

    fn output(&self, intermediate: MarkdownNode) -> MarkdownNode {
        intermediate
    }
}
