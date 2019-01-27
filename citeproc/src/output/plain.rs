use super::OutputFormat;

use crate::style::element::Formatting;

#[derive(Debug, Clone)]
pub struct PlainText {}

impl Default for PlainText {
    fn default() -> Self {
        PlainText {}
    }
}

impl OutputFormat for PlainText {
    type Build = String;
    type Output = String;

    fn plain(&self, s: &str) -> Self::Build {
        s.to_owned()
    }

    fn text_node(&self, s: String, _: Option<Formatting>) -> Self::Build {
        s
    }

    fn join_delim(&self, mut a: Self::Build, delim: &str, b: Self::Build) -> Self::Build {
        a.push_str(&delim);
        a.push_str(&b);
        a
    }

    fn seq(&self, mut nodes: impl Iterator<Item = Self::Build>) -> Self::Build {
        if let Some(first) = nodes.next() {
            nodes.fold(first, |mut a, b| {
                a.push_str(&b);
                a
            })
        } else {
            String::new()
        }
    }

    fn with_format(&self, a: Self::Build, _f: Option<Formatting>) -> Self::Build {
        a
    }

    fn group(
        &self,
        nodes: Vec<Self::Build>,
        delimiter: &str,
        _f: Option<Formatting>,
    ) -> Self::Build {
        nodes.join(delimiter)
    }

    fn output(&self, intermediate: Self::Build) -> Self::Output {
        intermediate
    }
}
