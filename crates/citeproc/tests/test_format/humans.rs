// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use crate::test_format::yaml::TestInitOptions;

use super::{Mode, TestCase};

use citeproc::prelude::*;
use citeproc::string_id::Cluster as ClusterStr;
use citeproc_io::{
    cite_compat_vec, output::markup::Markup, Cite, ClusterMode, Reference, SmartString,
};

use core::fmt::Write;
use lazy_static::lazy_static;
use serde::{Deserialize, Deserializer};
use std::mem;
use std::str::FromStr;

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum CompatCitationItem {
    Array(#[serde(with = "cite_compat_vec")] Vec<Cite<Markup>>),
    Map {
        #[serde(with = "cite_compat_vec")]
        cites: Vec<Cite<Markup>>,
        #[serde(
            flatten,
            default,
            deserialize_with = "ClusterMode::compat_opt",
            skip_serializing_if = "Option::is_none"
        )]
        mode: Option<ClusterMode>,
    },
}

impl CompatCitationItem {
    pub fn to_note_cluster(self, index: u32) -> ClusterStr<Markup> {
        let (v, mode) = match self {
            CompatCitationItem::Array(v) => (v, None),
            CompatCitationItem::Map { cites, mode } => (cites, mode),
        };
        ClusterStr {
            id: index.to_string().into(),
            cites: v,
            mode,
        }
    }
}

#[derive(Debug, PartialEq)]
enum ResultKind {
    Dots,
    Arrows,
}
#[derive(Debug, PartialEq)]
pub struct CiteResult {
    kind: ResultKind,
    // id: u32,
    note: ClusterNumber,
    text: String,
}
#[derive(Debug, PartialEq)]
pub struct Results(pub Vec<CiteResult>);

impl Results {
    pub fn output_independent(&self, options: &TestInitOptions) -> String {
        let mut output = String::new();
        for (n, res) in self.0.iter().enumerate() {
            // Whether or not something is recomputed is not part of the CSL spec. We will simply
            // ignore this.
            // output.push_str(if res.kind == ResultKind::Arrows {
            //     ">>"
            // } else {
            //     ".."
            // });
            output.push_str("[");
            write!(&mut output, "{}", n).unwrap();
            output.push_str("] ");
            output.push_str(&super::normalise_html(&res.text, options));
            output.push_str("\n");
        }
        output
    }
}

impl FromStr for Results {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use nom::{
            branch::alt,
            bytes::complete::{tag, take_until},
            character::complete::{char, digit1},
            combinator::map,
            multi::separated_list1,
            sequence::{delimited, preceded, tuple},
            IResult,
        };
        fn dots(inp: &str) -> IResult<&str, ResultKind> {
            map(alt((tag(".."), tag(">>"))), |s| match s {
                ".." => ResultKind::Dots,
                ">>" => ResultKind::Arrows,
                _ => unreachable!(),
            })(inp)
        }
        fn num(inp: &str) -> IResult<&str, u32> {
            map(delimited(char('['), digit1, char(']')), |ds: &str| {
                u32::from_str(ds).unwrap()
            })(inp)
        }
        fn formatted(inp: &str) -> IResult<&str, &str> {
            preceded(char(' '), take_until("\n"))(inp)
        }
        fn total(inp: &str) -> IResult<&str, CiteResult> {
            map(tuple((dots, num, formatted)), |(k, n, f)| CiteResult {
                kind: k,
                // id: n,
                // incorrect, but we don't actually know except by looking at the instructions what
                // the right note number is
                note: ClusterNumber::Note(IntraNote::Single(n)),
                text: f.to_string(),
            })(inp)
        }
        fn whole_thing(inp: &str) -> IResult<&str, Vec<CiteResult>> {
            separated_list1(char('\n'), total)(inp)
        }
        Ok(Results(whole_thing(s).unwrap().1))
    }
}

pub enum InstructionMode {
    Composite,
    AuthorOnly,
    SuppressAuthor,
}

