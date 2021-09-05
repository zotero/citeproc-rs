use crate::prelude::*;
use citeproc_db::{
    CiteData, ClusterId, ClusterNumber, LocaleFetcher, PredefinedLocales, StyleDatabase,
};
use citeproc_io::{output::markup::Markup, Cite, Reference};

use csl::Style;
use fnv::FnvHashSet;
use std::sync::Arc;

#[allow(dead_code)]
pub fn with_test_style<T>(s: &str, f: impl Fn(Style) -> T) -> T {
    let sty = Style::parse_for_test(
        &format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
{}"#,
            s
        ),
        None,
    )
    .unwrap();
    f(sty)
}

pub fn test_style_layout(s: &str) -> String {
    format!(
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
    )
}

pub fn with_test_citation<T>(mut f: impl FnMut(Style) -> T, s: &str) -> T {
    let sty = Style::parse_for_test(&test_style_layout(s), None).unwrap();
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
    storage: salsa::Storage<Self>,
    fetcher: Arc<dyn LocaleFetcher>,
    formatter: Markup,
}

impl salsa::Database for MockProcessor {}

impl ImplementationDetails for MockProcessor {
    fn get_formatter(&self) -> Markup {
        self.formatter.clone()
    }
    fn lookup_cluster_id(&self, _symbol: ClusterId) -> Option<SmartString> {
        None
    }
}

impl citeproc_db::HasFetcher for MockProcessor {
    fn get_fetcher(&self) -> Arc<dyn LocaleFetcher> {
        self.fetcher.clone()
    }
}

impl MockProcessor {
    pub fn rtf() -> Self {
        let mut new = Self::new();
        new.formatter = Markup::rtf();
        new
    }
    pub fn new() -> Self {
        let fetcher = Arc::new(PredefinedLocales::bundled_en_us());
        let mut db = MockProcessor {
            storage: Default::default(),
            fetcher,
            formatter: Markup::html(),
        };
        citeproc_db::safe_default(&mut db);
        crate::safe_default(&mut db);
        db
    }

    pub fn set_style_text(&mut self, style_text: &str) {
        let style = Style::parse_for_test(style_text, None).unwrap();
        use salsa::Durability;
        self.set_style_with_durability(Arc::new(style), Durability::MEDIUM);
    }

    pub fn init_clusters(&mut self, clusters: Vec<(ClusterId, ClusterNumber, Vec<Cite<Markup>>)>) {
        let mut cluster_ids = FnvHashSet::default();
        cluster_ids.reserve(clusters.len());
        for cluster in clusters {
            let (cluster_id, note_number, cites) = cluster;
            let mut ids = Vec::new();
            for (index, cite) in cites.into_iter().enumerate() {
                let cite_id = self.cite(CiteData::RealCite {
                    cluster: cluster_id,
                    index: index as u32,
                    cite: Arc::new(cite),
                });
                ids.push(cite_id);
            }
            self.set_cluster_cites(cluster_id, Arc::new(ids));
            self.set_cluster_note_number(cluster_id, Some(note_number));
            self.set_cluster_mode(cluster_id, None);
            cluster_ids.insert(cluster_id);
        }
        self.set_all_cluster_ids(Arc::new(cluster_ids));
    }

    pub fn insert_references(&mut self, refs: Vec<Reference>) {
        let keys = refs.iter().map(|r| r.id.clone()).collect();
        for r in refs {
            self.set_reference_input(r.id.clone(), Arc::new(r));
        }
        self.set_all_keys(Arc::new(keys));
    }

    #[allow(dead_code)]
    pub fn insert_cites(&mut self, cluster_id: ClusterId, cites: &[Cite<Markup>]) {
        let all_cluster_ids = self.all_cluster_ids();
        if !all_cluster_ids.contains(&cluster_id) {
            let mut new_all = (*all_cluster_ids).clone();
            new_all.insert(cluster_id);
            self.set_all_cluster_ids(Arc::new(new_all));
            self.set_cluster_note_number(cluster_id, None);
            self.set_cluster_mode(cluster_id, None);
        }

        let mut ids = Vec::new();
        for (index, cite) in cites.iter().enumerate() {
            let cite_id = self.cite(CiteData::RealCite {
                cluster: cluster_id,
                index: index as u32,
                cite: Arc::new(cite.clone()),
            });
            ids.push(cite_id);
        }
        self.set_cluster_cites(cluster_id, Arc::new(ids));
    }
}
