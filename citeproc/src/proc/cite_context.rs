use crate::input::*;
use crate::output::*;
use crate::style::element::{Position, Style};

// in Proc, 's has to live at least as long as 'c ('s: 'c)
pub struct CiteContext<'c, 'r: 'c, O: OutputFormat> {
    pub style: &'c Style,
    pub reference: &'c Reference<'r>,
    pub cite: &'c Cite<O::Output>,
    pub format: &'c O,
    pub position: Position,
    pub citation_number: u32,
}

pub struct Cluster<'a, 'c, 'r: 'a + 'c, O: OutputFormat> {
    pub cites: &'a [CiteContext<'c, 'r, O>],
}
