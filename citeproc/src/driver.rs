use crate::db::ReferenceDatabase;
use crate::input::*;
use crate::output::*;
use crate::proc::{CiteContext, IrState, Proc};
use crate::style::db::StyleDatabase;
use crate::Atom;
use csl::error::StyleError;
use csl::style::Position;
use csl::style::Style;
use std::str::FromStr;
use std::sync::Arc;

#[cfg(feature = "rayon")]
use rayon::prelude::*;

use crate::db::Processor;

pub struct Driver<O>
where
    O: OutputFormat + std::fmt::Debug,
{
    pub db: Processor,
    o: std::marker::PhantomData<O>,
}

impl<O> Driver<O>
where
    O: OutputFormat + std::fmt::Debug,
{

    pub fn new(style_string: &str) -> Result<Self, StyleError> {
        let db = Processor::new(style_string, )
        let style = Arc::new(Style::from_str(style_string)?);
        db.set_style(style);

        Ok(Driver {
            db,
            o: Default::default(),
        })
    }

    #[cfg(feature = "rayon")]
    pub fn snap(&self) -> Snap {
        use salsa::ParallelDatabase;
        Snap(self.db.snapshot())
    }

    pub fn single(&self, refr: &Reference) -> String {
        let ctx = CiteContext {
            reference: refr,
            cite: &Cite::basic(0, "ok"),
            position: Position::First,
            format: O::default(),
            citation_number: 1,
            disamb_pass: None,
        };
        let mut state = IrState::new();
        let (i, _) = self.db.style().intermediate(&self.db, &mut state, &ctx);
        let fmt = O::default();
        if let Some(flat) = i.flatten(&fmt) {
            let o = fmt.output(flat);
            serde_json::to_string(&o).unwrap()
        } else {
            "".to_string()
        }
    }

    pub fn pair(&self, cite: &Cite<O>, refr: &Reference) {
        let ctx = CiteContext {
            cite,
            reference: refr,
            position: Position::First,
            format: O::default(),
            citation_number: 1,
            disamb_pass: None,
        };
        let mut state = IrState::new();
        self.db.style().intermediate(&self.db, &mut state, &ctx);
    }

    pub fn multiple(&self, pairs: &[(&Cite<O>, &Reference)]) -> bool {
        #[cfg(feature = "rayon")]
        {
            use rayon::prelude::*;
            pairs
                .par_iter()
                .map_with(self.snap(), |snap, pair| {
                    let db = &*snap.0;
                    let ctx = CiteContext {
                        cite: pair.0,
                        reference: pair.1,
                        position: Position::First,
                        format: O::default(),
                        citation_number: 1,
                        disamb_pass: None,
                    };
                    let mut state = IrState::new();
                    db.style().intermediate(db, &mut state, &ctx).0
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
                    let fmt = O::default();
                    let ctx = CiteContext {
                        cite: pair.0,
                        reference: pair.1,
                        position: Position::First,
                        format: O::default(),
                        citation_number: 1,
                        disamb_pass: None,
                    };
                    let mut state = IrState::new();
                    db.style().intermediate(&self.db, &mut state, &ctx).0
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
        eprintln!("{:?}", self.db.style().macros.get(&s))
    }

    pub fn dump_style(&self) {
        eprintln!("{:?}", self.db.style())
    }

    pub fn dump_ir(&self, refr: &Reference) {
        let ctx = CiteContext {
            reference: refr,
            cite: &Cite::basic(0, "ok"),
            position: Position::First,
            format: O::default(),
            citation_number: 1,
            disamb_pass: None,
        };
        let mut state = IrState::new();
        let ir = self.db.style().intermediate(&self.db, &mut state, &ctx).0;
        eprintln!("{:?}", ir);
    }
}
