// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use citeproc::prelude::*;
use citeproc_io::{
    Cite, Cluster, ClusterId, ClusterNumber, IntraNote, Locator, NumericValue, Reference,
    Suppression,
};
use csl::Lang;
use csl::LocatorType;
use directories::ProjectDirs;
use lazy_static::lazy_static;
use std::mem;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, PartialEq)]
pub enum TomlFormat {
    Html,
    Rtf,
}

impl Default for TomlFormat {
    fn default() -> Self {
        TomlFormat::Html
    }
}

impl<'de> Deserialize<'de> for TomlFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "html" => TomlFormat::Html,
            "rtf" => TomlFormat::Rtf,
            x => panic!("unrecognized format, {}", x),
        })
    }
}

use super::{Filesystem, Mode};
use serde::de::{Deserialize, Deserializer};

#[derive(Deserialize, Default, Debug, PartialEq)]
pub struct TomlOptions {
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub format: TomlFormat,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct TomlStyle {
    pub csl: String,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct TomlLocales {}

#[derive(Deserialize, Debug, PartialEq)]
pub struct TomlResult {
    pub string: String,
}

use std::collections::HashMap;

#[derive(Deserialize, Debug, PartialEq)]
pub struct TomlInstruction {
    #[serde(default)]
    clusters: Vec<Cluster<Html>>,
    #[serde(default)]
    refs: Vec<Reference>,
    #[serde(default)]
    now: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct TomlTestCase {
    #[serde(default, rename = "test")]
    pub options: TomlOptions,
    pub style: TomlStyle,
    references: Vec<Reference>,
    result: Option<TomlResult>,
    instructions: Vec<TomlInstruction>,
    format: Option<SupportedFormat>,
}

impl TomlTestCase {
    pub fn execute(&mut self) -> String {
        if self.options.mode == Mode::Bibliography {
            panic!("bib tests not implemented");
        }
        let fet = Arc::new(Filesystem::project_dirs());
        let mut proc = Processor::new(InitOptions {
            style: &self.style.csl,
            fetcher: fet,
            format: self.format.unwrap_or(SupportedFormat::Html),
            test_mode: true,
            ..Default::default()
        })
        .expect("could not construct processor");

        proc.reset_references(self.references.clone());
        "".into()
        // Because citeproc-rs is a bit keen to escape things
        // Slashes are fine if they're not next to angle braces
        // let's hope they're not
        // res.replace("&#x2f;", "/")
    }
}
