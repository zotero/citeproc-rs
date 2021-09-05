// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::output::{markup::Markup, OutputFormat};
use crate::NumberLike;
use crate::String;
use csl::Atom;
use csl::LocatorType;
use serde::de::{Deserialize, Deserializer};

/// Represents one cite in someone's document, to exactly one reference.
///
/// ## Prefixes and suffixes
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
/// , { "id": "smith", "mode": "SuppressAuthor" }
/// , { "id": "smith", "mode": "AuthorOnly" }
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
#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", bound(deserialize = ""))]
pub struct Cite<O: OutputFormat> {
    #[serde(rename = "id", deserialize_with = "get_ref_id")]
    pub ref_id: Atom,

    #[serde(default)]
    pub prefix: Option<O::Input>,

    #[serde(default)]
    pub suffix: Option<O::Input>,

    /// Multiple locator functionality needs CSL support, so it is disabled via using
    /// `Locators::single_locator` for now.
    #[serde(default, flatten, deserialize_with = "Locators::single_locator")]
    pub locators: Option<Locators>,

    #[serde(default, flatten)]
    pub mode: Option<CiteMode>,
}

use std::fmt;

impl<O: OutputFormat> fmt::Debug for Cite<O> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Cite(")?;
        write!(f, "{:?}", AsRef::<str>::as_ref(&self.ref_id))?;
        if let Some(prefix) = self.prefix.as_ref() {
            write!(f, ", prefix: {:?}", prefix)?;
        }
        if let Some(suffix) = self.suffix.as_ref() {
            write!(f, ", suffix: {:?}", suffix)?;
        }
        if let Some(locators) = self.locators.as_ref() {
            write!(f, ", locators: {:?}", locators)?;
        }
        if let Some(mode) = self.mode.as_ref() {
            write!(f, ", mode: {:?}", mode)?;
        }
        write!(f, ")")
    }
}

/// Designed for use with `#[serde(with = "...")]`.
///
/// ```
/// use serde::Deserialize;
/// use citeproc_io::{Cite, CiteMode, CiteCompat, output::markup::Markup};
///
/// #[derive(Debug, PartialEq, Deserialize)]
/// struct CiteHolder(#[serde(with = "CiteCompat")] Cite<Markup>);
///
/// let json = r#"
/// [ { "id": "smith" }
/// , { "id": "smith", "suppress-author": true }
/// , { "id": "smith", "author-only": true }
/// ]"#;
/// let cites: Vec<CiteHolder> = serde_json::from_str(json).unwrap();
/// use pretty_assertions::assert_eq;
/// let basic_mode = |ref_id, mode| {
///     let mut cite = Cite::basic(ref_id);
///     cite.mode = Some(mode);
///     cite
/// };
/// assert_eq!(cites, vec![
///     CiteHolder(Cite::basic("smith")),
///     CiteHolder(basic_mode("smith", CiteMode::SuppressAuthor)),
///     CiteHolder(basic_mode("smith", CiteMode::AuthorOnly)),
/// ])
/// ```
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(remote = "Cite::<Markup>")]
pub struct CiteCompat {
    #[serde(rename = "id", deserialize_with = "get_ref_id")]
    pub ref_id: Atom,

    #[serde(default)]
    pub prefix: Option<String>,

    #[serde(default)]
    pub suffix: Option<String>,

    #[serde(default, flatten, deserialize_with = "Locators::single_locator")]
    pub locators: Option<Locators>,

    #[serde(default, flatten, deserialize_with = "CiteMode::compat")]
    pub mode: Option<CiteMode>,
}

pub mod cite_compat_vec {
    use super::*;
    pub fn deserialize<'de, D>(d: D) -> Result<Vec<Cite<Markup>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper(#[serde(with = "CiteCompat")] Cite<Markup>);
        let compat: Vec<Helper> = Deserialize::deserialize(d)?;
        let unwrapped = compat.into_iter().map(|Helper(x)| x).collect();
        Ok(unwrapped)
    }
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

/// Techincally reference IDs are allowed to be numbers.
pub fn get_ref_id<'de, D>(d: D) -> Result<Atom, D::Error>
where
    D: Deserializer<'de>,
{
    let s = NumberLike::deserialize(d)?;
    Ok(Atom::from(s.into_string()))
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
    // Used by get_locators
    #[allow(dead_code)]
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

    /// Only accepts `"locator": "abc", "label": "page|etc"`, no arrays.
    ///
    /// Locates singles in your area
    fn single_locator<'de, D>(d: D) -> Result<Option<Locators>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Option::<Locator>::deserialize(d)?.map(Locators::Single))
    }

    /// Single length locators arrays => Some(Locators::Single)
    /// Zero length => None
    ///
    /// Not yet used, pending CSL 1.1 locator array
    #[allow(dead_code)]
    fn get_locators<'de, D>(d: D) -> Result<Option<Locators>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Option::<Locators>::deserialize(d)?.and_then(|me| me.into_option()))
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Deserialize)]
#[serde(tag = "mode")]
pub enum CiteMode {
    AuthorOnly,
    SuppressAuthor,
}

impl CiteMode {
    /// Single length locators arrays => Some(Locators::Single)
    /// Zero length => None
    pub fn compat<'de, D>(d: D) -> Result<Option<CiteMode>, D::Error>
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
