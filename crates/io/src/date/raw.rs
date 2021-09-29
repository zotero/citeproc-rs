// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::{calendar, Date, DateOrRange, Edtf};

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while_m_n},
    character::is_digit,
    combinator::opt,
    sequence::preceded,
    IResult,
};

impl DateOrRange {
    /// The loosely specified citeproc-js convention
    pub(crate) fn from_raw_str(s: &str) -> Result<Self, ()> {
        if let Ok((_left_overs, parsed)) = parse_raw_str(s.as_bytes()) {
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
            None => calendar::date_from_csl_json_parts_ymd(y, 0, 0),
            Some(MonthDay::MonthDay(m, d)) => calendar::date_from_csl_json_parts_ymd(y, m, d),
            Some(MonthDay::Month(m)) => calendar::date_from_csl_json_parts_ymd(y, m, 0),
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

fn parse_raw_str(inp: &[u8]) -> IResult<&[u8], Edtf> {
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
