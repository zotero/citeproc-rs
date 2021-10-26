// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::humans::{CiteprocJsInstruction, CompatCitationItem};
use super::{Mode, TestCase};
use anyhow::Error;
use citeproc::{FormatOptions, SupportedFormat};
use citeproc_io::Reference;
use serde::Deserialize;

pub fn parse_yaml_test(s: &str) -> Result<TestCase, Error> {
    let yaml_test_case: YamlTestCase = serde_yaml::from_str(s)?;
    Ok(yaml_test_case.into())
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct YamlTestCase {
    pub mode: Mode,
    #[serde(default, flatten)]
    pub options: TestInitOptions,
    pub csl: String,
    pub input: Vec<Reference>,
    pub result: String,
    pub clusters: Option<Vec<CompatCitationItem>>,
    pub process_citation_clusters: Option<Vec<CiteprocJsInstruction>>,
}

fn bool_true() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(remote = "FormatOptions", rename_all = "kebab-case")]
struct KebabFormatOpts {
    #[serde(default = "bool_true")]
    link_anchors: bool,
}

#[derive(Debug, Deserialize, PartialEq, Default, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct TestInitOptions {
    #[serde(default)]
    pub csl_features: Option<csl::Features>,
    // Optional
    #[serde(default)]
    pub format: SupportedFormat,
    #[serde(default, with = "KebabFormatOpts")]
    pub format_options: FormatOptions,
    /// You might get this from a dependent style via `StyleMeta::parse(dependent_xml_string)`
    #[serde(default)]
    pub locale_override: Option<csl::Lang>,
    /// Disables sorting on the bibliography
    #[serde(default)]
    pub bibliography_no_sort: bool,

    // not in InitOptions, only for tests
    #[serde(default = "bool_true")]
    pub normalise: bool,
}

impl From<YamlTestCase> for TestCase {
    fn from(yaml: YamlTestCase) -> Self {
        TestCase::new(
            yaml.mode,
            yaml.options,
            yaml.csl,
            yaml.input,
            yaml.result,
            yaml.clusters.map(|cls| {
                cls.into_iter()
                    .enumerate()
                    .map(|(n, c_item)| c_item.to_note_cluster(n as u32 + 1u32))
                    .collect()
            }),
            yaml.process_citation_clusters,
        )
    }
}
