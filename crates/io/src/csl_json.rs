// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

// We implement serde::de::Deserialize for CSL-JSON spec for now.
// If you want to add a new input format, you can write one
// e.g. with a bibtex parser https://github.com/charlesvdv/nom-bibtex

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::borrow::Cow;
use std::collections::hash_map::Entry;
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
use super::numeric::NumericValue;
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
        Lang::from_str(key).map_err(|_e| de::Error::unknown_field(key, &["language"]))
    }
}

pub struct WrapLang(Lang);

impl<'de> Deserialize<'de> for WrapLang {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer
            .deserialize_identifier(LanguageVisitor)
            .map(WrapLang)
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

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "kebab-case")]
enum Field {
    Id,
    Type,
    Language,
    Any(WrapVar),
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum IdOrNumber {
    S(String),
    N(i32),
}

impl IdOrNumber {
    pub fn to_string(self) -> String {
        match self {
            IdOrNumber::S(s) => s,
            IdOrNumber::N(i) => i.to_string(),
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
                let mut id: Option<IdOrNumber> = None;
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
                            language = Some(map.next_value()?).map(|WrapLang(l)| l);
                        }
                        Field::Any(WrapVar(AnyVariable::Ordinary(v))) => {
                            match ordinary.entry(v) {
                                Entry::Occupied(_) => {
                                    // TODO: don't use Debug for this
                                    return Err(de::Error::duplicate_field("dunno"));
                                }
                                Entry::Vacant(ve) => {
                                    ve.insert(map.next_value()?);
                                }
                            }
                        }
                        Field::Any(WrapVar(AnyVariable::Number(v))) => {
                            match number.entry(v) {
                                Entry::Occupied(_) => {
                                    // TODO: don't use Debug for this
                                    return Err(de::Error::duplicate_field("dunno"));
                                }
                                Entry::Vacant(ve) => {
                                    ve.insert(map.next_value()?);
                                }
                            }
                        }
                        Field::Any(WrapVar(AnyVariable::Name(v))) => {
                            match name.entry(v) {
                                Entry::Occupied(_) => {
                                    // TODO: don't use Debug for this
                                    return Err(de::Error::duplicate_field("dunno"));
                                }
                                Entry::Vacant(ve) => {
                                    ve.insert(map.next_value()?);
                                }
                            }
                        }
                        Field::Any(WrapVar(AnyVariable::Date(v))) => {
                            match date.entry(v) {
                                Entry::Occupied(_) => {
                                    // TODO: don't use Debug for this
                                    return Err(de::Error::duplicate_field("dunno"));
                                }
                                Entry::Vacant(ve) => {
                                    ve.insert(map.next_value()?);
                                }
                            }
                        }
                    }
                }
                Ok(Reference {
                    id: id
                        .map(|i| csl::Atom::from(i.to_string()))
                        .ok_or_else(|| de::Error::missing_field("id"))?,
                    csl_type: csl_type.ok_or_else(|| de::Error::missing_field("type"))?.0,
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

impl<'de> Deserialize<'de> for NumericValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct NumericVisitor;

        impl<'de> Visitor<'de> for NumericVisitor {
            type Value = NumericValue;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an integer between 0 and 2^32, or a string")
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(NumericValue::from(Cow::Owned(value)))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_string(value.to_string())
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(NumericValue::from(Cow::Borrowed(value)))
            }

            fn visit_u8<E>(self, value: u8) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(NumericValue::num(u32::from(value)))
            }

            fn visit_u16<E>(self, value: u16) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(NumericValue::num(u32::from(value)))
            }

            fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(NumericValue::num(value))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                use std::u32;
                if value >= u64::from(u32::MIN) && value <= u64::from(u32::MAX) {
                    Ok(NumericValue::num(value as u32))
                } else {
                    Err(E::custom(format!("u32 out of range: {}", value)))
                }
            }
        }

        deserializer.deserialize_any(NumericVisitor)
    }
}

// newtype these so we can have a different implementation
struct DateParts(DateOrRange);

