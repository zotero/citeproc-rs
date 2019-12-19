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

use crate::output::markup::InlineElement;
use crate::output::micro_html::MicroNode;
use unic_segment::{WordBoundIndices, WordBounds, Words, Graphemes, GraphemeIndices};
use csl::{FontVariant, VerticalAlignment};

use phf::phf_set;

#[derive(Debug, Clone, Default)]
pub struct IngestOptions {
    pub replace_hyphens: bool,
    pub text_case: TextCase,
    pub quotes: LocalizedQuotes,
    pub strip_periods: bool,
    pub is_english: bool,
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
where
    F: Fn(char) -> I,
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
        }
    }
}

fn transform_uppercase_first(word: &str) -> Cow<'_, str> {
    transform_first_char_of_word(word, |c| c.to_uppercase())
}

static SPEC_STOPWORDS: phf::Set<&'static str> = phf_set! {
    "a",
    "an",
    "and",
    "as",
    "at",
    "but",
    "by",
    "down",
    "for",
    "from",
    "in",
    "into",
    "nor",
    "of",
    "on",
    "onto",
    "or",
    "over",
    "so",
    "the",
    "till",
    "to",
    "up",
    "via",
    "with",
    "yet",
};

static CITEPROC_JS_STOPWORD_REGEX: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
fn stopword_regex() -> &'static regex::Regex {
    let re = concat![
        // Match case insensitive (regex crate's simple case folding is fine)
        "(?i)",
        // Match the start only
        "^(?:",
        "about|",
        "above|",
        "across|",
        "afore|",
        "after|",
        "against|",
        "al|",
        "along|",
        "alongside|",
        "amid|",
        "amidst|",
        "among|",
        "amongst|",
        "anenst|",
        "apropos|",
        "apud|",
        "around|",
        "as|",
        "aside|",
        "astride|",
        "at|",
        "athwart|",
        "atop|",
        "barring|",
        "before|",
        "behind|",
        "below|",
        "beneath|",
        "beside|",
        "besides|",
        "between|",
        "beyond|",
        "but|",
        "by|",
        "circa|",
        "despite|",
        "down|",
        "during|",
        "et|",
        "except|",
        "for|",
        "forenenst|",
        "from|",
        "given|",
        "in|",
        "inside|",
        "into|",
        "lest|",
        "like|",
        "modulo|",
        "near|",
        "next|",
        "notwithstanding|",
        "of|",
        "off|",
        "on|",
        "onto|",
        "out|",
        "over|",
        "per|",
        "plus|",
        "pro|",
        "qua|",
        "sans|",
        "since|",
        "than|",
        "through|",
        "thru|",
        "throughout|",
        "thruout|",
        "till|",
        "to|",
        "toward|",
        "towards|",
        "under|",
        "underneath|",
        "until|",
        "unto|",
        "up|",
        "upon|",
        "versus|",
        "vs.|",
        "v.|",
        "vs|",
        "v|",
        "via|",
        "vis-à-vis|",
        "with|",
        "within|",
        "without|",
        "according to|",
        "ahead of|",
        "apart from|",
        "as for|",
        "as of|",
        "as per|",
        "as regards|",
        "aside from|",
        "back to|",
        "because of|",
        "close to|",
        "due to|",
        "except for|",
        "far from|",
        "inside of|",
        "instead of|",
        "near to|",
        "next to|",
        "on to|",
        "out from|",
        "out of|",
        "outside of|",
        "prior to|",
        "pursuant to|",
        "rather than|",
        "regardless of|",
        "such as|",
        "that of|",
        "up to|",
        "where as|",
        "or|",
        "yet|",
        "so|",
        "and|",
        "nor|",
        "a|",
        "an|",
        "the|",
        "c|",
        "ca|",
        // textcase_SkipNameParticlesInTitleCase
        "de|",
        "d|",
        "von|",
        "van",
        // Skip the | on the last one
        ")$"
    ];

    CITEPROC_JS_STOPWORD_REGEX.get_or_init(|| regex::Regex::new(re).unwrap())
}

#[test]
fn stopwords() {
    assert!(is_stopword("and"));
    assert!(!is_stopword("grandiloquent"));
}

fn is_stopword(word: &str) -> bool {
    stopword_regex().is_match(word)
}

fn upper_word_to_title(word: &str) -> Option<String> {
    let lowered = word.to_lowercase();
    let mut upper_gs = GraphemeIndices::new(word);
    if let Some((_, first_g)) = upper_gs.next() {
        let mut ret = String::with_capacity(word.len());
        ret.push_str(first_g);
        if let Some((rest_ix, _)) = upper_gs.next() {
            ret.push_str(&word[rest_ix..].to_lowercase());
        }
        return Some(ret);
    }
    None
}

