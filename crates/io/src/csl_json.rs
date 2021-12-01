// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

// We implement serde::de::Deserialize for CSL-JSON spec for now.
// If you want to add a new input format, you can write one
// e.g. with a bibtex parser https://github.com/charlesvdv/nom-bibtex

mod cow_str;

use crate::names::Name;
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::de::{Error, IgnoredAny};
use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

// You have to know which variant we're using before parsing a reference.
// Why? Because some variables are numbers in CSL-M, but standard vars in CSL. And other
// differences.
// It might be possible to go without this, by making anything that's a number in either variant
// Definitely a number, and enforcing it on the proc phase.
use csl::AnyVariable;
use csl::CslType;
use csl::Features;
use csl::GetAttribute;
use csl::Lang;

use super::date::{Date, DateOrRange};
use super::reference::Reference;
use fnv::FnvHashMap;
use std::marker::PhantomData;

struct LanguageVisitor;

impl<'de> Visitor<'de> for LanguageVisitor {
    type Value = Lang;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid language code")
    }

    fn visit_str<E>(self, key: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match Lang::parse(key) {
            Ok(lang) => Ok(lang),
            Err((_garbage, Some(half_parsed))) => Ok(half_parsed),
            Err((_remain, _)) => Err(de::Error::invalid_value(de::Unexpected::Str(key), &self)),
        }
    }
}

pub struct MaybeDate(Option<DateOrRange>);

pub struct WrapLang(Option<Lang>);

impl<'de> Deserialize<'de> for WrapLang {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(deserializer
            .deserialize_identifier(LanguageVisitor)
            .map(Some)
            .map(WrapLang)
            // For non-English garbage parses, see locale_TitleCaseGarbageLangEmptyLocale
            .unwrap_or(WrapLang(Some(Lang::klingon()))))
    }
}

struct CslVariantVisitor<T>(Features, &'static [&'static str], PhantomData<T>);

impl<'de, T: GetAttribute> Visitor<'de> for CslVariantVisitor<T> {
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("any variable")
    }

    fn visit_str<E>(self, key: &str) -> Result<T, E>
    where
        E: de::Error,
    {
        T::get_attr(key, &self.0).map_err(|_e| de::Error::unknown_field(key, self.1))
    }
}

#[derive(Debug, Deserialize)]
#[serde(field_identifier, rename_all = "kebab-case")]
enum Field<'a> {
    Id,
    Type,
    Language,
    // don't use plain `&'a str`, because that would fail when parsing from a serde::Value.
    #[serde(borrow, deserialize_with = "cow_str::deserialize_cow_str")]
    Any(Cow<'a, str>),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Hash)]
#[serde(untagged)]
pub enum NumberLike {
    Str(String),
    Num(u32),
}

impl NumberLike {
    pub fn into_string(self) -> String {
        match self {
            NumberLike::Str(s) => s,
            NumberLike::Num(i) => i.to_string(),
        }
    }
    pub fn to_number(&self) -> Result<u32, std::num::ParseIntError> {
        match self {
            NumberLike::Str(s) => s.parse(),
            NumberLike::Num(i) => Ok(*i),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum RelaxedBool {
    Str(String),
    Num(i32),
    Bool(bool),
}

impl Into<bool> for RelaxedBool {
    fn into(self) -> bool {
        self.to_bool()
    }
}

impl RelaxedBool {
    pub fn deserialize_bool<'de, D: serde::de::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
        Self::deserialize(d).map(|circa| circa.to_bool())
    }
    pub fn to_bool(&self) -> bool {
        match *self {
            RelaxedBool::Str(ref s) => s == "true",
            RelaxedBool::Num(n) => n != 0i32,
            RelaxedBool::Bool(value) => value,
        }
    }
}

struct WrapType(CslType);

impl<'de> Deserialize<'de> for WrapType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const FIELDS: &[&str] = &["a legal CSL type"];
        deserializer
            .deserialize_identifier(CslVariantVisitor(
                Features::new(),
                FIELDS,
                Default::default(),
            ))
            .map(WrapType)
    }
}

#[derive(Debug)]
struct WrapVar(AnyVariable);

impl<'de> Deserialize<'de> for WrapVar {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const FIELDS: &[&str] = &["any CSL variable"];
        deserializer
            .deserialize_identifier(CslVariantVisitor(
                Features::new(),
                FIELDS,
                Default::default(),
            ))
            .map(WrapVar)
    }
}

