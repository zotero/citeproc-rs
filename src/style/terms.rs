use crate::style::error::*;
use std::str::FromStr;

/// http://docs.citationstyles.org/en/stable/specification.html#locators
#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
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

/// http://docs.citationstyles.org/en/stable/specification.html#quotes
#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum QuotationMarks {
    OpenQuote,
    CloseQuote,
    OpenInnerQuote,
    CloseInnerQuote,
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum Seasons {
    Season01,
    Season02,
    Season03,
    Season04,
}

#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum Miscellaneous {
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
    Edition,
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

/// http://docs.citationstyles.org/en/stable/specification.html#months
#[derive(AsRefStr, EnumProperty, EnumString, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum Months {
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


/// http://docs.citationstyles.org/en/stable/specification.html#quotes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ordinals {
    Ordinal,
    Ordinal00ThroughOrdinal99(u32),
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

// impl std::convert::AsRef<str> for Ordinals {
//     fn as_ref(&self) -> &str {
//         use self::Ordinals::*;
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
//             Ordinal00ThroughOrdinal99(u) => {
//                 format!("ordinal-{:02}", u).as_ref()
//             },
//         }
//     }
// }


impl FromStr for Ordinals {
    type Err = UnknownAttributeValue;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use self::Ordinals::*;
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
                let segments: Vec<&str> = s.split('-').collect();
                match &segments[..] {
                    ["ordinal", val] => {
                        let n = val.parse::<u32>().unwrap_or(200);
                        if n <= 99 {
                            Ok(Ordinal00ThroughOrdinal99(n))
                        } else {
                            Err(UnknownAttributeValue::new(s))
                        }
                    }
                    _ => Err(UnknownAttributeValue::new(s))
                }
            }
        }
    }
}

#[cfg(test)]
#[test]
fn test_ordinals() {
    assert_eq!(Ok(Ordinals::Ordinal00ThroughOrdinal99(34)), Ordinals::from_str("ordinal-34"));
    assert_eq!(Ordinals::from_str("long-ordinal-08"), Ok(Ordinals::LongOrdinal08));
    assert!(Ordinals::from_str("ordinal-129").is_err());
}

