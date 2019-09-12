// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::humans::{CitationItem, CiteprocJsInstruction};
use super::{Mode, TestCase};
use citeproc_io::{Cite, Cluster2, Reference};

#[derive(Deserialize, Debug, PartialEq)]
pub struct YamlTestCase {
    pub mode: Mode,
    pub csl: String,
    pub input: Vec<Reference>,
    pub result: String,
    pub clusters: Option<Vec<CitationItem>>,
    pub process_citation_clusters: Option<Vec<CiteprocJsInstruction>>,
}

impl From<YamlTestCase> for TestCase {
    fn from(yaml: YamlTestCase) -> Self {
        TestCase {
            mode: yaml.mode,
            csl: yaml.csl,
            input: yaml.input,
            result: yaml.result,
            clusters: yaml.clusters.map(|cls| {
                cls.into_iter()
                    .enumerate()
                    .map(|(n, c_item)| c_item.to_note_cluster(n as u32 + 1u32))
                    .collect()
            }),
            process_citation_clusters: yaml.process_citation_clusters,
        }
    }
}
