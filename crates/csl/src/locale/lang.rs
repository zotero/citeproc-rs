// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use crate::SmartString;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum LocaleSource {
    Inline(Option<Lang>),
    File(Lang),
}

/// A parsable representation of `xml:lang`.
///
/// See http://www.datypic.com/sc/xsd/t-xsd_language.html
#[derive(Debug, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum Lang {
    /// ISO 639 language code, + optional hyphen and 2-letter ISO 3166 country code.
    ///
    /// i.e. `en` or `en-US`
    Iso(IsoLang, Option<IsoCountry>),
    /// IANA-assigned language codes
    Iana(SmartString),
    /// Agreed upon language ID (max 8 characters). You'll absolutely have to provide your own
    /// locale file.
    Unofficial(SmartString),
}

impl Default for Lang {
    fn default() -> Self {
        Lang::en_us()
    }
}

impl fmt::Display for Lang {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Lang::Iso(l, None) => write!(f, "{}", l),
            Lang::Iso(l, Some(c)) => write!(f, "{}-{}", l, c),
            Lang::Iana(u) => write!(f, "i-{}", u),
            Lang::Unofficial(u) => write!(f, "x-{}", u),
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Lang {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Lang {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl Lang {
    pub fn en_us() -> Self {
        Lang::Iso(IsoLang::English, Some(IsoCountry::US))
    }
    pub fn klingon() -> Self {
        Lang::Iso(IsoLang::Klingon, None)
    }
    #[cfg(test)]
    pub fn en_au() -> Self {
        Lang::Iso(IsoLang::English, Some(IsoCountry::AU))
    }
    pub fn iter(&self) -> impl Iterator<Item = LocaleSource> {
        use std::iter::once;
        self.inline_iter()
            .map(Some)
            .chain(once(None))
            .map(LocaleSource::Inline)
            .chain(self.file_iter().map(LocaleSource::File))
    }
    pub fn iter_fetchable_langs(&self) -> impl Iterator<Item = Lang> {
        self.iter().filter_map(|source| match source {
            LocaleSource::File(l) => Some(l),
            _ => None,
        })
    }
    fn file_iter(&self) -> FileIter {
        FileIter {
            current: Some(self.clone()),
        }
    }
    fn inline_iter(&self) -> InlineIter {
        InlineIter {
            current: Some(self.clone()),
        }
    }

    /// Useful for title-casing.
    pub fn is_english(&self) -> bool {
        match self {
            Lang::Iso(IsoLang::English, _) => true,
            _ => false,
        }
    }
}

use crate::attr::GetAttribute;
use crate::error::UnknownAttributeValue;
use crate::version::Features;
impl GetAttribute for Lang {
    fn get_attr(s: &str, _: &Features) -> Result<Self, UnknownAttributeValue> {
        Lang::from_str(s).map_err(|_| UnknownAttributeValue::new(s))
    }
}

#[test]
fn test_inline_iter() {
    let de_at = Lang::Iso(IsoLang::Deutsch, Some(IsoCountry::AT));
    let de = Lang::Iso(IsoLang::Deutsch, None);
    assert_eq!(de_at.inline_iter().collect::<Vec<_>>(), &[de_at, de]);
}

#[test]
fn test_file_iter() {
    let de_at = Lang::Iso(IsoLang::Deutsch, Some(IsoCountry::AT));
    let de_de = Lang::Iso(IsoLang::Deutsch, Some(IsoCountry::DE));
    let en_us = Lang::Iso(IsoLang::English, Some(IsoCountry::US));
    assert_eq!(
        de_at.file_iter().collect::<Vec<_>>(),
        &[de_at, de_de, en_us]
    );
}

#[test]
fn test_french() {
    let fr_fr = Lang::Iso(IsoLang::French, Some(IsoCountry::FR));
    let fr = Lang::Iso(IsoLang::French, None);
    let en_us = Lang::Iso(IsoLang::English, Some(IsoCountry::US));
    assert_eq!(
        fr_fr.iter().collect::<Vec<_>>(),
        &[
            LocaleSource::Inline(Some(fr_fr.clone())),
            LocaleSource::Inline(Some(fr)),
            LocaleSource::Inline(None),
            LocaleSource::File(fr_fr),
            LocaleSource::File(en_us),
        ]
    );
}

/// Language codes for `Lang::Iso`.
///
/// The 3-character codes are ISO 639-3.
#[derive(Debug, Clone, Eq, PartialEq, Hash, EnumString, PartialOrd, Ord)]
pub enum IsoLang {
    #[strum(serialize = "en", serialize = "eng")]
    English,
    #[strum(serialize = "de", serialize = "deu")]
    Deutsch,
    #[strum(serialize = "pt", serialize = "por")]
    Portuguese,
    #[strum(serialize = "zh", serialize = "zho")]
    Chinese,
    #[strum(serialize = "fr", serialize = "fra")]
    French,
    #[strum(serialize = "es", serialize = "esp")]
    Spanish,
    #[strum(serialize = "ja", serialize = "jpn")]
    Japanese,
    #[strum(serialize = "ar", serialize = "ara")]
    Arabic,

    // For non-English garbage parses, see locale_TitleCaseGarbageLangEmptyLocale
    #[strum(serialize = "tlh")]
    Klingon,
    /// The rest are not part of the fallback relation, so just treat them as strings.
    ///
    /// Also we save allocations for some popular languages!
    #[strum(default)]
    Other(SmartString),
}

impl IsoLang {
    fn short_code(&self) -> String {
        let s = match self {
            IsoLang::English => "en",
            IsoLang::Deutsch => "de",
            IsoLang::Portuguese => "pt",
            IsoLang::Spanish => "es",
            IsoLang::French => "fr",
            IsoLang::Chinese => "zh",
            IsoLang::Japanese => "ja",
            IsoLang::Arabic => "ar",
            IsoLang::Klingon => "tlh",
            IsoLang::Other(ref o) => &o,
        };
        String::from(s)
    }
}

impl fmt::Display for IsoLang {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.short_code())
    }
}

/// Countries for use `Lang::Iso` dialects.
///
/// These countries are used to do dialect fallback. Countries not used in that can be represented
/// as `IsoCountry::Other`. If a country is in the list, you don't need to allocate to refer to it,
/// so there are some non-participating countries in the list simply because it's faster.
#[derive(Debug, Clone, Eq, PartialEq, Hash, EnumString, PartialOrd, Ord)]
pub enum IsoCountry {
    /// United States
    US,
    /// Great Britain
    GB,
    /// Australia
    AU,
    /// Deutschland
    DE,
    /// Austria
    AT,
    /// Switzerland
    CH,
    /// China
    CN,
    /// Taiwan
    TW,
    /// Portugal
    PT,
    /// Brazil
    BR,
    /// Japan
    JP,
    /// Spain
    ES,
    /// France
    FR,
    /// Canada
    CA,
    #[strum(default)]
    Other(SmartString),
}

impl fmt::Display for IsoCountry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            IsoCountry::Other(ref o) => write!(f, "{}", o),
            _ => write!(f, "{:?}", self),
        }
    }
}

