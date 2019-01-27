use salsa::{Database, ParallelDatabase, Snapshot};
use serde_json;
use std::collections::HashSet;
use std::sync::Arc;

use crate::db::*;
use crate::input::{Cluster, ClusterId, Reference};
use crate::locale::db::HasFetcher;
use crate::locale::{db::*, LocaleFetcher};
use crate::output::Pandoc;
use crate::style::db::*;
use crate::Atom;

#[salsa::database(StyleDatabaseStorage, LocaleDatabaseStorage, ReferenceDatabaseStorage)]
pub struct RootDatabase {
    runtime: salsa::Runtime<Self>,
    fetcher: Arc<LocaleFetcher>,
}

impl RootDatabase {
    pub fn new(fetcher: Arc<LocaleFetcher>) -> Self {
        let mut db = RootDatabase {
            runtime: Default::default(),
            fetcher,
        };
        db.query_mut(StyleQuery).set((), Default::default());
        db
    }
}

/// This impl tells salsa where to find the salsa runtime.
impl salsa::Database for RootDatabase {
    fn salsa_runtime(&self) -> &salsa::Runtime<RootDatabase> {
        &self.runtime
    }
}

impl ParallelDatabase for RootDatabase {
    fn snapshot(&self) -> Snapshot<Self> {
        Snapshot::new(RootDatabase {
            runtime: self.runtime.snapshot(self),
            fetcher: self.fetcher.clone(),
        })
    }
}

impl HasFetcher for RootDatabase {
    fn get_fetcher(&self) -> Arc<LocaleFetcher> {
        self.fetcher.clone()
    }
}

impl RootDatabase {
    pub fn set_references(&mut self, json_str: &str) -> Result<(), serde_json::error::Error> {
        let refs: Vec<Reference> = serde_json::from_str(json_str)?;
        let keys: HashSet<Atom> = refs.iter().map(|r| r.id.clone()).collect();
        for r in refs {
            self.set_reference_input(r.id.clone(), Arc::new(r));
        }
        self.set_all_keys((), Arc::new(keys));
        Ok(())
    }
    pub fn init_clusters(&mut self, clusters: &[Cluster<Pandoc>]) {
        let mut cluster_ids = Vec::new();
        for cluster in clusters {
            let mut ids = Vec::new();
            for cite in cluster.cites.iter() {
                ids.push(cite.id);
                self.set_cite(cite.id, Arc::new(cite.clone()));
            }
            self.set_cluster_cites(cluster.id, Arc::new(ids));
            cluster_ids.push(cluster.id);
        }
        self.set_cluster_ids((), Arc::new(cluster_ids));
    }

    // cluster_ids is maintained manually
    // the cluster_cites relation is maintained manually

    pub fn delete_cluster(&mut self, id: ClusterId) {
        // delete associated cites
        // self.set_cluster_cites(id, Arc::new(Vec::new()));
        // let new = self
        //     .cluster_ids(())
        //     .iter()
        //     .filter(|i| **i != id)
        //     .cloned()
        //     .collect();
        // self.set_cluster_ids((), Arc::new(new));
    }

    pub fn insert_cluster(&mut self, cluster: Cluster<Pandoc>, before: Option<ClusterId>) {}

    pub fn replace_cluster(&mut self, cluster: Cluster<Pandoc>) {}
}
