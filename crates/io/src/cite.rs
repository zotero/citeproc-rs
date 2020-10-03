// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::output::OutputFormat;
use crate::NumberLike;
use csl::Atom;
use csl::LocatorType;

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
    pub fn has_affix(&self) -> bool {
        self.has_prefix() || self.has_suffix()
    }
    pub fn has_prefix(&self) -> bool {
        self.prefix.is_some()
    }
    pub fn has_suffix(&self) -> bool {
        self.suffix.is_some()
    }
}