struct FileIter {
    current: Option<Lang>,
}

struct InlineIter {
    current: Option<Lang>,
}

use std::mem;

impl Iterator for FileIter {
    type Item = Lang;
    fn next(&mut self) -> Option<Lang> {
        use self::IsoCountry::*;
        use self::IsoLang::*;
        use self::Lang::*;
        let next = self.current.as_ref().and_then(|curr| match curr {
            // Technically speaking most countries' English dialects are closer to en-GB than en-US,
            // but predictably implementing the spec is more important.
            Iso(English, Some(co)) if *co != US => Some(Iso(English, Some(US))),
            Iso(English, Some(US)) => None,
            Iso(Deutsch, Some(co)) if *co != DE => Some(Iso(Deutsch, Some(DE))),
            Iso(French, Some(co)) if *co != FR => Some(Iso(French, Some(FR))),
            Iso(Portuguese, Some(co)) if *co != PT => Some(Iso(Portuguese, Some(PT))),
            Iso(Chinese, Some(TW)) => Some(Iso(Chinese, Some(CN))),
            _ => Some(Iso(English, Some(US))),
        });
        mem::replace(&mut self.current, next)
    }
}

impl Iterator for InlineIter {
    type Item = Lang;
    fn next(&mut self) -> Option<Lang> {
        use self::Lang::*;
        let next = self.current.as_ref().and_then(|curr| match curr {
            Iso(lang, Some(_)) => Some(Iso(lang.clone(), None)),
            _ => None,
        });
        mem::replace(&mut self.current, next)
    }
}

