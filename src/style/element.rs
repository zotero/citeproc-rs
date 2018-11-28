use std::fmt;
use std::str::FromStr;
use crate::style::error::*;
use crate::style::get_attribute::GetAttribute;

// No EnumString; this one is manual for CSL-M
#[derive(AsStaticStr, Debug, PartialEq, Eq)]
#[strum(serialize_all="snake_case")]
pub enum Form {
    Long,
    Short,
    Count,
    Verb,
    VerbShort,
    Symbol,
    NotSet,
}

impl Form {
    pub fn from_str(s: &str) -> Result<Self, UnknownAttributeValue> {
        use self::Form::*;
        match s {
            "long" => Ok(Long),
            "short" => Ok(Short),
            "count" => Ok(Count),
            // not available usually
            // "verb" => Ok(Verb),
            // "verb-short" => Ok(VerbShort),
            "symbol" => Ok(Symbol),
            _ => Err(UnknownAttributeValue::new(s))
        }
    }
    pub fn from_str_names(s: &str) -> Result<Self, UnknownAttributeValue> {
        use self::Form::*;
        match s {
            "long" => Ok(Long),
            "short" => Ok(Short),
            "count" => Ok(Count),
            // available inside names block
            "verb" => Ok(Verb),
            "verb-short" => Ok(VerbShort),
            "symbol" => Ok(Symbol),
            _ => Err(UnknownAttributeValue::new(s))
        }
    }
}

