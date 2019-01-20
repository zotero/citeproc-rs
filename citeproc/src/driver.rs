use crate::input::*;
use crate::output::*;
use crate::proc::{CiteContext, Proc};
use crate::style::element::Position;
use crate::style::element::Style;
use crate::style::error::StyleError;
use crate::style::FromNode;
use roxmltree::Document;

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
        let ctx = CiteContext {
            style: &self.style,
            reference: refr,
            cite: &Cite::basic("ok".into(), &self.formatter.plain("")),
            position: Position::First,
            format: self.formatter,
            citation_number: 1,
        };
        let i = self.style.intermediate(&ctx);
        let flat = i.flatten(self.formatter);
        let o = self.formatter.output(flat);
        serde_json::to_string(&o).unwrap()
    }

    pub fn pair(&self, cite: &Cite<O>, refr: &Reference) {
        let ctx = CiteContext {
            style: &self.style,
            cite,
            reference: refr,
            position: Position::First,
            format: self.formatter,
            citation_number: 1,
        };
        self.style.intermediate(&ctx);
    }

    pub fn multiple(&self, pairs: &[(&Cite<O>, &Reference)]) -> bool {
        #[cfg(feature = "rayon")]
        {
            use rayon::prelude::*;
            pairs
                .par_iter()
                .map(|pair| {
                    self.style.intermediate(&CiteContext {
                        style: &self.style,
                        cite: pair.0,
                        reference: pair.1,
                        position: Position::First,
                        format: self.formatter,
                        citation_number: 1,
                    })
                })
                .any(|ir| {
                    if let crate::proc::IR::Rendered(None) = ir {
                        true
                    } else {
                        false
                    }
                })
        }
        #[cfg(not(feature = "rayon"))]
        {
            pairs
                .iter()
                .map(|pair| {
                    self.style.intermediate(&CiteContext {
                        style: &self.style,
                        cite: pair.0,
                        reference: pair.1,
                        position: Position::First,
                        format: self.formatter,
                        citation_number: 1,
                    })
                })
                .any(|ir| {
                    if let crate::proc::IR::Rendered(None) = ir {
                        true
                    } else {
                        false
                    }
                })
        }
    }

    pub fn dump_macro(&self, s: &str) {
        eprintln!("{:?}", self.style.macros.get(s))
    }

    pub fn dump_style(&self) {
        eprintln!("{:?}", self.style)
    }

    pub fn dump_ir(&self, refr: &Reference) {
        let ctx = CiteContext {
            style: &self.style,
            reference: refr,
            cite: &Cite::basic("ok".into(), &self.formatter.plain("")),
            position: Position::First,
            format: self.formatter,
            citation_number: 1,
        };
        let ir = self.style.intermediate(&ctx);
        eprintln!("{:?}", ir);
    }
}