impl FromStr for Lang {
    type Err = String;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if let Ok((remainder, parsed)) = parse_lang(&input) {
            if remainder.is_empty() {
                Ok(parsed)
            } else {
                Err("".into())
            }
        } else {
            Err("".into())
        }
    }
}

impl Lang {
    // Error contains a half-parsed version and any trailing garbage
    pub fn parse(input: &str) -> Result<Self, (&str, Option<Self>)> {
        if let Ok((remainder, parsed)) = parse_lang_garbage(&input) {
            if remainder.is_empty() {
                Ok(parsed)
            } else {
                Err((remainder, Some(parsed)))
            }
        } else {
            Err((input, None))
        }
    }
}

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while_m_n},
    combinator::{map, opt},
    sequence::{preceded, tuple},
    IResult,
};

fn iso_lang(inp: &str) -> IResult<&str, IsoLang> {
    map(take_while_m_n(2, 3, char::is_alphabetic), |lang| {
        // You can unwrap because codegen has a default case with no Err output
        IsoLang::from_str(lang).unwrap()
    })(inp)
}

fn iso_country(inp: &str) -> IResult<&str, IsoCountry> {
    map(
        preceded(tag("-"), take_while_m_n(2, 2, char::is_alphabetic)),
        |country| {
            // You can unwrap because codegen has a default case with no Err output
            IsoCountry::from_str(country).unwrap()
        },
    )(inp)
}

fn parse_iana(inp: &str) -> IResult<&str, Lang> {
    map(preceded(tag("i-"), take_while(|_| true)), |lang| {
        Lang::Iana(SmartString::from(lang))
    })(inp)
}

fn parse_unofficial(inp: &str) -> IResult<&str, Lang> {
    map(
        preceded(tag("x-"), take_while_m_n(1, 8, char::is_alphanumeric)),
        |lang| Lang::Unofficial(SmartString::from(lang)),
    )(inp)
}

fn parse_iso(inp: &str) -> IResult<&str, Lang> {
    map(tuple((iso_lang, opt(iso_country))), |(lang, country)| {
        Lang::Iso(lang, country)
    })(inp)
}

fn parse_iso_garbage(inp: &str) -> IResult<&str, Lang> {
    let (inp, iso) = iso_lang(inp)?;
    let (inp, _) = tag("-")(inp)?;
    Ok((inp, Lang::Iso(iso, None)))
}

fn parse_lang(inp: &str) -> IResult<&str, Lang> {
    alt((parse_unofficial, parse_iana, parse_iso))(inp)
}

fn parse_lang_garbage(inp: &str) -> IResult<&str, Lang> {
    alt((parse_unofficial, parse_iana, parse_iso, parse_iso_garbage))(inp)
}

#[test]
fn lang_from_str() {
    let de_at = Lang::Iso(IsoLang::Deutsch, Some(IsoCountry::AT));
    let de = Lang::Iso(IsoLang::Deutsch, None);
    let iana = Lang::Iana("Navajo".into());
    let unofficial = Lang::Unofficial("Newspeak".into());
    assert_eq!(Lang::from_str("de-AT"), Ok(de_at));
    assert_eq!(Lang::from_str("de"), Ok(de));
    assert_eq!(Lang::from_str("i-Navajo"), Ok(iana));
    assert_eq!(Lang::from_str("x-Newspeak"), Ok(unofficial));
}
