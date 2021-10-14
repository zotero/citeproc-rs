// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use std::str::FromStr;

use super::attr::{EnumGetAttribute, GetAttribute};
use super::error::*;
use super::version::Features;
use super::IsIndependent;
use super::Style;

#[derive(Debug, Eq, Copy, Clone, PartialEq, EnumProperty, Hash)]
pub enum AnyVariable {
    Ordinary(Variable),
    Name(NameVariable),
    Date(DateVariable),
    Number(NumberVariable),
}

impl FromStr for AnyVariable {
    type Err = strum::ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use self::AnyVariable::*;
        if let Ok(v) = Variable::from_str(s) {
            return Ok(Ordinary(v));
        } else if let Ok(v) = NameVariable::from_str(s) {
            return Ok(Name(v));
        } else if let Ok(v) = DateVariable::from_str(s) {
            return Ok(Date(v));
        } else if let Ok(v) = NumberVariable::from_str(s) {
            return Ok(Number(v));
        }
        Err(strum::ParseError::VariantNotFound)
    }
}

impl EnumGetAttribute for Variable {}
impl EnumGetAttribute for NameVariable {}
impl EnumGetAttribute for NumberVariable {}
impl EnumGetAttribute for DateVariable {}

impl GetAttribute for AnyVariable {
    fn get_attr(s: &str, features: &Features) -> Result<Self, UnknownAttributeValue> {
        use self::AnyVariable::*;
        if let Ok(v) = Variable::get_attr(s, features) {
            return Ok(Ordinary(v));
        } else if let Ok(v) = NameVariable::get_attr(s, features) {
            return Ok(Name(v));
        } else if let Ok(v) = DateVariable::get_attr(s, features) {
            return Ok(Date(v));
        } else if let Ok(v) = NumberVariable::get_attr(s, features) {
            return Ok(Number(v));
        }
        Err(UnknownAttributeValue::new(s))
    }
}

/// Contrary to the CSL-M spec's declaration that number variables in a regular `<text variable>`
/// "should fail validation", that is perfectly valid, because "number variables are a subset of the
/// standard variables":
/// [Spec](https://docs.citationstyles.org/en/stable/specification.html#number-variables)

#[derive(Debug, Eq, Copy, Clone, PartialEq, Hash)]
pub enum StandardVariable {
    Ordinary(Variable),
    Number(NumberVariable),
}

impl From<&StandardVariable> for AnyVariable {
    fn from(sv: &StandardVariable) -> Self {
        match sv {
            StandardVariable::Number(n) => AnyVariable::Number(*n),
            StandardVariable::Ordinary(o) => AnyVariable::Ordinary(*o),
        }
    }
}

impl IsIndependent for StandardVariable {
    fn is_independent(&self) -> bool {
        match self {
            StandardVariable::Number(n) => n.is_independent(),
            StandardVariable::Ordinary(o) => o.is_independent(),
        }
    }
}

impl GetAttribute for StandardVariable {
    fn get_attr(s: &str, features: &Features) -> Result<Self, UnknownAttributeValue> {
        use self::StandardVariable::*;
        if let Ok(v) = Variable::get_attr(s, features) {
            return Ok(Ordinary(v));
        } else if let Ok(v) = NumberVariable::get_attr(s, features) {
            return Ok(Number(v));
        }
        Err(UnknownAttributeValue::new(s))
    }
}

impl IsIndependent for AnyVariable {
    fn is_independent(&self) -> bool {
        match self {
            AnyVariable::Ordinary(ov) => ov.is_independent(),
            AnyVariable::Number(nv) => nv.is_independent(),
            _ => false,
        }
    }
}

