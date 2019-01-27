use crate::db::ReferenceDatabase;
use crate::input::*;
use crate::output::*;
use crate::proc::{IrState, Proc};
use crate::style::db::StyleDatabase;
use crate::style::db::StyleQuery;
use crate::style::element::Position;
use crate::style::element::Style;
use crate::style::error::StyleError;
use crate::style::FromNode;
use crate::Atom;
use roxmltree::Document;
use salsa::Database;
use std::collections::HashSet;
use std::sync::Arc;

use crate::db_impl::RootDatabase;

pub struct Driver<O>
where
    O: OutputFormat + std::fmt::Debug,
{
    db: RootDatabase,
    o: std::marker::PhantomData<O>,
}

// need a Clone impl for map_with
// thanks to rust-analyzer for the tip
struct Snap(salsa::Snapshot<RootDatabase>);
impl Clone for Snap {
    fn clone(&self) -> Self {
        use salsa::ParallelDatabase;
        Snap(self.0.snapshot())
    }
}

impl<O> Driver<O>
where
    O: OutputFormat + std::fmt::Debug,
{
    pub fn new(style_string: &str, mut db: RootDatabase) -> Result<Self, StyleError> {
        let doc = Document::parse(&style_string)?;
        let style = Arc::new(Style::from_node(&doc.root_element())?);
        db.set_style((), style);

        db.init_clusters(&[
            Cluster {
                id: 0,
                cites: vec![Cite::basic(0, "quagmire2018".into())],
            },
            Cluster {
                id: 1,
                cites: vec![Cite::basic(1, "quagmire2018".into())],
            },
        ]);

        dbg!(db.cite_positions(()));
        dbg!(db.ir(1));

        Ok(Driver {
            db,
            o: Default::default(),
        })
    }

    #[cfg(feature = "rayon")]
    fn snap(&self) -> Snap {
        use salsa::ParallelDatabase;
        Snap(self.db.snapshot())
    }

    pub fn single(&self, refr: &Reference) -> String {
        let fmt = O::default();
        let ctx = CiteContext {
            reference: refr,
            cite: &Cite::basic(0, "ok".into()),
            position: Position::First,
            format: &fmt,
            citation_number: 1,
        };
        let mut state = IrState::new();
        let (i, _) = self.db.style(()).intermediate(&self.db, &mut state, &ctx);
        let index = self.db.inverted_index(());
        let mut matching_ids = HashSet::new();
        for tok in state.tokens.iter() {
            // ignore tokens which matched NO references; they are just part of the style,
            // like <text value="xxx"/>. Of course:
            //   - <text value="xxx"/> WILL match any references that have a field with
            //     "xxx" in it.
            //   - You have to make sure all text is transformed equivalently.
            //   So TODO: make all text ASCII uppercase first!
            if let Some(ids) = index.get(tok) {
                for x in ids {
                    matching_ids.insert(x.clone());
                }
            }
        }
        // dbg!(state);
        // dbg!(matching_ids);
        let flat = i.flatten(&fmt);
        let o = fmt.output(flat);
        serde_json::to_string(&o).unwrap()
    }

    pub fn pair(&self, cite: &Cite<O>, refr: &Reference) {
        let fmt = O::default();
        let ctx = CiteContext {
            cite,
            reference: refr,
            position: Position::First,
            format: &fmt,
            citation_number: 1,
        };
        let mut state = IrState::new();
        self.db.style(()).intermediate(&self.db, &mut state, &ctx);
    }

    pub fn multiple(&self, pairs: &[(&Cite<O>, &Reference)]) -> bool {
        #[cfg(feature = "rayon")]
        {
            use rayon::prelude::*;
            pairs
                .par_iter()
                .map_with(self.snap(), |snap, pair| {
                    let fmt = O::default();
                    let db = &*snap.0;
                    let ctx = CiteContext {
                        cite: pair.0,
                        reference: pair.1,
                        position: Position::First,
                        format: &fmt,
                        citation_number: 1,
                    };
                    let mut state = IrState::new();
                    db.style(()).intermediate(db, &mut state, &ctx).0
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
                        format: &fmt,
                        citation_number: 1,
                    };
                    let mut state = IrState::new();
                    db.style(()).intermediate(&self.db, &mut state, &ctx).0
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
        eprintln!("{:?}", self.db.style(()).macros.get(&s))
    }

    pub fn dump_style(&self) {
        eprintln!("{:?}", self.db.style(()))
    }

    pub fn dump_ir(&self, refr: &Reference) {
        let fmt = O::default();
        let ctx = CiteContext {
            reference: refr,
            cite: &Cite::basic(0, "ok".into()),
            position: Position::First,
            format: &fmt,
            citation_number: 1,
        };
        let mut state = IrState::new();
        let ir = self.db.style(()).intermediate(&self.db, &mut state, &ctx).0;
        eprintln!("{:?}", ir);
    }
}