fn transform_sentence_case(mut s: String, seen_one: bool, is_last: bool, is_uppercase: bool) -> String {
    if is_uppercase {
        transform_each_word(&s, seen_one, is_last, |word, is_first, no_stop| {
            if is_first {
                if let Some(upper) = upper_word_to_title(word) {
                    return Cow::Owned(upper);
                }
            }
            Cow::Owned(word.to_lowercase())
        })
    } else {
        transform_first_word(s, transform_uppercase_first)
    }
}

fn title_case_word(word: &str, entire_is_uppercase: bool, no_stopword: bool) -> Cow<'_, str> {
    let expect = "only called with nonempty words";
    debug!("title_case_word {}", word);
    if !no_stopword && is_stopword(word) {
        return Cow::Owned(word.to_lowercase());
    }
    if !word.chars().any(|c| c.is_ascii_alphabetic()) {
        // Entirely non-English
        // e.g. "β" in "β-Carotine"
        return Cow::Borrowed(word);
    }
    if entire_is_uppercase {
        if let Some(ret) = upper_word_to_title(word) {
            return Cow::Owned(ret);
        }
    }
    transform_first_char_of_word(word, |c| c.to_uppercase())
}

fn transform_title_case(s: &str, seen_one: bool, is_last: bool) -> String {
    transform_each_word(&s, seen_one, is_last, |word, _is_first, no_stop| title_case_word(word, false, no_stop))
}

fn transform_each_word(s: &str, seen_one: bool, is_last: bool, transform: impl Fn(&str, bool, bool) -> Cow<'_, str>) -> String {
    let mut acc = String::with_capacity(s.len());
    let mut is_first = !seen_one;
    let mut bounds = WordBoundIndices::new(s).peekable();
    for (ix, substr) in bounds {
        if is_word(substr) {
            let before = &s[..ix].chars().rev().filter(|c| !c.is_whitespace()).nth(0);
            let follows_colon = *before == Some(':')
                || *before == Some('?')
                || *before == Some('!')
                || *before == Some('.');
            let rest = &s[ix + substr.len()..];
            let is_last = is_last && (rest.is_empty() || !is_word(rest));
            let no_stopword = is_first || is_last || follows_colon;
            let word = substr;
            let tx = transform(word, is_first, no_stopword);
            acc.push_str(&tx);
        } else {
            acc.push_str(substr);
        }
        is_first = false;
    }
    acc
}

