use crate::prelude::*;
use citeproc_db::{LocaleFetcher, PredefinedLocales, StyleDatabase};
use citeproc_io::{output::markup::Markup, Cite, Cluster, IntraNote, Reference};
use csl::Atom;

use csl::Style;
use std::collections::HashSet;
use std::sync::Arc;

#[allow(dead_code)]
pub fn with_test_style<T>(s: &str, f: impl Fn(Style) -> T) -> T {
    use std::str::FromStr;
    let sty = Style::from_str(&format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
{}"#,
        s
    ))
    .unwrap();
    f(sty)
}

pub fn with_test_citation<T>(f: impl Fn(Style) -> T, s: &str) -> T {
    use std::str::FromStr;
    let sty = Style::from_str(&format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
    <style class="note" version="1.0.1">
        <citation>
            <layout>
                {}
            </layout>
        </citation>
    </style>
"#,
        s
    ))
    .unwrap();
    f(sty)
}

#[allow(clippy::large_enum_variant)]
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
        let fetcher = Arc::new(PredefinedLocales::bundled_en_us());
        let mut db = MockProcessor {
            runtime: Default::default(),
            fetcher,
        };
        citeproc_db::safe_default(&mut db);
        db
    }

    pub fn set_style_text(&mut self, style_text: &str) {
        use csl::Style;
        use std::str::FromStr;
        let style = Style::from_str(style_text).unwrap();
        use salsa::Durability;
        self.set_style_with_durability(Arc::new(style), Durability::MEDIUM);
    }

    pub fn init_clusters(&mut self, clusters: Vec<Cluster<Markup>>) {
        let mut cluster_ids = Vec::new();
        for cluster in clusters {
            let Cluster { id: cluster_id, cites } = cluster;
            let mut ids = Vec::new();
            for (index, cite) in cites.into_iter().enumerate() {
                let cite_id = self.cite(cluster_id, index as u32, Arc::new(cite));
                ids.push(cite_id);
            }
            self.set_cluster_cites(cluster_id, Arc::new(ids));
            self.set_cluster_note_number(cluster_id, None);
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
