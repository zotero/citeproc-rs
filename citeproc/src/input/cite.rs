use std::borrow::Cow;
use super::DateOrRange;
use crate::input::NumericValue;
use crate::output::OutputFormat;
use crate::style::terms::LocatorType;

/// Represents one cite in someone's document, to exactly one reference.
///
/// Prefixes and suffixes
#[derive(Deserialize)]
pub struct Cite<'ci, O: OutputFormat> {
    #[serde(borrow)]
    pub id: Cow<'ci, str>,
    pub prefix: O::Build,
    pub suffix: O::Build,
    pub locator_type: Option<LocatorType>,

    // TODO: gotta be careful not to look up locators in the hashmap, even though
    // they are 'variables'.
    // in CSL-M they are number variables. also review the rest of the
    // vars that are like this
    #[serde(borrow)]
    pub locator: Option<NumericValue<'ci>>,
    // TODO: parse these out of the locator
    // CSL-M only
    #[serde(borrow)]
    pub locator_extra: Option<Cow<'ci, str>>,
    // CSL-M only
    #[serde(borrow)]
    pub locator_date: Option<DateOrRange<'ci>>,

    // pub note_number: u32,
    pub near_note: bool,

    // TODO: allow suppression of any variables
    pub author_in_text: bool,
    pub suppress_author: bool,
    // Is this necessary?
    // citeHash       :: Int
}

impl<'r, O: OutputFormat> Cite<'r, O> {
    pub fn basic(id: &'r str, prefix: &O::Build) -> Self {
        Cite {
            id: Cow::Borrowed(id),
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
