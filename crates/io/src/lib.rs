// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

//! This module describes non-CSL inputs to the processor. Specifically, it aims to implement
//! [CSL-JSON](https://citeproc-js.readthedocs.io/en/latest/csl-json/markup.html)
//! ([schema](https://github.com/citation-style-language/schema/blob/master/csl-data.json)).
//!
//! These are constructed at deserialization time, i.e. when a [Reference][]
//! is read from CSL-JSON. Any data types that contain strings from the original JSON are typically
//! done with `&'r str` borrows under the [Reference][]'s lifetime `'r`, so the original JSON string
//! cannot be thrown away, but a large number of string allocations is saved.
//!
//! [Reference]: struct.Reference.html

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate log;

mod cite;
mod csl_json;
mod date;
mod names;
mod numeric;
pub mod output;
mod reference;
pub(crate) mod unicode;
pub mod utils;

pub use csl_json::IdOrNumber;

pub use self::cite::*;
pub use self::date::*;
pub use self::names::*;
pub use self::numeric::*;
pub use self::reference::*;

use self::output::LocalizedQuotes;
use csl::TextCase;
use std::borrow::Cow;

use unic_segment::{Words, WordBounds, WordBoundIndices};
use crate::output::micro_html::MicroNode;

#[derive(Debug, Clone, Default)]
pub struct IngestOptions {
    pub replace_hyphens: bool,
    pub text_case: TextCase,
    pub quotes: LocalizedQuotes,
    pub strip_periods: bool,
}

/// https://stackoverflow.com/a/38406885
fn uppercase_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

// from the unic_segment example code
fn has_alphanumeric(s: &&str) -> bool {
    is_word(*s)
}
fn is_word(s: &str) -> bool {
    s.chars().any(|ch| ch.is_alphanumeric())
}

fn transform_first_char_of_word<F, I>(word: &str, f: F) -> Cow<'_, str>
    where F: Fn(char) -> I,
          I: Iterator<Item = char> + Clone,
{
    // Naively capitalizes without a stopword filter
    let mut len = word.len();
    let mut chars = word.chars();
    match chars.next() {
        None => Cow::Borrowed(word),
        Some(first) => {
            let tx = f(first);
            // Don't allocate for Already Capitalized Words
            if tx.clone().count() == 1 && tx.clone().nth(0) == Some(first) {
                return Cow::Borrowed(word);
            }
            let mut s = String::with_capacity(len);
            s.extend(tx);
            // Fast convert back from iterator which knows its own byte offset
            s.push_str(chars.as_str());
            Cow::Owned(s)
        },
    }
}

fn transform_uppercase_first(word: &str) -> Cow<'_, str> {
    transform_first_char_of_word(word, |c| c.to_uppercase())
}

fn transform_each_word(s: &str, transform: impl Fn(&str) -> Cow<'_, str>) -> String {
    let mut acc = String::with_capacity(s.len());
    for substr in WordBounds::new(s) {
        if is_word(substr) {
            let word = substr;
            let tx = transform(word);
            acc.push_str(&tx);
        } else {
            acc.push_str(substr);
        }
    }
    acc
}

fn transform_first_word<'a>(mut s: String, transform: impl Fn(&str) -> Cow<'_, str>) -> String {
    let mut bounds = WordBoundIndices::new(&s);
    let mut seen_word = false;
    while let Some((ix, bound)) = bounds.next() {
        if is_word(bound) && !seen_word {
            let tx = transform(bound);
            if tx != bound {
                let mut ret = String::with_capacity(s.len());
                ret.push_str(&s[..ix]);
                ret.push_str(&tx);
                if let Some((rest_ix, _)) = bounds.next() {
                    ret.push_str(&s[rest_ix..]);
                }
                return ret;
            } else {
                return s;
            }
        }
    }
    s
}

fn string_contains_word(s: &str) -> bool {
    Words::new(s, has_alphanumeric).next().is_some()
}

impl IngestOptions {
    pub fn plain<'s>(&self, s: &'s str) -> Cow<'s, str> {
        let mut cow = Cow::Borrowed(s);
        if self.replace_hyphens {
            cow = Cow::Owned(cow.replace('-', "\u{2013}"));
        }
        if self.strip_periods {
            cow = Cow::Owned(cow.replace('.', ""));
        }
        cow
    }
    pub fn apply_text_case(&self, micros: &mut [MicroNode]) {
        self.apply_text_case_inner(micros, false);
    }
    pub fn apply_text_case_inner(&self, micros: &mut [MicroNode], mut seen_one: bool) -> bool {
        let mut mine = false;
        for micro in micros.iter_mut() {
            // order or short-circuits matters
            match micro {
                MicroNode::Text(ref mut txt) => {
                    let text = std::mem::replace(txt, String::new());
                    *txt = self.transform_case(text, seen_one);
                    seen_one = string_contains_word(txt.as_ref()) || seen_one;
                }
                MicroNode::NoCase(children) => {
                    seen_one = seen_one || self.contains_word(children.as_ref());
                }
                MicroNode::Formatted(children, _) | MicroNode::Quoted{ children, .. } => {
                    seen_one = self.apply_text_case_inner(children.as_mut(), seen_one) || seen_one;
                }
            }
        }
        seen_one
    }
    fn contains_word(&self, micros: &[MicroNode]) -> bool {
        micros.iter().any(|m| match m {
            MicroNode::Text(txt) => string_contains_word(&txt),
            MicroNode::Formatted(children, _) | MicroNode::Quoted { children, .. } | MicroNode::NoCase(children) => {
                self.contains_word(children.as_ref())
            }
        })
    }
    fn transform_case(&self, s: String, seen_one: bool) -> String {
        match self.text_case {
            TextCase::Lowercase => s.to_lowercase(),
            TextCase::Uppercase => s.to_uppercase(),
            TextCase::CapitalizeFirst if !seen_one => {
                transform_first_word(s, transform_uppercase_first)
            }
            TextCase::CapitalizeAll => {
                transform_each_word(&s, transform_uppercase_first)
            }
            TextCase::Sentence => {
                // TODO: sentence case, but only do the initial capital if !seen_one
                transform_first_word(s, transform_uppercase_first)
            },
            TextCase::Title => {
                transform_each_word(&s, transform_uppercase_first)
            }
            TextCase::None | _ => s,
        }
    }
    pub fn default_with_quotes(quotes: LocalizedQuotes) -> Self {
        IngestOptions {
            quotes,
            ..Default::default()
        }
    }
}