struct DateInt(i32);

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

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                value
                    .parse::<i32>()
                    .map_err(|_| de::Error::invalid_value(de::Unexpected::Str(value), &self))
                    .map(DateInt)
            }

            fn visit_i8<E>(self, value: i8) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DateInt(i32::from(value)))
            }

            fn visit_i16<E>(self, value: i16) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DateInt(i32::from(value)))
            }

            fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DateInt(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                use std::i32;
                if value >= i64::from(i32::MIN) && value <= i64::from(i32::MAX) {
                    Ok(DateInt(value as i32))
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
                    Ok(DateInt(value as i32))
                } else {
                    Err(E::custom(format!("i32 out of range: {}", value)))
                }
            }
        }
        deserializer.deserialize_any(ParseIntVisitor)
    }
}

struct DateUInt(u32);

impl<'de> Deserialize<'de> for DateUInt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ParseIntVisitor;
        impl<'de> Visitor<'de> for ParseIntVisitor {
            type Value = DateUInt;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(
                    "an unsigned integer or a string that's actually just an unsigned integer",
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                value
                    .parse::<u32>()
                    .map_err(|_| de::Error::invalid_value(de::Unexpected::Str(value), &self))
                    .map(DateUInt)
            }

            fn visit_u8<E>(self, value: u8) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DateUInt(u32::from(value)))
            }

            fn visit_u16<E>(self, value: u16) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DateUInt(u32::from(value)))
            }

            fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DateUInt(value))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                use std::u32;
                if value <= u64::from(u32::MAX - 1) {
                    Ok(DateUInt(value as u32))
                } else {
                    Err(E::custom(format!("u32 out of range: {}", value)))
                }
            }
        }
        deserializer.deserialize_any(ParseIntVisitor)
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SingleDatePartVisitor;

        impl<'de> Visitor<'de> for SingleDatePartVisitor {
            type Value = Date;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a date-part, e.g. [2004, 8, 19]")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let year: DateInt = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let month = seq.next_element()?.unwrap_or(DateUInt(0)).0;
                let day = seq.next_element()?.unwrap_or(DateUInt(0)).0;
                let month = if month >= 1 && month <= 12 { month } else { 0 };
                let day = if day >= 1 && day <= 31 { day } else { 0 };
                Ok(Date::new(year.0, month, day))
            }

            // citeproc-rs may wish to parse its own pandoc Meta blocks without forking out
            // (since MetaInlines are already-parsed markdown or whatver your input format is).
            // in that case, it would have to recognise a different date structure.
            // https://github.com/jgm/pandoc-citeproc/issues/309
            // https://github.com/jgm/pandoc-citeproc/issues/103
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
                let from = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                match seq.next_element()? {
                    Some(to) => Ok(DateParts(DateOrRange::Range(from, to))),
                    None => Ok(DateParts(DateOrRange::Single(from))),
                }
            }
        }
        deserializer.deserialize_seq(DatePartsVisitor)
    }
}

/// TODO:implement seasons
impl<'de> Deserialize<'de> for DateOrRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "kebab-case")]
        enum DateType {
            DateParts,
            Season,
            Circa,
            Literal,
            Raw,
        }

        struct DateVisitor;

        impl<'de> Visitor<'de> for DateVisitor {
            type Value = DateOrRange;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a date")
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                FromStr::from_str(value).or_else(|_| Ok(DateOrRange::Literal(value.to_string())))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                FromStr::from_str(&value).or_else(|_| Ok(DateOrRange::Literal(value)))
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut found = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        DateType::Raw => {
                            let v: Cow<'de, str> = map.next_value()?;
                            found = Some(
                                DateOrRange::from_str(&v)
                                    .unwrap_or_else(|_| DateOrRange::Literal(v.into_owned())),
                            )
                        }
                        DateType::Literal => found = Some(DateOrRange::Literal(map.next_value()?)),
                        DateType::DateParts => {
                            let dp: DateParts = map.next_value()?;
                            found = Some(dp.0);
                        }
                        DateType::Season => unimplemented!(),
                        DateType::Circa => unimplemented!(),
                    }
                }
                found.ok_or_else(|| de::Error::missing_field("raw|literal|etc"))
            }
        }

        const DATE_TYPES: &[&str] = &["date-parts", "season", "circa", "literal", "raw"];
        deserializer.deserialize_struct("DateOrRange", DATE_TYPES, DateVisitor)
    }
}
