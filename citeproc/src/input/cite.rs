use super::Date;
use crate::style::terms::LocatorType;
use crate::input::NumericValue;

// AffixType is generic to allow for any data in here;
// could be MSWord stuff, or Pandoc Formatted = [Inline],
// or someone's Markdown

pub struct Cite<'r, AffixType> {
    pub id: &'r str,
    pub prefix: AffixType,
    pub suffix: AffixType,
    pub label: LocatorType,

    // TODO: gotta be careful not to look up locators in the hashmap, even though
    // they are 'variables'.
    // in CSL-M they are number variables. also review the rest of the
    // vars that are like this
    pub locator: Option<NumericValue<'r>>,
    // csl-m
    pub locator_extra: Option<&'r str>,
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

impl<'r, T> Cite<'r, T>
where
    T: Clone,
{
    pub fn basic(id: &'r str, prefix: &T) -> Self {
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
