use super::OutputFormat;
use crate::style::element::{
    FontStyle, FontVariant, FontWeight, Formatting, TextDecoration, VerticalAlignment,
};
use crate::utils::{Intercalate, JoinMany};

use self::definition::Inline::*;
use self::definition::{Attr, Inline};

#[derive(Debug, Clone, PartialEq, Eq)]
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
                TextDecoration::Underline => vec![Span(attr_class("underline"), current)],
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

impl OutputFormat for Pandoc {
    type Build = Vec<Inline>;
    type Output = Vec<Inline>;

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
            .filter_map(|t| match t {
                Str(ref s) if s == "" => None,
                _ => Some(t),
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
    use self::definition::*;
    let fl = |ils, st| flip_flop_inlines(ils, st);
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
        let b = f.text_node("emph".into(), Some(Formatting::italic()));
        let c = f.plain("normal");
        let group = f.group(vec![a, b, c], " ", Some(Formatting::italic()));
        let out = f.output(group.clone());
        assert_ne!(group, out);
    }

}

mod definition {
    use std::collections::HashMap;

    use serde::ser::SerializeStruct;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    const PANDOC_API_VERSION: &'static [i32] = &[1, 17, 0, 5];

    #[derive(Debug, Clone, PartialEq)]
    pub struct Pandoc(pub Meta, pub Vec<Block>);

    impl Serialize for Pandoc {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut value = serializer.serialize_struct("Pandoc", 3)?;
            value.serialize_field("pandoc-api-version", PANDOC_API_VERSION)?;
            value.serialize_field("meta", &self.0)?;
            value.serialize_field("blocks", &self.1)?;
            value.end()
        }
    }

    impl<'a> Deserialize<'a> for Pandoc {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'a>,
        {
            #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
            #[serde(rename = "Pandoc")]
            struct Inner {
                meta: Meta,
                blocks: Vec<Block>,
                #[serde(rename = "pandoc-api-version")]
                version: Vec<i32>,
            }

            let value = Inner::deserialize(deserializer)?;
            // FIXME: Should check this, but need a better error.
            assert!(
                value.version[0] == PANDOC_API_VERSION[0]
                    && value.version[1] == PANDOC_API_VERSION[1]
            );
            Ok(Pandoc(value.meta, value.blocks))
        }
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    pub struct Meta(pub HashMap<String, MetaValue>);

    impl Meta {
        pub fn null() -> Meta {
            Meta(HashMap::new())
        }

        pub fn is_null(&self) -> bool {
            self.0.is_empty()
        }

        pub fn lookup(&self, key: &String) -> Option<&MetaValue> {
            self.0.get(key)
        }
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    #[serde(tag = "t", content = "c")]
    pub enum MetaValue {
        MetaMap(HashMap<String, MetaValue>),
        MetaList(Vec<MetaValue>),
        MetaBool(bool),
        MetaString(String),
        MetaInlines(Vec<Inline>),
        MetaBlocks(Vec<Block>),
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    #[serde(tag = "t", content = "c")]
    pub enum Block {
        Plain(Vec<Inline>),
        Para(Vec<Inline>),
        LineBlock(Vec<Vec<Inline>>),
        CodeBlock(Attr, String),
        RawBlock(Format, String),
        BlockQuote(Vec<Block>),
        OrderedList(ListAttributes, Vec<Vec<Block>>),
        BulletList(Vec<Vec<Block>>),
        DefinitionList(Vec<(Vec<Inline>, Vec<Vec<Block>>)>),
        Header(i32, Attr, Vec<Inline>),
        HorizontalRule,
        Table(
            Vec<Inline>,
            Vec<Alignment>,
            Vec<f64>,
            Vec<TableCell>,
            Vec<Vec<TableCell>>,
        ),
        Div(Attr, Vec<Block>),
        Null,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    #[serde(tag = "t", content = "c")]
    pub enum Inline {
        Str(String),
        Emph(Vec<Inline>),
        Strong(Vec<Inline>),
        Strikeout(Vec<Inline>),
        Superscript(Vec<Inline>),
        Subscript(Vec<Inline>),
        SmallCaps(Vec<Inline>),
        Quoted(QuoteType, Vec<Inline>),
        Cite(Vec<Citation>, Vec<Inline>),
        Code(Attr, String),
        Space,
        SoftBreak,
        LineBreak,
        Math(MathType, String),
        RawInline(Format, String),
        Link(Attr, Vec<Inline>, Target),
        Image(Attr, Vec<Inline>, Target),
        // So we can implement Eq
        // Note(Vec<Block>),
        Span(Attr, Vec<Inline>),
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    #[serde(tag = "t", content = "c")]
    pub enum Alignment {
        AlignLeft,
        AlignRight,
        AlignCenter,
        AlignDefault,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct ListAttributes(pub i32, pub ListNumberStyle, pub ListNumberDelim);

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    #[serde(tag = "t", content = "c")]
    pub enum ListNumberStyle {
        DefaultStyle,
        Example,
        Decimal,
        LowerRoman,
        UpperRoman,
        LowerAlpha,
        UpperAlpha,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    #[serde(tag = "t", content = "c")]
    pub enum ListNumberDelim {
        DefaultDelim,
        Period,
        OneParen,
        TwoParens,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct Format(pub String);

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct Attr(pub String, pub Vec<String>, pub Vec<(String, String)>);

    impl Attr {
        pub fn null() -> Attr {
            Attr(String::new(), Vec::new(), Vec::new())
        }
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    pub struct TableCell(pub Vec<Block>);

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    #[serde(tag = "t", content = "c")]
    pub enum QuoteType {
        SingleQuote,
        DoubleQuote,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct Target(pub String, pub String);

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    #[serde(tag = "t", content = "c")]
    pub enum MathType {
        DisplayMath,
        InlineMath,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct Citation {
        #[serde(rename = "citationId")]
        pub citation_id: String,
        #[serde(rename = "citationPrefix")]
        pub citation_prefix: Vec<Inline>,
        #[serde(rename = "citationSuffix")]
        pub citation_suffix: Vec<Inline>,
        #[serde(rename = "citationMode")]
        pub citation_mode: CitationMode,
        #[serde(rename = "citationNoteNum")]
        pub citation_note_num: i32,
        #[serde(rename = "citationHash")]
        pub citation_hash: i32,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    #[serde(tag = "t", content = "c")]
    pub enum CitationMode {
        AuthorInText,
        SuppressAuthor,
        NormalCitation,
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn meta_null() {
            assert!(Meta::null().is_null());
        }
    }

}
