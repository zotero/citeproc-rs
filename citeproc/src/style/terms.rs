use crate::style::error::*;
use fnv::FnvHashMap;

use super::get_attribute::GetAttribute;
use nom::types::CompleteStr;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextTermSelector {
    Simple(SimpleTermSelector),
    Gendered(GenderedTermSelector),
    Role(RoleTermSelector),
    // You can't render ordinals using a <text> node, only using <number>
}

pub enum AnyTermName {
    Edition,
    Month(MonthTerm),
    Loc(LocatorType),

    Misc(MiscTerm),
    Season(SeasonTerm),
    Quote(QuoteTerm),

    Role(RoleTerm),

    Ordinal(OrdinalTerm),
}

impl GetAttribute for AnyTermName {
    fn get_attr(
        s: &str,
        csl_version: super::version::CslVersion,
    ) -> Result<Self, UnknownAttributeValue> {
        use self::AnyTermName::*;
        if let Ok(v) = MiscTerm::get_attr(s, csl_version.clone()) {
            return Ok(Misc(v));
        } else if let Ok(v) = MonthTerm::get_attr(s, csl_version.clone()) {
            return Ok(Month(v));
        } else if s == "edition" {
            return Ok(Edition);
        } else if let Ok(v) = LocatorType::get_attr(s, csl_version.clone()) {
            return Ok(Loc(v));
        } else if let Ok(v) = SeasonTerm::get_attr(s, csl_version.clone()) {
            return Ok(Season(v));
        } else if let Ok(v) = QuoteTerm::get_attr(s, csl_version.clone()) {
            return Ok(Quote(v));
        } else if let Ok(v) = RoleTerm::get_attr(s, csl_version.clone()) {
            return Ok(Role(v));
        } else if let Ok(v) = OrdinalTerm::get_attr(s, csl_version.clone()) {
            return Ok(Ordinal(v));
        }
        Err(UnknownAttributeValue::new(s))
    }
}