impl IsIndependent for Variable {
    fn is_independent(&self) -> bool {
        match self {
            // Variable::CitationLabel is not independent, it just implies a YearSuffix
            // which is, and that is handled in FreeCondWalker::text_variable()
            Variable::LocatorExtra | Variable::YearSuffix | Variable::Hereinafter => true,
            _ => false,
        }
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
#[non_exhaustive]
pub enum Variable {
    /// Not sure where this is from, but it appears sometimes.
    #[strum(serialize = "journalAbbreviation", serialize = "journal-abbreviation")]
    JournalAbbreviation,
    /// abstract of the item (e.g. the abstract of a journal article)
    Abstract,
    /// reader’s notes about the item content
    Annote,
    /// archive storing the item
    Archive,
    /// storage location within an archive (e.g. a box and folder number)
    /// technically the spec says use an underscore, but that's probably a typo.
    #[strum(serialize = "archive_location", serialize = "archive-location")]
    ArchiveLocation,
    /// geographic location of the archive,
    ArchivePlace,
    /// issuing or judicial authority (e.g. “USPTO” for a patent, “Fairfax Circuit Court” for a legal case)
    /// CSL-M only
    #[strum(props(csl = "1", cslM = "0"))]
    Authority,
    /// active={true} call number (to locate the item in a library)
    CallNumber,
    /// label identifying the item in in-text citations of label styles (e.g. “Ferr78”). May be assigned by the CSL processor based on item metadata.
    CitationLabel,
    /// title of the collection holding the item (e.g. the series title for a book)
    CollectionTitle,
    /// Not technically part of the spec, but https://forums.zotero.org/discussion/75366/accommodating-both-full-series-names-and-series-abbreviations
    CollectionTitleShort,
    /// title of the container holding the item (e.g. the book title for a book chapter, the journal title for a journal article)
    ContainerTitle,
    /// short/abbreviated form of “container-title” (also accessible through the “short” form of the “container-title” variable)
    ContainerTitleShort,
    /// physical (e.g. size) or temporal (e.g. running time) dimensions of the item
    Dimensions,
    /// Digital Object Identifier (e.g. “10.1128/AEM.02591-07”)
    #[strum(serialize = "DOI", serialize = "doi")]
    DOI,
    /// name of the related event (e.g. the conference name when citing a conference paper)
    Event,
    /// geographic location of the related event (e.g. “Amsterdam, the Netherlands”)
    EventPlace,

    /// class, type or genre of the item (e.g. “adventure” for an adventure movie, “PhD dissertation” for a PhD thesis)
    Genre,
    /// International Standard Book Number
    #[strum(serialize = "ISBN", serialize = "isbn")]
    ISBN,
    /// International Standard Serial Number
    #[strum(serialize = "ISSN", serialize = "issn")]
    ISSN,
    /// geographic scope of relevance (e.g. “US” for a US patent)
    Jurisdiction,
    /// keyword(s) or tag(s) attached to the item
    Keyword,
    /// medium description (e.g. “CD”, “DVD”, etc.)
    Medium,
    /// (short) inline note giving additional item details (e.g. a concise summary or commentary)
    Note,
    /// original publisher, for items that have been republished by a different publisher
    OriginalPublisher,
    /// geographic location of the original publisher (e.g. “London, UK”)
    OriginalPublisherPlace,
    /// title of the original version (e.g. “Война и мир”, the untranslated Russian title of “War and Peace”)
    OriginalTitle,
    /// PubMed Central reference number
    #[strum(serialize = "PMCID", serialize = "pmcid")]
    PMCID,
    /// PubMed reference number
    #[strum(serialize = "PMID", serialize = "pmid")]
    PMID,
    /// publisher
    Publisher,
    /// geographic location of the publisher
    PublisherPlace,
    /// resources related to the procedural history of a legal case
    References,
    /// title of the item reviewed by the current item
    ReviewedTitle,
    /// scale of e.g. a map
    Scale,
    /// container section holding the item (e.g. “politics” for a newspaper article).
    /// TODO: CSL-M appears to interpret this as a number variable?
    Section,
    /// from whence the item originates (e.g. a library catalog or database)
    Source,
    /// (publication) status of the item (e.g. “forthcoming”)
    Status,
    /// primary title of the item
    Title,
    /// short/abbreviated form of “title” (also accessible through the “short” form of the “title” variable)
    #[strum(serialize = "title-short", serialize = "shortTitle")]
    TitleShort,
    ///  URL (e.g. “https://aem.asm.org/cgi/content/full/74/9/2766”)
    #[strum(serialize = "URL", serialize = "url")]
    URL,
    /// version of the item (e.g. “2.0.9” for a software program)
    Version,
    /// disambiguating year suffix in author-date styles (e.g. “a” in “Doe, 1999a”)
    YearSuffix,

    // These are in the CSL-JSON spec
    CitationKey,
    Division,
    EventTitle,
    PartTitle,
    ReviewedGenre,
    #[strum(serialize = "archive-collection", serialize = "archive_collection")]
    ArchiveCollection,
    VolumeTitleShort,

    /// CSL-M only
    // Intercept Hereinafter at CiteContext, as it isn't known at Reference-time.
    // Global-per-document config should be its own thing separate from references.
    // TODO: delete any noRef="true" and replace with serde directives not to read from
    // CSL-JSON.
    #[strum(props(csl = "0", cslM = "1", noRef = "true"))]
    Hereinafter,
    /// CSL-M only
    #[strum(props(csl = "0", cslM = "1"))]
    LocatorExtra,
    /// CSL-M only
    #[strum(props(csl = "0", cslM = "1"))]
    VolumeTitle,

    /// CSL-M only
    ///
    /// Not documented in the CSL-M spec.
    #[strum(props(csl = "0", cslM = "1"))]
    Committee,

    /// CSL-M only
    ///
    /// Not documented in the CSL-M spec. See [Indigo Book][ib] section 'R26. Short Form
    /// Citation for Court Documents' for its intended use case, and the Juris-M [US cheat
    /// sheet][uscs]
    ///
    /// [uscs]: https://juris-m.github.io/cheat-sheets/us.pdf
    ///
    /// [ib]: https://law.resource.org/pub/us/code/blue/IndigoBook.html
    #[strum(props(csl = "0", cslM = "1"))]
    DocumentName,

    /// CSL-M only
    ///
    /// Not documented in the CSL-M spec.
    ///
    /// TODO: I think variable="gazette-flag" may have been superseded by type="gazette",
    /// but clearly you can still tick the "Gazette Ref" checkbox in Juris-M on a statute.
    /// Ask Frank. See also https://juris-m.github.io/cheat-sheets/us.pdf
    #[strum(props(csl = "0", cslM = "1"))]
    GazetteFlag,

    // TODO: should not be accessible in condition blocks
    Language,
}

impl Variable {
    pub fn should_replace_hyphens(self) -> bool {
        false
    }
    pub fn hyperlink(self, value: &str) -> Option<&str> {
        match self {
            Variable::URL => Some(value),
            Variable::DOI => Some(value),
            Variable::PMCID => Some(value),
            Variable::PMID => Some(value),
            _ => None,
        }
    }
}

impl IsIndependent for NumberVariable {
    fn is_independent(&self) -> bool {
        match self {
            NumberVariable::Locator => true,
            NumberVariable::FirstReferenceNoteNumber => true,
            _ => false,
        }
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
#[non_exhaustive]
pub enum NumberVariable {
    ChapterNumber,
    CollectionNumber,
    Edition,
    Issue,
    Number,
    NumberOfPages,
    NumberOfVolumes,
    Volume,

    /// Locator, Page and PageFirst, FRRN, and CiteNumber: These are technically meant to be standard variables in CSL 1.0.1, but the spec
    /// requires us to treat them as numerics for `<label plural="contextual">` anyway.
    ///
    /// a cite-specific pinpointer within the item (e.g. a page number within a book, or a volume in a multi-volume work). Must be accompanied in the input data by a label indicating the locator type (see the Locators term list), which determines which term is rendered by cs:label when the “locator” variable is selected.
    Locator,

    /// range of pages the item (e.g. a journal article) covers in a container (e.g. a journal issue)
    Page,
    /// first page of the range of pages the item (e.g. a journal article) covers in a container (e.g. a journal issue)
    PageFirst,

    /// number of a preceding note containing the first reference to the item. Assigned by the CSL processor. The variable holds no value for non-note-based styles, or when the item hasn’t been cited in any preceding notes.
    FirstReferenceNoteNumber,

    /// index (starting at 1) of the cited reference in the bibliography (generated by the CSL processor)
    CitationNumber,

    /// feature = var_publications
    #[strum(props(feature = "var_publications"))]
    PublicationNumber,

    #[strum(props(feature = "var_supplement"))]
    Supplement,

    /// CSL-M only
    #[strum(props(csl = "0", cslM = "1"))]
    Authority,

    // From CSL-JSON schema
    Part,
    Printing,
}

impl NumberVariable {
    pub fn should_replace_hyphens(self, style: &Style) -> bool {
        match self {
            NumberVariable::Locator => true,
            NumberVariable::Page => style.page_range_format.is_some(),
            _ => false,
        }
    }
    pub fn is_quantity(self) -> bool {
        match self {
            NumberVariable::NumberOfVolumes => true,
            NumberVariable::NumberOfPages => true,
            _ => false,
        }
    }
}

#[derive(
    AsRefStr, EnumProperty, EnumString, Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd,
)]
#[strum(serialize_all = "kebab_case")]
#[non_exhaustive]
pub enum NameVariable {
    /// author
    Author,
    /// editor of the collection holding the item (e.g. the series editor for a book)
    CollectionEditor,
    /// composer (e.g. of a musical score)
    Composer,
    /// author of the container holding the item (e.g. the book author for a book chapter)
    ContainerAuthor,
    /// director (e.g. of a film)
    Director,
    /// editor
    Editor,
    /// managing editor (“Directeur de la Publication” in French)
    EditorialDirector,
    /// illustrator (e.g. of a children’s book)
    Illustrator,
    /// interviewer (e.g. of an interview)
    Interviewer,
    /// ?
    OriginalAuthor,
    /// recipient (e.g. of a letter)
    Recipient,
    /// author of the item reviewed by the current item
    ReviewedAuthor,
    /// translator
    Translator,

    /// feature = var_editortranslator
    #[strum(
        serialize = "editortranslator",
        props(feature = "var_editortranslator")
    )]
    EditorTranslator,