impl<'de> Deserialize<'de> for Reference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ReferenceVisitor;

        impl<'de> Visitor<'de> for ReferenceVisitor {
            type Value = Reference;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Reference")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut id: Option<NumberLike> = None;
                let mut csl_type: Option<WrapType> = None;
                let mut language = None;
                let mut ordinary = FnvHashMap::default();
                let mut number = FnvHashMap::default();
                let mut name = FnvHashMap::default();
                let mut date = FnvHashMap::default();
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Id => {
                            id = Some(map.next_value()?);
                        }
                        Field::Type => {
                            csl_type = Some(map.next_value()?);
                        }
                        Field::Language => {
                            let wrap: WrapLang = map.next_value()?;
                            language = wrap.0;
                        }
                        Field::Any(var_name) => {
                            match AnyVariable::get_attr(&var_name, &Features::default()) {
                                Err(_unknown) => {
                                    // Unknown variable. Let it slide.
                                    log::warn!("reference had unknown variable `{}`", var_name);
                                    let _: IgnoredAny = map.next_value()?;
                                }
                                Ok(AnyVariable::Ordinary(v)) => {
                                    ordinary.insert(v, map.next_value()?);
                                }
                                Ok(AnyVariable::Number(v)) => {
                                    number.insert(v, map.next_value()?);
                                }
                                Ok(AnyVariable::Name(v)) => {
                                    let names: Vec<Name> = map.next_value()?;
                                    name.insert(v, names);
                                }
                                Ok(AnyVariable::Date(v)) => {
                                    if let MaybeDate(Some(d)) = map.next_value()? {
                                        date.insert(v, d);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Reference {
                    id: id
                        .map(|i| csl::Atom::from(i.into_string()))
                        .ok_or_else(|| de::Error::missing_field("id"))?,
                    csl_type: csl_type.unwrap_or(WrapType(CslType::Article)).0,
                    language,
                    ordinary,
                    number,
                    name,
                    date,
                })
            }
        }

        const FIELDS: &[&str] = &["id", "type", "any variable name"];
        deserializer.deserialize_struct("Reference", FIELDS, ReferenceVisitor)
    }
}

// newtype these so we can have a different implementation
struct DateParts(Option<DateOrRange>);

struct DateInt(Option<i32>);

impl<'de> Deserialize<'de> for DateInt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ParseIntVisitor;
        impl<'de> Visitor<'de> for ParseIntVisitor {
            type Value = DateInt;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an integer or a string that's actually just an integer")
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(&value)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.is_empty() {
                    return Ok(DateInt(None));
                }
                value
                    .parse::<i32>()
                    .map_err(|_| de::Error::invalid_value(de::Unexpected::Str(value), &self))
                    .map(Some)
                    .map(DateInt)
            }

            fn visit_i8<E>(self, value: i8) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DateInt(Some(i32::from(value))))
            }

            fn visit_i16<E>(self, value: i16) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DateInt(Some(i32::from(value))))
            }

            fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DateInt(Some(value)))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                use std::i32;
                if value >= i64::from(i32::MIN) && value <= i64::from(i32::MAX) {
                    Ok(DateInt(Some(value as i32)))
                } else {
                    Err(E::custom(format!("i32 out of range: {}", value)))
                }
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                use std::u16;
                if value >= u64::from(u16::MIN) && value <= u64::from(u16::MAX - 1) {
                    Ok(DateInt(Some(value as i32)))
                } else {
                    Err(E::custom(format!("i32 out of range: {}", value)))
                }
            }
        }
        deserializer.deserialize_any(ParseIntVisitor)
    }
}

struct OptDate(Option<Date>);

impl<'de> Deserialize<'de> for OptDate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SingleDatePartVisitor;

        impl<'de> Visitor<'de> for SingleDatePartVisitor {
            type Value = OptDate;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a date-part as a number or string-number, e.g. 2004, \"8\"")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: SeqAccess<'de>,
            {
                if let Some(DateInt(Some(year))) = seq.next_element::<DateInt>()? {
                    let month = seq
                        .next_element::<DateInt>()?
                        .unwrap_or(DateInt(None))
                        .0
                        .unwrap_or(0);
                    let day = seq
                        .next_element::<DateInt>()?
                        .unwrap_or(DateInt(None))
                        .0
                        .unwrap_or(0);

                    // ignore any additional entries in the array
                    while let Some(_) = seq.next_element::<IgnoredAny>()? {}

                    let month = if month >= 1 && month <= 16 {
                        month
                    } else if month >= 21 && month <= 24 {
                        month - 8
                    } else {
                        0
                    };
                    let day = if day >= 1 && day <= 31 { day } else { 0 };
                    Ok(OptDate(Some(Date::new(year, month as u32, day as u32))))
                } else {
                    Ok(OptDate(None))
                }
            }
        }

        deserializer.deserialize_seq(SingleDatePartVisitor)
    }
}

