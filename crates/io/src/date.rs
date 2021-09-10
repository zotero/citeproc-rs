// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::String;
use edtf::level_1::{Certainty, Edtf, Matcher, Terminal};
use std::cmp::Ordering;

pub(crate) use edtf::level_1::Date;

// This is a fairly primitive date type, possible CSL-extensions could get more fine-grained, and
// then we'd just use chrono::DateTime and support ISO input
#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct LegacyDate {
    /// think 10,000 BC; it's a signed int
    /// "not present" is expressed by not having a date in the first place
    pub year: i32,
    /// range 1 to 16 inclusive
    /// 0 is "not present"
    /// > 12 is a season identified by (month - 12)
    pub month: u32,
    /// range 1 to 31 inclusive
    /// 0 is "not present"
    pub day: u32,
    /// aka is_uncertain_date
    pub circa: bool,
}

impl PartialOrd for DateOrRange {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (DateOrRange::Literal { .. }, _) | (_, DateOrRange::Literal { .. }) => None,
            _ => None,
        }
    }
}

#[test]
fn test_date_ord() {
    // years only
    assert!(Date::from_ymd(2000, 0, 0) < Date::from_ymd(2001, 0, 0));
    // Less specific comes first (2000 < May 2000 < 1 May 2000)
    assert!(Date::from_ymd(2000, 0, 0) < Date::from_ymd(2000, 5, 0));
    assert!(Date::from_ymd(2000, 5, 0) < Date::from_ymd(2000, 5, 1));

    assert!(Date::from_ymd(2000, 0, 0) < Date::from_ymd(2001, 0, 0));
}

impl LegacyDate {
    pub fn has_month(&self) -> bool {
        self.month != 0
    }
    pub fn has_day(&self) -> bool {
        self.day != 0
    }
    pub fn new_circa(y: i32, m: u32, d: u32) -> Self {
        let mut d = LegacyDate::new(y, m, d);
        d.circa = true;
        d
    }
    pub fn new(y: i32, m: u32, d: u32) -> Self {
        LegacyDate {
            year: y,
            month: m,
            day: d,
            circa: false,
        }
    }
    pub fn from_parts(parts: &[i32]) -> Option<Self> {
        let m = *parts.get(1).unwrap_or(&0);
        let d = *parts.get(2).unwrap_or(&0);
        Some(LegacyDate {
            year: *parts.get(0)?,
            month: if m >= 1 && m <= 16 { m as u32 } else { 0 },
            day: if d >= 1 && d <= 31 { d as u32 } else { 0 },
            circa: false,
        })
    }

    // it isn't possible to parse an invalid month, bad ones default to zero (see tests)
    // and cause the day not to parse as well.
    // so we only need to default days to zero when they are invalid.
    // Maybe just leave this as is. You would need to know leap years, hence a whole bunch
    // of date math that isn't all that valuable (real publications don't make those
    // mistakes anyway, and users who don't know when months end... it's on them)
    // plus there is no non-numeric day output for CSL. So it doesn't matter.
    // fn invalid_default() {
    // }
}

// TODO: implement deserialize for date-parts array, date-parts raw, { year, month, day }
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum DateOrRange {
    Edtf(Edtf),
    Literal { literal: String, circa: bool },
}

