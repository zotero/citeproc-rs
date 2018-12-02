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
    fn text_node(&self, s: &str, _: &Formatting) -> Self::Build {
        s.to_owned()
    }
    fn group(&self, nodes: &[Self::Build], delim: &str, _: &Formatting) -> Self::Build {
        nodes.join(delim)
    }
    fn output(&self, intermediate: Self::Build) -> Self::Output {
        intermediate
    }
}
