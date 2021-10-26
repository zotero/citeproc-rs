// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::{LocalizedQuotes, OutputFormat};
use crate::utils::{Intercalate, JoinMany};
use csl::{FontStyle, FontVariant, FontWeight, Formatting, TextDecoration, VerticalAlignment};

use pandoc_types::definition::Inline::*;
use pandoc_types::definition::{Attr, Inline, QuoteType, Target};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Pandoc {}

impl Default for Pandoc {
    fn default() -> Self {
        Pandoc {}
    }
}

impl Pandoc {
    /// Wrap some nodes with formatting
    ///
    /// In pandoc, Emph, Strong and SmallCaps, Superscript and Subscript are all single-use styling
    /// elements. So formatting with two of those styles at once requires wrapping twice, in any
    /// order.

    fn fmt_vec(&self, inlines: Vec<Inline>, formatting: Option<Formatting>) -> Vec<Inline> {
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
                Some(TextDecoration::Underline) => vec![Span(attr_class("underline"), current)],
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

impl OutputFormat for Pandoc {
    type Input = Vec<Inline>;
    type Build = Vec<Inline>;
    type Output = Vec<Inline>;

    #[inline]
    fn ingest(&self, input: Self::Input) -> Self::Build {
        input
    }

    #[inline]
    fn plain(&self, s: &str) -> Self::Build {
        self.text_node(s.to_owned(), None)
    }

    fn text_node(&self, text: String, f: Option<Formatting>) -> Vec<Inline> {
        let v: Vec<Inline> = text
            // TODO: write a nom tokenizer, don't use split/intercalate
            .split(' ')
            .map(|s| Str(s.to_owned()))
            .intercalate(&Space)
            .into_iter()
            .filter(|t| match t {
                Str(ref s) if s == "" => false,
                _ => true,
            })
            .collect();

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
            LocalizedQuotes::Double(..) => QuoteType::DoubleQuote,
        };
        vec![Inline::Quoted(qt, b)]
    }

    fn output(&self, inter: Vec<Inline>) -> Vec<Inline> {
        let null = FlipFlopState::default();
        flip_flop_inlines(&inter, &null)
        // TODO: convert quotes to inner and outer quote terms
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

fn flip_flop_inlines(inlines: &[Inline], state: &FlipFlopState) -> Vec<Inline> {
    inlines
        .iter()
        .map(|inl| flip_flop(inl, state).unwrap_or_else(|| inl.clone()))
        .collect()
}

fn flip_flop(inline: &Inline, state: &FlipFlopState) -> Option<Inline> {
    use pandoc_types::definition::*;
    let fl = |ils: &[Inline], st| flip_flop_inlines(ils, st);
    match inline {
        // Note(ref blocks) => {
        //     if let Some(Block::Para(ref ils)) = blocks.into_iter().nth(0) {
        //         Some(Note(vec![Block::Para(fl(ils, state))]))
        //     } else {
        //         None
        //     }
        // }
        Emph(ref ils) => {
            let mut flop = state.clone();
            flop.in_emph = !flop.in_emph;
            if state.in_emph {
                Some(Span(attr_class("csl-no-emph"), fl(ils, &flop)))
            } else {
                Some(Emph(fl(ils, &flop)))
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

        Strikeout(ref ils) => {
            let subs = fl(ils, state);
            Some(Strikeout(subs))
        }

        Superscript(ref ils) => {
            let subs = fl(ils, state);
            Some(Superscript(subs))
        }

        Subscript(ref ils) => {
            let subs = fl(ils, state);
            Some(Subscript(subs))
        }

        Link(attr, ref ils, t) => {
            let subs = fl(ils, state);
            Some(Link(attr.clone(), subs, t.clone()))
        }

        _ => None,
    }

    // a => a
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_space() {
        let f = Pandoc::default();
        assert_eq!(f.plain(" ")[0], Space);
        assert_eq!(f.plain("  "), &[Space, Space]);
        assert_eq!(f.plain(" h "), &[Space, Str("h".into()), Space]);
        assert_eq!(
            f.plain("  hello "),
            &[Space, Space, Str("hello".into()), Space]
        );
    }

    #[test]
    fn test_flip_emph() {
        let f = Pandoc::default();
        let a = f.plain("normal");
        let b = f.text_node("emph".into(), Some(Formatting::italic()));
        let c = f.plain("normal");
        let group = f.group(vec![a, b, c], " ", Some(Formatting::italic()));
        let out = f.output(group.clone());
        assert_ne!(group, out);
    }
}
