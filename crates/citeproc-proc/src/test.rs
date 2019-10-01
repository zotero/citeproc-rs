use crate::prelude::*;
use citeproc_db::{LocaleFetcher, PredefinedLocales, StyleDatabase};
use citeproc_io::{output::markup::Markup, Cite, Cluster2, IntraNote, Reference};
use csl::locale::Lang;
use csl::Atom;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[salsa::database(
    citeproc_db::StyleDatabaseStorage,
    citeproc_db::LocaleDatabaseStorage,
    citeproc_db::CiteDatabaseStorage,
    crate::db::IrDatabaseStorage
)]
pub struct MockProcessor {
    runtime: salsa::Runtime<Self>,
    fetcher: Arc<dyn LocaleFetcher>,
}

impl HasFormatter for MockProcessor {
    fn get_formatter(&self) -> Markup {
        Markup::html()
    }
}

impl salsa::Database for MockProcessor {
    fn salsa_runtime(&self) -> &salsa::Runtime<MockProcessor> {
        &self.runtime
    }
}

impl citeproc_db::HasFetcher for MockProcessor {
    fn get_fetcher(&self) -> Arc<dyn LocaleFetcher> {
        self.fetcher.clone()
    }
}

impl MockProcessor {
    pub fn new() -> Self {
        let mut m = HashMap::new();
        m.insert(
            Lang::en_us(),
            include_str!("../../citeproc-wasm/src/locales-en-US.xml").to_string(),
        );
        let fetcher = Arc::new(PredefinedLocales(m));
        let mut db = MockProcessor {
            runtime: Default::default(),
            fetcher,
        };
        citeproc_db::safe_default(&mut db);
        db
    }

    pub fn set_style_text(&mut self, style_text: &str) {
        use csl::style::Style;
        use std::str::FromStr;
        let style = Style::from_str(style_text).unwrap();
        use salsa::Durability;
        self.set_style_with_durability(Arc::new(style), Durability::MEDIUM);
    }

    pub fn init_clusters(&mut self, clusters: Vec<Cluster2<Markup>>) {
        let mut cluster_ids = Vec::new();
        for cluster in clusters {
            let (cluster_id, number, cites) = cluster.split();
            let mut ids = Vec::new();
            for cite in cites.iter() {
                let cite_id = self.cite(cluster_id, Arc::new(cite.clone()));
                ids.push(cite_id);
            }
            self.set_cluster_cites(cluster_id, Arc::new(ids));
            self.set_cluster_note_number(cluster_id, number);
            cluster_ids.push(cluster_id);
        }
        self.set_cluster_ids(Arc::new(cluster_ids));
    }

    pub fn set_references(&mut self, refs: Vec<Reference>) {
        let keys: HashSet<Atom> = refs.iter().map(|r| r.id.clone()).collect();
        for r in refs {
            self.set_reference_input(r.id.clone(), Arc::new(r));
        }
        self.set_all_keys(Arc::new(keys));
    }
}
