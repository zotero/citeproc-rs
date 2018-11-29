// type Citations = [[Cite]];
use crate::style::element::{ Formatting };

use serde::{ Serialize };

#[derive(Serialize, Deserialize, Debug)]
pub struct Output<T> {
    pub citations: Vec<T>,
    pub bibliography: Vec<T>,
    pub citation_ids: Vec<String>,
}

pub trait Format<T, O : Serialize> {
    // affixes are not included in the formatting on a text node.
    // affixes are converted into text nodes themselves, with Formatting::default() passed.
    // http://docs.citationstyles.org/en/stable/specification.html#affixes
    fn text_node(&self, s: &str, formatting: &Formatting) -> T;
    fn group(&self, nodes: &[T], delimiter: &str, formatting: &Formatting) -> T;
    fn output(&self, intermediate: T) -> O;

    fn plain(&self, s: &str) -> T {
        self.text_node(s, &Formatting::default())
    }
}

