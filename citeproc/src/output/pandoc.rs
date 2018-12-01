use super::OutputFormat;
use crate::style::element::{
    FontStyle, FontVariant, FontWeight, Formatting, TextDecoration, VerticalAlignment,
};
use crate::utils::{Intercalate, JoinMany};

extern crate pandoc_types;
use pandoc_types::definition::Inline::*;
use pandoc_types::definition::*;

pub struct Pandoc {}

impl Pandoc {
    pub fn new() -> Self {
        Pandoc {}
    }

    fn fmt_vec(&self, inlines: Vec<Inline>, f: &Formatting) -> Option<Inline> {
        let mut current: Result<Inline, Vec<Inline>> = Err(inlines);

        let maybe = |cur| {
            match cur {
                // first time
                Err(v) => Ok(v),
                // rest
                Ok(e) => Ok(vec![e]),
            }
        };

        current = match f.font_style {
            FontStyle::Italic | FontStyle::Oblique => maybe(current).map(Emph),
            _ => current,
        };

        current = match f.font_weight {
            FontWeight::Bold => maybe(current).map(Strong),
            // Light => unimplemented!(),
            _ => current,
        };
        current = match f.font_variant {
            FontVariant::SmallCaps => maybe(current).map(SmallCaps),
            _ => current,
        };
        current = match f.text_decoration {
            TextDecoration::Underline => maybe(current).map(|v| Span(attr_class("underline"), v)),
            _ => current,
        };
        current = match f.vertical_alignment {
            VerticalAlignment::Superscript => maybe(current).map(Superscript),
            VerticalAlignment::Subscript => maybe(current).map(Subscript),
            _ => current,
        };

        current.ok()
    }
}

impl OutputFormat<Vec<Inline>, Vec<Inline>> for Pandoc {
    fn text_node(&self, text: &str, f: &Formatting) -> Vec<Inline> {
        let fmts: Vec<Inline> = text.split(' ').map(|s| Str(s.to_owned())).collect();

        fmts.intercalate(&Space)
            .into_iter()
            .filter_map(|t| match t {
                Space => Some(t),
                Str(ref s) if s == "" => None,
                _ => Some(self.fmt_vec(vec![t.clone()], f).unwrap_or(t)),
            })
            .collect()
    }

    fn group(&self, nodes: &[Vec<Inline>], d: &str, f: &Formatting) -> Vec<Inline> {
        let delim = self.text_node(d, &Formatting::default());
        let joined = nodes.join_many(&delim);
        self.fmt_vec(joined.clone(), f)
            .map(|single| vec![single])
            .unwrap_or(joined)
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

fn flip_flop_inlines(inlines: &Vec<Inline>, state: &FlipFlopState) -> Vec<Inline> {
    inlines
        .into_iter()
        .map(|inl| flip_flop(inl, state).unwrap_or_else(|| inl.clone()))
        .collect()
}

fn flip_flop(inline: &Inline, state: &FlipFlopState) -> Option<Inline> {
    let fl = |ils, st| flip_flop_inlines(ils, st);
    match inline {
        Note(ref blocks) => {
            if let Some(Block::Para(ref ils)) = blocks.into_iter().nth(0) {
                Some(Note(vec![Block::Para(fl(ils, state))]))
            } else {
                None
            }
        }

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
            if state.in_outer_quotes {
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
        let f = Pandoc::new();
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
        let f = Pandoc::new();
        let a = f.plain("normal");
        let b = f.text_node("emph", &Formatting::italic());
        let c = f.plain("normal");
        let group = f.group(&[a, b, c], " ", &Formatting::italic());
        let out = f.output(group.clone());
        assert_ne!(group, out);
    }

}
