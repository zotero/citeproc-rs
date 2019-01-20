use super::DateOrRange;
use crate::input::NumericValue;
use crate::output::OutputFormat;
use crate::style::terms::LocatorType;

use crate::Atom;

/// Represents one cite in someone's document, to exactly one reference.
///
/// Prefixes and suffixes
#[derive(Deserialize)]
pub struct Cite<O: OutputFormat> {
    pub id: Atom,
    pub prefix: O::Build,
    pub suffix: O::Build,
    pub locator_type: Option<LocatorType>,

    // TODO: gotta be careful not to look up locators in the hashmap, even though
    // they are 'variables'.
    // in CSL-M they are number variables. also review the rest of the
    // vars that are like this
    pub locator: Option<NumericValue>,
    // TODO: parse these out of the locator
    // CSL-M only
    pub locator_extra: Option<String>,
    // CSL-M only
    pub locator_date: Option<DateOrRange>,

    // pub note_number: u32,
    pub near_note: bool,

    // TODO: allow suppression of any variables
    pub author_in_text: bool,
    pub suppress_author: bool,
    // Is this necessary?
    // citeHash       :: Int
}

impl<O: OutputFormat> Cite<O> {
    pub fn basic(id: Atom, prefix: &O::Build) -> Self {
        Cite {
            id,
            prefix: prefix.clone(),
            suffix: prefix.clone(),
            locator: None,
            locator_type: None,
            locator_extra: None,
            locator_date: None,
            near_note: false,
            author_in_text: false,
            suppress_author: false,
        }
    }
}
