// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::String;
use std::cmp::Ordering;

/// TODO: parse 2018-3-17 as if it were '03'

// This is a fairly primitive date type, possible CSL-extensions could get more fine-grained, and
// then we'd just use chrono::DateTime and support ISO input
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Date {
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

impl PartialOrd for Date {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Date {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut cmp = self.year.cmp(&other.year);
        if self.month < 13 && other.month < 13 {
            // less specific comes first, so zeroes (absent) can just be compared directly
            cmp = cmp.then(self.month.cmp(&other.month))
        }
        cmp.then(self.day.cmp(&other.day))
    }
}

impl PartialOrd for DateOrRange {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (DateOrRange::Literal { .. }, _) | (_, DateOrRange::Literal { .. }) => None,
            (DateOrRange::Single(a), DateOrRange::Single(b)) => Some(a.cmp(b)),
            (DateOrRange::Range(a1, a2), DateOrRange::Single(b)) => Some(a1.cmp(b).then(a2.cmp(b))),
            (DateOrRange::Single(a), DateOrRange::Range(b1, b2)) => Some(a.cmp(b1).then(a.cmp(b2))),
            (DateOrRange::Range(a1, a2), DateOrRange::Range(b1, b2)) => {
                Some(a1.cmp(b1).then(a2.cmp(b2)))
            }
        }
    }
}

#[test]
fn test_date_ord() {
    // years only
    assert!(Date::new(2000, 0, 0) < Date::new(2001, 0, 0));
    // Less specific comes first (2000 < May 2000 < 1 May 2000)
    assert!(Date::new(2000, 0, 0) < Date::new(2000, 5, 0));
    assert!(Date::new(2000, 5, 0) < Date::new(2000, 5, 1));

    assert!(Date::new(2000, 0, 0) < Date::new(2001, 0, 0));
}

// TODO: implement PartialOrd?

