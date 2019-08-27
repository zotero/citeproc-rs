// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

#![allow(dead_code)]

use citeproc_proc::IrDatabase;
use citeproc_io::ClusterId;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum DocUpdate {
    // We recomputed a cluster -- it now needs to be re-rendered
    Cluster(ClusterId),
    // Update a bibliography entry at an index
    BibEntry(u32),
    WholeBibliography,
}

use citeproc_io::output::{html::Html, OutputFormat};
use std::sync::Arc;

#[derive(Default, Debug, Clone, Eq, PartialEq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSummary<O: OutputFormat = Html> {
    // A list of clusters that were updated, paired with the formatted output for each
    pub clusters: Vec<(ClusterId, Arc<O::Output>)>,
    // Not sure if this way is better than DocUpdate::BibEntry(id)
    // pub bib_entries: Vec<(u32, Arc<O::Output>)>,
}

impl UpdateSummary {
    pub fn summarize(db: &impl IrDatabase, updates: &[DocUpdate]) -> Self {
        let ids = updates.iter().filter_map(|&u| match u {
            DocUpdate::Cluster(x) => Some(x),
            _ => None,
        });
        let mut set = fnv::FnvHashSet::default();
        for id in ids {
            set.insert(id);
        }
        let mut clusters = Vec::with_capacity(set.len());
        for id in set {
            clusters.push((id, db.built_cluster(id)));
        }
        UpdateSummary { clusters }
    }
}
