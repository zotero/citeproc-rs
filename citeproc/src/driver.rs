use crate::input::*;
use crate::output::*;
use crate::proc::Proc;
use crate::style::element::Style;
use crate::style::error::{CslError, StyleError};
use crate::style::FromNode;
use roxmltree::Document;

impl From<CslError> for StyleError {
    fn from(err: CslError) -> Self {
        StyleError::Invalid(err)
    }
}

pub struct Driver<'a, O>
where
    O: OutputFormat + std::fmt::Debug,
{
    style: Style,
    formatter: &'a O,
}

impl<'a, O> Driver<'a, O>
where
    O: OutputFormat + std::fmt::Debug,
{
    pub fn new(style_string: &str, formatter: &'a O) -> Result<Self, StyleError> {
        let doc = Document::parse(&style_string)?;
        let style = Style::from_node(&doc.root_element())?;
        Ok(Driver { style, formatter })
    }

    pub fn single(&self, refr: &Reference) -> String {
        let i = self.style.intermediate(self.formatter, &refr);
        let flat = i.flatten(self.formatter);
        let o = self.formatter.output(flat);
        serde_json::to_string(&o).unwrap()
    }

    pub fn dump_style(&self) {
        println!("{:?}", self.style)
    }

    pub fn dump_ir(&self, refr: &Reference) {
        let ir = self.style.intermediate(self.formatter, &refr);
        println!("{:?}", ir);
    }
}
