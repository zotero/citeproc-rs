mod pandoc;
// mod markdown;
mod plain;
use std::marker::{Send, Sync};

pub use self::pandoc::Pandoc;
pub use self::plain::PlainText;
// pub use self::markdown::Markdown;

use crate::style::element::{Affixes, Formatting};
use serde::{Serialize, de::DeserializeOwned};

#[derive(Serialize, Deserialize, Debug)]
pub struct Output<T> {
    pub citations: Vec<T>,
    pub bibliography: Vec<T>,
    pub citation_ids: Vec<String>,
}

pub trait OutputFormat: Send + Sync + std::fmt::Debug {
    type Build: std::fmt::Debug + DeserializeOwned + Serialize + Default + Clone + Send + Sync;
    type Output: Serialize + Clone + Send + Sync;

    /// Affixes are not included in the formatting on a text node. They are converted into text
    /// nodes themselves, with no formatting except whatever is applied by a parent group.
    ///
    /// [Spec](https://docs.citationstyles.org/en/stable/specification.html#affixes)

    // TODO: make formatting an Option<&Formatting>
    fn text_node(&self, s: String, formatting: Option<&Formatting>) -> Self::Build;

    /// Group some text nodes. You might want to optimise for the case where delimiter is empty.
    fn group(
        &self,
        nodes: Vec<Self::Build>,
        delimiter: &str,
        formatting: Option<&Formatting>,
    ) -> Self::Build;

    fn seq(&self, nodes: impl Iterator<Item = Self::Build>) -> Self::Build;

    fn join_delim(&self, a: Self::Build, delim: &str, b: Self::Build) -> Self::Build;

    fn output(&self, intermediate: Self::Build) -> Self::Output;

    fn plain(&self, s: &str) -> Self::Build;

    fn affixed_text(
        &self,
        s: String,
        format_inner: Option<&Formatting>,
        affixes: &Affixes,
    ) -> Self::Build {
        self.affixed(self.text_node(s, format_inner), affixes)
    }

    fn affixed(&self, b: Self::Build, affixes: &Affixes) -> Self::Build {
        use std::iter::once;
        let pre = affixes.prefix.is_empty();
        let suf = affixes.suffix.is_empty();
        match (pre, suf) {
            (true, true) => b,

            (false, true) => self.seq(once(self.plain(&affixes.prefix)).chain(once(b))),

            (true, false) => self.seq(once(b).chain(once(self.plain(&affixes.suffix)))),

            (false, false) => self.seq(
                once(self.plain(&affixes.prefix))
                    .chain(once(b))
                    .chain(once(self.plain(&affixes.suffix))),
            ),
        }
    }

    fn with_format(&self, a: Self::Build, f: Option<&Formatting>) -> Self::Build;
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

    // #[test]
    // fn test_plain() {
    //     let f = PlainText::new();
    //     let o = f.text_node("hi".into(), Some(&Formatting::italic()));
    //     let o2 = f.text_node("mom".into(), None);
    //     let o3 = f.group(&[o, o2], " ", Some(&Formatting::italic()));
    //     let serialized = serde_json::to_string(&o3).unwrap();
    //     assert_eq!(serialized, "\"hi mom\"");
    // }

}