fn transform_first_word<'a>(mut s: String, transform: impl Fn(&str) -> Cow<'_, str>) -> String {
    let mut bounds = WordBoundIndices::new(&s);
    while let Some((ix, bound)) = bounds.next() {
        if is_word(bound) {
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
    pub fn apply_text_case_inner(&self, inlines: &mut [InlineElement], mut seen_one: bool, is_uppercase: bool) -> bool {
        let mut mine = false;
        let len = inlines.len();
        for (ix, inline) in inlines.iter_mut().enumerate() {
            let is_last = ix == len - 1;
            // order or short-circuits matters
            match inline {

                InlineElement::Text(txt) => {
                    let text = std::mem::replace(txt, String::new());
                    *txt = self.transform_case(text, seen_one, is_last, is_uppercase);
                    seen_one = string_contains_word(txt.as_ref()) || seen_one;
                },
                InlineElement::Micro(micros) => {
                    seen_one = self.apply_text_case_micro_inner(micros.as_mut(), seen_one, is_uppercase) || seen_one;
                }
                InlineElement::Quoted { inlines, .. }
                | InlineElement::Div(_, inlines)
                    | InlineElement::Anchor {
                        content: inlines,
                        ..
                    }
                => {
                    seen_one = self.apply_text_case_inner(inlines.as_mut(), seen_one, is_uppercase) || seen_one;
                }
                InlineElement::Formatted(inlines, formatting) if
                    formatting.font_variant != Some(FontVariant::SmallCaps)
                    && formatting.vertical_alignment != Some(VerticalAlignment::Superscript)
                    && formatting.vertical_alignment != Some(VerticalAlignment::Subscript)
                => {
                    seen_one = self.apply_text_case_inner(inlines.as_mut(), seen_one, is_uppercase) || seen_one;
                }
                InlineElement::Formatted(inlines, _) => {
                    seen_one = seen_one || self.contains_word(inlines.as_ref());
                }
            }
        }
        seen_one
    }
    pub fn apply_text_case_micro(&self, micros: &mut [MicroNode]) {
        let is_uppercase = self.is_uppercase_micro(micros);
        self.apply_text_case_micro_inner(micros, false, is_uppercase);
    }
    pub fn apply_text_case_micro_inner(&self, micros: &mut [MicroNode], mut seen_one: bool, is_uppercase: bool) -> bool {
        let mut mine = false;
        let len = micros.len();
        for (ix, micro) in micros.iter_mut().enumerate() {
            let is_last = ix == len - 1;
            use crate::output::FormatCmd;
            // order or short-circuits matters
            match micro {
                MicroNode::Text(ref mut txt) => {
                    let text = std::mem::replace(txt, String::new());
                    *txt = self.transform_case(text, seen_one, is_last, is_uppercase);
                    seen_one = string_contains_word(txt.as_ref()) || seen_one;
                }
                MicroNode::Formatted(children, FormatCmd::VerticalAlignmentSuperscript)
                | MicroNode::Formatted(children, FormatCmd::FontVariantSmallCaps)
                | MicroNode::Formatted(children, FormatCmd::VerticalAlignmentSubscript)
                | MicroNode::NoCase(children) => {
                    seen_one = seen_one || self.contains_word_micro(children.as_ref());
                }
                MicroNode::Formatted(children, _) | MicroNode::Quoted { children, .. } => {
                    seen_one = self.apply_text_case_micro_inner(children.as_mut(), seen_one, is_uppercase) || seen_one;
                }
            }
        }
        seen_one
    }
    fn contains_word(&self, micros: &[InlineElement]) -> bool {
        any_inlines(string_contains_word, false, micros)
    }
    fn contains_word_micro(&self, micros: &[MicroNode]) -> bool {
        any_micros(string_contains_word, false, micros)
    }
    pub fn is_uppercase(&self, inlines: &[InlineElement]) -> bool {
        any_inlines(any_lowercase, true, inlines)
    }
    fn is_uppercase_micro(&self, micros: &[MicroNode]) -> bool {
        any_micros(any_lowercase, true, micros)
    }
    pub fn transform_case(&self, s: String, seen_one: bool, is_last: bool, entire_is_uppercase: bool) -> String {
        match self.text_case {
            TextCase::Lowercase => s.to_lowercase(),
            TextCase::Uppercase => s.to_uppercase(),
            TextCase::CapitalizeFirst => {
                transform_first_word(s, transform_uppercase_first)
            }
            TextCase::Sentence if !seen_one => {
                transform_sentence_case(s, seen_one, is_last, entire_is_uppercase)
            }
            // Fallback is nothing
            TextCase::Title if self.is_english => {
                debug!("Title casing: {:?}", s);
                transform_title_case(&s, seen_one, is_last)
            }
            TextCase::CapitalizeAll => transform_each_word(&s, seen_one, is_last, |word, _, _| transform_uppercase_first(word)),
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

fn any_lowercase(s: &str) -> bool {
    s.chars().any(|c| c.is_lowercase())
}

fn any_inlines<F: Fn(&str) -> bool + Copy>(f: F, invert: bool, inlines: &[InlineElement]) -> bool {
    inlines.iter().any(|i| match i {
        InlineElement::Text(txt) => f(txt.as_ref()),
        InlineElement::Micro(micros) => any_micros(f, invert, micros.as_ref()),
        InlineElement::Quoted { inlines, .. }
        | InlineElement::Div(_, inlines)
        | InlineElement::Anchor {
            content: inlines,
            ..
        }
        | InlineElement::Formatted(inlines, _) => {
            any_inlines(f, invert, inlines.as_ref()) ^ invert
        }
    }) ^ invert
}

fn any_micros<F: Fn(&str) -> bool + Copy>(f: F, invert: bool, micros: &[MicroNode]) -> bool {
    micros.iter().any(|m| match m {
        MicroNode::Text(txt) => f(txt.as_ref()),
        MicroNode::Formatted(children, _)
        | MicroNode::Quoted { children, .. }
        | MicroNode::NoCase(children) => any_micros(f, invert, children) ^ invert,
    }) ^ invert
}

#[test]
fn test_any_micros() {
    fn parse(x: &str) -> Vec<MicroNode> { MicroNode::parse(x, &Default::default()) }
    fn upper(x: &str) -> bool { any_micros(any_lowercase, true, &parse(x)) }
    assert_eq!(upper("Hello, <sup>superscript</sup>"), false);
    assert_eq!(upper("HELLOSUPERSCRIPT"), true);
    assert_eq!(upper("HELLO, <sup>SUPERSCRIPT</sup>"), true);
}
