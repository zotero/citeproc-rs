// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

#![allow(dead_code)]

use super::Processor;
use citeproc_io::output::{markup::Markup, OutputFormat};
use citeproc_io::ClusterId;
use citeproc_proc::db::IrDatabase;
use std::str::FromStr;
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
    pub fn summarize(db: &Processor, updates: &[DocUpdate]) -> Self {
        let ids = updates.iter().map(|&u| match u {
            DocUpdate::Cluster(x) => x,
        });
        let mut set = fnv::FnvHashSet::default();
        for id in ids {
            set.insert(id);
        }
        let mut clusters = Vec::with_capacity(set.len());
        for id in set {
            if id == 0 {
                // It's a dummy value for preview_citation_cluster.
                continue;
            }
            if let Some(output) = db.get_cluster(id) {
                clusters.push((id, output));
            }
        }
        UpdateSummary {
            clusters,
            bibliography: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, Ord, PartialOrd, PartialEq)]
pub enum IncludeUncited {
    /// The default
    None,
    /// All references, cited or not, are included in the bibliography.
    All,
    /// Specifically these references are included in the bibliography whether cited or not.
    Specific(Vec<String>),
}

impl Default for IncludeUncited {
    fn default() -> Self {
        IncludeUncited::None
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SupportedFormat {
    Html,
    Rtf,
    Plain,
    TestHtml,
}

impl FromStr for SupportedFormat {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "html" => Ok(SupportedFormat::Html),
            "rtf" => Ok(SupportedFormat::Rtf),
            "plain" => Ok(SupportedFormat::Plain),
            _ => Err(()),
        }
    }
}

impl<'de> serde::de::Deserialize<'de> for SupportedFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        use serde::de::Error as DeError;
        SupportedFormat::from_str(s.as_str())
            .map_err(|()| DeError::custom(format!("unknown format {}", s.as_str())))
    }
}
