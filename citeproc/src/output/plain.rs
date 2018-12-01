use super::OutputFormat;

use crate::style::element::Formatting;

pub struct PlainText {}
impl PlainText {
    pub fn new() -> Self {
        PlainText {}
    }
}
impl OutputFormat<String, String> for PlainText {
    fn text_node(&self, s: &str, _: &Formatting) -> String {
        s.to_owned()
    }
    fn group(&self, nodes: &[String], delim: &str, _: &Formatting) -> String {
        nodes.join(delim)
    }
    fn output(&self, intermediate: String) -> String {
        intermediate
    }
}