impl DateOrRange {
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        Self::Edtf(Edtf::Date(Date::from_ymd(year, month, day)))
    }
    pub fn with_circa(mut self, circa: bool) -> Self {
        self.set_circa(circa);
        self
    }
    pub fn set_circa(&mut self, circa: bool) {
        match self {
            DateOrRange::Edtf(edtf) => match edtf {
                Edtf::Date(d) => *d = d.and_certainty(Certainty::Uncertain),
                _ => {}
            },
            DateOrRange::Literal { circa: c, .. } => *c = circa,
        }
    }
    pub fn is_uncertain_date(&self) -> bool {
        match *self {
            DateOrRange::Edtf(edtf) => match edtf.as_matcher() {
                Matcher::Date(_, certainty) => certainty != Certainty::Certain,
                Matcher::Interval(Terminal::Fixed(_, certainty), _)
                | Matcher::Interval(_, Terminal::Fixed(_, certainty)) => {
                    certainty != Certainty::Certain
                }
                _ => false,
            },
            DateOrRange::Literal { circa, .. } => circa,
        }
    }
    pub fn single(&self) -> Option<Date> {
        if let DateOrRange::Edtf(Edtf::Date(d)) = self {
            Some(*d)
        } else {
            None
        }
    }
    fn as_edtf(&self) -> Option<&Edtf> {
        match self {
            Self::Edtf(edtf) => Some(edtf),
            _ => None,
        }
    }
    pub fn single_or_first(&self) -> Option<Date> {
        match self.as_edtf()?.as_matcher() {
            // These are a bit awkward -- it seems that more often than not, we want the actual
            // Date object and not the precision/certainty pair, because we can always get that
            // if it's needed.
            Matcher::Date(d, _) => Date::from_precision_opt(d),
            Matcher::Interval(Terminal::Fixed(d, _), _) => Date::from_precision_opt(d),
            Matcher::Interval(_, Terminal::Fixed(d, _)) => Date::from_precision_opt(d),
            _ => None,
        }
    }
    pub fn from_parts(parts: &[&[i32]]) -> Option<Self> {
        Some(match parts {
            [single] => Edtf::Date(date_from_parts(single)?),
            // it's fine if people want to tack 2 million extra date-parts arrays on the end
            // go ahead, make my day
            [from, to, ..] => Edtf::Interval(date_from_parts(from)?, date_from_parts(to)?),
            _ => return None,
        })
        .map(DateOrRange::Edtf)
    }
}

fn date_from_parts(parts: &[i32]) -> Option<Date> {
    let m = *parts.get(1).unwrap_or(&0);
    let d = *parts.get(2).unwrap_or(&0);
    let year = *parts.get(0)?;
    let month = if (1..=12).contains(&m) {
        m as u32
    } else if (13..=16).contains(&m) {
        m as u32 + 8
    } else if (21..=24).contains(&m) {
        m as u32
    } else {
        0
    };
    let day = if d >= 1 && d <= 31 { d as u32 } else { 0 };
    Some(Date::from_ymd_opt(year, month, day)?)
}

impl From<Edtf> for DateOrRange {
    fn from(d: Edtf) -> Self {
        Self::Edtf(d)
    }
}

impl From<Date> for DateOrRange {
    fn from(d: Date) -> Self {
        Self::Edtf(d.into())
    }
}

impl From<(Date, Date)> for DateOrRange {
    fn from(interval: (Date, Date)) -> Self {
        Self::Edtf(interval.into())
    }
}

impl DateOrRange {
    /// The loosely specified citeproc-js convention
    pub(crate) fn from_raw_str(s: &str) -> Result<Self, ()> {
        if let Ok((_left_overs, parsed)) = range(s.as_bytes()) {
            Ok(Self::Edtf(parsed))
        } else {
            Err(())
        }
    }
}

#[cfg(test)]
#[test]
fn test_date_parsing() {
    assert_eq!(
        DateOrRange::from_raw_str("-1998-09-21"),
        Ok(DateOrRange::new(-1998, 9, 21))
    );
    assert_eq!(
        DateOrRange::from_raw_str("+1998-09-21"),
        Ok(DateOrRange::new(1998, 9, 21))
    );
    assert_eq!(
        DateOrRange::from_raw_str("1998-09-21"),
        Ok(DateOrRange::new(1998, 9, 21))
    );
    assert_eq!(
        DateOrRange::from_raw_str("1998-09"),
        Ok(DateOrRange::new(1998, 9, 0))
    );
    assert_eq!(
        DateOrRange::from_raw_str("1998"),
        Ok(DateOrRange::new(1998, 0, 0))
    );
    assert_eq!(
        DateOrRange::from_raw_str("1998trailing"),
        Ok(DateOrRange::new(1998, 0, 0))
    );
    // can't parse 13 as a month, default to no month
    assert_eq!(
        DateOrRange::from_raw_str("1998-13"),
        Ok(DateOrRange::new(1998, 0, 0))
    );
    // can't parse 34 as a day, default to no day
    assert_eq!(
        DateOrRange::from_raw_str("1998-12-34"),
        Ok(DateOrRange::new(1998, 12, 0))
    );
}

