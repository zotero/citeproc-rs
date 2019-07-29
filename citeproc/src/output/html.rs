// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::{LocalizedQuotes, OutputFormat};
use crate::utils::{Intercalate, JoinMany};
use csl::style::{
    FontStyle, FontVariant, FontWeight, Formatting, TextDecoration, VerticalAlignment,
};

use pandoc_types::definition::{QuoteType, Attr};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Html;

/// TODO: serialize and deserialize using an HTML parser?
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum InlineElement {
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
    }
}

#[derive(Clone)]
struct HtmlOptions {
    use_classes: bool,
    // TODO: is it enough to have one set of localized quotes for the entire style?
    // quotes: LocalizedQuotes,
}

#[test]
fn test_html() {
    let tester = vec![Emph(vec![Strong(vec![Text("hello".into())])])];
    let html = InlineElement::to_html(&tester, &HtmlOptions {
        use_classes: false,
        // quotes: LocalizedQuotes::
    });
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
                // TODO: attrs.2 (key-value pairs)?
                s.push_str(">");
                for i in inners {
                    i.to_html_inner(s, options);
                }
                s.push_str("</span>");
            }
            Strong(inners) => {
                s.push_str("<strong>");
                for i in inners {
                    i.to_html_inner(s, options);
                }
                s.push_str("</strong>");
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
                // todo: use underline class if options.use_classes
                s.push_str(r#"<span style="text-decoration: underline;">"#);
                for i in inners {
                    i.to_html_inner(s, options);
                }
                s.push_str("</span>");
            }
            SmallCaps(inners) => {
                // todo: use smallcaps class if options.use_classes
                s.push_str(r#"<span style="font-variant:small-caps;">"#);
                for i in inners {
                    i.to_html_inner(s, options);
                }
                s.push_str("</span>");
            }
            Quoted(_qt, inners) => {
                // todo: use smallcaps class if options.use_classes
                s.push_str(r#"<q>"#);
                for i in inners {
                    i.to_html_inner(s, options);
                }
                s.push_str("</q>");
            }
            Anchor { title: _, url, content } => {
                // todo: use smallcaps class if options.use_classes
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

use self::InlineElement::*;

impl Default for Html {
    fn default() -> Self {
        Html
    }
}

impl Html {
    /// Wrap some nodes with formatting
    ///
    /// In pandoc, Emph, Strong and SmallCaps, Superscript and Subscript are all single-use styling
    /// elements. So formatting with two of those styles at once requires wrapping twice, in any
    /// order.

    fn fmt_vec(&self, inlines: Vec<InlineElement>, formatting: Option<Formatting>) -> Vec<InlineElement> {
        if let Some(f) = formatting {
            let mut current = inlines;

            current = match f.font_style {
                FontStyle::Italic | FontStyle::Oblique => vec![Emph(current)],
                _ => current,
            };
            current = match f.font_weight {
                FontWeight::Bold => vec![Strong(current)],
                // Light => unimplemented!(),
                _ => current,
            };
            current = match f.font_variant {
                FontVariant::SmallCaps => vec![SmallCaps(current)],
                _ => current,
            };
            current = match f.text_decoration {
                TextDecoration::Underline => vec![Underline(current)],
                _ => current,
            };
            current = match f.vertical_alignment {
                VerticalAlignment::Superscript => vec![Superscript(current)],
                VerticalAlignment::Subscript => vec![Subscript(current)],
                _ => current,
            };

            current
        } else {
            inlines
        }
    }
}

impl OutputFormat for Html {
    type Build = Vec<InlineElement>;
    type Output = String;

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
        let html = InlineElement::to_html(&flipped, &HtmlOptions {
            use_classes: true
        });
        html
    }
}

#[derive(Default, Debug, Clone)]
struct FlipFlopState {
    in_emph: bool,
    in_strong: bool,
    in_small_caps: bool,
    in_outer_quotes: bool,
}

fn attr_class(class: &str) -> Attr {
    Attr("".to_owned(), vec![class.to_owned()], vec![])
}

fn flip_flop_inlines(inlines: &[InlineElement], state: &FlipFlopState) -> Vec<InlineElement> {
    inlines
        .iter()
        .map(|inl| flip_flop(inl, state).unwrap_or_else(|| inl.clone()))
        .collect()
}

fn flip_flop(inline: &InlineElement, state: &FlipFlopState) -> Option<InlineElement> {
    use pandoc_types::definition::*;
    let fl = |ils: &[InlineElement], st| flip_flop_inlines(ils, st);
    match inline {
        Emph(ref ils) => {
            let mut flop = state.clone();
            flop.in_emph = !flop.in_emph;
            let subs = fl(ils, &flop);
            if state.in_emph {
                // unimplemented!("spans with csl-no-emph classes")
                Some(Span(attr_class("csl-no-emph"), subs))
            } else {
                Some(Emph(subs))
            }
        }

        Strong(ref ils) => {
            let mut flop = state.clone();
            flop.in_strong = !flop.in_strong;
            let subs = fl(ils, &flop);
            if state.in_strong {
                Some(Span(attr_class("csl-no-strong"), subs))
            } else {
                Some(Strong(subs))
            }
        }

        SmallCaps(ref ils) => {
            let mut flop = state.clone();
            flop.in_small_caps = !flop.in_small_caps;
            let subs = fl(ils, &flop);
            if state.in_small_caps {
                Some(Span(attr_class("csl-no-smallcaps"), subs))
            } else {
                Some(SmallCaps(subs))
            }
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

        Anchor {title, url, content } => {
            let subs = fl(content, state);
            Some(Anchor { title: title.clone(), url: url.clone(), content: subs })
        }

        Underline(ref ils) => {
            let subs = fl(ils, state);
            Some(Underline(subs))
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
        assert_ne!(group, out);

        let html = InlineElement::to_html(&out, &HtmlOptions {
            use_classes: false,
            // quotes: LocalizedQuotes::
        });
        assert_eq!(html, "<i>normal <span class=\"csl-no-emph \">emph</span> normal</i>");
    }

}
