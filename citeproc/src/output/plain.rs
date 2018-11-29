use crate::output::Format;

use crate::style::element::Formatting;

pub struct PlainTextFormat {}
impl PlainTextFormat {
    pub fn new() -> Self {
        PlainTextFormat {}
    }
}
impl Format<String, String> for PlainTextFormat {
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
