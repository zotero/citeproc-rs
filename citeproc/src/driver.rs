use crate::db::ReferenceDatabase;
use crate::input::*;
use crate::output::*;
use crate::proc::{CiteContext, IrState, Proc};
use crate::style::db::StyleDatabase;
use crate::style::element::Position;
use crate::style::element::Style;
use crate::style::error::StyleError;
use crate::style::FromNode;
use crate::Atom;
use roxmltree::Document;
use std::sync::Arc;

#[cfg(feature = "rayon")]
use rayon::prelude::*;

use crate::db_impl::RootDatabase;

pub struct Driver<O>
where
    O: OutputFormat + std::fmt::Debug,
{
    pub db: RootDatabase,
    o: std::marker::PhantomData<O>,
}

// need a Clone impl for map_with
// thanks to rust-analyzer for the tip
pub struct Snap(pub salsa::Snapshot<RootDatabase>);
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

    pub fn compute(&self) {
        // If you're not runnning in parallel, there is no optimal parallelization order
        // So just do nothing.
        #[cfg(feature = "rayon")]
        {
            let cluster_ids = self.db.cluster_ids(());
            let cite_ids = self.db.all_cite_ids(());
            // compute ir2s, so the first year_suffixes call doesn't trigger all ir2s on a
            // single rayon thread
            cite_ids
                .par_iter()
                .for_each_with(self.snap(), |snap, &cite_id| {
                    snap.0.ir_gen2_add_given_name(cite_id);
                });
            self.db.year_suffixes(());
            cluster_ids
                .par_iter()
                .for_each_with(self.snap(), |snap, &cluster_id| {
                    snap.0.built_cluster(cluster_id);
                });
        }
    }

    pub fn single(&self, refr: &Reference) -> String {
        let ctx = CiteContext {
            reference: refr,
            cite: &Cite::basic(0, "ok"),
            position: Position::First,
            format: O::default(),
            citation_number: 1,
            re_evaluation: None,
        };
        let mut state = IrState::new();
        let (i, _) = self.db.style(()).intermediate(&self.db, &mut state, &ctx);
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
            re_evaluation: None,
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
                    let db = &*snap.0;
                    let ctx = CiteContext {
                        cite: pair.0,
                        reference: pair.1,
                        position: Position::First,
                        format: O::default(),
                        citation_number: 1,
                        re_evaluation: None,
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
                        format: O::default(),
                        citation_number: 1,
                        re_evaluation: None,
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
        let ctx = CiteContext {
            reference: refr,
            cite: &Cite::basic(0, "ok"),
            position: Position::First,
            format: O::default(),
            citation_number: 1,
            re_evaluation: None,
        };
        let mut state = IrState::new();
        let ir = self.db.style(()).intermediate(&self.db, &mut state, &ctx).0;
        eprintln!("{:?}", ir);
    }
}
