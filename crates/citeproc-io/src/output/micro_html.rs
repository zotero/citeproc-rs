#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct MicroHtml(pub String);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum MicroNode {
    Text(String),

    Italic(Vec<MicroNode>),
    Bold(Vec<MicroNode>),
    Superscript(Vec<MicroNode>),
    Subscript(Vec<MicroNode>),
    SmallCaps(Vec<MicroNode>),

    /// TODO: text-casing during ingestion
    NoCase(Vec<MicroNode>),

    /// When you flip-flop formatting away, this is what it becomes
    DissolvedFormat(Vec<MicroNode>),
}

use html5ever::driver::ParseOpts;
use html5ever::interface::QualName;
use html5ever::rcdom::{Handle, NodeData, RcDom};
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::TreeBuilderOpts;
use html5ever::{local_name, parse_fragment, Namespace};

// Based on the MIT-licensed html_sanitizer crate:
//
// https://github.com/Trangar/html_sanitizer/blob/master/src/lib.rs

struct TagParser {
    dom: RcDom,
}

use stringreader::StringReader;

impl TagParser {
    fn new(input: &str) -> Self {
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
                .filter(|a| tag.allowed_attributes.iter().any(|b| b == a.0))
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
                        output.push(s)
                    }
                }
                NodeData::Comment { .. } => {}
                NodeData::Element { .. } => unreachable!(),
                NodeData::ProcessingInstruction { target, contents } => println!(
                    "Unknown enum tag: NodeData::ProcessingInstruction {{ {:?} {:?} }}",
                    target, contents
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

    allowed_attributes: Vec<String>,
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
    pub fn allow_attribute(&mut self, attr: String) {
        self.allowed_attributes.push(attr);
    }

    /// Allow the given attributes. These attributes do not have to exist in the `attrs` tag.
    ///
    /// When this HTML node gets printed, these attributes will also get printed.
    pub fn allow_attributes(&mut self, attr: &[String]) {
        self.allowed_attributes.reserve(attr.len());
        for attr in attr {
            self.allowed_attributes.push(attr.clone());
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

pub trait HtmlReader<T> {
    fn constructor(&self, tag: &Tag, children: Vec<T>) -> Vec<T>;
    fn plain(&self, s: &str) -> Option<T>;
    fn filter(&self, tag: &mut Tag) {
        if tag.name == "html" || tag.name == "body" {
            // ignore <html> and <body> tags, but still parse their children
            tag.ignore_self();
        } else if tag.name == "i" || tag.name == "b" || tag.name == "sup" || tag.name == "sub" {
            // ok
        } else if tag.name == "span" {
            tag.allow_attribute(String::from("style"));
            tag.allow_attribute(String::from("class"));
        } else {
            tag.ignore_self();
        }
    }
}

struct MicroHtmlReader {
    options: IngestOptions,
}

impl HtmlReader<MicroNode> for MicroHtmlReader {
    fn constructor(&self, tag: &Tag, children: Vec<MicroNode>) -> Vec<MicroNode> {
        let single = match tag.name {
            "b" => MicroNode::Bold(children),
            "i" => MicroNode::Italic(children),
            "sup" => MicroNode::Superscript(children),
            "sub" => MicroNode::Subscript(children),
            "span" => match tag.attrs {
                // very specific!
                [("style", "font-variant: small-caps;")] => MicroNode::SmallCaps(children),
                [("class", "nocase")] => MicroNode::NoCase(children),
                _ => return vec![],
            },
            x => return vec![],
        };
        vec![single]
    }

    fn plain(&self, s: &str) -> Option<MicroNode> {
        let x = if self.options.replace_hyphens {
            s.replace('-', "\u{2013}")
        } else {
            s.to_string()
        };
        Some(MicroNode::Text(x))
    }
}

use crate::IngestOptions;

impl MicroNode {
    /// TODO: catch errors and get the input back as a String
    pub fn parse(fragment: &str, options: IngestOptions) -> Vec<MicroNode> {
        let mut tag_parser = TagParser::new(&fragment);
        let result: Vec<MicroNode> = tag_parser.walk(&MicroHtmlReader { options });
        result
    }
}

#[test]
fn test_sanitize() {
    let fragment =
        r#"<span class="nocase"><i class="whatever">Italic</i></span> <img src="5" /> <b>Bold</b>"#;
    let result = MicroNode::parse(fragment, Default::default());
    use MicroNode::*;
    assert_eq!(
        result,
        &[
            NoCase(vec![Italic(vec![Text("Italic".to_string())]),]),
            Text(" ".to_string()),
            Text(" ".to_string()),
            Bold(vec![Text("Bold".to_string())])
        ]
    );
}
