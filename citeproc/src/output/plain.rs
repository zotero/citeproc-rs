use super::OutputFormat;

use crate::style::element::Formatting;

#[derive(Debug)]
pub struct PlainText {}
impl PlainText {
    pub fn new() -> Self {
        PlainText {}
    }
}
impl OutputFormat for PlainText {
    type Build = String;
    type Output = String;

    fn plain(&self, s: &str) -> Self::Build {
        s.to_owned()
    }

    fn text_node(&self, s: String, _: Option<&Formatting>) -> Self::Build {
        s
    }

    fn group(&self, nodes: &[Self::Build], delim: &str, _: Option<&Formatting>) -> Self::Build {
        nodes.join(delim)
    }

    fn output(&self, intermediate: Self::Build) -> Self::Output {
        intermediate
    }
}
