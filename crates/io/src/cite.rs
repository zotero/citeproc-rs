// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::output::OutputFormat;
use crate::NumberLike;
use csl::Atom;
use csl::LocatorType;

pub type ClusterId = u32;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Deserialize)]
pub enum Suppression {
    // For author-in-text, or whatever the style author wants to put inline.
    //
    // E.g. the author, or party names for a case.
    InText,
    // For the rest.
    //
    // E.g. the cite with the author suppressed, or a case without party names.
    Rest,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Deserialize)]
pub struct Locator {
    pub locator: NumberLike,
    #[serde(default, rename = "label")]
    pub loc_type: LocatorType,
}

impl Locator {
    pub fn type_of(&self) -> LocatorType {
        self.loc_type
    }
    pub fn value(&self) -> &NumberLike {
        &self.locator
    }
}

use serde::de::{Deserialize, Deserializer};

/// Techincally reference IDs are allowed to be numbers.
pub fn get_ref_id<'de, D>(d: D) -> Result<Atom, D::Error>
where
    D: Deserializer<'de>,
{
    use super::csl_json::NumberLike;
    let s = NumberLike::deserialize(d)?;
    Ok(Atom::from(s.into_string()))
}

/// Represents one cite in someone's document, to exactly one reference.
///
/// Prefixes and suffixes
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", bound(deserialize = ""))]
pub struct Cite<O: OutputFormat> {
    #[serde(rename = "id", deserialize_with = "get_ref_id")]
    pub ref_id: Atom,

    #[serde(default)]
    pub prefix: Option<O::Input>,

    #[serde(default)]
    pub suffix: Option<O::Input>,

    #[serde(default)]
    pub suppression: Option<Suppression>,

    // TODO: Enforce len() == 1 in CSL mode
    #[serde(default, flatten, deserialize_with = "get_locators")]
    pub locators: Option<Locators>,
}

/// Accepts either
/// `{ "locator": "54", "label": "page" }` or
/// `{ "locators": [["chapter", "19"], ["page", "581"]] }`.
#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum Locators {
    Single(Locator),
    Multiple { locators: Vec<Locator> },
}

impl Locators {
    pub fn single(&self) -> Option<&Locator> {
        match self {
            Locators::Single(l) => Some(l),
            Locators::Multiple { locators } => locators.get(0),
        }
    }
    fn into_option(self) -> Option<Self> {
        match self {
            Locators::Multiple { locators } => {
                if locators.is_empty() {
                    None
                } else if locators.len() == 1 {
                    let first = locators.into_iter().nth(0).unwrap();
                    Some(Locators::Single(first))
                } else {
                    Some(Locators::Multiple { locators })
                }
            }
            l => Some(l),
        }
    }
}

/// Single length locators arrays => Some(Locators::Single)
/// Zero length => None
fn get_locators<'de, D>(d: D) -> Result<Option<Locators>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<Locators>::deserialize(d)?.and_then(|me| me.into_option()))
}

impl<O: OutputFormat> Eq for Cite<O> {}
impl<O: OutputFormat> PartialEq for Cite<O> {
    fn eq(&self, other: &Self) -> bool {
        self.ref_id == other.ref_id
            && self.prefix == other.prefix
            && self.suffix == other.suffix
            && self.suppression == other.suppression
            && self.locators == other.locators
    }
}

use std::hash::{Hash, Hasher};
impl<O: OutputFormat> Hash for Cite<O> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.ref_id.hash(h);
        self.prefix.hash(h);
        self.suffix.hash(h);
        self.suppression.hash(h);
        self.locators.hash(h);
    }
}

impl<O: OutputFormat> Cite<O> {
    pub fn basic(ref_id: impl Into<Atom>) -> Self {
        Cite {
            ref_id: ref_id.into(),
            prefix: Default::default(),
            suffix: Default::default(),
            suppression: None,
            locators: None,
        }
    }
}

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

/// The cluster number affects two things:
///
/// * The ordering of cites
/// * The `first-reference-note-number` variable
/// [`csl::variables::NumberVariable::FirstReferenceNoteNumber`]
///
/// Clusters can appear in footnotes, or in the body text of a document.
/// In JSON, that is `{ "note": 8, "id": ..., "cites": ... }` or `{ "inText": 5, ...}`.
///
/// Because footnotes can sometimes contain more than one cite cluster, there is a facility for
/// providing one extra value to discriminate between these. The following would be the second
/// cluster in the 8th footnote.
///
/// ```json
/// { "note": [8, 2], ... }
/// ```
///
/// It is up to the library consumer to ensure multi-cluster notes are each updated to include the
/// discriminant, i.e. to swap what was originally `{ "note": 8 }` to `{ "note": [8, 1] }`. Note
/// `8` (without a discriminant) will come before `[8, 1]`, but will not reliably have any ordering
/// with respect to another `8`.
///
/// Similarly, it is up to a library consumer to make sure no clusters have the same number as any
/// other.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(bound(deserialize = ""))]
pub struct Cluster<O: OutputFormat> {
    pub id: ClusterId,
    pub cites: Vec<Cite<O>>,
}

impl<O: OutputFormat> Cluster<O> {
    pub fn id(&self) -> ClusterId {
        self.id
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterPosition {
    pub id: ClusterId,
    /// If this is None, the piece is an in-text cluster. If it is Some, it is a note cluster.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<u32>,
}

