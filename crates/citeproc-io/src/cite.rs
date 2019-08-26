// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::output::OutputFormat;
use super::{DateOrRange, NumericValue};
use csl::terms::LocatorType;
use csl::Atom;

pub type CiteId = u32;
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
pub struct Locator(pub LocatorType, pub NumericValue);

impl Locator {
    pub fn type_of(&self) -> LocatorType {
        self.0
    }
    pub fn value(&self) -> &NumericValue {
        &self.1
    }
}

use serde::de::{Deserialize, Deserializer};

/// Techincally reference IDs are allowed to be numbers.
fn get_ref_id<'de, D>(d: D) -> Result<Atom, D::Error>
where
    D: Deserializer<'de>,
{
    use super::csl_json::IdOrNumber;
    let s = IdOrNumber::deserialize(d)?;
    Ok(Atom::from(s.0))
}

/// Represents one cite in someone's document, to exactly one reference.
///
/// Prefixes and suffixes
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", bound(deserialize = ""))]
pub struct Cite<O: OutputFormat> {
    #[serde(rename = "citeId")]
    pub id: CiteId,

    #[serde(rename = "id", deserialize_with = "get_ref_id")]
    pub ref_id: Atom,

    #[serde(default)]
    pub prefix: Option<O::Input>,

    #[serde(default)]
    pub suffix: Option<O::Input>,

    #[serde(default)]
    pub suppression: Option<Suppression>,

    // TODO: parse these out of the locator string
    // Enforce len() == 1 in CSL mode
    #[serde(default)]
    pub locators: Vec<Locator>,

    // CSL-M only
    #[serde(default)]
    pub locator_extra: Option<String>,
    // CSL-M only
    #[serde(default)]
    pub locator_date: Option<DateOrRange>,
}

impl<O: OutputFormat> Cite<O> {
    pub fn basic(id: CiteId, ref_id: impl Into<Atom>) -> Self {
        Cite {
            id,
            ref_id: ref_id.into(),
            prefix: Default::default(),
            suffix: Default::default(),
            suppression: None,
            locators: Vec::new(),
            locator_extra: None,
            locator_date: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", bound(deserialize = ""))]
pub struct Cluster<O: OutputFormat> {
    pub id: ClusterId,
    pub cites: Vec<Cite<O>>,
    pub note_number: u32,
}
