use super::FormatCmd;
use crate::output::LocalizedQuotes;
use crate::IngestOptions;
use crate::String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum MicroNode {
    Text(String),

    Formatted(Vec<MicroNode>, FormatCmd),

    Quoted {
        /// Holds false until flip_flop_nodes gives it a meaningful value
        is_inner: bool,
        localized: LocalizedQuotes,
        children: Vec<MicroNode>,
    },

    NoCase(Vec<MicroNode>),

    NoDecor(Vec<MicroNode>),
}

impl MicroNode {
    pub(crate) fn take_text(&mut self) -> Option<String> {
        match self {
            MicroNode::Text(ref mut text) => Some(std::mem::replace(text, String::new())),
            _ => None,
        }
    }
}

impl MicroNode {
    pub fn parse(fragment: &str, options: &IngestOptions) -> Vec<MicroNode> {
        if options.is_attribute {
            let plain = options.plain(fragment);
            super::superscript::parse_sup_sub(&plain)
        } else {
            let mut tag_parser = TagParser::new(&fragment);
            let result: Vec<MicroNode> = tag_parser.walk(&MicroHtmlReader { options });
            if options.no_parse_quotes {
                result
            } else {
                super::parse_quotes::parse_quotes(result, options)
            }
        }
    }
}

pub trait HtmlReader<T> {
    fn constructor(&self, tag: &Tag, children: Vec<T>) -> Vec<T>;
    fn plain(&self, s: &str) -> Option<Vec<T>>;
    fn filter(&self, tag: &mut Tag) {
        if tag.name == "html" || tag.name == "body" {
            // ignore <html> and <body> tags, but still parse their children
            tag.ignore_self();
        } else if tag.name == "i" || tag.name == "b" || tag.name == "sup" || tag.name == "sub" {
            // ok
        } else if tag.name == "span" {
            tag.allow_attribute("style");
            tag.allow_attribute("class");
        } else {
            tag.ignore_self();
        }
    }
}

pub fn micro_html_to_string(fragment: &str, options: &IngestOptions) -> String {
    let mut parser = TagParser::new(&fragment);
    let reader = PlainHtmlReader {
        options: options.clone(),
    };
    let result: Vec<String> = parser.walk(&reader);
    let mut res: Option<String> = None;
    for r in result {
        res = match res {
            Some(ref mut acc) => {
                acc.push_str(&r);
                continue;
            }
            None => Some(r),
        }
    }
    res.unwrap_or_default()
}

struct PlainHtmlReader {
    options: IngestOptions,
}

impl HtmlReader<String> for PlainHtmlReader {
    fn constructor(&self, tag: &Tag, children: Vec<String>) -> Vec<String> {
        match tag.name {
            "i" => children,
            "b" => children,
            "sup" => children,
            "sub" => children,
            "span" => match tag.attrs {
                // very specific!
                [("style", "font-variant:small-caps;")]
                | [("style", "font-variant: small-caps;")]
                | [("class", "nocase")]
                | [("class", "nodecor")] => children,
                _ => return vec![],
            },
            _ => return vec![],
        }
    }

    fn plain(&self, s: &str) -> Option<Vec<String>> {
        let plain = self.options.plain(s);
        Some(vec![plain.as_ref().into()])
    }
}

struct MicroHtmlReader<'a> {
    options: &'a IngestOptions,
}

impl HtmlReader<MicroNode> for MicroHtmlReader<'_> {
    fn constructor(&self, tag: &Tag, children: Vec<MicroNode>) -> Vec<MicroNode> {
        let single = match tag.name {
            "i" => MicroNode::Formatted(children, FormatCmd::FontStyleItalic),
            "b" => MicroNode::Formatted(children, FormatCmd::FontWeightBold),
            "sup" => MicroNode::Formatted(children, FormatCmd::VerticalAlignmentSuperscript),
            "sub" => MicroNode::Formatted(children, FormatCmd::VerticalAlignmentSubscript),
            "span" => match tag.attrs {
                // very specific!
                [("style", "font-variant:small-caps;")]
                | [("style", "font-variant: small-caps;")] => {
                    MicroNode::Formatted(children, FormatCmd::FontVariantSmallCaps)
                }
                [("class", "nocase")] => MicroNode::NoCase(children),
                [("class", "nodecor")] => MicroNode::NoDecor(children),
                // TODO: do we really want <span class="unrecognised">Children</span> to be removed
                // completely?
                _ => return vec![],
            },
            // TODO: Same here
            _ => return vec![],
        };
        vec![single]
    }

    fn plain<'input>(&self, s: &'input str) -> Option<Vec<MicroNode>> {
        let plain = self.options.plain(s);
        Some(super::superscript::parse_sup_sub(&plain))
    }
}

#[test]
fn test_sanitize() {
    let fragment =
        r#"<span class="nocase"><i class="whatever">Italic</i></span> <img src="5" /> <b>Bold</b>"#;
    let result = MicroNode::parse(fragment, &Default::default());
    use FormatCmd::*;
    use MicroNode::*;
    assert_eq!(
        result,
        &[
            NoCase(vec![Formatted(
                vec![Text("Italic".into())],
                FontStyleItalic
            ),]),
            Text(" ".into()),
            Text(" ".into()),
            Formatted(vec![Text("Bold".into())], FontWeightBold)
        ]
    );
}

// The following is based on the MIT-licensed html_sanitizer crate,
// and adjusted to work on *inline* HTML, not entire documents.
//
// https://github.com/Trangar/html_sanitizer/blob/master/src/lib.rs

use html5ever::driver::ParseOpts;
use html5ever::interface::QualName;
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::TreeBuilderOpts;
use html5ever::{local_name, parse_fragment, Namespace};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

