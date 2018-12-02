use crate::input::*;
use crate::output::*;
use crate::proc::Proc;
use crate::style::element::Style;
use crate::style::error::{ CslError, StyleError };
use crate::style::FromNode;
use roxmltree::Document;
use typed_arena::Arena;
// use serde::Serialize;

rental! {
    mod rent_style {
        use crate::style::element::{ Style, Element };
        #[rental]
        pub struct RentStyle {
            arena: Box<typed_arena::Arena<Element>>,
            style: Style<'arena>,
        }
    }
}

use self::rent_style::RentStyle;

pub struct Driver<'a, O>
where
    O: OutputFormat + std::fmt::Debug,
{
    style: RentStyle,
    formatter: &'a O,
}

use rental::RentalError;

impl<T> From<RentalError<CslError, T>> for StyleError {
    fn from(err: RentalError<CslError, T>) -> Self {
        StyleError::Invalid(err.0)
    }
}

impl<'a, O> Driver<'a, O>
where
    O: OutputFormat + std::fmt::Debug,
{
    pub fn new(style_string: &str, formatter: &'a O) -> Result<Self, StyleError> {
        let doc = Document::parse(&style_string)?;
        let style = RentStyle::try_new(Box::new(Arena::new()), |arena| {
            Style::from_node(&doc.root_element(), &arena)
        })?;
        Ok(Driver { style, formatter })
    }

    pub fn single(&self, refr: &Reference) -> String {
        self.style.rent(|style| {
            let i = style.intermediate(self.formatter, &refr);
            let flat = i.flatten(self.formatter);
            let o = self.formatter.output(flat);
            serde_json::to_string(&o).unwrap()
        })
    }

    pub fn dump_style(&self) {
        self.style.rent(|style| println!("{:?}", style))
    }

    pub fn dump_ir(&self, refr: &Reference) {
        self.style.rent(|style| {
            let ir = style.intermediate(self.formatter, &refr);
            println!("{:?}", ir);
        })
    }
}
