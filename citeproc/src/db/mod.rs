// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

mod ir;
pub use ir::IrDatabase;
mod cite;
pub use cite::CiteDatabase;
mod xml;
pub use xml::{LocaleDatabase, LocaleFetcher, LocaleFetchError, StyleDatabase};

#[cfg(test)]
pub use self::xml::Predefined;

#[cfg(test)]
mod test;

use self::cite::CiteDatabaseStorage;
use self::ir::IrDatabaseStorage;
use self::xml::{HasFetcher, LocaleDatabaseStorage, StyleDatabaseStorage};

#[cfg(feature = "rayon")]
use salsa::{ParallelDatabase, Snapshot};
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;

use csl::error::StyleError;
use csl::locale::Lang;
use csl::style::{Position, Style};

use crate::input::{Cite, CiteId, Cluster, ClusterId, Reference};
use crate::output::OutputFormat;
use crate::output::Pandoc;
use crate::proc::{CiteContext, IrState};
use crate::Atom;

#[salsa::database(
    StyleDatabaseStorage,
    LocaleDatabaseStorage,
    CiteDatabaseStorage,
    IrDatabaseStorage
)]
pub struct Processor {
    runtime: salsa::Runtime<Self>,
    pub fetcher: Arc<dyn LocaleFetcher>,
}

/// This impl tells salsa where to find the salsa runtime.
impl salsa::Database for Processor {
    fn salsa_runtime(&self) -> &salsa::Runtime<Processor> {
        &self.runtime
    }

    fn salsa_event(&self, event_fn: impl Fn() -> salsa::Event<Self>) {
        use self::__SalsaDatabaseKeyKind::IrDatabaseStorage as RDS;
        use self::ir::IrDatabaseGroupKey__ as GroupKey;
        use salsa::EventKind::*;
        match event_fn().kind {
            WillExecute { database_key } => match database_key.kind {
                RDS(GroupKey::built_cluster(_key)) => {
                    // eprintln!("cluster #{:?} recomputed", key);
                }
                _ => {}
            },
            _ => {}
        }
    }
}

#[cfg(feature = "rayon")]
impl ParallelDatabase for Processor {
    fn snapshot(&self) -> Snapshot<Self> {
        Snapshot::new(Processor {
            runtime: self.runtime.snapshot(self),
            fetcher: self.fetcher.clone(),
        })
    }
}

impl HasFetcher for Processor {
    fn get_fetcher(&self) -> Arc<dyn LocaleFetcher> {
        self.fetcher.clone()
    }
}

// need a Clone impl for map_with
// thanks to rust-analyzer for the tip
#[cfg(feature = "rayon")]
struct Snap(pub salsa::Snapshot<Processor>);
#[cfg(feature = "rayon")]
impl Clone for Snap {
    fn clone(&self) -> Self {
        Snap(self.0.snapshot())
    }
}

impl Processor {
    pub(crate) fn safe_default(fetcher: Arc<dyn LocaleFetcher>) -> Self {
        let mut db = Processor {
            runtime: Default::default(),
            fetcher,
        };
        // TODO: way more salsa::inputs
        db.set_style(Default::default());
        db.set_all_uncited(Default::default());
        db.set_cluster_ids(Arc::new(vec![]));
        db.set_locale_input_langs(Default::default());
        db
    }

    pub fn new(style_string: &str, fetcher: Arc<dyn LocaleFetcher>) -> Result<Self, StyleError> {
        let mut db = Processor::safe_default(fetcher);
        let style = Arc::new(Style::from_str(style_string)?);
        db.set_style(style);
        Ok(db)
    }

    #[cfg(test)]
    pub fn test_db() -> Self {
        use self::xml::Predefined;
        Processor::safe_default(Arc::new(Predefined(Default::default())))
    }

    #[cfg(feature = "rayon")]
    fn snap(&self) -> Snap {
        Snap(self.snapshot())
    }

    pub fn compute(&self) {
        // If you're not runnning in parallel, there is no optimal parallelization order
        // So just do nothing.
        #[cfg(feature = "rayon")]
        {
            use rayon::prelude::*;
            let cluster_ids = self.cluster_ids();
            let cite_ids = self.all_cite_ids();
            // compute ir2s, so the first year_suffixes call doesn't trigger all ir2s on a
            // single rayon thread
            cite_ids
                .par_iter()
                .for_each_with(self.snap(), |snap, &cite_id| {
                    snap.0.ir_gen2_add_given_name(cite_id);
                });
            self.year_suffixes();
            cluster_ids
                .par_iter()
                .for_each_with(self.snap(), |snap, &cluster_id| {
                    snap.0.built_cluster(cluster_id);
                });
        }
    }

