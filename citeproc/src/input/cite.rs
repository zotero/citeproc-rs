use super::Date;
use crate::input::NumericValue;
use crate::output::OutputFormat;
use crate::style::terms::LocatorType;

/// Represents one cite in someone's document, to exactly one reference.
///
/// Prefixes and suffixes
pub struct Cite<'ci, O: OutputFormat> {
    pub id: &'ci str,
    pub prefix: O::Output,
    pub suffix: O::Output,
    pub label: LocatorType,

    // TODO: gotta be careful not to look up locators in the hashmap, even though
    // they are 'variables'.
    // in CSL-M they are number variables. also review the rest of the
    // vars that are like this
    pub locator: Option<NumericValue<'ci>>,
    // csl-m
    pub locator_extra: Option<&'ci str>,
    // csl-m
    pub locator_date: Option<Date>,

    // pub note_number: u32,
    pub near_note: bool,

    // TODO: allow suppression of any variables
    pub author_in_text: bool,
    pub suppress_author: bool,
    // Is this necessary?
    // citeHash       :: Int
}

impl<'r, O: OutputFormat> Cite<'r, O> {
    pub fn basic(id: &'r str, prefix: &O::Output) -> Self {
        Cite {
            id: id,
            prefix: prefix.clone(),
            suffix: prefix.clone(),
            label: LocatorType::Page,
            locator: None,
            locator_extra: None,
            locator_date: None,
            near_note: false,
            author_in_text: false,
            suppress_author: false,
        }
    }
}
