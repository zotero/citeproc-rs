mod pandoc;
// mod markdown;
mod plain;

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

pub trait OutputFormat {
    type Build: std::fmt::Debug + Clone;
    type Output: Serialize + Clone;
    // affixes are not included in the formatting on a text node.
    // affixes are converted into text nodes themselves, with Formatting::default() passed.
    // http://docs.citationstyles.org/en/stable/specification.html#affixes
    fn text_node(&self, s: &str, formatting: &Formatting) -> Self::Build;
    fn group(&self, nodes: &[Self::Build], delimiter: &str, formatting: &Formatting)
        -> Self::Build;
    fn output(&self, intermediate: Self::Build) -> Self::Output;

    #[cfg_attr(feature = "flame_it", flame("OutputFormat"))]
    fn plain(&self, s: &str) -> Self::Build {
        self.text_node(s, &Formatting::default())
    }

    #[cfg_attr(feature = "flame_it", flame("OutputFormat"))]
    fn affixed(&self, s: &str, format_inner: &Formatting, affixes: &Affixes) -> Self::Build {
        let null_f = Formatting::default();
        self.group(
            &[
                self.text_node(&affixes.prefix, &null_f),
                self.text_node(s, format_inner),
                self.text_node(&affixes.suffix, &null_f),
            ],
            "",
            &null_f,
        )
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
    fn plain() {
        let f = PlainText::new();
        let o = f.text_node("hi", &Formatting::italic());
        let o2 = f.text_node("mom", &Formatting::default());
        let o3 = f.group(&[o, o2], " ", &Formatting::italic());
        let serialized = serde_json::to_string(&o3).unwrap();
        assert_eq!(serialized, "\"hi mom\"");
    }

}
