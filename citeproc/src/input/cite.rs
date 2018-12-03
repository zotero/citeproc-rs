use super::Date;
use crate::style::terms::LocatorType;

// AffixType is generic to allow for any data in here;
// could be MSWord stuff, or Pandoc Formatted = [Inline],
// or someone's Markdown

pub struct Cite<AffixType> {
    pub id: String,
    pub prefix: AffixType,
    pub suffix: AffixType,
    pub label: LocatorType,

    // TODO: gotta be careful not to look up locators in the hashmap, even though
    // they are 'variables'.
    // in CSL-M they are number variables. also review the rest of the
    // vars that are like this
    pub locator: String,
    // csl-m
    pub locator_extra: Option<String>,
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

impl<T> Cite<T>
where
    T: Clone,
{
    pub fn basic(id: &str, prefix: &T) -> Self {
        Cite {
            id: id.to_owned(),
            prefix: prefix.clone(),
            suffix: prefix.clone(),
            label: LocatorType::Page,
            locator: "5".to_owned(),
            locator_extra: None,
            locator_date: None,
            near_note: false,
            author_in_text: false,
            suppress_author: false,
        }
    }
}
