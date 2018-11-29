use crate::style::element::Position;
use crate::style::terms::LocatorType;

// AffixType is generic to allow for any data in here;
// could be MSWord stuff, or Pandoc Formatted = [Inline],
// or someone's Markdown

pub struct Cite<AffixType> {
    pub id: String,
    pub prefix: AffixType,
    pub suffix: AffixType,
    pub label: LocatorType,

    pub locator: String,
    // csl-m
    pub locator_extra: String,
    // csl-m
    pub locator_date: String, // TODO: make this a date

    pub note_number: u32,
    pub position: Position,
    pub near_note: bool,

    // TODO: allow suppression of any variables
    pub author_in_text: bool,
    pub suppress_author: bool,
    // Is this necessary?
    // citeHash       :: Int
}
