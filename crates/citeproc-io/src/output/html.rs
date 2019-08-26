// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::{LocalizedQuotes, OutputFormat};
use crate::utils::JoinMany;
use crate::IngestOptions;
use csl::style::{
    FontStyle, FontVariant, FontWeight, Formatting, TextDecoration, VerticalAlignment,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Html {
    Html(HtmlOptions),
    Rtf(RtfOptions),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum QuoteType {
    SingleQuote,
    DoubleQuote,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq)]
pub struct Attr(pub String, pub Vec<String>, pub Vec<(String, String)>);

/// TODO: serialize and deserialize using an HTML parser?
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum InlineElement {
    /// This is how we can flip-flop only user-supplied styling.
    /// Inside this is parsed micro html
    Micro(Vec<MicroNode>),

    Emph(Vec<InlineElement>),
    Strong(Vec<InlineElement>),
    SmallCaps(Vec<InlineElement>),
    Underline(Vec<InlineElement>),
    Superscript(Vec<InlineElement>),
    Subscript(Vec<InlineElement>),
    Span(Attr, Vec<InlineElement>),
    Quoted(QuoteType, Vec<InlineElement>),
    Text(String),
    Anchor {
        title: String,
        url: String,
        content: Vec<InlineElement>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RtfOptions {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HtmlOptions {
    // TODO: is it enough to have one set of localized quotes for the entire style?
    // quotes: LocalizedQuotes,
    use_b_for_strong: bool,
}

impl Default for HtmlOptions {
    fn default() -> Self {
        HtmlOptions {
            use_b_for_strong: false,
        }
    }
}

impl HtmlOptions {
    pub fn test_suite() -> Self {
        HtmlOptions {
            use_b_for_strong: true,
        }
    }
}

#[test]
fn test_html() {
    let tester = vec![Emph(vec![Strong(vec![Text("hello".into())])])];
    let html = InlineElement::to_html(&tester, &HtmlOptions::default());
    assert_eq!(html, "<i><strong>hello</strong></i>");
}

impl InlineElement {
    fn to_html(inlines: &[InlineElement], options: &HtmlOptions) -> String {
        let mut s = String::new();
        for i in inlines {
            i.to_html_inner(&mut s, options);
        }
        s
    }
    fn to_html_inner(&self, s: &mut String, options: &HtmlOptions) {
        match self {
            Micro(micro) => {
                s.push_str("TODO: micro_html output");
            }
            Emph(inners) => {
                s.push_str("<i>");
                for i in inners {
                    i.to_html_inner(s, options);
                }
                s.push_str("</i>");
            }
            Span(attrs, inners) => {
                s.push_str("<span");
                if attrs.0.len() > 0 {
                    s.push_str(r#" id=""#);
                    s.push_str(&attrs.0);
                    s.push_str(r#"""#);
                }
                if attrs.1.len() > 0 {
                    s.push_str(r#" class=""#);
                    for class in attrs.1.iter() {
                        s.push_str(&class);
                        s.push_str(" ");
                    }
                    s.push_str(r#"""#);
                }
                if attrs.2.len() > 0 {
                    for (key, value) in attrs.2.iter() {
                        s.push_str(" ");
                        s.push_str(&key);
                        s.push_str("=\"");
                        s.push_str(&value);
                        s.push_str("\"");
                    }
                }
                s.push_str(">");
                for i in inners {
                    i.to_html_inner(s, options);
                }
                s.push_str("</span>");
            }
            Strong(inners) => {
                if options.use_b_for_strong {
                    s.push_str("<b>");
                } else {
                    s.push_str("<strong>");
                }
                for i in inners {
                    i.to_html_inner(s, options);
                }
                if options.use_b_for_strong {
                    s.push_str("</b>");
                } else {
                    s.push_str("</strong>");
                }
            }
            Superscript(inners) => {
                s.push_str(r#"<sup>"#);
                for i in inners {
                    i.to_html_inner(s, options);
                }
                s.push_str("</sup>");
            }
            Subscript(inners) => {
                s.push_str(r#"<sub>"#);
                for i in inners {
                    i.to_html_inner(s, options);
                }
                s.push_str("</sub>");
            }
            Underline(inners) => {
                s.push_str(r#"<span style="text-decoration: underline;">"#);
                for i in inners {
                    i.to_html_inner(s, options);
                }
                s.push_str("</span>");
            }
            SmallCaps(inners) => {
                s.push_str(r#"<span style="font-variant:small-caps;">"#);
                for i in inners {
                    i.to_html_inner(s, options);
                }
                s.push_str("</span>");
            }
            Quoted(_qt, inners) => {
                s.push_str(r#"<q>"#);
                for i in inners {
                    i.to_html_inner(s, options);
                }
                s.push_str("</q>");
            }
            Anchor {
                title: _,
                url,
                content,
            } => {
                s.push_str(r#"<a href=""#);
                // TODO: HTML-quoted-escape? the url?
                s.push_str(&url);
                s.push_str(r#"">"#);
                for i in content {
                    i.to_html_inner(s, options);
                }
                s.push_str("</a>");
            }
            Text(text) => {
                // TODO: HTML-escape the text
                s.push_str(&text);
            }
        }
    }
}

impl InlineElement {
    fn to_rtf(inlines: &[InlineElement], options: &RtfOptions) -> String {
        let mut s = String::new();
        for i in inlines {
            i.to_rtf_inner(&mut s, options);
        }
        s
    }
    fn to_rtf_inner(&self, s: &mut String, options: &RtfOptions) {
        match self {
            Micro(micro) => {
                s.push_str("TODO: micro_html output");
            }
            Emph(inners) => {
                s.push_str(r"{\\i{}");
                for i in inners {
                    i.to_rtf_inner(s, options);
                }
                s.push_str(r"}");
            }
            Span(attrs, inners) => {
                s.push_str("<span");
                if attrs.0.len() > 0 {
                    s.push_str(r#" id=""#);
                    s.push_str(&attrs.0);
                    s.push_str(r#"""#);
                }
                if attrs.1.len() > 0 {
                    s.push_str(r#" class=""#);
                    for class in attrs.1.iter() {
                        s.push_str(&class);
                        s.push_str(" ");
                    }
                    s.push_str(r#"""#);
                }
                if attrs.2.len() > 0 {
                    for (key, value) in attrs.2.iter() {
                        s.push_str(" ");
                        s.push_str(&key);
                        s.push_str("=\"");
                        s.push_str(&value);
                        s.push_str("\"");
                    }
                }
                s.push_str(">");
                for i in inners {
                    i.to_rtf_inner(s, options);
                }
                s.push_str("</span>");
            }
            Strong(inners) => {
                s.push_str("<strong>");
                for i in inners {
                    i.to_rtf_inner(s, options);
                }
                s.push_str("</strong>");
            }
            Superscript(inners) => {
                s.push_str(r#"<sup>"#);
                for i in inners {
                    i.to_rtf_inner(s, options);
                }
                s.push_str("</sup>");
            }
            Subscript(inners) => {
                s.push_str(r#"<sub>"#);
                for i in inners {
                    i.to_rtf_inner(s, options);
                }
                s.push_str("</sub>");
            }
            Underline(inners) => {
                s.push_str(r#"<span style="text-decoration: underline;">"#);
                for i in inners {
                    i.to_rtf_inner(s, options);
                }
                s.push_str("</span>");
            }
            SmallCaps(inners) => {
                s.push_str(r#""#);
                for i in inners {
                    i.to_rtf_inner(s, options);
                }
                s.push_str("</span>");
            }
            Quoted(_qt, inners) => {
                s.push_str(r#"<q>"#);
                for i in inners {
                    i.to_rtf_inner(s, options);
                }
                s.push_str("</q>");
            }
            Anchor {
                title: _,
                url,
                content,
            } => {
                s.push_str(r#"<a href=""#);
                // TODO: HTML-quoted-escape? the url?
                s.push_str(&url);
                s.push_str(r#"">"#);
                for i in content {
                    i.to_rtf_inner(s, options);
                }
                s.push_str("</a>");
            }
            Text(text) => {
                // TODO: HTML-escape the text
                s.push_str(&text);
            }
        }
    }
}

use self::InlineElement::*;

impl Default for Html {
    fn default() -> Self {
        Html::Html(HtmlOptions::default())
    }
}

impl Html {
    /// Wrap some nodes with formatting
    ///
    /// In pandoc, Emph, Strong and SmallCaps, Superscript and Subscript are all single-use styling
    /// elements. So formatting with two of those styles at once requires wrapping twice, in any
    /// order.

    fn fmt_vec(
        &self,
        inlines: Vec<InlineElement>,
        formatting: Option<Formatting>,
    ) -> Vec<InlineElement> {
        if let Some(f) = formatting {
            let mut current = inlines;

            current = match f.font_style {
                Some(FontStyle::Italic) | Some(FontStyle::Oblique) => vec![Emph(current)],
                _ => current,
            };
            current = match f.font_weight {
                Some(FontWeight::Bold) => vec![Strong(current)],
                // Light => unimplemented!(),
                _ => current,
            };
            current = match f.font_variant {
                Some(FontVariant::SmallCaps) => vec![SmallCaps(current)],
                _ => current,
            };
            current = match f.text_decoration {
                Some(TextDecoration::Underline) => vec![Underline(current)],
                _ => current,
            };
            current = match f.vertical_alignment {
                Some(VerticalAlignment::Superscript) => vec![Superscript(current)],
                Some(VerticalAlignment::Subscript) => vec![Subscript(current)],
                _ => current,
            };

            current
        } else {
            inlines
        }
    }
}

use super::micro_html::{MicroHtml, MicroNode};

impl OutputFormat for Html {
    type Input = String;
    type Build = Vec<InlineElement>;
    type Output = String;

    fn ingest(&self, input: &str, options: IngestOptions) -> Self::Build {
        vec![InlineElement::Micro(MicroNode::parse(input, options))]
    }

    #[inline]
    fn plain(&self, s: &str) -> Self::Build {
        self.text_node(s.to_owned(), None)
    }

    fn text_node(&self, text: String, f: Option<Formatting>) -> Vec<InlineElement> {
        let v = vec![Text(text)];
        self.fmt_vec(v, f)
    }

    #[inline]
    fn seq(&self, nodes: impl Iterator<Item = Self::Build>) -> Self::Build {
        itertools::concat(nodes)
    }

    fn join_delim(&self, a: Self::Build, delim: &str, b: Self::Build) -> Self::Build {
        [a, b].join_many(&self.plain(delim))
    }

    fn group(
        &self,
        nodes: Vec<Self::Build>,
        delimiter: &str,
        formatting: Option<Formatting>,
    ) -> Self::Build {
        if nodes.len() == 1 {
            self.fmt_vec(nodes.into_iter().nth(0).unwrap(), formatting)
        } else {
            let delim = self.plain(delimiter);
            self.fmt_vec(nodes.join_many(&delim), formatting)
        }
    }

    #[inline]
    fn with_format(&self, a: Self::Build, f: Option<Formatting>) -> Self::Build {
        self.fmt_vec(a, f)
    }

    fn quoted(&self, b: Self::Build, quotes: &LocalizedQuotes) -> Self::Build {
        let qt = match quotes {
            LocalizedQuotes::Single(..) => QuoteType::SingleQuote,
            LocalizedQuotes::Double(..) => QuoteType::SingleQuote,
            // Would this be better? Only allow
            // LocalizedQuotes::Double(open, close) |
            // LocalizedQuotes::Single(open, close) => {
            //     return self.affixed(b, Affixes { prefix: open.clone(), suffix: close.clone() });
            // }
        };
        vec![InlineElement::Quoted(qt, b)]
    }

    fn hyperlinked(&self, a: Self::Build, target: Option<&str>) -> Self::Build {
        // TODO: allow internal linking using the Attr parameter (e.g.
        // first-reference-note-number)
        if let Some(target) = target {
            vec![InlineElement::Anchor {
                title: "".into(),
                url: target.into(),
                content: a,
            }]
        } else {
            a
        }
    }

    fn output(&self, inter: Vec<InlineElement>) -> Self::Output {
        let null = FlipFlopState::default();
        let flipped = flip_flop_inlines(&inter, &null);
        let string = match self {
            Html::Html(ref options) => InlineElement::to_html(&flipped, options),
            Html::Rtf(ref options) => InlineElement::to_rtf(&flipped, options),
        };
        string
    }
}

#[derive(Default, Debug, Clone)]
struct FlipFlopState {
    in_emph: bool,
    in_strong: bool,
    in_small_caps: bool,
    in_outer_quotes: bool,
}

// fn attr_class(class: &str) -> Attr {
//     Attr("".to_owned(), vec![class.to_owned()], vec![])
// }

fn attr_style(style: &str) -> Attr {
    Attr(
        "".to_owned(),
        vec![],
        vec![("style".into(), style.to_owned())],
    )
}

fn flip_flop_inlines(inlines: &[InlineElement], state: &FlipFlopState) -> Vec<InlineElement> {
    inlines
        .iter()
        .map(|inl| flip_flop(inl, state).unwrap_or_else(|| inl.clone()))
        .collect()
}

fn flip_flop(inline: &InlineElement, state: &FlipFlopState) -> Option<InlineElement> {
    let fl = |ils: &[InlineElement], st| flip_flop_inlines(ils, st);
    match inline {
        Emph(ref ils) => {
            let mut flop = state.clone();
            flop.in_emph = !flop.in_emph;
            let subs = fl(ils, &flop);
            if state.in_emph {
                Some(Span(attr_style("font-style: initial;"), subs))
            } else {
                Some(Emph(subs))
            }
        }

        Strong(ref ils) => {
            let mut flop = state.clone();
            flop.in_strong = !flop.in_strong;
            let subs = fl(ils, &flop);
            if state.in_strong {
                Some(Span(attr_style("font-weight: initial;"), subs))
            } else {
                Some(Strong(subs))
            }
        }

        SmallCaps(ref ils) => {
            let mut flop = state.clone();
            flop.in_small_caps = !flop.in_small_caps;
            let subs = fl(ils, &flop);
            if state.in_small_caps {
                Some(Span(attr_style("font-variant: normal;"), subs))
            } else {
                Some(SmallCaps(subs))
            }
        }

        // don't flip-flop underlines
        Underline(ref ils) => {
            let subs = fl(ils, state);
            Some(Underline(subs))
        }

        Quoted(ref _q, ref ils) => {
            let mut flop = state.clone();
            flop.in_outer_quotes = !flop.in_outer_quotes;
            let subs = fl(ils, &flop);
            if !state.in_outer_quotes {
                Some(Quoted(QuoteType::SingleQuote, subs))
            } else {
                Some(Quoted(QuoteType::DoubleQuote, subs))
            }
        }

        Superscript(ref ils) => {
            let subs = fl(ils, state);
            Some(Superscript(subs))
        }

        Subscript(ref ils) => {
            let subs = fl(ils, state);
            Some(Subscript(subs))
        }

        Anchor {
            title,
            url,
            content,
        } => {
            let subs = fl(content, state);
            Some(Anchor {
                title: title.clone(),
                url: url.clone(),
                content: subs,
            })
        }

        _ => None,
    }

    // a => a
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_flip_emph() {
        let f = Html::default();
        let a = f.plain("normal");
        let b = f.text_node("emph".into(), Some(Formatting::italic()));
        let c = f.plain("normal");
        let group = f.group(vec![a, b, c], " ", Some(Formatting::italic()));
        let out = f.output(group.clone());

        let group_str = InlineElement::to_html(&group, &HtmlOptions::default());
        assert_ne!(group_str, out);
        assert_eq!(
            out,
            "<i>normal <span style=\"font-style: initial;\">emph</span> normal</i>"
        );
    }
}