impl<'de> Deserialize<'de> for DateParts {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct DatePartsVisitor;
        impl<'de> Visitor<'de> for DatePartsVisitor {
            type Value = DateParts;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a date-parts block, e.g. [[2004,8,19]]")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: SeqAccess<'de>,
            {
                if let Some(OptDate(Some(from))) = seq.next_element()? {
                    let result = match seq.next_element()? {
                        Some(OptDate(Some(to))) => {
                            Ok(DateParts(Some(DateOrRange::Range(from, to))))
                        }
                        _ => Ok(DateParts(Some(DateOrRange::Single(from)))),
                    };
                    // ignore any additional date arrays (nonsense)
                    while let Some(_) = seq.next_element::<IgnoredAny>()? {}
                    result
                } else {
                    Ok(DateParts(None))
                }
            }
        }
        deserializer.deserialize_seq(DatePartsVisitor)
    }
}

impl<'de> Deserialize<'de> for MaybeDate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "kebab-case")]
        enum DateType<'a> {
            DateParts,
            Season,
            Circa,
            Literal,
            Raw,
            Year,
            Edtf,
            #[serde(borrow, deserialize_with = "cow_str::deserialize_cow_str")]
            Unknown(Cow<'a, str>),
        }

        struct DateVisitor;

        impl<'de> Visitor<'de> for DateVisitor {
            type Value = MaybeDate;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a date")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                FromStr::from_str(value)
                    .or_else(|_| {
                        Ok(DateOrRange::Literal {
                            literal: value.into(),
                            circa: false,
                        })
                    })
                    .map(|x| MaybeDate(Some(x)))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                FromStr::from_str(&value)
                    .or_else(|_| {
                        Ok(DateOrRange::Literal {
                            literal: value.into(),
                            circa: false,
                        })
                    })
                    .map(|x| MaybeDate(Some(x)))
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut found = None;
                let mut found_season: Option<NumberLike> = None;
                let mut found_circa: Option<bool> = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        DateType::Raw => {
                            let v: Cow<'de, str> = map.next_value()?;
                            if found.is_none() {
                                found = Some(DateOrRange::from_str(&v).unwrap_or_else(|_| {
                                    DateOrRange::Literal {
                                        literal: v.as_ref().into(),
                                        circa: false,
                                    }
                                }))
                            }
                        }
                        DateType::Literal => {
                            found = Some(DateOrRange::Literal {
                                literal: map.next_value()?,
                                circa: false,
                            })
                        }
                        DateType::DateParts => {
                            let dp: DateParts = match map.next_value() {
                                Ok(dp) => dp,
                                Err(e) => {
                                    log::warn!("failed to parse date-parts: {:?}", e);
                                    continue;
                                }
                            };
                            if dp.0.is_some() {
                                found = dp.0;
                            }
                        }
                        DateType::Edtf => {
                            log::warn!("unimplemented: edtf date support");
                            let _: IgnoredAny = map.next_value()?;
                            continue;
                        }
                        DateType::Season => found_season = Some(map.next_value()?),
                        DateType::Circa => {
                            if let Ok(circa) = map.next_value::<RelaxedBool>() {
                                found_circa = Some(circa.to_bool())
                            } else {
                                log::warn!("invalid value for circa");
                            }
                        }
                        DateType::Year => {
                            if let Ok(year) = map.next_value() {
                                let date = Date {
                                    year,
                                    month: 0,
                                    day: 0,
                                    circa: false,
                                };
                                found = Some(DateOrRange::Single(date));
                            }
                        }
                        DateType::Unknown(k) => {
                            log::warn!("unknown date variable key: {}", k);
                            let _: IgnoredAny = map.next_value()?;
                        }
                    }
                }
                Ok(found
                    .ok_or(())
                    // .ok_or_else(|| de::Error::missing_field("raw|literal|etc"))
                    .and_then(|mut found| {
                        if let Some(season) = found_season {
                            if let DateOrRange::Single(ref mut date) = found {
                                if !date.has_day() && !date.has_month() {
                                    let season = season
                                        .to_number()
                                        .map_err(|e| {
                                            V::Error::custom(format!(
                                                "season {:?} was not an integer: {}",
                                                season, e
                                            ))
                                        })
                                        .and_then(|unsigned| {
                                            if unsigned < 1 || unsigned > 4 {
                                                Err(V::Error::custom(format!(
                                                    "season {} was not in range [1, 4]",
                                                    unsigned
                                                )))
                                            } else {
                                                Ok(unsigned as u32)
                                            }
                                        });
                                    if let Ok(mut season) = season {
                                        if season > 20 {
                                            // handle 21, 22, 23, 24
                                            season -= 20;
                                        }
                                        date.month = season + 12;
                                    }
                                }
                            }
                        }
                        if let Some(circa) = found_circa {
                            found.set_circa(circa)
                        }
                        Ok(MaybeDate(Some(found)))
                    })
                    .ok()
                    .unwrap_or(MaybeDate(None)))
            }
        }

        const DATE_TYPES: &[&str] = &["date-parts", "season", "circa", "literal", "raw"];
        deserializer.deserialize_struct("DateOrRange", DATE_TYPES, DateVisitor)
    }
}
