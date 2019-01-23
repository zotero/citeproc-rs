use crate::input::*;
use crate::output::*;
use crate::proc::{CiteContext, IrState, Proc};
use crate::style::element::Position;
use crate::style::element::Style;
use crate::style::error::StyleError;
use crate::style::FromNode;
use crate::Atom;
use roxmltree::Document;

use crate::db_impl::RootDatabase;

pub struct Driver<'a, O>
where
    O: OutputFormat + std::fmt::Debug,
{
    style: Style,
    formatter: &'a O,
    db: RootDatabase,
}

impl<'a, O> Driver<'a, O>
where
    O: OutputFormat + std::fmt::Debug,
{
    pub fn new(style_string: &str, formatter: &'a O, db: RootDatabase) -> Result<Self, StyleError> {
        let doc = Document::parse(&style_string)?;
        let style = Style::from_node(&doc.root_element())?;
        Ok(Driver {
            style,
            formatter,
            db,
        })
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
        let mut state = IrState::new();
        let (i, _) = self.style.intermediate(&self.db, &mut state, &ctx);
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
        let mut state = IrState::new();
        self.style.intermediate(&self.db, &mut state, &ctx);
    }

    pub fn multiple(&self, pairs: &[(&Cite<O>, &Reference)]) -> bool {
        // Feature disabled for now, because using Rayon's threadpool might deadlock the
        // database
        #[cfg(feature = "rayon")]
        {
            use rayon::prelude::*;
            use salsa::ParallelDatabase;
            let snapshot = self.db.snapshot();
            pairs
                .par_iter()
                .map(|pair| {
                    let ctx = CiteContext {
                        style: &self.style,
                        cite: pair.0,
                        reference: pair.1,
                        position: Position::First,
                        format: self.formatter,
                        citation_number: 1,
                    };
                    let mut state = IrState::new();
                    self.style.intermediate(&snapshot, &mut state, &ctx).0
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
                    let ctx = CiteContext {
                        style: &self.style,
                        cite: pair.0,
                        reference: pair.1,
                        position: Position::First,
                        format: self.formatter,
                        citation_number: 1,
                    };
                    let mut state = IrState::new();
                    self.style.intermediate(&self.db, &mut state, &ctx).0
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

    pub fn dump_macro(&self, s: Atom) {
        eprintln!("{:?}", self.style.macros.get(&s))
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
        let mut state = IrState::new();
        let ir = self.style.intermediate(&self.db, &mut state, &ctx).0;
        eprintln!("{:?}", ir);
    }
}