#[cfg(test)]
#[test]
fn test_range_parsing() {
    assert_eq!(
        DateOrRange::from_raw_str("1998-09-21/2001-08-16"),
        Ok(DateOrRange::Edtf(Edtf::Interval(
            Date::from_ymd(1998, 9, 21),
            Date::from_ymd(2001, 8, 16)
        )))
    );
    assert_eq!(
        DateOrRange::from_raw_str("1998-09-21/2001-08"),
        Ok(DateOrRange::Edtf(Edtf::Interval(
            Date::from_ymd(1998, 9, 21),
            Date::from_ymd(2001, 8, 0)
        )))
    );
    assert_eq!(
        DateOrRange::from_raw_str("1998-09/2001-08-01"),
        Ok(DateOrRange::Edtf(Edtf::Interval(
            Date::from_ymd(1998, 9, 0),
            Date::from_ymd(2001, 8, 1)
        )))
    );
    assert_eq!(
        DateOrRange::from_raw_str("1998/2001"),
        Ok(DateOrRange::Edtf(Edtf::Interval(
            Date::from_ymd(1998, 0, 0),
            Date::from_ymd(2001, 0, 0)
        )))
    );
}

#[cfg(test)]
#[test]
fn test_from_parts() {
    assert_eq!(
        DateOrRange::from_parts(&[&[1998, 9, 21]]),
        Some(DateOrRange::new(1998, 09, 21))
    );
    assert_eq!(
        DateOrRange::from_parts(&[&[1998, 9]]),
        Some(DateOrRange::new(1998, 9, 0))
    );
    assert_eq!(
        DateOrRange::from_parts(&[&[1998]]),
        Some(DateOrRange::new(1998, 0, 0))
    );
    assert_eq!(
        DateOrRange::from_parts(&[&[1998, 9, 21], &[2001, 8, 16]]),
        Some(DateOrRange::Edtf(Edtf::Interval(
            Date::from_ymd(1998, 9, 21),
            Date::from_ymd(2001, 8, 16)
        )))
    );
    assert_eq!(
        DateOrRange::from_parts(&[&[1998, 9, 21], &[2001, 8]]),
        Some(DateOrRange::Edtf(Edtf::Interval(
            Date::from_ymd(1998, 9, 21),
            Date::from_ymd(2001, 8, 0)
        )))
    );
    assert_eq!(
        DateOrRange::from_parts(&[&[1998, 9], &[2001, 8, 1]]),
        Some(DateOrRange::Edtf(Edtf::Interval(
            Date::from_ymd(1998, 9, 0),
            Date::from_ymd(2001, 8, 1)
        )))
    );
    assert_eq!(
        DateOrRange::from_parts(&[&[1998], &[2001]]),
        Some(DateOrRange::Edtf(Edtf::Interval(
            Date::from_ymd(1998, 0, 0),
            Date::from_ymd(2001, 0, 0)
        )))
    );
}

// substantial portions of the following copied from
// and updated to nom v4
// https://github.com/badboy/iso8601/blob/master/src/parsers.rs
/*
Copyright (c) 2015 Jan-Erik Rediger, Hendrik Sollich

Permission is hereby granted, free of charge, to any person obtaining
a copy of this software and associated documentation files (the
"Software"), to deal in the Software without restriction, including
without limitation the rights to use, copy, modify, merge, publish,
distribute, sublicense, and/or sell copies of the Software, and to
permit persons to whom the Software is furnished to do so, subject to
the following conditions:

The above copyright notice and this permission notice shall be
included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
*/

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while_m_n},
    character::is_digit,
    combinator::opt,
    sequence::preceded,
    IResult,
};

use std::str::{from_utf8_unchecked, FromStr};

fn to_string(s: &[u8]) -> &str {
    unsafe { from_utf8_unchecked(s) }
}
fn to_i32(s: &str) -> i32 {
    FromStr::from_str(s).unwrap()
}
fn to_u32(s: &str) -> u32 {
    FromStr::from_str(s).unwrap()
}

fn buf_to_u32(s: &[u8]) -> u32 {
    to_u32(to_string(s))
}
fn buf_to_i32(s: &[u8]) -> i32 {
    to_i32(to_string(s))
}

fn take_4_digits(inp: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while_m_n(4, 4, is_digit)(inp)
}

fn year_prefix(inp: &[u8]) -> IResult<&[u8], &[u8]> {
    alt((tag("+"), tag("-")))(inp)
}