impl Date {
    pub fn has_month(&self) -> bool {
        self.month != 0
    }
    pub fn has_day(&self) -> bool {
        self.day != 0
    }
    pub fn new_circa(y: i32, m: u32, d: u32) -> Self {
        let mut d = Date::new(y, m, d);
        d.circa = true;
        d
    }
    pub fn new(y: i32, m: u32, d: u32) -> Self {
        Date {
            year: y,
            month: m,
            day: d,
            circa: false,
        }
    }
    pub fn from_parts(parts: &[i32]) -> Option<Self> {
        let m = *parts.get(1).unwrap_or(&0);
        let d = *parts.get(2).unwrap_or(&0);
        Some(Date {
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
    Single(Date),
    Range(Date, Date),
    Literal { literal: String, circa: bool },
}

impl DateOrRange {
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        DateOrRange::Single(Date::new(year, month, day))
    }
    pub fn with_circa(mut self, circa: bool) -> Self {
        self.set_circa(circa);
        self
    }
    pub fn set_circa(&mut self, circa: bool) {
        match self {
            DateOrRange::Single(d) => d.circa = circa,
            DateOrRange::Range(d1, d2) => {
                d1.circa = circa;
                d2.circa = circa;
            }
            DateOrRange::Literal { circa: c, .. } => *c = circa,
        }
    }
    pub fn is_uncertain_date(&self) -> bool {
        match *self {
            DateOrRange::Single(d) => d.circa,
            DateOrRange::Range(d1, d2) => d1.circa || d2.circa,
            DateOrRange::Literal { circa, .. } => circa,
        }
    }
    pub fn single(&self) -> Option<Date> {
        if let DateOrRange::Single(d) = self {
            Some(*d)
        } else {
            None
        }
    }
    pub fn single_or_first(&self) -> Option<Date> {
        match self {
            DateOrRange::Single(d) => Some(*d),
            DateOrRange::Range(d, _) => Some(*d),
            _ => None,
        }
    }
    pub fn from_parts(parts: &[&[i32]]) -> Option<Self> {
        if parts.is_empty() {
            None
        } else if parts.len() == 1 {
            Some(DateOrRange::Single(Date::from_parts(parts[0])?))
        } else {
            Some(DateOrRange::Range(
                Date::from_parts(parts[0])?,
                Date::from_parts(parts[1])?,
            ))
        }
    }
}

impl From<Date> for DateOrRange {
    fn from(d: Date) -> Self {
        Self::Single(d)
    }
}

impl From<(Date, Date)> for DateOrRange {
    fn from(d: (Date, Date)) -> Self {
        Self::Range(d.0, d.1)
    }
}

impl FromStr for DateOrRange {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok((_left_overs, parsed)) = range(s.as_bytes()) {
            Ok(parsed)
        } else {
            Err(())
        }
    }
}

#[cfg(test)]
#[test]
fn test_date_parsing() {
    assert_eq!(
        DateOrRange::from_str("-1998-09-21"),
        Ok(DateOrRange::new(-1998, 9, 21))
    );
    assert_eq!(
        DateOrRange::from_str("+1998-09-21"),
        Ok(DateOrRange::new(1998, 9, 21))
    );
    assert_eq!(
        DateOrRange::from_str("1998-09-21"),
        Ok(DateOrRange::new(1998, 9, 21))
    );
    assert_eq!(
        DateOrRange::from_str("1998-09"),
        Ok(DateOrRange::new(1998, 9, 0))
    );
    assert_eq!(
        DateOrRange::from_str("1998"),
        Ok(DateOrRange::new(1998, 0, 0))
    );
    assert_eq!(
        DateOrRange::from_str("1998trailing"),
        Ok(DateOrRange::new(1998, 0, 0))
    );
    // can't parse 13 as a month, default to no month
    assert_eq!(
        DateOrRange::from_str("1998-13"),
        Ok(DateOrRange::new(1998, 0, 0))
    );
    // can't parse 34 as a day, default to no day
    assert_eq!(
        DateOrRange::from_str("1998-12-34"),
        Ok(DateOrRange::new(1998, 12, 0))
    );
}

#[cfg(test)]
#[test]
fn test_range_parsing() {
    assert_eq!(
        DateOrRange::from_str("1998-09-21/2001-08-16"),
        Ok(DateOrRange::Range(
            Date::new(1998, 9, 21),
            Date::new(2001, 8, 16)
        ))
    );
    assert_eq!(
        DateOrRange::from_str("1998-09-21/2001-08"),
        Ok(DateOrRange::Range(
            Date::new(1998, 9, 21),
            Date::new(2001, 8, 0)
        ))
    );
    assert_eq!(
        DateOrRange::from_str("1998-09/2001-08-01"),
        Ok(DateOrRange::Range(
            Date::new(1998, 9, 0),
            Date::new(2001, 8, 1)
        ))
    );
    assert_eq!(
        DateOrRange::from_str("1998/2001"),
        Ok(DateOrRange::Range(
            Date::new(1998, 0, 0),
            Date::new(2001, 0, 0)
        ))
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
        Some(DateOrRange::Range(
            Date::new(1998, 9, 21),
            Date::new(2001, 8, 16)
        ))
    );
    assert_eq!(
        DateOrRange::from_parts(&[&[1998, 9, 21], &[2001, 8]]),
        Some(DateOrRange::Range(
            Date::new(1998, 9, 21),
            Date::new(2001, 8, 0)
        ))
    );
    assert_eq!(
        DateOrRange::from_parts(&[&[1998, 9], &[2001, 8, 1]]),
        Some(DateOrRange::Range(
            Date::new(1998, 9, 0),
            Date::new(2001, 8, 1)
        ))
    );
    assert_eq!(
        DateOrRange::from_parts(&[&[1998], &[2001]]),
        Some(DateOrRange::Range(
            Date::new(1998, 0, 0),
            Date::new(2001, 0, 0)
        ))
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

fn ymd_date(inp: &[u8]) -> IResult<&[u8], Date> {
    let (rem1, y) = year(inp)?;
    let (rem2, md) = opt(month_day)(rem1)?;
    Ok((
        rem2,
        match md {
            None => Date::new(y, 0, 0),
            Some(MonthDay::MonthDay(m, d)) => Date::new(y, m, d),
            Some(MonthDay::Month(m)) => Date::new(y, m, 0),
        },
    ))
}

fn and_ymd(inp: &[u8]) -> IResult<&[u8], Date> {
    let (rem1, _) = tag("/")(inp)?;
    Ok(ymd_date(rem1)?)
}

fn range(inp: &[u8]) -> IResult<&[u8], DateOrRange> {
    let (rem1, d1) = ymd_date(inp)?;
    let (rem2, d2o) = opt(and_ymd)(rem1)?;
    Ok((
        rem2,
        match d2o {
            None => DateOrRange::Single(d1),
            Some(d2) => DateOrRange::Range(d1, d2),
        },
    ))
}
