// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::{Format, Mode, TestCase};

use citeproc::prelude::*;
use citeproc_io::{
    Cite, Cluster, ClusterId, ClusterNumber, IntraNote, Locators, Reference, Suppression,
};

use lazy_static::lazy_static;
use std::mem;
use std::str::FromStr;

/// Techincally reference IDs are allowed to be numbers.
fn get_ref_id<'de, D>(d: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    use citeproc_io::NumberLike;
    let s = NumberLike::deserialize(d)?;
    Ok(s.into_string())
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum CitationItem {
    Array(Vec<CiteprocJsCite>),
    Map { cites: Vec<CiteprocJsCite> },
}

impl CitationItem {
    pub fn to_note_cluster(self, index: u32) -> Cluster<Markup> {
        let v = match self {
            CitationItem::Array(v) => v,
            CitationItem::Map { cites } => cites,
        };
        let cites = v.iter().map(CiteprocJsCite::to_cite).collect();
        Cluster { id: index, cites }
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct CiteprocJsCite {
    #[serde(deserialize_with = "get_ref_id")]
    id: String,

    #[serde(default, flatten)]
    locators: Option<Locators>,

    #[serde(default)]
    prefix: Option<String>,
    #[serde(default)]
    suffix: Option<String>,
    #[serde(default)]
    suppress_author: bool,
    #[serde(default)]
    author_only: bool,
}

impl CiteprocJsCite {
    fn to_cite(&self) -> Cite<Markup> {
        Cite {
            ref_id: csl::Atom::from(self.id.as_str()),
            prefix: self.prefix.clone(),
            suffix: self.suffix.clone(),
            locators: self.locators.clone(),
            suppression: match (self.suppress_author, self.author_only) {
                (false, true) => Some(Suppression::InText),
                (true, false) => Some(Suppression::Rest),
                (false, false) => None,
                _ => panic!("multiple citation modes passed to CiteprocJsCite"),
            },
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
    id: u32,
    note: ClusterNumber,
    text: String,
}
#[derive(Debug, PartialEq)]
pub struct Results(pub Vec<CiteResult>);

impl Results {
    pub fn output_independent(&self) -> String {
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
            output.push_str(&format!("{}", n));
            output.push_str("] ");
            output.push_str(&res.text);
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
            multi::separated_nonempty_list,
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
                id: n,
                // incorrect, but we don't actually know except by looking at the instructions what
                // the right note number is
                note: ClusterNumber::Note(IntraNote::Single(n)),
                text: crate::normalise_html(&f),
            })(inp)
        }
        fn whole_thing(inp: &str) -> IResult<&str, Vec<CiteResult>> {
            separated_nonempty_list(char('\n'), total)(inp)
        }
        Ok(Results(whole_thing(s).unwrap().1))
    }
}

use serde::de::{Deserialize, Deserializer};

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

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "mode", rename = "kebab-case")]
pub enum ModeProperties {
    Composite {
        #[serde(default)]
        infix: String,
    },
    AuthorOnly,
    SuppressAuthor,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
struct Properties {
    #[serde(rename = "noteIndex", alias = "note")]
    note_index: u32,
    #[serde(default, flatten)]
    mode: Option<ModeProperties>,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct ClusterInstruction {
    #[serde(rename = "citationID", alias = "id")]
    cluster_id: String,
    #[serde(rename = "citationItems", alias = "cites")]
    citation_items: Vec<CiteprocJsCite>,
    properties: Properties,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct PrePost(String, u32);

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
    cluster_ids_mapping: HashMap<String, ClusterId>,
    current_note_numbers: HashMap<ClusterId, ClusterNumber>,
    proc: &'a mut Processor,
    next_id: ClusterId,
}

impl JsExecutor<'_> {
    pub fn new<'a>(proc: &'a mut Processor) -> JsExecutor<'a> {
        JsExecutor {
            cluster_ids_mapping: HashMap::new(),
            current_note_numbers: HashMap::new(),
            proc,
            next_id: 0,
        }
    }
    fn get_id(&mut self, string_id: &str) -> ClusterId {
        if self.cluster_ids_mapping.contains_key(string_id) {
            return *self.cluster_ids_mapping.get(string_id).unwrap();
        } else {
            self.cluster_ids_mapping
                .insert(string_id.to_string(), self.next_id);
            let id = self.next_id;
            self.next_id += 1;
            return id;
        }
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
                id,
                note,
                text: crate::normalise_html(&text),
            })
        }
        for &id in self.current_note_numbers.keys() {
            if mod_clusters.contains_key(&id) {
                continue;
            }
            let &note = self.current_note_numbers.get(&id).unwrap();
            if let Some(text) = self.proc.get_cluster(id) {
                results.push(CiteResult {
                    kind: ResultKind::Dots,
                    id,
                    note,
                    text: crate::normalise_html(&text),
                })
            }
        }
        results.sort_by_key(|x| x.note);
        Results(results)
    }

    fn to_renumbering(&mut self, renum: &mut Vec<(ClusterId, ClusterNumber)>, prepost: &[PrePost]) {
        for &PrePost(ref string, note_number) in prepost.iter() {
            let id = self.get_id(&string);
            let count = renum
                .iter()
                .filter_map(|(_, n)| match n {
                    ClusterNumber::Note(IntraNote::Multi(x, x2)) if *x == note_number => Some(x2),
                    _ => None,
                })
                .count() as u32;
            let nn = ClusterNumber::Note(IntraNote::Multi(note_number, count));
            renum.push((id, nn))
        }
    }

    /// Note: this does not work very well. The way citeproc-js runs its own cannot easily be
    /// deciphered.
    pub fn execute(&mut self, instruction: &CiteprocJsInstruction) {
        self.proc.drain();

        let CiteprocJsInstruction { cluster, pre, post } = instruction;
        let id = self.get_id(&*cluster.cluster_id);
        let note = cluster.properties.note_index;

        let mut cites = Vec::new();
        for cite_item in cluster.citation_items.iter() {
            cites.push(cite_item.to_cite());
        }

        let mut renum = Vec::new();
        self.to_renumbering(&mut renum, pre);
        self.to_renumbering(&mut renum, &[PrePost(cluster.cluster_id.clone(), note)]);
        self.to_renumbering(&mut renum, post);
        self.proc.insert_cluster(Cluster { id, cites });
        self.proc.renumber_clusters(&renum);
        for (i, nn) in renum {
            self.current_note_numbers.insert(i, nn);
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

pub fn parse_human_test(contents: &str) -> TestCase {
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
                    "citation" => Some((Mode::Citation, SupportedFormat::TestHtml)),
                    "bibliography" => Some((Mode::Bibliography, SupportedFormat::TestHtml)),
                    "citation-rtf" => Some((Mode::Citation, SupportedFormat::Rtf)),
                    "bibliography-rtf" => Some((Mode::Bibliography, SupportedFormat::Rtf)),
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

    TestCase::new(
        mode.map(|(m, _)| m).unwrap_or(Mode::Citation),
        mode.map(|(_, f)| Format(f))
            .unwrap_or(Format(SupportedFormat::TestHtml)),
        csl.expect("test case without a CSL section"),
        input.expect("test case without an INPUT section"),
        result
            .map(|x| crate::normalise_html(&x))
            .expect("test case without a RESULT section"),
        citation_items.map(|items: Vec<CitationItem>| {
            items
                .into_iter()
                .enumerate()
                .map(|(n, c_item): (usize, CitationItem)| c_item.to_note_cluster(n as u32 + 1u32))
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
