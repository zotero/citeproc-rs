use crate::input::*;
use crate::output::*;
use crate::proc::CiteContext;
use crate::proc::Proc;
use crate::style::element::Position;
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

    pub fn single(&self, refr: &Reference, prefix: &O::Output) -> String {
        let ctx = CiteContext {
            style: &self.style,
            reference: refr,
            cite: &Cite::basic("ok", prefix),
            position: Position::First,
            format: self.formatter,
            citation_number: 1,
        };
        let i = self.style.intermediate(&ctx);
        let flat = i.flatten(self.formatter);
        let o = self.formatter.output(flat);
        serde_json::to_string(&o).unwrap()
    }

    pub fn dump_style(&self) {
        println!("{:?}", self.style)
    }

    // pub fn dump_ir(&self, refr: &Reference) {
    //     let ir = self.style.intermediate(ctx: &CiteContext<'c, 'r>);
    //     println!("{:?}", ir);
    // }
}
