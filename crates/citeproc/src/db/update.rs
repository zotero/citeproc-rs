// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

#![allow(dead_code)]

use citeproc_io::output::{markup::Markup, OutputFormat};
use citeproc_io::ClusterId;
use citeproc_proc::db::IrDatabase;
use std::sync::Arc;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum DocUpdate {
    // We recomputed a cluster -- it now needs to be re-rendered
    Cluster(ClusterId),
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SecondFieldAlign {
    Flush,
    Margin,
}

/// Mostly imitates the citeproc-js API.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BibliographyMeta<O: OutputFormat = Markup> {
    pub max_offset: u32,
    /// Represents line spacing between entries
    pub entry_spacing: u32,
    /// Represents line spacing within entries
    pub line_spacing: u32,
    /// Whether hanging-indent should be applied
    pub hanging_indent: bool,

    // XXX: the CSL spec does a bad job explaining how to implement this.
    /// When the second-field-align CSL option is set, this returns either “flush” or “margin”.
    /// The calling application should align text in bibliography output as described in the CSL specification.
    /// Where second-field-align is not set, this is undefined.
    pub second_field_align: Option<SecondFieldAlign>,

    /// Contains information along the lines of citeproc-js' `bibstart` and `bibend` strings for
    /// open and close tags
    pub format_meta: O::BibMeta,
}

use csl::Atom;
use fnv::FnvHashMap;

#[derive(Clone, Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BibliographyUpdate<O: OutputFormat = Markup> {
    /// Contains Reference Ids mapped to their bibliography outputs
    pub updated_entries: FnvHashMap<Atom, Arc<O::Output>>,
    /// None if the sort is the same, otherwise contains all entries in order
    /// Entries that cease to be present in the list between updates are considered to have been removed.
    pub entry_ids: Option<Vec<Atom>>,
}

impl BibliographyUpdate {
    pub fn new() -> Self {
        BibliographyUpdate::default()
    }
}

#[derive(Default, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSummary<O: OutputFormat = Markup> {
    /// A list of clusters that were updated, paired with the formatted output for each
    pub clusters: Vec<(ClusterId, Arc<O::Output>)>,
    pub bibliography: Option<BibliographyUpdate>,
}

impl UpdateSummary {
    pub fn summarize(db: &impl IrDatabase, updates: &[DocUpdate]) -> Self {
        let ids = updates.iter().map(|&u| match u {
            DocUpdate::Cluster(x) => x,
        });
        let mut set = fnv::FnvHashSet::default();
        for id in ids {
            set.insert(id);
        }
        let mut clusters = Vec::with_capacity(set.len());
        for id in set {
            clusters.push((id, db.built_cluster(id)));
        }
        UpdateSummary {
            clusters,
            bibliography: None,
        }
    }
}
