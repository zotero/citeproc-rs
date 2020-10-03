// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2020 Corporation for Digital Scholarship

use citeproc_io::{Cite, output::OutputFormat};
use serde_derive::Deserialize;

use string_interner::DefaultSymbol;

pub type ClusterId = DefaultSymbol;

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq, Ord)]
#[serde(untagged)]
pub enum IntraNote {
    Single(u32),
    Multi(u32, u32),
}

impl IntraNote {
    pub fn note_number(&self) -> u32 {
        match self {
            IntraNote::Single(x) | IntraNote::Multi(x, _) => *x,
        }
    }
}

impl PartialOrd for IntraNote {
    fn partial_cmp(&self, other: &IntraNote) -> Option<Ordering> {
        use IntraNote::*;
        match (self, other) {
            (Single(_), Multi(..)) => Some(Ordering::Less),
            (Multi(..), Single(_)) => Some(Ordering::Greater),
            (Single(a), Single(b)) => a.partial_cmp(b),
            (Multi(a, b), Multi(c, d)) => a.partial_cmp(c).and_then(|e| {
                if e == Ordering::Equal {
                    b.partial_cmp(d)
                } else {
                    Some(e)
                }
            }),
        }
    }
}

#[derive(Deserialize, Ord, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Copy, Debug)]
pub enum ClusterNumber {
    InText(u32),
    Note(IntraNote),
}

impl ClusterNumber {
    pub fn sub_note(self, note: IntraNote) -> Option<u32> {
        use ClusterNumber::*;
        use IntraNote::*;
        match self {
            Note(self_note) => match (self_note, note) {
                (Single(a), Single(b))
                | (Single(a), Multi(b, _))
                | (Multi(a, _), Single(b))
                | (Multi(a, _), Multi(b, _)) => Some(a - b),
            },
            _ => None,
        }
    }
}

use std::cmp::Ordering;
impl PartialOrd for ClusterNumber {
    fn partial_cmp(&self, other: &ClusterNumber) -> Option<Ordering> {
        use ClusterNumber::*;
        match (self, other) {
            (InText(_), Note(_)) => Some(Ordering::Less),
            (Note(_), InText(_)) => Some(Ordering::Greater),
            (InText(a), InText(b)) => a.partial_cmp(b),
            (Note(a), Note(b)) => a.partial_cmp(b),
        }
    }
}