    // TODO: make this use
    pub fn single(&self, ref_id: &Atom) -> <Pandoc as OutputFormat>::Output {
        let fmt = Pandoc::default();
        let refr = match self.reference(ref_id.clone()) {
            None => return fmt.output(fmt.plain("Reference not found")),
            Some(r) => r,
        };
        let ctx = CiteContext {
            reference: &refr,
            cite: &Cite::basic(0, "ok"),
            position: Position::First,
            format: Pandoc::default(),
            citation_number: 1,
            disamb_pass: None,
        };
        let style = self.style();
        let mut state = IrState::new();
        use crate::proc::Proc;
        let ir = style.intermediate(self, &mut state, &ctx).0;

        ir.flatten(&fmt)
            .map(|flat| fmt.output(flat))
            .unwrap_or(<Pandoc as OutputFormat>::Output::default())
    }

    pub fn set_references(&mut self, refs: Vec<Reference>) {
        let keys: HashSet<Atom> = refs.iter().map(|r| r.id.clone()).collect();
        for r in refs {
            self.set_reference_input(r.id.clone(), Arc::new(r));
        }
        self.set_all_keys(Arc::new(keys));
    }

    pub fn init_clusters(&mut self, clusters: Vec<Cluster<Pandoc>>) {
        let mut cluster_ids = Vec::new();
        for cluster in clusters {
            let mut ids = Vec::new();
            for cite in cluster.cites.iter() {
                ids.push(cite.id);
                self.set_cite(cite.id, Arc::new(cite.clone()));
            }
            self.set_cluster_cites(cluster.id, Arc::new(ids));
            self.set_cluster_note_number(cluster.id, cluster.note_number);
            cluster_ids.push(cluster.id);
        }
        self.set_cluster_ids(Arc::new(cluster_ids));
    }

    // cluster_ids is maintained manually
    // the cluster_cites relation is maintained manually

    pub fn remove_cluster(&mut self, id: ClusterId) {
        self.set_cluster_cites(id, Arc::new(Vec::new()));
        let cluster_ids = self.cluster_ids();
        let cluster_ids: Vec<_> = (*cluster_ids)
            .iter()
            .filter(|&i| *i != id)
            .cloned()
            .collect();
        self.set_cluster_ids(Arc::new(cluster_ids));
        // delete associated cites
        // self.set_cluster_cites(id, Arc::new(Vec::new()));
        // let new = self
        //     .cluster_ids()
        //     .iter()
        //     .filter(|i| **i != id)
        //     .cloned()
        //     .collect();
        // self.set_cluster_ids(Arc::new(new));
    }

    // pub fn insert_cluster(&mut self, cluster: Cluster<Pandoc>, before: Option<ClusterId>) {}

    // pub fn replace_cluster(&mut self, cluster: Cluster<Pandoc>) {}

    // Getters, because the query groups have too much exposed to publish.

    pub fn get_cite(&self, id: CiteId) -> Arc<Cite<Pandoc>> {
        self.cite(id)
    }

    pub fn get_cluster(&self, id: ClusterId) -> Arc<<Pandoc as OutputFormat>::Output> {
        self.built_cluster(id)
    }

    pub fn get_reference(&self, ref_id: Atom) -> Option<Arc<Reference>> {
        self.reference(ref_id)
    }

    pub fn get_style(&self) -> Arc<Style> {
        self.style()
    }

    pub fn store_locales(&mut self, locales: Vec<(Lang, String)>) {
        let mut langs = (*self.locale_input_langs()).clone();
        for (lang, xml) in locales {
            langs.insert(lang.clone());
            self.set_locale_input_xml(lang, Arc::new(xml));
        }
        self.set_locale_input_langs(Arc::new(langs));
    }

    pub fn get_langs_in_use(&self) -> Vec<Lang> {
        let mut langs: Vec<Lang> = self
            .all_keys()
            .iter()
            .filter_map(|ref_id| self.reference(ref_id.clone()))
            .filter_map(|refr| refr.language.clone())
            .collect();
        let style = self.style();
        langs.push(style.default_locale.clone());
        langs
    }

    pub fn has_cached_locale(&self, lang: &Lang) -> bool {
        let langs = self.locale_input_langs();
        langs.contains(lang)
    }
}
