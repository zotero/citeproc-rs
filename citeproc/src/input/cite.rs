use super::{DateOrRange, NumericValue};
use crate::output::OutputFormat;
use crate::style::terms::LocatorType;

use crate::Atom;

pub type CiteId = u64;
pub type ClusterId = u64;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
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

use pandoc_types::definition::CitationMode;
impl Suppression {
    pub fn from_pandoc_mode(mode: CitationMode) -> Option<Self> {
        match mode {
            CitationMode::AuthorInText => Some(Suppression::InText),
            CitationMode::SuppressAuthor => Some(Suppression::Rest),
            CitationMode::NormalCitation => None,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Locator(LocatorType, NumericValue);

impl Locator {
    pub fn type_of(&self) -> LocatorType {
        self.0
    }
    pub fn value(&self) -> &NumericValue {
        &self.1
    }
}

/// Represents one cite in someone's document, to exactly one reference.
///
/// Prefixes and suffixes
#[derive(Debug, Clone)]
pub struct Cite<O: OutputFormat> {
    pub id: CiteId,
    pub ref_id: Atom,
    pub prefix: O::Build,
    pub suffix: O::Build,
    pub suppression: Option<Suppression>,
    // TODO: parse these out of the locator string
    // Enforce len() == 1 in CSL mode
    pub locators: Vec<Locator>,
    // CSL-M only
    pub locator_extra: Option<String>,
    // CSL-M only
    pub locator_date: Option<DateOrRange>,
}

impl<O: OutputFormat> Cite<O> {
    pub fn basic(id: CiteId, ref_id: impl Into<Atom>) -> Self {
        Cite {
            id,
            ref_id: ref_id.into(),
            prefix: O::Build::default(),
            suffix: O::Build::default(),
            suppression: None,
            locators: Vec::new(),
            locator_extra: None,
            locator_date: None,
        }
    }
}

#[derive(Debug)]
pub struct Cluster<O: OutputFormat> {
    pub id: ClusterId,
    pub cites: Vec<Cite<O>>,
    pub note_number: u32,
}