impl<'de> Deserialize<'de> for InstructionMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "author-only" => InstructionMode::AuthorOnly,
            "composite" => InstructionMode::Composite,
            "suppress-author" => InstructionMode::SuppressAuthor,
            _ => panic!("unrecognized instruction mode"),
        })
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
struct Properties {
    #[serde(rename = "noteIndex", alias = "note")]
    note_index: u32,
    #[serde(
        flatten,
        default,
        deserialize_with = "ClusterMode::compat_opt",
        skip_serializing_if = "Option::is_none"
    )]
    mode: Option<ClusterMode>,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct ClusterInstruction {
    #[serde(rename = "citationID", alias = "id")]
    cluster_id: SmartString,
    #[serde(rename = "citationItems", alias = "cites", with = "cite_compat_vec")]
    citation_items: Vec<Cite<Markup>>,
    properties: Properties,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct PrePost(SmartString, u32);

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct CiteprocJsInstruction {
    cluster: ClusterInstruction,
    pre: Vec<PrePost>,
    post: Vec<PrePost>,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[serde(untagged)]
pub enum Instruction2 {
    Map(CiteprocJsInstruction),
    Array(ClusterInstruction, Vec<PrePost>, Vec<PrePost>),
}

impl From<Instruction2> for CiteprocJsInstruction {
    fn from(other: Instruction2) -> Self {
        match other {
            Instruction2::Map(i) => i,
            Instruction2::Array(cluster, pre, post) => CiteprocJsInstruction { cluster, pre, post },
        }
    }
}

use std::collections::HashMap;

pub struct JsExecutor<'a> {
    current_note_numbers: HashMap<ClusterId, ClusterNumber>,
    proc: &'a mut Processor,
}

impl JsExecutor<'_> {
    pub fn new<'a>(proc: &'a mut Processor) -> JsExecutor<'a> {
        JsExecutor {
            current_note_numbers: HashMap::new(),
            proc,
        }
    }
    fn get_id(&mut self, string_id: &str) -> ClusterId {
        self.proc.cluster_id(string_id)
    }

    pub fn get_results(&self) -> Results {
        let updates = self.proc.batched_updates();
        let mut mod_clusters = HashMap::new();
        let mut results = Vec::<CiteResult>::new();
        for (id, text) in updates.clusters {
            mod_clusters.insert(id, true);
            let &note = self.current_note_numbers.get(&id).unwrap();
            let text = (*text).clone();
            results.push(CiteResult {
                kind: ResultKind::Arrows,
                // id,
                note,
                text: text.to_string(),
            })
        }
        // for &id in self.current_note_numbers.keys() {
        //     if mod_clusters.contains_key(&id) {
        //         continue;
        //     }
        //     let &note = self.current_note_numbers.get(&id).unwrap();
        //     if let Some(text) = self.proc.get_cluster(id) {
        //         results.push(CiteResult {
        //             kind: ResultKind::Dots,
        //             id,
        //             note,
        //             text: super::normalise_html(&text),
        //         })
        //     }
        // }
        results.sort_by_key(|x| x.note);
        Results(results)
    }

    fn to_renumbering(&mut self, renum: &mut Vec<ClusterPosition>, prepost: &[PrePost]) {
        for &PrePost(ref string_id, note_number) in prepost.iter() {
            let id = self.get_id(string_id);
            let note = if note_number == 0 {
                None
            } else {
                Some(note_number)
            };
            renum.push(ClusterPosition { id: Some(id), note })
        }
    }

    pub fn execute(&mut self, instructions: &[CiteprocJsInstruction]) {
        self.proc.drain();
        let mut renum = Vec::new();
        for CiteprocJsInstruction { cluster, pre, post } in instructions {
            let ClusterInstruction {
                cluster_id,
                citation_items,
                properties,
            } = cluster;
            let Properties { mode, note_index } = properties;

            renum.clear();
            self.to_renumbering(&mut renum, pre);
            self.to_renumbering(
                &mut renum,
                &[PrePost(cluster.cluster_id.clone(), *note_index)],
            );
            self.to_renumbering(&mut renum, post);
            self.proc.insert_cluster_str(ClusterStr {
                id: cluster_id.clone(),
                mode: mode.clone(),
                cites: citation_items.to_vec(),
            });
            self.proc.set_cluster_order(&renum).unwrap();
            for &ClusterPosition { id, .. } in &renum {
                if let Some(id) = id {
                    if let Some(actual_note) = self.proc.get_cluster_note_number(id) {
                        self.current_note_numbers.insert(id, actual_note);
                    }
                }
            }
        }
    }
}

enum Chunk {
    // Required sections
    Mode(String),

    /// Interpretation depends on which mode you're using
    ///
    /// https://github.com/citation-style-language/test-suite#result
    Result(String),

    /// XML CSL style
    ///
    /// https://github.com/citation-style-language/test-suite#csl
    Csl(String),

    /// JSON Reference[] list
    ///
    /// https://github.com/citation-style-language/test-suite#input
    Input(String),

    // Optional sections
    /// JSON LIST of LISTS of bibliography entries as item IDs
    ///
    /// https://github.com/citation-style-language/test-suite#bibentries
    BibEntries(String),
    /// JSON input to bibliography mode for limiting bib output
    ///
    /// https://github.com/citation-style-language/test-suite#bibsection
    BibSection(String),
    /// JSON list of lists of cites (ie Cluster[].map(cl => cl.cites))
    ///
    /// https://github.com/citation-style-language/test-suite#citation-items
    CitationItems(String),
    /// JSON list of lists of objects that represent calls to processCitationCluster
    ///
    /// https://github.com/citation-style-language/test-suite#citations
    Citations(String),
}

// fn format_human_test(test_case: &TestCase) -> String {
//     let mut out = String::new();
//     out += ">>===== MODE =====>>";
//     out += match test_case.mode {
//         Mode::Citation => "citation",
//         Mode::Bibliography => "bibliography",
//     };
//     out += "<<===== MODE =====<<";
//     out += ">>===== INPUT =====>>";
//     // out += &serde_json::to_string_pretty(&test_case.input).unwrap();
//     out += "<<===== INPUT =====<<";
//     out
// }

