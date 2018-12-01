use crate::style::element::Position;
use crate::style::terms::LocatorType;
use super::reference::Date;

// AffixType is generic to allow for any data in here;
// could be MSWord stuff, or Pandoc Formatted = [Inline],
// or someone's Markdown

pub struct Cite<'c, AffixType> {
    pub id: &'c str,
    pub prefix: AffixType,
    pub suffix: AffixType,
    pub label: LocatorType,

    pub locator: &'c str,
    // csl-m
    pub locator_extra: &'c str,
    // csl-m
    pub locator_date: Date,

    pub note_number: u32,
    pub position: Position,
    pub near_note: bool,

    // TODO: allow suppression of any variables
    pub author_in_text: bool,
    pub suppress_author: bool,
    // Is this necessary?
    // citeHash       :: Int
}