struct TagParser {
    dom: RcDom,
}

use stringreader::StringReader;

impl<'input> TagParser {
    fn new(input: &'input str) -> Self {
        let opts = ParseOpts {
            tree_builder: TreeBuilderOpts {
                drop_doctype: true,
                scripting_enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let html_p = QualName::new(
            None,
            Namespace::from("http://www.w3.org/1999/xhtml"),
            local_name!("p"),
        );
        let mut reader = StringReader::new(input);
        let dom = parse_fragment(RcDom::default(), opts, html_p, vec![])
            .from_utf8()
            .read_from(&mut reader)
            .unwrap();
        // println!("Errors: {:?}", dom.errors);

        TagParser { dom }
    }

    fn internal_walk_micro<T, R>(handle: &Handle, callbacks: &R) -> Vec<T>
    where
        R: HtmlReader<T>,
    {
        let mut output = Vec::new();

        if let NodeData::Element { name, attrs, .. } = &handle.data {
            let name = &name.local;
            let attrs = attrs.borrow();
            let mut attributes = Vec::<(&str, &str)>::new();
            for attr in attrs.iter() {
                attributes.push((&attr.name.local, &attr.value));
            }
            let mut tag = Tag::from_name_and_attrs(name, &attributes);
            callbacks.filter(&mut tag);

            if tag.ignore_self && tag.ignore_contents {
                return output;
            }
            // if let Some(rewrite) = tag.rewrite {
            //     return rewrite;
            // }

            let attrs: Vec<(&str, &str)> = tag
                .attrs
                .iter()
                .filter(|a| tag.allowed_attributes.iter().any(|b| b == &a.0))
                .cloned()
                .collect();

            if !tag.ignore_self && !tag.ignore_contents {
                let proposed = Tag::from_name_and_attrs(tag.name, &attrs);
                let mut children = Vec::new();
                for child in handle.children.borrow().iter() {
                    children.extend(TagParser::internal_walk_micro(child, callbacks));
                }
                output.extend(callbacks.constructor(&proposed, children));
            } else if tag.ignore_self {
                for child in handle.children.borrow().iter() {
                    output.extend(TagParser::internal_walk_micro(child, callbacks));
                }
            } else if tag.ignore_contents {
                let proposed = Tag::from_name_and_attrs(tag.name, &attrs);
                output.extend(callbacks.constructor(&proposed, vec![]));
            }
        } else {
            match &handle.data {
                NodeData::Document => {}
                NodeData::Doctype { .. } => {}
                NodeData::Text { contents } => {
                    let cont = &contents.borrow();
                    if let Some(s) = callbacks.plain(cont) {
                        output.extend(s.into_iter())
                    }
                }
                NodeData::Comment { .. } => {}
                NodeData::Element { .. } => unreachable!(),
                NodeData::ProcessingInstruction { .. } => debug!(
                    // "Unknown enum tag: NodeData::ProcessingInstruction {{ {:?} {:?} }}",
                    // target, contents
                    "Unknown enum tag: NodeData::ProcessingInstruction",
                ),
            }
            for child in handle.children.borrow().iter() {
                output.extend(TagParser::internal_walk_micro(child, callbacks));
            }
        }
        output
    }

    /// Recursively walk through all the HTML nodes, calling `callback` for each tag.
    fn walk<T, R>(&mut self, callbacks: &R) -> Vec<T>
    where
        R: HtmlReader<T>,
    {
        TagParser::internal_walk_micro(&self.dom.document, callbacks)
    }
}

/// Represents a single HTML node. You can read the `name` and `attrs` properties to figure out what tag you're sanitizing.
///
/// By default all html nodes will be printed, but attributes will be stripped from a tag unless they are added with `allow_attribute` and `allow_attributes`.
pub struct Tag<'a> {
    /// The name of the HTML tag, e.g. 'div', 'img', etc.
    pub name: &'a str,

    /// The attributes of the HTML tag, e.g. ('style', 'width: 100%').
    pub attrs: &'a [(&'a str, &'a str)],

    allowed_attributes: Vec<&'static str>,
    ignore_self: bool,
    ignore_contents: bool,
}

impl<'a> Tag<'a> {
    fn from_name_and_attrs(name: &'a str, attrs: &'a [(&'a str, &'a str)]) -> Tag<'a> {
        Tag {
            name,
            attrs,
            // rewrite: None,
            allowed_attributes: Vec::new(),
            ignore_self: false,
            ignore_contents: false,
        }
    }

    /// Allow the given attribute. This attribute does not have to exist in the `attrs` tag.
    ///
    /// When this HTML node gets printed, this attribute will also get printed.
    pub fn allow_attribute(&mut self, attr: &'static str) {
        self.allowed_attributes.push(attr);
    }

    /// Allow the given attributes. These attributes do not have to exist in the `attrs` tag.
    ///
    /// When this HTML node gets printed, these attributes will also get printed.
    pub fn allow_attributes(&mut self, attrs: &[&'static str]) {
        self.allowed_attributes.reserve(attrs.len());
        for attr in attrs {
            self.allowed_attributes.push(attr);
        }
    }

    /// Ignore this tag. This means that the HTML Node will not be printed in the output. In addition, all the child nodes and text content will also not be printed.
    pub fn ignore_self_and_contents(&mut self) {
        self.ignore_self = true;
        self.ignore_contents = true;
    }

    /// Ignore this tag. This means that the HTML Node will not be printed in the output. All child nodes and text content will be printed.
    pub fn ignore_self(&mut self) {
        self.ignore_self = true;
    }
}