    /// CSL-M only
    #[strum(props(feature = "cslm_legal"))]
    Authority,

    /// The dummy name variable is always empty. Use it to force all name variables called through
    /// a cs:names node to render through cs:substitute, and so suppress whichever is chosen for
    /// rendering to be suppressed through the remainder of the current cite.
    // TODO: make this an error for CSL-JSON but not for `<names variable="dummy">`.
    #[strum(props(feature = "var_dummy_name"))]
    Dummy,

    // From the CSL-JSON schema
    Curator,
    ScriptWriter,
    Performer,
    Producer,
    ExecutiveProducer,
    Guest,
    Narrator,
    Chair,
    Compiler,
    Contributor,
    SeriesCreator,
    Organizer,
    Host,
}

impl Default for NameVariable {
    fn default() -> Self {
        NameVariable::Dummy
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
#[non_exhaustive]
pub enum DateVariable {
    /// date the item has been accessed
    Accessed,
    /// ?
    Container,
    /// date the related event took place
    EventDate,
    /// date the item was issued/published
    Issued,
    /// (issue) date of the original version
    OriginalDate,
    /// date the item (e.g. a manuscript) has been submitted for publication
    Submitted,
    /// feature = var_locator_date
    #[strum(props(feature = "var_locator_date"))]
    LocatorDate,
    /// feature = var_publications
    #[strum(props(feature = "var_publications"))]
    PublicationDate,
    /// feature = var_publications
    #[strum(props(feature = "var_publications"))]
    AvailableDate,
}