pub fn parse_human_test(contents: &str, csl_features: Option<csl::Features>) -> TestCase {
    use regex::Regex;
    lazy_static! {
        static ref BEGIN: Regex = Regex::new(r">>=+ ([A-Z\-]+) =+>>").unwrap();
    }
    lazy_static! {
        static ref END: Regex = Regex::new(r"<<=+ ([A-Z\-]+) =+<<").unwrap();
    }
    let mut state = None;
    let mut chunks = vec![];
    // some of the files use two or four equals signs, most use five.
    for line in contents.lines() {
        if END.is_match(line) {
            if state.is_some() {
                let mut chunk = None;
                mem::swap(&mut state, &mut chunk);
                chunks.push(chunk.unwrap());
            }
        } else if let Some(caps) = BEGIN.captures(line) {
            state = match caps.get(1).unwrap().as_str() {
                "MODE" => Some(Chunk::Mode(String::new())),
                "CSL" => Some(Chunk::Csl(String::new())),
                "INPUT" => Some(Chunk::Input(String::new())),
                "RESULT" => Some(Chunk::Result(String::new())),
                "BIBENTRIES" => Some(Chunk::BibEntries(String::new())),
                "BIBSECTION" => Some(Chunk::BibSection(String::new())),
                "CITATION-ITEMS" => Some(Chunk::CitationItems(String::new())),
                "CITATIONS" => Some(Chunk::Citations(String::new())),
                x => panic!("unrecognized block: {}", x),
            }
        } else {
            if let Some(ref mut state) = state {
                match state {
                    Chunk::Mode(ref mut s)
                    | Chunk::Csl(ref mut s)
                    | Chunk::Input(ref mut s)
                    | Chunk::Result(ref mut s)
                    | Chunk::BibSection(ref mut s)
                    | Chunk::BibEntries(ref mut s)
                    | Chunk::CitationItems(ref mut s)
                    | Chunk::Citations(ref mut s) => {
                        if !s.is_empty() {
                            s.push_str("\n");
                        }
                        s.push_str(line);
                    }
                }
            }
            // otherwise, it's a comment
        }
    }

    let mut mode = None;
    let mut csl = None;
    let mut input: Option<Vec<Reference>> = None;
    let mut result = None;

    // TODO
    let mut bib_entries = None;
    let mut bib_section = None;
    let mut citation_items = None;
    let mut process_citation_clusters: Option<Vec<Instruction2>> = None;

    for chunk in chunks {
        match chunk {
            Chunk::Mode(m) => {
                mode = mode.or_else(|| match m.as_str() {
                    "citation" => Some((Mode::Citation, SupportedFormat::Html, false)),
                    "bibliography" => Some((Mode::Bibliography, SupportedFormat::Html, false)),
                    "bibliography-nosort" => {
                        Some((Mode::Bibliography, SupportedFormat::Html, true))
                    }
                    "citation-rtf" => Some((Mode::Citation, SupportedFormat::Rtf, false)),
                    "bibliography-rtf" => Some((Mode::Bibliography, SupportedFormat::Rtf, false)),
                    _ => panic!("unknown mode {}", m),
                })
            }
            Chunk::Csl(s) => csl = csl.or_else(|| Some(s)),
            Chunk::Input(s) => {
                input = input.or_else(|| {
                    Some(
                        serde_json::from_str(&s)
                            .expect("could not parse references in INPUT section"),
                    )
                })
            }
            Chunk::Result(s) => result = result.or_else(|| Some(s)),
            Chunk::BibEntries(s) => bib_entries = bib_entries.or_else(|| Some(s)),
            Chunk::BibSection(s) => bib_section = bib_section.or_else(|| Some(s)),
            Chunk::CitationItems(s) => {
                citation_items = citation_items.or_else(|| {
                    Some(serde_json::from_str(&s).expect("could not parse CITATION-ITEMS"))
                })
            }
            Chunk::Citations(s) => {
                process_citation_clusters = process_citation_clusters
                    .or_else(|| Some(serde_json::from_str(&s).expect("could not parse CITATIONS")))
            }
        }
    }

    let options = TestInitOptions {
        format: mode.map(|(_, f, _)| f).unwrap_or(SupportedFormat::Html),
        format_options: FormatOptions {
            // disable these for txt format tests
            link_anchors: false,
        },
        csl_features,
        bibliography_no_sort: mode.map_or(false, |(_, _, nosort)| nosort),
        locale_override: None,
        normalise: true,
    };

    let norm_result = result
        .map(|x| super::normalise_html(&x, &options))
        .expect("test case without a RESULT section");

    TestCase::new(
        mode.map(|(m, _, _)| m).unwrap_or(Mode::Citation),
        options,
        csl.expect("test case without a CSL section"),
        input.expect("test case without an INPUT section"),
        norm_result,
        citation_items.map(|items: Vec<CompatCitationItem>| {
            items
                .into_iter()
                .enumerate()
                .map(|(n, c_item): (usize, CompatCitationItem)| {
                    c_item.to_note_cluster(n as u32 + 1u32)
                })
                .collect()
        }),
        process_citation_clusters.map(|inst2s| {
            inst2s
                .into_iter()
                .map(|x| CiteprocJsInstruction::from(x))
                .collect()
        }),
    )
}
