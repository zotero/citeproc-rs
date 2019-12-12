// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

#[macro_use]
extern crate serde_derive;

pub use citeproc as citeproc;

use citeproc::prelude::*;
use citeproc_io::{Cite, Cluster, Reference};
use csl::Lang;

use directories::ProjectDirs;

use serde::{Deserialize, Deserializer};

use std::path::PathBuf;

use std::sync::Arc;

pub mod humans;
// pub mod toml;
pub mod yaml;

use humans::{CiteprocJsInstruction, JsExecutor, Results};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Mode {
    Citation,
    Bibliography,
}
impl Default for Mode {
    fn default() -> Self {
        Mode::Citation
    }
}

impl<'de> Deserialize<'de> for Mode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "citation" => Mode::Citation,
            "bibliography" => Mode::Bibliography,
            _ => panic!("unrecognized test mode"),
        })
    }
}

#[derive(Deserialize, Copy, Clone, Debug, PartialEq)]
pub struct Format(SupportedFormat);
impl Default for Format {
    fn default() -> Self {
        Format(SupportedFormat::TestHtml)
    }
}

#[derive(Deserialize)]
pub struct TestCase {
    pub mode: Mode,
    #[serde(default)]
    pub format: Format,
    pub csl: String,
    pub input: Vec<Reference>,
    pub result: String,
    pub clusters: Option<Vec<Cluster<Markup>>>,
    pub process_citation_clusters: Option<Vec<CiteprocJsInstruction>>,
    #[serde(skip, default = "unreach")]
    pub processor: Processor,
}

fn unreach() -> Processor {
    unreachable!()
}

impl TestCase {
    pub fn new(
        mode: Mode,
        format: Format,
        csl: String,
        input: Vec<Reference>,
        result: String,
        clusters: Option<Vec<Cluster<Markup>>>,
        process_citation_clusters: Option<Vec<CiteprocJsInstruction>>,
    ) -> Self {
        let processor = {
            let fet = Arc::new(Filesystem::project_dirs());
            Processor::new(&csl, fet, true, format.0).expect("could not construct processor")
        };
        TestCase {
            mode,
            format,
            csl,
            input,
            result,
            clusters,
            process_citation_clusters,
            processor,
        }
    }
    pub fn execute(&mut self) -> Option<String> {
        let mut res = String::new();
        if let Some(ref instructions) = &self.process_citation_clusters {
            self.result.push_str("\n");
            self.processor.set_references(self.input.clone());
            let mut executor = JsExecutor::new(&mut self.processor);
            for instruction in instructions.iter() {
                executor.execute(instruction);
            }
            use std::str::FromStr;
            match self.mode {
                Mode::Citation => {
                    let desired = Results::from_str(&self.result).unwrap();
                    self.result = desired.output_independent();
                    let actual = executor.get_results();
                    Some(actual.output_independent())
                }
                Mode::Bibliography => Some(get_bib_string(&self.processor)),
            }
        // turns out it's easier to just produce the string the same way
        } else {
            let mut clusters_auto = Vec::new();
            let clusters = if let Some(ref clusters) = &self.clusters {
                clusters
            } else {
                let mut cites = Vec::new();
                // TODO: assemble cites/clusters the other few available ways
                for refr in self.input.iter() {
                    cites.push(Cite::basic(&refr.id));
                }
                clusters_auto.push(Cluster { id: 1, cites });
                &clusters_auto
            };

            self.processor.set_references(self.input.clone());
            self.processor.init_clusters(clusters.clone());
            let positions: Vec<_> = clusters
                .iter()
                .enumerate()
                .map(|(ix, cluster)| ClusterPosition {
                    id: cluster.id,
                    note: Some(ix as u32 + 1),
                })
                .collect();

            self.processor.set_cluster_order(&positions).unwrap();
            let mut pushed = false;
            for cluster in clusters.iter() {
                if let Some(html) = self.processor.get_cluster(cluster.id()) {
                    if pushed {
                        res.push_str("\n");
                    }
                    res.push_str(&*html);
                    pushed = true;
                }
            }
            match self.mode {
                Mode::Citation => {
                    if self.result == "[CSL STYLE ERROR: reference with no printed form.]" {
                        self.result = String::new()
                    }
                    // Because citeproc-rs is a bit keen to escape things
                    // Slashes are fine if they're not next to angle braces
                    // let's hope they're not
                    Some(
                        res.replace("&#x2f;", "/")
                            // citeproc-js uses the #38 version
                            .replace("&amp;", "&#38;"),
                    )
                }
                Mode::Bibliography => Some(get_bib_string(&self.processor)),
            }
        }
    }
}

fn get_bib_string(proc: &Processor) -> String {
    let bib = proc.get_bibliography();
    let fmt = &proc.formatter;
    let mut string = String::new();
    string.push_str("<div class=\"csl-bib-body\">");
    for entry in bib {
        string.push('\n');
        match fmt {
            Markup::Html(_) => {
                string.push_str("  <div class=\"csl-entry\">");
                string.push_str(&entry);
                string.push_str("</div>");
            }
            _ => {
                string.push_str(&entry);
            }
        }
    }
    string.push_str("\n</div>");
    string
}

struct Filesystem {
    root: PathBuf,
}

impl Filesystem {
    fn new(repo_dir: impl Into<PathBuf>) -> Self {
        Filesystem {
            root: repo_dir.into(),
        }
    }
    fn project_dirs() -> Self {
        let pd = ProjectDirs::from("net", "cormacrelf", "citeproc-rs")
            .expect("No home directory found.");
        let mut locales_dir = pd.cache_dir().to_owned();
        locales_dir.push("locales");
        Self::new(locales_dir)
    }
}

use std::{fs, io};

impl LocaleFetcher for Filesystem {
    fn fetch_string(&self, lang: &Lang) -> Result<Option<String>, LocaleFetchError> {
        let mut path = self.root.clone();
        path.push(&format!("locales-{}.xml", lang));
        let read = fs::read_to_string(path);
        match read {
            Ok(string) => Ok(Some(string)),
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => Ok(None),
                _ => Err(LocaleFetchError::Io(e)),
            },
        }
    }
}
