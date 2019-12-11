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

fn transform_first_char_of_word(s: &str, f: impl Fn(char) -> char) -> String {
    // Naively capitalizes without a stopword filter
    let chars = s.chars();
    for c in words {
    }
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
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
        match self.text_case {
            TextCase::None => cow,
            TextCase::Lowercase => Cow::Owned(cow.to_lowercase()),
            TextCase::Uppercase => Cow::Owned(cow.to_uppercase()),
            TextCase::Uppercase => Cow::Owned(cow.to_uppercase()),
            TextCase::CapitalizeFirst => Cow::Owned(uppercase_first(&cow)),
            TextCase::CapitalizeAll => Cow::Owned(uppercase_first(&cow)),,
            TextCase::Sentence,
            TextCase::Title,
        }
    }
    pub fn default_with_quotes(quotes: LocalizedQuotes) -> Self {
        IngestOptions {
            quotes,
            ..Default::default()
        }
    }
}
