mod pandoc;
// mod markdown;
mod plain;
use std::marker::{Send, Sync};

pub use self::pandoc::Pandoc;
pub use self::plain::PlainText;
// pub use self::markdown::Markdown;

use crate::style::element::{Affixes, Formatting};
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug)]
pub struct Output<T> {
    pub citations: Vec<T>,
    pub bibliography: Vec<T>,
    pub citation_ids: Vec<String>,
}

pub trait OutputFormat : Send + Sync {
    type Build: std::fmt::Debug + Default + Clone + Send + Sync;
    type Output: Serialize + Clone + Send + Sync;

    /// Affixes are not included in the formatting on a text node.
    /// They are converted into text nodes themselves, with Formatting::default() passed.
    ///
    /// [Spec](https://docs.citationstyles.org/en/stable/specification.html#affixes)

    // TODO: make formatting an Option<&Formatting>
    fn text_node(&self, s: String, formatting: &Formatting) -> Self::Build;

    fn group(&self, nodes: &[Self::Build], delimiter: &str, formatting: &Formatting)
        -> Self::Build;
    fn output(&self, intermediate: Self::Build) -> Self::Output;

    #[cfg_attr(feature = "flame_it", flame("OutputFormat"))]
    fn plain(&self, s: &str) -> Self::Build {
        self.text_node(s.to_owned(), &Formatting::default())
    }

    #[cfg_attr(feature = "flame_it", flame("OutputFormat"))]
    fn affixed(&self, s: String, format_inner: &Formatting, affixes: &Affixes) -> Self::Build {
        let pre = affixes.prefix.is_empty();
        let suf = affixes.suffix.is_empty();
        let null_f = Formatting::default();
        match (pre, suf) {
            (false, false) => self.text_node(s, format_inner),
            (false, true) => self.group(&[
                    self.text_node(s, format_inner),
                    self.text_node(affixes.suffix.to_owned(), &null_f),
                ],
                "", &null_f),

            (true, false) => self.group(&[
                    self.text_node(affixes.prefix.to_owned(), &null_f),
                    self.text_node(s, format_inner),
                ],
                "", &null_f),

            (true, true) => self.group(&[
                    self.text_node(affixes.prefix.to_owned(), &null_f),
                    self.text_node(s, format_inner),
                    self.text_node(affixes.suffix.to_owned(), &null_f),
                ],
                "", &null_f),
        }
    }
}

#[cfg(test)]
mod test {

    use crate::style::element::Formatting;

    use super::OutputFormat;
    use super::PlainText;

    // #[test]
    // fn markdown() {
    //     let f = Markdown::new();
    //     let o = f.text_node("hi", &Formatting::italic());
    //     let o2 = f.text_node("mom", &Formatting::bold());
    //     let o3 = f.group(&[o, o2], " ", &Formatting::italic());
    //     let serialized = serde_json::to_string(&o3).unwrap();
    //     assert_eq!(serialized, "\"_hi **mom**_\"");
    // }

    #[test]
    fn test_plain() {
        let f = PlainText::new();
        let o = f.text_node("hi".into(), &Formatting::italic());
        let o2 = f.text_node("mom".into(), &Formatting::default());
        let o3 = f.group(&[o, o2], " ", &Formatting::italic());
        let serialized = serde_json::to_string(&o3).unwrap();
        assert_eq!(serialized, "\"hi mom\"");
    }

}
