use crate::style::error::*;
use crate::style::get_attribute::{GetAttribute, CSL_VERSION};
use crate::style::terms::LocatorType;
use crate::style::variables::*;
use std::fmt;
use std::str::FromStr;

#[derive(AsRefStr, EnumString, EnumProperty, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "snake_case")]
pub enum Form {
    Long,
    Short,
    Count,
    Symbol,
    NotSet,
}

impl Default for Form {
    fn default() -> Self {
        Form::Long
    }
}

#[derive(AsRefStr, EnumString, EnumProperty, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "snake_case")]
pub enum NameLabelForm {
    Long,
    Short,
    Count,
    Verb,
    VerbShort,
    Symbol,
    NotSet,
}

impl Default for NameLabelForm {
    fn default() -> Self {
        NameLabelForm::Long
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum NumericForm {
    Numeric,
    Ordinal,
    Roman,
    LongOrdinal,
}

impl Default for NumericForm {
    fn default() -> Self {
        NumericForm::Numeric
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Affixes {
    pub prefix: String,
    pub suffix: String,
}

impl Default for Affixes {
    fn default() -> Self {
        Affixes {
            prefix: "".into(),
            suffix: "".into(),
        }
    }
}

#[derive(Eq, Clone, PartialEq)]
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

impl Formatting {
    pub fn bold() -> Self {
        let mut f = Formatting::default();
        f.font_weight = FontWeight::Bold;
        f
    }
    pub fn italic() -> Self {
        let mut f = Formatting::default();
        f.font_style = FontStyle::Italic;
        f
    }
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
        write!(f, "Affixes {{ ")?;
        if !self.prefix.is_empty() {
            write!(f, "prefix: {:?}, ", self.prefix)?;
        }
        if !self.suffix.is_empty() {
            write!(f, "suffix: {:?}, ", self.suffix)?;
        }
        write!(f, "}}")
    }
}

impl fmt::Debug for Formatting {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let default = Formatting::default();
        write!(f, "Formatting {{ ")?;
        if self.font_style != default.font_style {
            write!(f, "font_style: {:?}, ", self.font_style)?;
        }
        if self.font_variant != default.font_variant {
            write!(f, "font_variant: {:?}, ", self.font_variant)?;
        }
        if self.font_weight != default.font_weight {
            write!(f, "font_weight: {:?}, ", self.font_weight)?;
        }
        if self.text_decoration != default.text_decoration {
            write!(f, "text_decoration: {:?}, ", self.text_decoration)?;
        }
        if self.vertical_alignment != default.vertical_alignment {
            write!(f, "vertical_alignment: {:?}, ", self.vertical_alignment)?;
        }
        if self.display != default.display {
            write!(f, "display: {:?}, ", self.display)?;
        }
        if self.strip_periods != default.strip_periods {
            write!(f, "strip_periods: {:?}, ", self.strip_periods)?;
        }
        if self.hyperlink != default.hyperlink {
            write!(f, "hyperlink: {:?}, ", self.hyperlink)?;
        }
        write!(f, "}}")
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum FormattingDisplay {
    None,
    Block,
    LeftMargin,
    RightInline,
    Indent,
}

impl Default for FormattingDisplay {
    fn default() -> Self {
        FormattingDisplay::None
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
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
    fn default() -> Self {
        TextCase::None
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

impl Default for FontStyle {
    fn default() -> Self {
        FontStyle::Normal
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum FontVariant {
    Normal,
    SmallCaps,
}

impl Default for FontVariant {
    fn default() -> Self {
        FontVariant::Normal
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum FontWeight {
    Normal,
    Bold,
    Light,
}

impl Default for FontWeight {
    fn default() -> Self {
        FontWeight::Normal
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum TextDecoration {
    None,
    Underline,
}

impl Default for TextDecoration {
    fn default() -> Self {
        TextDecoration::None
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
pub enum VerticalAlignment {
    #[strum(serialize = "baseline")]
    Baseline,
    #[strum(serialize = "sup")]
    Superscript,
    #[strum(serialize = "sub")]
    Subscript,
}

impl Default for VerticalAlignment {
    fn default() -> Self {
        VerticalAlignment::Baseline
    }
}

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct Delimiter(pub String);

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum Plural {
    Contextual,
    Always,
    Never,
}
impl Default for Plural {
    fn default() -> Self {
        Plural::Contextual
    }
}

#[derive(Debug, EnumProperty, Eq, Clone, PartialEq)]
pub enum LabelVariable {
    Locator,
    Page,
    Number(NumberVariable),
}

impl FromStr for LabelVariable {
    type Err = UnknownAttributeValue;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use self::LabelVariable::*;
        match s {
            "locator" => Ok(Locator),
            "page" => Ok(Page),
            x => Ok(Number(NumberVariable::get_attr(x, CSL_VERSION)?)),
        }
    }
}

impl AsRef<str> for LabelVariable {
    fn as_ref(&self) -> &str {
        use self::LabelVariable::*;
        match *self {
            Locator => "locator",
            Page => "page",
            Number(ref n) => n.as_ref(),
        }
    }
}

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct Condition {
    pub match_type: Match,
    pub disambiguate: bool,
    pub is_numeric: Vec<NumberVariable>,
    pub variable: Vec<AnyVariable>,
    pub position: Vec<Position>,
    pub csl_type: Vec<CslType>,
    pub locator: Vec<LocatorType>,
    pub is_uncertain_date: Vec<DateVariable>,
}

impl Condition {
    pub fn is_empty(&self) -> bool {
        self.is_numeric.is_empty() &&
        self.variable.is_empty() &&
        self.position.is_empty() &&
        self.csl_type.is_empty() &&
        self.locator.is_empty() &&
        self.is_uncertain_date.is_empty()
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum Match {
    Any,
    All,
    None,
    #[strum(props(csl101 = "0", cslM = "1"))]
    Nand,
}

impl Default for Match {
    fn default() -> Self {
        Match::Any
    }
}

#[derive(Debug, Eq, Clone, PartialEq)]
// in CSL 1.0.1, conditions.len() == 1
pub struct IfThen(pub Conditions, pub Vec<Element>);

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct Conditions(pub Match, pub Vec<Condition>);

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct Else(pub Vec<Element>);

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct Choose(pub IfThen, pub Vec<IfThen>, pub Else);

type Quotes = bool;

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct Names(
    pub Vec<NameVariable>,
    pub Vec<Name>,
    pub Option<NameLabel>,
    pub Formatting,
    pub Delimiter,
    pub Option<Substitute>,
);

#[derive(Debug, Eq, Clone, PartialEq)]
pub enum Element {
    // <cs:choose>
    Choose(Choose),
    // <cs:text>
    Macro(String, Formatting, Affixes, Quotes),
    // <cs:text>
    Const(String, Formatting, Affixes, Quotes),
    // <cs:text>
    Variable(Variable, Formatting, Affixes, Form, Delimiter, Quotes),
    // <cs:term>
    Term(String, Form, Formatting, Affixes, bool), // bool is plural
    // <cs:label>
    Label(LabelVariable, Form, Formatting, Affixes, Plural),
    // <cs:number>
    Number(NumberVariable, NumericForm, Formatting, Affixes, Plural),
    // <cs:names>
    Names(Names),
    // <cs:group>
    Group(Formatting, Delimiter, Vec<Element>), // done
    // <cs:date>
    Date(Date),
}

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct NameLabel {
    pub form: NameLabelForm,
    pub formatting: Formatting,
    pub delimiter: Delimiter,
    pub plural: Plural,
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum DelimiterPrecedes {
    Contextual,
    AfterInvertedName,
    Always,
    Never,
}

impl Default for DelimiterPrecedes {
    fn default() -> Self {
        DelimiterPrecedes::Contextual
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum NameForm {
    Long,
    Short,
    Count,
}
impl Default for NameForm {
    fn default() -> Self {
        NameForm::Long
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum NameAsSortOrder {
    First,
    All,
}
impl Default for NameAsSortOrder {
    fn default() -> Self {
        NameAsSortOrder::All
    }
}

#[derive(Eq, Clone, PartialEq)]
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

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Name {{ .. }}")
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum NamePartName {
    Given,
    Family,
}

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct NamePart {
    pub name: NamePartName,
    pub text_case: TextCase,
    pub formatting: Formatting,
}

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct Substitute(pub Vec<Element>);

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum GivenNameDisambiguationRule {
    AllNames,
    AllNamesWithInitials,
    PrimaryName,
    PrimaryNameWithInitials,
    ByCite,
}
impl Default for GivenNameDisambiguationRule {
    fn default() -> Self {
        GivenNameDisambiguationRule::ByCite
    }
}

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct Citation {
    pub disambiguate_add_names: bool,
    pub disambiguate_add_givenname: bool,
    pub givenname_disambiguation_rule: GivenNameDisambiguationRule,
    pub disambiguate_add_year_suffix: bool,
    pub layout: Layout,
}

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct Layout {
    pub formatting: Formatting,
    pub affixes: Affixes,
    pub delimiter: Delimiter,
    pub elements: Vec<Element>,
}

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct MacroMap {
    pub name: String,
    pub elements: Vec<Element>,
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum StyleClass {
    InText,
    Note,
}

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct Info {}
#[derive(Debug, Eq, Clone, PartialEq)]
pub struct Style {
    pub class: StyleClass,
    pub macros: Vec<MacroMap>,
    pub citation: Citation,
    pub info: Info,
}

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct RangeDelimiter(pub String);

impl Default for RangeDelimiter {
    fn default() -> Self {
        RangeDelimiter("".to_owned())
    }
}

impl std::convert::AsRef<str> for RangeDelimiter {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl FromStr for RangeDelimiter {
    type Err = UnknownAttributeValue;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(RangeDelimiter(s.to_owned()))
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum DateParts {
    YearMonthDay,
    YearMonth,
    Year,
}

impl Default for DateParts {
    fn default() -> Self {
        DateParts::YearMonthDay
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum DatePartName {
    Day,
    Month,
    Year,
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum DayForm {
    Numeric,
    NumericLeadingZeros,
    Ordinal,
}
impl Default for DayForm {
    fn default() -> Self {
        DayForm::Numeric
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum MonthForm {
    Long,
    Short,
    Numeric,
    NumericLeadingZeros,
}
impl Default for MonthForm {
    fn default() -> Self {
        MonthForm::Long
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum YearForm {
    Long,
    Short,
}
impl Default for YearForm {
    fn default() -> Self {
        YearForm::Long
    }
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum DateForm {
    Text,
    Numeric,
    #[strum(serialize = "")]
    NotSet,
}
impl Default for DateForm {
    fn default() -> Self {
        DateForm::NotSet
    }
}

#[derive(Debug, Display, Eq, Clone, PartialEq)]
pub enum DatePartForm {
    Day(DayForm),
    Month(MonthForm),
    Year(YearForm),
}

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct DatePart {
    pub form: DatePartForm,
    pub name: DatePartName,
    pub affixes: Affixes,
    pub formatting: Formatting,
    pub text_case: TextCase,
    pub range_delimiter: RangeDelimiter,
}

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct Date {
    pub variable: DateVariable,
    pub form: DateForm,
    pub date_parts_attr: DateParts,
    pub date_parts: Vec<DatePart>,
    pub delimiter: Delimiter,
    pub affixes: Affixes,
    pub formatting: Formatting,
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum Position {
    First,
    Ibid,
    IbidWithLocator,
    Subsequent,
    NearNote,
}

/// http://docs.citationstyles.org/en/stable/specification.html#appendix-v-page-range-formats
#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum PageRangeFormat {
    Chicago,
    Expanded,
    Minimal,
    MinimalTwo,
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "kebab_case")]
pub enum CslType {
    Article,
    ArticleMagazine,
    ArticleNewspaper,
    ArticleJournal,
    Bill,
    Book,
    Broadcast,
    Chapter,
    Dataset,
    Entry,
    EntryDictionary,
    EntryEncyclopedia,
    Figure,
    Graphic,
    Interview,
    Legislation,
    #[strum(serialize = "legal_case")]
    LegalCase,
    Manuscript,
    Map,
    #[strum(serialize = "motion_picture")]
    MotionPicture,
    #[strum(serialize = "musical_score")]
    MusicalScore,
    Pamphlet,
    PaperConference,
    Patent,
    Post,
    PostWeblog,
    #[strum(serialize = "personal_communication")]
    PersonalCommunication,
    Report,
    Review,
    ReviewBook,
    Song,
    Speech,
    Thesis,
    Treaty,
    Webpage,

    #[strum(props(csl101 = "0", cslM = "1"))]
    Classic,
    #[strum(props(csl101 = "0", cslM = "1"))]
    Gazette,
    #[strum(props(csl101 = "0", cslM = "1"))]
    Hearing,
    #[strum(props(csl101 = "0", cslM = "1"))]
    Regulation,
    #[strum(props(csl101 = "0", cslM = "1"))]
    Video,
}