fn year(inp: &[u8]) -> IResult<&[u8], i32> {
    let (rem1, pref) = opt(year_prefix)(inp)?;
    let (rem2, y) = take_4_digits(rem1)?;
    Ok((
        rem2,
        match pref {
            Some(b"-") => -buf_to_i32(y),
            _ => buf_to_i32(y),
        },
    ))
}

fn char_between<'a>(min: char, max: char) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], &'a [u8]> {
    take_while_m_n(1, 1, move |c: u8| c >= (min as u8) && c <= (max as u8))
}

// DD
fn day_zero(inp: &[u8]) -> IResult<&[u8], u32> {
    let (rem, dig) = preceded(tag("0"), char_between('1', '9'))(inp)?;
    Ok((rem, buf_to_u32(dig)))
}

fn day_one(inp: &[u8]) -> IResult<&[u8], u32> {
    let (rem, dig) = preceded(tag("1"), char_between('0', '9'))(inp)?;
    Ok((rem, 10 + buf_to_u32(dig)))
}

fn day_two(inp: &[u8]) -> IResult<&[u8], u32> {
    let (rem, dig) = preceded(tag("2"), char_between('0', '9'))(inp)?;
    Ok((rem, 20 + buf_to_u32(dig)))
}

fn day_three(inp: &[u8]) -> IResult<&[u8], u32> {
    let (rem, dig) = preceded(tag("3"), char_between('0', '1'))(inp)?;
    Ok((rem, 30 + buf_to_u32(dig)))
}

fn day(inp: &[u8]) -> IResult<&[u8], u32> {
    alt((day_zero, day_one, day_two, day_three))(inp)
}

// MM
fn lower_month(inp: &[u8]) -> IResult<&[u8], u32> {
    let (rem, dig) = preceded(tag("0"), char_between('1', '9'))(inp)?;
    Ok((rem, buf_to_u32(dig)))
}

fn upper_month(inp: &[u8]) -> IResult<&[u8], u32> {
    let (rem, dig) = preceded(tag("1"), char_between('0', '2'))(inp)?;
    Ok((rem, 10 + buf_to_u32(dig)))
}

fn month(inp: &[u8]) -> IResult<&[u8], u32> {
    alt((lower_month, upper_month))(inp)
}

// Custom stuff built on top of that:

fn and_day(inp: &[u8]) -> IResult<&[u8], u32> {
    let (rem, _) = tag("-")(inp)?;
    Ok(day(rem)?)
}

enum MonthDay {
    Month(u32),
    MonthDay(u32, u32),
}

fn month_day(inp: &[u8]) -> IResult<&[u8], MonthDay> {
    let (rem1, _) = tag("-")(inp)?;
    let (rem2, m) = month(rem1)?;
    let (rem3, ad) = opt(and_day)(rem2)?;
    Ok((
        rem3,
        match ad {
            Some(d) => MonthDay::MonthDay(m, d),
            None => MonthDay::Month(m),
        },
    ))
}

fn ymd_date(inp: &[u8]) -> IResult<&[u8], Option<Date>> {
    let (rem1, y) = year(inp)?;
    let (rem2, md) = opt(month_day)(rem1)?;
    Ok((
        rem2,
        match md {
            None => Date::from_ymd_opt(y, 0, 0),
            Some(MonthDay::MonthDay(m, d)) => Date::from_ymd_opt(y, m, d),
            Some(MonthDay::Month(m)) => Date::from_ymd_opt(y, m, 0),
        },
    ))
}

fn and_ymd(inp: &[u8]) -> IResult<&[u8], Option<Date>> {
    let (rem1, _) = tag("/")(inp)?;
    Ok(ymd_date(rem1)?)
}

fn parse_error(inp: &[u8]) -> nom::Err<nom::error::Error<&[u8]>> {
    nom::Err::Error(nom::error::Error::new(inp, nom::error::ErrorKind::ParseTo))
}

fn range(inp: &[u8]) -> IResult<&[u8], Edtf> {
    let (rem1, first) = ymd_date(inp)?;
    let d1 = first.ok_or_else(|| parse_error(inp))?;
    let (rem2, d2o) = opt(and_ymd)(rem1)?;
    Ok((
        rem2,
        match d2o {
            None => Edtf::Date(d1),
            Some(d2) => {
                let d2 = d2.ok_or_else(|| parse_error(rem1))?;
                Edtf::Interval(d1, d2)
            }
        },
    ))
}
