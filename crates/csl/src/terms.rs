// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::MonthForm;
use crate::error::*;
use crate::version::Features;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use super::attr::{EnumGetAttribute, GetAttribute};
use super::variables::{NameVariable, NumberVariable};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TextTermSelector {
    Simple(SimpleTermSelector),
    Gendered(GenderedTermSelector),
    Role(RoleTermSelector),
    // You can't render ordinals using a <text> node, only using <number>
}

// It's awkward to check if a term is equal to a TTS. So implement PartialEq for some convenient
// `tts == MiscTerm::NoDate` etc

macro_rules! tts_eq {
    ($Term:ident, |$t:ident| $SelPat:pat => $eq:expr) => {
        impl PartialEq<$Term> for TextTermSelector {
            fn eq(&self, $t: &$Term) -> bool {
                match self {
                    $SelPat => $eq,
                    _ => false,
                }
            }
        }
    };
}

tts_eq!(MiscTerm, |x| Self::Simple(SimpleTermSelector::Misc(t, _)) => x == t);
tts_eq!(Category, |x| Self::Simple(SimpleTermSelector::Category(t, _)) => x == t);
tts_eq!(QuoteTerm, |x| Self::Simple(SimpleTermSelector::Quote(t)) => x == t);
tts_eq!(NumberVariable, |x| Self::Gendered(GenderedTermSelector::Number(t, _)) => x == t);
tts_eq!(LocatorType, |x| Self::Gendered(GenderedTermSelector::Locator(t, _)) => x == t);
tts_eq!(MonthTerm, |x| Self::Gendered(GenderedTermSelector::Month(t, _)) => x == t);
tts_eq!(SeasonTerm, |x| Self::Gendered(GenderedTermSelector::Season(t, _)) => x == t);
tts_eq!(RoleTerm, |x| Self::Role(RoleTermSelector(t, _)) => x == t);

pub enum AnyTermName {
    Number(NumberVariable),
    Month(MonthTerm),
    Loc(LocatorType),

    Misc(MiscTerm),
    Category(Category),
    Season(SeasonTerm),
    Quote(QuoteTerm),

    Role(RoleTerm),

    Ordinal(OrdinalTerm),
}

impl EnumGetAttribute for MonthTerm {}
impl EnumGetAttribute for LocatorType {}
impl EnumGetAttribute for MiscTerm {}
impl EnumGetAttribute for Category {}
impl EnumGetAttribute for SeasonTerm {}
impl EnumGetAttribute for QuoteTerm {}
impl EnumGetAttribute for RoleTerm {}
impl EnumGetAttribute for OrdinalTerm {}
impl EnumGetAttribute for OrdinalMatch {}
impl EnumGetAttribute for Gender {}
impl EnumGetAttribute for TermForm {}
impl EnumGetAttribute for TermFormExtended {}
impl EnumGetAttribute for TermPlurality {}

impl GetAttribute for AnyTermName {
    fn get_attr(s: &str, features: &Features) -> Result<Self, UnknownAttributeValue> {
        if let Ok(v) = MiscTerm::get_attr(s, features) {
            return Ok(AnyTermName::Misc(v));
        } else if let Ok(v) = MonthTerm::get_attr(s, features) {
            return Ok(AnyTermName::Month(v));
        } else if let Ok(v) = NumberVariable::get_attr(s, features) {
            return Ok(AnyTermName::Number(v));
        } else if let Ok(v) = LocatorType::get_attr(s, features) {
            return Ok(AnyTermName::Loc(v));
        } else if let Ok(v) = SeasonTerm::get_attr(s, features) {
            return Ok(AnyTermName::Season(v));
        } else if let Ok(v) = QuoteTerm::get_attr(s, features) {
            return Ok(AnyTermName::Quote(v));
        } else if let Ok(v) = RoleTerm::get_attr(s, features) {
            return Ok(AnyTermName::Role(v));
        } else if let Ok(v) = OrdinalTerm::get_attr(s, features) {
            return Ok(AnyTermName::Ordinal(v));
        } else if let Ok(v) = Category::get_attr(s, features) {
            return Ok(AnyTermName::Category(v));
        }
        Err(UnknownAttributeValue::new(s))
    }
}

/// TermSelector is used
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SimpleTermSelector {
    Misc(MiscTerm, TermFormExtended),
    Category(Category, TermForm),
    Quote(QuoteTerm),
}

