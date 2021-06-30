// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::output::OutputFormat;
use crate::NumberLike;
use csl::Atom;
use csl::LocatorType;

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

#[derive(Clone, Eq, PartialEq, Hash, Debug, Deserialize)]
pub enum CiteMode {
    AuthorOnly,
    SuppressAuthor,
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
///
/// ## Special Citation Forms
///
/// See [Special Citation Forms](https://citeproc-js.readthedocs.io/en/latest/running.html#special-citation-forms)
///
///
/// ```
/// use serde::Deserialize;
/// use citeproc_io::{Cite, CiteMode, output::markup::Markup};
/// let json = r#"
/// [ { "id": "smith" }
/// , { "id": "smith", "suppress-author": true }
/// , { "id": "smith", "author-only": true }
/// ]"#;
/// let cites: Vec<Cite<Markup>> = serde_json::from_str(json).unwrap();
/// use pretty_assertions::assert_eq;
/// let basic_mode = |ref_id, mode| {
///     let mut cite = Cite::basic(ref_id);
///     cite.mode = Some(mode);
///     cite
/// };
/// assert_eq!(cites, vec![
///     Cite::basic("smith"),
///     basic_mode("smith", CiteMode::SuppressAuthor),
///     basic_mode("smith", CiteMode::AuthorOnly),
/// ])
/// ```
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", bound(deserialize = ""))]
pub struct Cite<O: OutputFormat> {
    #[serde(rename = "id", deserialize_with = "get_ref_id")]
    pub ref_id: Atom,

    #[serde(default)]
    pub prefix: Option<O::Input>,

    #[serde(default)]
    pub suffix: Option<O::Input>,

    // TODO: Enforce len() == 1 in CSL mode
    #[serde(default, flatten, deserialize_with = "get_locators")]
    pub locators: Option<Locators>,

    #[serde(default, flatten, deserialize_with = "get_mode_flags")]
    pub mode: Option<CiteMode>,
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

/// Single length locators arrays => Some(Locators::Single)
/// Zero length => None
fn get_mode_flags<'de, D>(d: D) -> Result<Option<CiteMode>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Truthy {
        Boolean(bool),
        Number(i32),
    }

    impl Default for Truthy {
        fn default() -> Self {
            Self::Boolean(false)
        }
    }

    impl Truthy {
        fn is_truthy(&self) -> bool {
            match *self {
                Truthy::Boolean(b) => b,
                Truthy::Number(x) => x > 0,
            }
        }
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "kebab-case")]
    struct ModeFlags {
        #[serde(default)]
        suppress_author: Truthy,
        #[serde(default)]
        author_only: Truthy,
        #[serde(default)]
        composite: Option<serde::de::IgnoredAny>,
    }

    impl ModeFlags {
        fn to_mode<E: serde::de::Error>(self) -> Result<Option<CiteMode>, E> {
            if self.composite.is_some() {
                return Err(E::custom(
                    "`composite` mode not supported on Cite, only on Cluster",
                ));
            }
            match (
                self.author_only.is_truthy(),
                self.suppress_author.is_truthy(),
            ) {
                (true, true) => Err(E::custom(
                    "must supply only one of `author-only` or `suppress-author` on Cite",
                )),
                (true, _) => Ok(Some(CiteMode::AuthorOnly)),
                (_, true) => Ok(Some(CiteMode::SuppressAuthor)),
                _ => Ok(None),
            }
        }
    }

    ModeFlags::deserialize(d)
        .map_err(|e| {
            log::warn!("{}", e);
            e
        })?
        .to_mode()
}

use std::hash::{Hash, Hasher};
impl<O: OutputFormat> Hash for Cite<O> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.ref_id.hash(h);
        self.prefix.hash(h);
        self.suffix.hash(h);
        self.locators.hash(h);
    }
}

impl<O: OutputFormat> Cite<O> {
    pub fn basic(ref_id: impl Into<Atom>) -> Self {
        Cite {
            ref_id: ref_id.into(),
            prefix: Default::default(),
            suffix: Default::default(),
            locators: None,
            mode: None,
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