impl Default for Form {
    fn default() -> Self { Form::Long }
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum NumericForm {
    Numeric,
    Ordinal,
    Roman,
    LongOrdinal,
}

impl Default for NumericForm {
    fn default() -> Self { NumericForm::Numeric }
}

#[derive(PartialEq, Eq)]
pub struct Affixes {
    pub prefix: String,
    pub suffix: String,
}

impl Default for Affixes {
    fn default() -> Self {
        Affixes {
            prefix: "".to_owned(),
            suffix: "".to_owned(),
        }
    }
}

#[derive(Eq, PartialEq)]
pub struct Formatting {
    pub font_style: FontStyle,
    pub font_variant: FontVariant,
    pub font_weight: FontWeight,
    pub vertical_alignment: VerticalAlignment,
    pub text_decoration: TextDecoration,
    // TODO: refactor
    pub display: FormattingDisplay,
    // TODO: refactor
    pub strip_periods: bool,
    pub hyperlink: String,
}

impl Default for Formatting {
    fn default() -> Self {
        Formatting {
            font_style: FontStyle::default(),
            font_variant: FontVariant::default(),
            font_weight: FontWeight::default(),
            text_decoration: TextDecoration::default(),
            vertical_alignment: VerticalAlignment::default(),
            display: FormattingDisplay::default(),
            strip_periods: false,
            hyperlink: "".to_owned(),
        }
    }
}

impl fmt::Debug for Affixes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Affixes {{ ");
        if self.prefix.len() > 0 { write!(f, "prefix: {:?}, ", self.prefix); }
        if self.suffix.len() > 0 { write!(f, "suffix: {:?}, ", self.suffix); }
        write!(f, "}}")
    }
}

impl fmt::Debug for Formatting {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let default = Formatting::default();
        write!(f, "Formatting {{ ");
        if self.font_style != default.font_style { write!(f, "font_style: {:?}, ", self.font_style); }
        if self.font_variant != default.font_variant { write!(f, "font_variant: {:?}, ", self.font_variant); }
        if self.font_weight != default.font_weight { write!(f, "font_weight: {:?}, ", self.font_weight); }
        if self.text_decoration != default.text_decoration { write!(f, "text_decoration: {:?}, ", self.text_decoration); }
        if self.vertical_alignment != default.vertical_alignment { write!(f, "vertical_alignment: {:?}, ", self.vertical_alignment); }
        if self.display != default.display { write!(f, "display: {:?}, ", self.display); }
        if self.strip_periods != default.strip_periods { write!(f, "strip_periods: {:?}, ", self.strip_periods); }
        if self.hyperlink != default.hyperlink { write!(f, "hyperlink: {:?}, ", self.hyperlink); }
        write!(f, "}}")
    }
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum FormattingDisplay {
    None,
    Block,
    LeftMargin,
    RightInline,
    Indent
}

impl Default for FormattingDisplay {
    fn default() -> Self { FormattingDisplay::None }
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum TextCase {
    None,
    Lowercase,
    Uppercase,
    CapitalizeFirst,
    CapitalizeAll,
    Sentence,
    Title,
}

impl Default for TextCase {
    fn default() -> Self { TextCase::None }
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

impl Default for FontStyle {
    fn default() -> Self { FontStyle::Normal }
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum FontVariant {
    Normal,
    SmallCaps,
}

impl Default for FontVariant {
    fn default() -> Self { FontVariant::Normal }
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum FontWeight {
    Normal,
    Bold,
    Light,
}

impl Default for FontWeight {
    fn default() -> Self { FontWeight::Normal }
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum TextDecoration {
    None,
    Underline,
}

impl Default for TextDecoration {
    fn default() -> Self { TextDecoration::None }
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
pub enum VerticalAlignment {
    #[strum(serialize="baseline")]
    Baseline,
    #[strum(serialize="sup", serialize="superscript")]
    Superscript,
    #[strum(serialize="sub", serialize="subscript")]
    Subscript,
}

impl Default for VerticalAlignment {
    fn default() -> Self { VerticalAlignment::Baseline }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Delimiter(pub String);

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum Plural {
    Contextual,
    Always,
    Never,
}
impl Default for Plural {
    fn default() -> Self { Plural::Contextual }
}

#[derive(Debug, Eq, PartialEq)]
pub enum LabelVariable {
    Locator,
    Page,
    Number(NumberVariable)
}

impl FromStr for LabelVariable {
    type Err = UnknownAttributeValue;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use self::LabelVariable::*;
        match s {
            "locator" => Ok(Locator),
            "page" => Ok(Page),
            x => Ok(Number(NumberVariable::get_attr(x)?))
        }
    }
}


#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum NumberVariable {
    ChapterNumber,
    CollectionNumber,
    Edition,
    Issue,
    Number,
    NumberOfPages,
    NumberOfVolumes,
    Volume,
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum Position {
    First,
    Ibid,
    IbidWithLocator,
    Subsequent,
    NearNote,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Condition {
    pub match_type: Match,
    pub disambiguate: bool,
    pub is_numeric: Vec<Variable>,
    pub variable: Vec<Variable>,
    pub position: Vec<Position>,
    pub csl_type: Vec<CslType>,
    pub is_uncertain_date: Vec<DateVariable>,
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum Match {
    Any,
    All,
    None,
    // Nand,
}
impl Default for Match {
    fn default() -> Self { Match::Any }
}

#[derive(Debug, Eq, PartialEq)]
pub struct IfThen(pub Condition, pub Vec<Element>);

#[derive(Debug, Eq, PartialEq)]
pub struct Else(pub Vec<Element>);

#[derive(Debug, Eq, PartialEq)]
pub enum Element {
    Choose(IfThen, Vec<IfThen>, Else),
    Macro(String, Formatting, Affixes),
    Const(String, Formatting, Affixes),
    Variable(String, Form, Formatting, Affixes, Delimiter),
    Term(String, Form, Formatting, Affixes, bool), // bool is plural
    Label(LabelVariable, Form, Formatting, Affixes, Plural),
    Number(NumberVariable, NumericForm, Formatting, Affixes, Plural),
    Names(Vec<String>, Vec<Name>, Option<NameLabel>, Formatting, Delimiter, Option<Substitute>),
    Group(Formatting, Delimiter, Vec<Element>), // done
    Date(Date)
}

#[derive(Debug, Eq, PartialEq)]
pub struct NameLabel {
    pub form: Form,
    pub formatting: Formatting,
    pub delimiter: Delimiter,
    pub plural: Plural,
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum DelimiterPrecedes {
    Contextual,
    AfterInvertedName,
    Always,
    Never,
}

impl Default for DelimiterPrecedes {
    fn default() -> Self { DelimiterPrecedes::Contextual }
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum NameForm {
    Long,
    Short,
    Count,
}
impl Default for NameForm {
    fn default() -> Self { NameForm::Long }
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum NameAsSortOrder {
    First,
    All,
}
impl Default for NameAsSortOrder {
    fn default() -> Self { NameAsSortOrder::All }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Name {
    pub and: String,
    pub delimiter: Delimiter,
    pub delimiter_precedes_et_al: DelimiterPrecedes,
    pub delimiter_precedes_last: DelimiterPrecedes,
    pub et_al_min: u32,
    pub et_al_use_first: u32,
    pub et_al_subsequent_min: u32,
    pub et_al_subsequent_use_first: u32,
    pub et_al_use_last: bool, // default is false
    pub form: NameForm,
    pub initialize: bool, // default is true
    pub initialize_with: String,
    pub name_as_sort_order: NameAsSortOrder, // TODO: work out default
    pub sort_separator: String,
    pub formatting: Formatting,
    pub affixes: Affixes,
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum NamePartName {
    Given,
    Family,
}

#[derive(Debug, Eq, PartialEq)]
pub struct NamePart {
    pub name: NamePartName,
    pub text_case: TextCase,
    pub formatting: Formatting,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Substitute(pub Vec<Element>);

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum GivenNameDisambiguationRule {
    AllNames,
    AllNamesWithInitials,
    PrimaryName,
    PrimaryNameWithInitials,
    ByCite
}
impl Default for GivenNameDisambiguationRule {
    fn default() -> Self { GivenNameDisambiguationRule::ByCite }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Citation {
    pub disambiguate_add_names: bool,
    pub disambiguate_add_givenname: bool,
    pub givenname_disambiguation_rule: GivenNameDisambiguationRule,
    pub disambiguate_add_year_suffix: bool,
    pub layout: Layout,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Layout {
    pub formatting: Formatting,
    pub affixes: Affixes,
    pub delimiter: Delimiter,
    pub elements: Vec<Element>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct MacroMap {
    pub name: String,
    pub elements: Vec<Element>,
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum StyleClass {
    InText,
    Note
}

#[derive(Debug, Eq, PartialEq)]
pub struct Info {
}
#[derive(Debug, Eq, PartialEq)]
pub struct Style {
    pub class: StyleClass,
    pub macros: Vec<MacroMap>,
    pub citation: Citation,
    pub info: Info
}

#[derive(Debug, Eq, PartialEq)]
pub struct RangeDelimiter(pub String);

impl Default for RangeDelimiter {
    fn default() -> Self { RangeDelimiter("".into()) }
}

// TODO: check against list
impl FromStr for RangeDelimiter {
    type Err = UnknownAttributeValue;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(RangeDelimiter(s.to_owned()))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct CslType(pub String);

// TODO: check against list
impl FromStr for CslType {
    type Err = UnknownAttributeValue;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(CslType(s.to_owned()))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct DateVariable(pub String);

// TODO: check against list
impl FromStr for DateVariable {
    type Err = UnknownAttributeValue;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(DateVariable(s.to_owned()))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Variable(pub String);

// TODO: check against list
impl FromStr for Variable {
    type Err = UnknownAttributeValue;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Variable(s.to_owned()))
    }
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum DateParts {
    YearMonthDay,
    YearMonth,
    Year,
}

impl Default for DateParts {
    fn default() -> Self { DateParts::YearMonthDay }
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum DatePartName {
    Day,
    Month,
    Year,
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum DayForm { 
    Numeric,
    NumericLeadingZeros,
    Ordinal,
}
impl Default for DayForm {
    fn default() -> Self { DayForm::Numeric }
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum MonthForm { 
    Long,
    Short,
    Numeric,
    NumericLeadingZeros,
}
impl Default for MonthForm {
    fn default() -> Self { MonthForm::Long }
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum YearForm { 
    Long,
    Short,
}
impl Default for YearForm {
    fn default() -> Self { YearForm::Long }
}


#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum DateForm { 
    Text,
    Numeric,
    #[strum(serialize="")]
    NotSet,
}
impl Default for DateForm {
    fn default() -> Self { DateForm::NotSet }
}

#[derive(Debug, Display, Eq, PartialEq)]
pub enum DatePartForm {
    Day(DayForm),
    Month(MonthForm),
    Year(YearForm),
}

#[derive(Debug, Eq, PartialEq)]
pub struct DatePart {
    pub form: DatePartForm,
    pub name: DatePartName,
    pub affixes: Affixes,
    pub formatting: Formatting,
    pub text_case: TextCase,
    pub range_delimiter: RangeDelimiter,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Date {
    pub form: DateForm,
    pub date_parts_attr: DateParts,
    pub date_parts: Vec<DatePart>,
    pub delimiter: Delimiter,
    pub affixes: Affixes,
    pub formatting: Formatting,
}

// pub enum Variable {
// abstract
//     abstract of the item (e.g. the abstract of a journal article)
// annote
//     reader’s notes about the item content
// archive
//     archive storing the item
// archive_location
//     storage location within an archive (e.g. a box and folder number)
// archive-place
//     geographic location of the archive
// authority
//     issuing or judicial authority (e.g. “USPTO” for a patent, “Fairfax Circuit Court” for a legal case)
// call-number
//     call number (to locate the item in a library)
// citation-label
//     label identifying the item in in-text citations of label styles (e.g. “Ferr78”). May be assigned by the CSL processor based on item metadata.
// citation-number
//     index (starting at 1) of the cited reference in the bibliography (generated by the CSL processor)
// collection-title
//     title of the collection holding the item (e.g. the series title for a book)
// container-title
//     title of the container holding the item (e.g. the book title for a book chapter, the journal title for a journal article)
// container-title-short
//     short/abbreviated form of “container-title” (also accessible through the “short” form of the “container-title” variable)
// dimensions
//     physical (e.g. size) or temporal (e.g. running time) dimensions of the item
// DOI
//     Digital Object Identifier (e.g. “10.1128/AEM.02591-07”)
// event
//     name of the related event (e.g. the conference name when citing a conference paper)
// event-place
//     geographic location of the related event (e.g. “Amsterdam, the Netherlands”)
// first-reference-note-number
//     number of a preceding note containing the first reference to the item. Assigned by the CSL processor. The variable holds no value for non-note-based styles, or when the item hasn’t been cited in any preceding notes.
// genre
//     class, type or genre of the item (e.g. “adventure” for an adventure movie, “PhD dissertation” for a PhD thesis)
// ISBN
//     International Standard Book Number
// ISSN
//     International Standard Serial Number
// jurisdiction
//     geographic scope of relevance (e.g. “US” for a US patent)
// keyword
//     keyword(s) or tag(s) attached to the item
// locator
//     a cite-specific pinpointer within the item (e.g. a page number within a book, or a volume in a multi-volume work). Must be accompanied in the input data by a label indicating the locator type (see the Locators term list), which determines which term is rendered by cs:label when the “locator” variable is selected.
// medium
//     medium description (e.g. “CD”, “DVD”, etc.)
// note
//     (short) inline note giving additional item details (e.g. a concise summary or commentary)
// original-publisher
//     original publisher, for items that have been republished by a different publisher
// original-publisher-place
//     geographic location of the original publisher (e.g. “London, UK”)
// original-title
//     title of the original version (e.g. “Война и мир”, the untranslated Russian title of “War and Peace”)
// page
//     range of pages the item (e.g. a journal article) covers in a container (e.g. a journal issue)
// page-first
//     first page of the range of pages the item (e.g. a journal article) covers in a container (e.g. a journal issue)
// PMCID
//     PubMed Central reference number
// PMID
//     PubMed reference number
// publisher
//     publisher
// publisher-place
//     geographic location of the publisher
// references
//     resources related to the procedural history of a legal case
// reviewed-title
//     title of the item reviewed by the current item
// scale
//     scale of e.g. a map
// section
//     container section holding the item (e.g. “politics” for a newspaper article)
// source
//     from whence the item originates (e.g. a library catalog or database)
// status
//     (publication) status of the item (e.g. “forthcoming”)
// title
//     primary title of the item
// title-short
//     short/abbreviated form of “title” (also accessible through the “short” form of the “title” variable)
// URL
//     Uniform Resource Locator (e.g. “http://aem.asm.org/cgi/content/full/74/9/2766”)
// version
//     version of the item (e.g. “2.0.9” for a software program)
// year-suffix
//     disambiguating year suffix in author-date styles (e.g. “a” in “Doe, 1999a”) 
// }

