// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

#[macro_use]
extern crate serde_derive;

pub use citeproc;
pub use citeproc_proc;

use citeproc::prelude::string_id::Cluster as ClusterStr;
use citeproc::prelude::*;
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

pub struct TestCase {
    pub mode: Mode,
    pub format: Format,
    pub csl_features: Option<csl::Features>,
    pub bibliography_no_sort: bool,
    pub csl: String,
    pub input: Vec<Reference>,
    pub result: String,
    pub clusters: Option<Vec<Cluster>>,
    pub process_citation_clusters: Option<Vec<CiteprocJsInstruction>>,
    pub processor: Processor,
}

impl Clone for TestCase {
    fn clone(&self) -> Self {
        let mut processor = {
            let fet = Arc::new(Filesystem::project_dirs());
            Processor::new(InitOptions {
                style: &self.csl,
                fetcher: Some(fet),
                format: self.format.0,
                test_mode: true,
                bibliography_no_sort: self.bibliography_no_sort,
                ..Default::default()
            })
            .expect("could not construct processor")
        };
        processor.reset_references(self.input.clone());
        Warmup::maximum().execute(&mut processor);
        TestCase {
            processor,
            mode: self.mode.clone(),
            csl_features: self.csl_features.clone(),
            format: self.format.clone(),
            bibliography_no_sort: self.bibliography_no_sort,
            csl: self.csl.clone(),
            input: self.input.clone(),
            result: self.result.clone(),
            clusters: self.clusters.clone(),
            process_citation_clusters: self.process_citation_clusters.clone(),
        }
    }
}

impl TestCase {
    pub fn new(
        mode: Mode,
        format: Format,
        csl_features: Option<csl::Features>,
        bibliography_no_sort: bool,
        csl: String,
        input: Vec<Reference>,
        result: String,
        clusters: Option<Vec<ClusterStr>>,
        process_citation_clusters: Option<Vec<CiteprocJsInstruction>>,
    ) -> Self {
        let mut processor = {
            let fet = Arc::new(Filesystem::project_dirs());
            Processor::new(InitOptions {
                style: &csl,
                fetcher: Some(fet),
                csl_features: csl_features.clone(),
                format: format.0,
                test_mode: true,
                bibliography_no_sort,
                ..Default::default()
            })
            .expect("could not construct processor")
        };
        let clusters = clusters.map(|vec| {
            vec.into_iter()
                .map(|str_cluster| Cluster {
                    id: processor.new_cluster(&str_cluster.id),
                    cites: str_cluster.cites,
                    mode: str_cluster.mode,
                })
                .collect()
        });
        processor.reset_references(input.clone());
        Warmup::maximum().execute(&mut processor);
        TestCase {
            mode,
            format,
            bibliography_no_sort,
            csl,
            csl_features,
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
            if self.mode == Mode::Citation {
                self.result.push_str("\n");
            }
            let mut executor = JsExecutor::new(&mut self.processor);
            executor.execute(instructions);
            let actual = executor.get_results();
            use std::str::FromStr;
            match self.mode {
                Mode::Citation => {
                    let desired = Results::from_str(&self.result).unwrap();
                    self.result = desired.output_independent();
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
                    cites.push(Cite::basic(&*refr.id));
                }
                clusters_auto.push(Cluster {
                    id: self.processor.random_cluster_id(),
                    cites,
                    mode: None,
                });
                &clusters_auto
            };

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
                if let Some(html) = self.processor.get_cluster(cluster.id) {
                    if pushed {
                        res.push_str("\n");
                    }
                    res.push_str(&*html);
                    pushed = true;
                }
            }
            match self.mode {
                Mode::Citation => {
                    // Because citeproc-rs is a bit keen to escape things
                    // Slashes are fine if they're not next to angle braces
                    // let's hope they're not
                    Some(normalise_html(&res))
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
                string.push_str(&entry.value);
                string.push_str("</div>");
            }
            _ => {
                string.push_str(&entry.value);
            }
        }
    }
    string.push_str("\n</div>\n");
    normalise_html(&string)
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

#[derive(Default)]
pub struct Warmup {
    default_locale: bool,
    _other_locales: Vec<Lang>,
    ref_dfa: bool,
}

impl Warmup {
    pub fn maximum() -> Self {
        Warmup {
            default_locale: true,
            _other_locales: vec![],
            ref_dfa: true,
        }
    }
    pub fn execute(&self, proc: &mut Processor) {
        if self.default_locale {
            proc.default_locale();
        }
        if self.ref_dfa {
            // Precompute dfas
            // We don't know what 'cited_keys()' is yet, so just do all of them
            for k in proc.all_keys().iter() {
                let _dfa = proc
                    .ref_dfa(k.clone())
                    .expect("cited_keys should all exist");
            }
        }
    }
}

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}

pub fn normalise_html(strg: &str) -> String {
    let rep = strg
        .replace("&#x2f;", "/")
        .replace("&#x27;", "'")
        .replace("&#60;", "&lt;")
        .replace("&#62;", "&gt;")
        .replace("&quot;", "\"")
        // citeproc-js uses the #38 version
        .replace("&#38;", "&amp;")
        // citeproc-js puts successive unicode superscript transforms in their own tags,
        // citeproc-rs joins them.
        .replace("</sup><sup>", "");
    let newlines = regex!(r"(?m)>\n*\s*<(/?)div");
    let mut rep = newlines.replace_all(&rep, ">\n<${1}div").into_owned();
    rep.truncate(rep.trim_end().trim_end_matches('\n').len());
    rep
}

#[test]
fn test_normalise() {
    assert_eq!(normalise_html("<div>\n  <div>"), "<div><div>".to_owned());
}