/// TermSelector is used
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimpleTermSelector {
    Misc(MiscTerm, TermForm),
    Season(SeasonTerm, TermForm),
    Quote(QuoteTerm, TermForm),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrdinalTermSelector(pub OrdinalTerm, pub Gender, pub OrdinalMatch);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GenderedTermSelector {
    /// Edition is the only MiscTerm that can have a gender, so it's here instead
    Edition(TermForm),
    Locator(LocatorType, TermForm),
    Month(MonthTerm, TermForm),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoleTermSelector(pub RoleTerm, pub RoleTermForm);

pub type SimpleMapping = FnvHashMap<SimpleTermSelector, TermPlurality>;
pub type GenderedMapping = FnvHashMap<GenderedTermSelector, GenderedTerm>;
pub type OrdinalMapping = FnvHashMap<OrdinalTermSelector, String>;
pub type RoleMapping = FnvHashMap<RoleTermSelector, TermPlurality>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenderedTerm(pub TermPlurality, pub Gender);

#[derive(AsRefStr, EnumString, EnumProperty, Debug, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
pub enum TermForm {
    Long,
    Short,
    Symbol,
}

impl Default for TermForm {
    fn default() -> Self {
        TermForm::Long
    }
}

/// Includes the extra Verb and VerbShort variants
#[derive(AsRefStr, EnumString, EnumProperty, Debug, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
pub enum RoleTermForm {
    Long,
    Short,
    Symbol,
    Verb,
    VerbShort,
}

impl Default for RoleTermForm {
    fn default() -> Self {
        RoleTermForm::Long
    }
}

#[derive(AsRefStr, EnumString, EnumProperty, Debug, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
pub enum TermPlurality {
    Pluralized { single: String, multiple: String },
    Always(String),
}

impl TermPlurality {
    pub fn no_plural_allowed(self) -> Option<String> {
        match self {
            TermPlurality::Always(s) => Some(s),
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
///    gets back `GenderedTerm(TermPlurality::Always("issue"), Gender::Feminine)`
/// 3. It then needs an ordinal to match `Gender::Feminine`, so it looks up, in order:
///
///    1. `OrdinalTermSelector(Mod100(1), Feminine, WholeNumber)` and finds no match;
///    2. `OrdinalTermSelector(Mod100(1), Feminine, LastTwoDigits)` and finds a match with content
///       `FFF`.
///
#[derive(AsStaticStr, EnumString, EnumProperty, Debug, Clone, PartialEq, Eq, Hash)]
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
#[derive(AsStaticStr, EnumString, EnumProperty, Debug, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
pub enum OrdinalMatch {
    LastTwoDigits,
    WholeNumber,
}

impl Default for OrdinalMatch {
    fn default() -> Self {
        OrdinalMatch::LastTwoDigits
    }
}

/// [Spec](https://docs.citationstyles.org/en/stable/specification.html#locators)
#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
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
    SubVerbo,
    Verse,
    Volume,
}

/// [Spec](https://docs.citationstyles.org/en/stable/specification.html#quotes)
#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
pub enum QuoteTerm {
    OpenQuote,
    CloseQuote,
    OpenInnerQuote,
    CloseInnerQuote,
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
pub enum SeasonTerm {
    Season01,
    Season02,
    Season03,
    Season04,
}

/// Yes, this differs slightly from NameVariable.
/// It includes "editortranslator" for the names special case.
#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
pub enum RoleTerm {
    Author,
    CollectionEditor,
    Composer,
    ContainerAuthor,
    Director,
    Editor,
    EditorialDirector,
    // No camel case on the T, that would be "editor-translator", not right
    Editortranslator,
    Illustrator,
    Interviewer,
    OriginalAuthor,
    Recipient,
    ReviewedAuthor,
    Translator,
}

/// This is all the "miscellaneous" terms from the spec, EXCEPT `edition`. Edition is the only one
/// that matches "terms accompanying the number variables" in [option (a)
/// here](https://docs.citationstyles.org/en/stable/specification.html#gender-specific-ordinals)

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
pub enum MiscTerm {
    Accessed,
    Ad,
    And,
    AndOthers,
    Anonymous,
    At,
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
    InPress,
    Internet,
    Interview,
    Letter,
    NoDate,
    Online,
    PresentedAt,
    Reference,
    Retrieved,
    Scale,
    Version,
}

/// [Spec](https://docs.citationstyles.org/en/stable/specification.html#months)
#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab_case")]
pub enum MonthTerm {
    Month01,
    Month02,
    Month03,
    Month04,
    Month05,
    Month06,
    Month07,
    Month08,
    Month00,
    Month10,
    Month11,
    Month12,
}

/// [Spec](https://docs.citationstyles.org/en/stable/specification.html#quotes)
#[derive(EnumProperty, Debug, Clone, PartialEq, Eq, Hash)]
pub enum OrdinalTerm {
    Ordinal,
    Mod100(u32),
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
                if let Ok((CompleteStr(""), o)) = zero_through_99(CompleteStr(s)) {
                    Ok(o)
                } else {
                    Err(UnknownAttributeValue::new(s))
                }
            }
        }
    }
}

fn is_digit(chr: char) -> bool {
    chr as u8 >= 0x30 && chr as u8 <= 0x39
}

named!(two_digit_num<CompleteStr, u32>,
    map_res!(
        take_while_m_n!(2, 2, is_digit),
        |s: CompleteStr| s.0.parse()
    ));

named!(zero_through_99<CompleteStr, OrdinalTerm>,
    map!(
        preceded!(tag!("ordinal-"), call!(two_digit_num)),
        |n| OrdinalTerm::Mod100(n)
    ));

#[cfg(test)]
#[test]
fn test_ordinals() {
    assert_eq!(
        Ok(OrdinalTerm::Mod100(34)),
        OrdinalTerm::from_str("ordinal-34")
    );
    assert_eq!(
        OrdinalTerm::from_str("long-ordinal-08"),
        Ok(OrdinalTerm::LongOrdinal08)
    );
    assert!(OrdinalTerm::from_str("ordinal-129").is_err());
}