impl SimpleTermSelector {
    pub fn fallback(self) -> Box<dyn Iterator<Item = Self>> {
        match self {
            SimpleTermSelector::Misc(t, form) => {
                Box::new(form.fallback().map(move |x| SimpleTermSelector::Misc(t, x)))
            }
            SimpleTermSelector::Category(t, form) => Box::new(
                form.fallback()
                    .map(move |x| SimpleTermSelector::Category(t, x)),
            ),
            SimpleTermSelector::Quote(t) => Box::new(
                // Quotes don't do fallback. Not spec'd, but what on earth is the long form of a
                // close quote mark? "', she said sarcastically."?
                std::iter::once(SimpleTermSelector::Quote(t)),
            ),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct OrdinalTermSelector(pub OrdinalTerm, pub Gender);

struct OrdinalTermIter(Option<OrdinalTerm>);

impl Iterator for OrdinalTermIter {
    type Item = OrdinalTerm;
    fn next(&mut self) -> Option<Self::Item> {
        let next = if let Some(x) = self.0 {
            match x {
                // The end
                OrdinalTerm::Ordinal => None,
                // Second last
                OrdinalTerm::Mod100(_, OrdinalMatch::LastDigit) => Some(OrdinalTerm::Ordinal),
                OrdinalTerm::Mod100(n, OrdinalMatch::LastTwoDigits) => {
                    Some(OrdinalTerm::Mod100(n % 10, OrdinalMatch::LastDigit))
                }
                OrdinalTerm::Mod100(n, OrdinalMatch::WholeNumber) => {
                    Some(OrdinalTerm::Mod100(n, OrdinalMatch::LastTwoDigits))
                }
                // The rest are LongOrdinal*, which should fall back with LongOrdinal01 -> Mod100(1), etc.
                t => Some(OrdinalTerm::Mod100(
                    t.to_number(),
                    OrdinalMatch::WholeNumber,
                )),
            }
        } else {
            None
        };
        mem::replace(&mut self.0, next)
    }
}

impl OrdinalTerm {
    pub fn fallback(self) -> impl Iterator<Item = Self> {
        OrdinalTermIter(Some(self))
    }
}

impl OrdinalTermSelector {
    pub fn fallback(self) -> impl Iterator<Item = OrdinalTermSelector> {
        use std::iter::once;
        let OrdinalTermSelector(term, gender) = self;
        term.fallback().flat_map(move |term| {
            once(OrdinalTermSelector(term, gender))
                .chain(once(OrdinalTermSelector(term, Gender::Neuter)))
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum GenderedTermSelector {
    /// Edition is the only MiscTerm that can have a gender, so it's here instead
    Number(NumberVariable, TermForm),
    Locator(LocatorType, TermForm),
    Month(MonthTerm, TermForm),
    Season(SeasonTerm, TermForm),
}

impl GenderedTermSelector {
    pub fn from_number_variable(
        loc_type: Option<LocatorType>,
        var: NumberVariable,
        form: TermForm,
    ) -> Option<GenderedTermSelector> {
        match var {
            NumberVariable::Locator => match loc_type {
                None => None,
                Some(l) => Some(GenderedTermSelector::Locator(l, form)),
            },
            v => Some(GenderedTermSelector::Number(v, form)),
        }
    }

    pub fn from_month_u32(month_or_season: u32, form: MonthForm) -> Option<Self> {
        let term_form = match form {
            MonthForm::Long => TermForm::Long,
            MonthForm::Short => TermForm::Short,
            // Not going to be using the terms anyway
            _ => return None,
        };
        if month_or_season == 0 || month_or_season > 16 {
            return None;
        }
        let sel = if month_or_season > 12 {
            // it's a season; 1 -> Spring, etc
            let season = month_or_season - 12;
            GenderedTermSelector::Season(
                match season {
                    1 => SeasonTerm::Season01,
                    2 => SeasonTerm::Season02,
                    3 => SeasonTerm::Season03,
                    4 => SeasonTerm::Season04,
                    _ => return None,
                },
                term_form,
            )
        } else {
            GenderedTermSelector::Month(
                MonthTerm::from_u32(month_or_season).expect("we know it's a month now"),
                term_form,
            )
        };
        Some(sel)
    }

    pub fn normalise(self) -> Self {
        use GenderedTermSelector::*;
        match self {
            Number(NumberVariable::Page, x) => Locator(LocatorType::Page, x),
            Number(NumberVariable::Issue, x) => Locator(LocatorType::Issue, x),
            Number(NumberVariable::Volume, x) => Locator(LocatorType::Volume, x),
            g => g,
        }
    }

    pub fn fallback(self) -> Box<dyn Iterator<Item = Self>> {
        match self {
            GenderedTermSelector::Number(t, form) => Box::new(
                form.fallback()
                    .map(move |x| GenderedTermSelector::Number(t, x)),
            ),
            GenderedTermSelector::Locator(t, form) => Box::new(
                form.fallback()
                    .map(move |x| GenderedTermSelector::Locator(t, x)),
            ),
            GenderedTermSelector::Month(t, form) => Box::new(
                form.fallback()
                    .map(move |x| GenderedTermSelector::Month(t, x)),
            ),
            GenderedTermSelector::Season(t, form) => Box::new(
                form.fallback()
                    .map(move |x| GenderedTermSelector::Season(t, x)),
            ),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct RoleTermSelector(pub RoleTerm, pub TermFormExtended);

impl RoleTermSelector {
    pub fn fallback(self) -> Box<dyn Iterator<Item = Self>> {
        Box::new(self.1.fallback().map(move |x| RoleTermSelector(self.0, x)))
    }
    pub fn from_name_variable(var: NameVariable, form: TermFormExtended) -> Option<Self> {
        let term = RoleTerm::from_name_var(var);
        term.map(|t| RoleTermSelector(t, form))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenderedTerm(pub TermPlurality, pub Gender);

#[derive(AsRefStr, EnumString, EnumProperty, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
pub enum TermForm {
    Long,
    Short,
    Symbol,
    // XXX form="static", e.g. in jm-oscola, for CSL-M only
}

impl Default for TermForm {
    fn default() -> Self {
        TermForm::Long
    }
}

pub struct TermFallbackIter(Option<TermForm>);

impl TermForm {
    pub fn fallback(self) -> TermFallbackIter {
        TermFallbackIter(Some(self))
    }
}

impl Iterator for TermFallbackIter {
    type Item = TermForm;
    fn next(&mut self) -> Option<TermForm> {
        use self::TermForm::*;
        let next = match self.0 {
            Some(Symbol) => Some(Short),
            Some(Short) => Some(Long),
            _ => None,
        };
        mem::replace(&mut self.0, next)
    }
}
/// Includes the extra Verb and VerbShort variants
#[derive(AsRefStr, EnumString, EnumProperty, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
pub enum TermFormExtended {
    Long,
    Short,
    Symbol,
    Verb,
    VerbShort,
}

impl Default for TermFormExtended {
    fn default() -> Self {
        TermFormExtended::Long
    }
}

pub struct TermFallbackExtendedIter(Option<TermFormExtended>);

impl TermFormExtended {
    pub fn fallback(self) -> TermFallbackExtendedIter {
        TermFallbackExtendedIter(Some(self))
    }
}

use std::mem;

impl Iterator for TermFallbackExtendedIter {
    type Item = TermFormExtended;
    fn next(&mut self) -> Option<TermFormExtended> {
        use self::TermFormExtended::*;
        let next = match self.0 {
            Some(VerbShort) => Some(Verb),
            Some(Symbol) => Some(Short),
            Some(Verb) => Some(Long),
            Some(Short) => Some(Long),
            _ => None,
        };
        mem::replace(&mut self.0, next)
    }
}

#[derive(AsRefStr, EnumString, EnumProperty, Debug, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
pub enum TermPlurality {
    Pluralized { single: String, multiple: String },
    Invariant(String),
}

impl TermPlurality {
    pub fn get(&self, plural: bool) -> &str {
        if plural {
            self.plural()
        } else {
            self.singular()
        }
    }
    pub fn plural(&self) -> &str {
        match self {
            TermPlurality::Invariant(s) => &s,
            TermPlurality::Pluralized { multiple, .. } => &multiple,
        }
    }
    pub fn singular(&self) -> &str {
        match self {
            TermPlurality::Invariant(s) => &s,
            TermPlurality::Pluralized { single, .. } => &single,
        }
    }
    pub fn no_plural_allowed(&self) -> Option<&str> {
        match self {
            TermPlurality::Invariant(s) => Some(&s),
            _ => None,
        }
    }
}

/// Represents a gender for the purpose of *defining* or *selecting* a term.
///
/// `gender="feminine"` is an output property of a term:
///
///   * you only define `<term name="edition">` once per locale, and that localization has a
///   specific gender.
///
/// `gender-form="feminine"` is part of the selector:
///
///   * you can define multiple `<term name="ordinal-01">`s, each with a different `gender-form`,
///   such that when the style needs an ordinal with a specific gender, it can fetch one.
///
/// So, for `{ "issue": 1 }`:
///
/// ```xml
/// <term name="issue" gender="feminine">issue</term>
/// <term name="ordinal-01" gender-form="feminine">FFF</term>
/// <term name="ordinal-01" gender-form="masculine">MMM</term>
/// ...
/// <number variable="issue" form="ordinal" suffix=" " />
/// <label variable="issue" />
/// ```

/// Produces `1FFF issue`, because:
///
/// 1. `cs:number` wants to render the number `1` as an ordinal, so it needs to know the underlying
///    gender of the variable's associated noun.
/// 2. `cs:number` looks up `GenderedTermSelector::Locator(LocatorType::Issue, TermForm::Long)` and
///    gets back `GenderedTerm(TermPlurality::Invariant("issue"), Gender::Feminine)`
/// 3. It then needs an ordinal to match `Gender::Feminine`, so it looks up, in order:
///
///    1. `OrdinalTermSelector(Mod100(1), Feminine, LastDigit)` and finds a match with content
///       `FFF`.
///    2. Would also look up OridnalMatch::LastTwoDigits Neuter
///
#[derive(AsStaticStr, EnumString, EnumProperty, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
pub enum Gender {
    Masculine,
    Feminine,
    /// (Neuter is the default if left unspecified)
    Neuter,
}

impl Default for Gender {
    fn default() -> Self {
        Gender::Neuter
    }
}

/// [Spec](https://docs.citationstyles.org/en/stable/specification.html#ordinal-suffixes)
/// LastTwoDigits is the default
#[derive(AsStaticStr, EnumString, EnumProperty, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
pub enum OrdinalMatch {
    /// Default for `Mod100(n) if n < 10`. Matches 9, 29, 109, 129.
    LastDigit,
    /// Default for `Mod100(n) if n >= 10`. Matches 9, 109.
    LastTwoDigits,
    /// Not a default. Matches 9.
    WholeNumber,
}

impl Default for OrdinalMatch {
    fn default() -> Self {
        OrdinalMatch::LastDigit
    }
}

/// [Spec](https://docs.citationstyles.org/en/stable/specification.html#locators)
#[derive(AsRefStr, EnumProperty, EnumString, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
#[strum(serialize_all = "kebab_case")]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
#[non_exhaustive]
#[repr(u32)]
pub enum LocatorType {
    Book,
    Chapter,
    Column,
    Figure,
    Folio,
    Issue,
    Line,
    Note,
    Opus,
    Page,
    Paragraph,
    Part,
    Section,
    // hyphenated is when it's a variable matcher, spaced is as a term name
    #[strum(serialize = "sub-verbo", serialize = "sub verbo")]
    #[cfg_attr(feature = "serde", serde(rename = "sub-verbo", alias = "sub verbo"))]
    SubVerbo,
    Verse,
    Volume,

    #[strum(props(feature = "legal_locators"))]
    Article,
    #[strum(props(feature = "legal_locators"))]
    Subparagraph,
    #[strum(props(feature = "legal_locators"))]
    Rule,
    #[strum(props(feature = "legal_locators"))]
    Subsection,
    #[strum(props(feature = "legal_locators"))]
    Schedule,
    #[strum(props(feature = "legal_locators"))]
    Title,

    /// feature = term_unpublished
    #[strum(props(feature = "term_unpublished"))]
    Unpublished,

    // Not documented but in use?
    #[strum(props(csl = "0", cslM = "1"))]
    Supplement,
}

impl Default for LocatorType {
    fn default() -> Self {
        LocatorType::Page
    }
}

/// [Spec](https://docs.citationstyles.org/en/stable/specification.html#quotes)
#[derive(AsRefStr, EnumProperty, EnumString, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
pub enum QuoteTerm {
    OpenQuote,
    CloseQuote,
    OpenInnerQuote,
    CloseInnerQuote,
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Copy, Clone, PartialEq, Eq, Hash)]
// Strum's auto kebab_case doesn't hyphenate to "season-01", so manual it is
pub enum SeasonTerm {
    #[strum(serialize = "season-01")]
    Season01,
    #[strum(serialize = "season-02")]
    Season02,
    #[strum(serialize = "season-03")]
    Season03,
    #[strum(serialize = "season-04")]
    Season04,
}

/// Yes, this differs slightly from NameVariable.
/// It includes "editortranslator" for the names special case.
#[derive(AsRefStr, EnumProperty, EnumString, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
#[non_exhaustive]
pub enum RoleTerm {
    Author,
    CollectionEditor,
    Composer,
    ContainerAuthor,
    Director,
    Editor,
    EditorialDirector,
    #[strum(serialize = "editortranslator")]
    EditorTranslator,
    Illustrator,
    Interviewer,
    OriginalAuthor,
    Recipient,
    ReviewedAuthor,
    Translator,

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

impl RoleTerm {
    pub fn from_name_var(var: NameVariable) -> Option<Self> {
        // TODO: how do we get RoleTerm::EditorTranslator?
        Some(match var {
            NameVariable::Author => RoleTerm::Author,
            NameVariable::CollectionEditor => RoleTerm::CollectionEditor,
            NameVariable::Composer => RoleTerm::Composer,
            NameVariable::ContainerAuthor => RoleTerm::ContainerAuthor,
            NameVariable::Director => RoleTerm::Director,
            NameVariable::Editor => RoleTerm::Editor,
            NameVariable::EditorialDirector => RoleTerm::EditorialDirector,
            NameVariable::EditorTranslator => RoleTerm::EditorTranslator,
            NameVariable::Illustrator => RoleTerm::Illustrator,
            NameVariable::Interviewer => RoleTerm::Interviewer,
            NameVariable::OriginalAuthor => RoleTerm::OriginalAuthor,
            NameVariable::Recipient => RoleTerm::Recipient,
            NameVariable::ReviewedAuthor => RoleTerm::ReviewedAuthor,
            NameVariable::Translator => RoleTerm::Translator,
            NameVariable::Curator => RoleTerm::Curator,
            NameVariable::ScriptWriter => RoleTerm::ScriptWriter,
            NameVariable::Performer => RoleTerm::Performer,
            NameVariable::Producer => RoleTerm::Producer,
            NameVariable::ExecutiveProducer => RoleTerm::ExecutiveProducer,
            NameVariable::Guest => RoleTerm::Guest,
            NameVariable::Narrator => RoleTerm::Narrator,
            NameVariable::Chair => RoleTerm::Chair,
            NameVariable::Compiler => RoleTerm::Compiler,
            NameVariable::Contributor => RoleTerm::Contributor,
            NameVariable::SeriesCreator => RoleTerm::SeriesCreator,
            NameVariable::Organizer => RoleTerm::Organizer,
            NameVariable::Host => RoleTerm::Host,
            // CSL-M only
            NameVariable::Authority => {
                warn!("unimplemented: CSL-M authority role term");
                return None;
            }
            // CSL-M only
            NameVariable::Dummy => return None,
        })
    }
}

/// This is all the "miscellaneous" terms from the spec, EXCEPT `edition`. Edition is the only one
/// that matches "terms accompanying the number variables" in [option (a)
/// here](https://docs.citationstyles.org/en/stable/specification.html#gender-specific-ordinals)

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[strum(serialize_all = "kebab_case")]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
#[non_exhaustive]
pub enum Category {
    Anthropology,
    Astronomy,
    Biology,
    Botany,
    Chemistry,
    Communications,
    Engineering,
    /// Used for generic styles like Harvard and APA
    GenericBase,
    Geography,
    Geology,
    History,
    Humanities,
    Law,
    Linguistics,
    Literature,
    Math,
    Medicine,
    Philosophy,
    Physics,
    /// Accepts both kebab-case and snake_case as the snake_case is anomalous
    #[strum(serialize = "political-science", serialize = "political_science")]
    PoliticalScience,
    Psychology,
    Science,
    /// Accepts both kebab-case and snake_case as the snake_case is anomalous
    #[strum(serialize = "social-science", serialize = "social_science")]
    SocialScience,
    Sociology,
    Theology,
    Zoology,
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
#[non_exhaustive]
pub enum MiscTerm {
    Accessed,
    Ad,
    And,
    #[strum(serialize = "and others")]
    AndOthers,
    Anonymous,
    At,
    #[strum(serialize = "available at")]
    AvailableAt,
    Bc,
    By,
    Circa,
    Cited,
    // Edition,
    EtAl,
    Forthcoming,
    From,
    Ibid,
    In,
    #[strum(serialize = "in press")]
    InPress,
    Internet,
    Interview,
    Letter,
    #[strum(serialize = "no date")]
    NoDate,
    Online,
    #[strum(serialize = "presented at")]
    PresentedAt,
    Reference,
    Retrieved,
    Scale,
    Version,

    // not technically in the list in either spec:

    // https://github.com/Juris-M/citeproc-js/blob/30ceaf50a0ef86517a9a8cd46362e450133c7f91/src/util_number.js#L522-L545
    // https://github.com/Juris-M/citeproc-js/blob/30ceaf50a0ef86517a9a8cd46362e450133c7f91/src/node_datepart.js#L164-L176
    PageRangeDelimiter,
    YearRangeDelimiter,
}

/// [Spec](https://docs.citationstyles.org/en/stable/specification.html#months)
#[derive(AsRefStr, EnumProperty, EnumString, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MonthTerm {
    #[strum(serialize = "month-01")]
    Month01,
    #[strum(serialize = "month-02")]
    Month02,
    #[strum(serialize = "month-03")]
    Month03,
    #[strum(serialize = "month-04")]
    Month04,
    #[strum(serialize = "month-05")]
    Month05,
    #[strum(serialize = "month-06")]
    Month06,
    #[strum(serialize = "month-07")]
    Month07,
    #[strum(serialize = "month-08")]
    Month08,
    #[strum(serialize = "month-09")]
    Month09,
    #[strum(serialize = "month-10")]
    Month10,
    #[strum(serialize = "month-11")]
    Month11,
    #[strum(serialize = "month-12")]
    Month12,
}

impl MonthTerm {
    pub fn from_u32(m: u32) -> Option<Self> {
        use self::MonthTerm::*;
        match m {
            1 => Some(Month01),
            2 => Some(Month02),
            3 => Some(Month03),
            4 => Some(Month04),
            5 => Some(Month05),
            6 => Some(Month06),
            7 => Some(Month07),
            8 => Some(Month08),
            9 => Some(Month09),
            10 => Some(Month10),
            11 => Some(Month11),
            12 => Some(Month12),
            _ => None,
        }
    }
}

/// [Spec](https://docs.citationstyles.org/en/stable/specification.html#quotes)
#[derive(EnumProperty, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum OrdinalTerm {
    Ordinal,
    Mod100(u32, OrdinalMatch),
    LongOrdinal01,
    LongOrdinal02,
    LongOrdinal03,
    LongOrdinal04,
    LongOrdinal05,
    LongOrdinal06,
    LongOrdinal07,
    LongOrdinal08,
    LongOrdinal09,
    LongOrdinal10,
}

impl OrdinalTerm {
    /// Returns 0 for OrdinalTerm::Ordinal, and clipped by the match property for Mod100. Useful for fallback to ordinal terms.
    pub fn to_number(&self) -> u32 {
        match self {
            OrdinalTerm::Ordinal => 0,
            OrdinalTerm::Mod100(n, m) => match m {
                OrdinalMatch::LastDigit => n % 10,
                OrdinalMatch::LastTwoDigits | OrdinalMatch::WholeNumber => n % 100,
            },
            OrdinalTerm::LongOrdinal01 => 1,
            OrdinalTerm::LongOrdinal02 => 2,
            OrdinalTerm::LongOrdinal03 => 3,
            OrdinalTerm::LongOrdinal04 => 4,
            OrdinalTerm::LongOrdinal05 => 5,
            OrdinalTerm::LongOrdinal06 => 6,
            OrdinalTerm::LongOrdinal07 => 7,
            OrdinalTerm::LongOrdinal08 => 8,
            OrdinalTerm::LongOrdinal09 => 9,
            OrdinalTerm::LongOrdinal10 => 10,
        }
    }
    pub fn from_number_long(n: u32) -> Self {
        match n {
            1 => OrdinalTerm::LongOrdinal01,
            2 => OrdinalTerm::LongOrdinal02,
            3 => OrdinalTerm::LongOrdinal03,
            4 => OrdinalTerm::LongOrdinal04,
            5 => OrdinalTerm::LongOrdinal05,
            6 => OrdinalTerm::LongOrdinal06,
            7 => OrdinalTerm::LongOrdinal07,
            8 => OrdinalTerm::LongOrdinal08,
            9 => OrdinalTerm::LongOrdinal09,
            10 => OrdinalTerm::LongOrdinal10,
            // Less code than panicking
            _ => OrdinalTerm::Ordinal,
        }
    }
    pub fn from_number_for_selector(n: u32, long: bool) -> Self {
        if long && n > 0 && n <= 10 {
            OrdinalTerm::from_number_long(n)
        } else {
            OrdinalTerm::Mod100(n % 100, OrdinalMatch::WholeNumber)
        }
    }
}

// impl std::convert::AsRef<str> for OrdinalTerm {
//     fn as_ref(&self) -> &str {
//         use self::OrdinalTerm::*;
//         match *self {
//             Ordinal => "ordinal",
//             LongOrdinal01 => "long-ordinal-01",
//             LongOrdinal02 => "long-ordinal-02",
//             LongOrdinal03 => "long-ordinal-03",
//             LongOrdinal04 => "long-ordinal-04",
//             LongOrdinal05 => "long-ordinal-05",
//             LongOrdinal06 => "long-ordinal-06",
//             LongOrdinal07 => "long-ordinal-07",
//             LongOrdinal08 => "long-ordinal-08",
//             LongOrdinal09 => "long-ordinal-09",
//             LongOrdinal10 => "long-ordinal-10",
//             Mod100(u) => {
//                 format!("ordinal-{:02}", u).as_ref()
//             },
//         }
//     }
// }

/// For Mod100, gives the default OrdinalMatch only; that must be parsed separately and overwritten
/// if present.
impl FromStr for OrdinalTerm {
    type Err = UnknownAttributeValue;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use self::OrdinalTerm::*;
        match s {
            "ordinal" => Ok(Ordinal),
            "long-ordinal-01" => Ok(LongOrdinal01),
            "long-ordinal-02" => Ok(LongOrdinal02),
            "long-ordinal-03" => Ok(LongOrdinal03),
            "long-ordinal-04" => Ok(LongOrdinal04),
            "long-ordinal-05" => Ok(LongOrdinal05),
            "long-ordinal-06" => Ok(LongOrdinal06),
            "long-ordinal-07" => Ok(LongOrdinal07),
            "long-ordinal-08" => Ok(LongOrdinal08),
            "long-ordinal-09" => Ok(LongOrdinal09),
            "long-ordinal-10" => Ok(LongOrdinal10),
            _ => {
                if let Ok(("", n)) = zero_through_99(s) {
                    Ok(OrdinalTerm::Mod100(n, OrdinalMatch::default_for(n)))
                } else {
                    Err(UnknownAttributeValue::new(s))
                }
            }
        }
    }
}

use nom::{
    bytes::complete::{tag, take_while_m_n},
    combinator::map_res,
    sequence::preceded,
    IResult,
};

fn is_digit(chr: char) -> bool {
    chr as u8 >= 0x30 && (chr as u8) <= 0x39
}

fn two_digit_num(inp: &str) -> IResult<&str, u32> {
    map_res(take_while_m_n(2, 2, is_digit), |s: &str| s.parse())(inp)
}

fn zero_through_99(inp: &str) -> IResult<&str, u32> {
    preceded(tag("ordinal-"), two_digit_num)(inp)
}

#[cfg(test)]
#[test]
fn test_ordinals() {
    assert_eq!(
        Ok(OrdinalTerm::Mod100(34, OrdinalMatch::default_for(34))),
        OrdinalTerm::from_str("ordinal-34")
    );
    assert_eq!(
        OrdinalTerm::from_str("long-ordinal-08"),
        Ok(OrdinalTerm::LongOrdinal08)
    );
    assert!(OrdinalTerm::from_str("ordinal-129").is_err());
}
