// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use nom::*;
/// TODO: parse 2018-3-17 as if it were '03'

// This is a fairly primitive date type, possible CSL-extensions could get more fine-grained, and
// then we'd just use chrono::DateTime and support ISO input
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Date {
    /// think 10,000 BC; it's a signed int
    /// "not present" is expressed by not having a date in the first place
    pub year: i32,
    /// range 1 to 12 inclusive
    /// 0 is "not present"
    pub month: u32,
    /// range 1 to 31 inclusive
    /// 0 is "not present"
    pub day: u32,
}

// TODO: implement PartialOrd?

impl Date {
    pub fn has_month(&self) -> bool {
        self.month != 0
    }
    pub fn has_day(&self) -> bool {
        self.day != 0
    }
    pub fn new(y: i32, m: u32, d: u32) -> Self {
        Date {
            year: y,
            month: m,
            day: d,
        }
    }
    pub fn from_parts(parts: &[i32]) -> Option<Self> {
        let m = *parts.get(1).unwrap_or(&0);
        let d = *parts.get(2).unwrap_or(&0);
        Some(Date {
            year: *parts.get(0)?,
            month: if m >= 1 && m <= 12 { m as u32 } else { 0 },
            day: if d >= 1 && d <= 31 { d as u32 } else { 0 },
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
    Literal(String),
}

impl DateOrRange {
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        DateOrRange::Single(Date { year, month, day })
    }
    pub fn single(&self) -> Option<Date> {
        if let DateOrRange::Single(ref d) = self {
            Some(d.clone())
        } else {
            None
        }
    }
    pub fn single_or_first(&self) -> Option<Date> {
        match self {
            DateOrRange::Single(ref d) => Some(d.clone()),
            DateOrRange::Range(ref d, _) => Some(d.clone()),
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
        Ok(DateOrRange::new(-1998, 09, 21))
    );
    assert_eq!(
        DateOrRange::from_str("+1998-09-21"),
        Ok(DateOrRange::new(1998, 09, 21))
    );
    assert_eq!(
        DateOrRange::from_str("1998-09-21"),
        Ok(DateOrRange::new(1998, 09, 21))
    );
    assert_eq!(
        DateOrRange::from_str("1998-09"),
        Ok(DateOrRange::new(1998, 09, 0))
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

named!(take_4_digits, take_while_m_n!(4, 4, is_digit));

// year
named!(year_prefix, alt!(tag!("+") | tag!("-")));
named!(
    year<i32>,
    do_parse!(
        pref: opt!(complete!(year_prefix))
            >> year: call!(take_4_digits)
            >> (match pref {
                Some(b"-") => -buf_to_i32(year),
                _ => buf_to_i32(year),
            })
    )
);

// https://github.com/Geal/nom/blob/master/doc/how_nom_macros_work.md
macro_rules! char_between(
    ($input:expr, $min:expr, $max:expr) => (
        {
        fn f(c: u8) -> bool { c >= ($min as u8) && c <= ($max as u8)}
        #[allow(clippy::double_comparisons)]
        take_while_m_n!($input, 1, 1, f)
        }
    );
);

// DD
named!(
    day_zero<u32>,
    do_parse!(tag!("0") >> s: char_between!('1', '9') >> (buf_to_u32(s)))
);
named!(
    day_one<u32>,
    do_parse!(tag!("1") >> s: char_between!('0', '9') >> (10 + buf_to_u32(s)))
);
named!(
    day_two<u32>,
    do_parse!(tag!("2") >> s: char_between!('0', '9') >> (20 + buf_to_u32(s)))
);
named!(
    day_three<u32>,
    do_parse!(tag!("3") >> s: char_between!('0', '1') >> (30 + buf_to_u32(s)))
);
named!(day<u32>, alt!(day_zero | day_one | day_two | day_three));

named!(
    lower_month<u32>,
    do_parse!(tag!("0") >> s: char_between!('1', '9') >> (buf_to_u32(s)))
);
named!(
    upper_month<u32>,
    do_parse!(tag!("1") >> s: char_between!('0', '2') >> (10 + buf_to_u32(s)))
);
named!(month<u32>, alt!(lower_month | upper_month));

// Custom stuff built on top of that:

named!(and_day<u32>, do_parse!(tag!("-") >> d: day >> (d)));

enum MonthDay {
    Month(u32),
    MonthDay(u32, u32),
}

named!(
    month_day<MonthDay>,
    do_parse!(
        tag!("-")
            >> m: month
            >> ad: opt!(complete!(and_day))
            >> (match ad {
                Some(d) => MonthDay::MonthDay(m, d),
                None => MonthDay::Month(m),
            })
    )
);

// YYYY-MM-DD
named!(
    ymd_date<Date>,
    do_parse!(
        y: year
            >> md: opt!(complete!(month_day))
            >> ({
                match md {
                    None => Date {
                        year: y,
                        month: 0,
                        day: 0,
                    },
                    Some(MonthDay::MonthDay(m, d)) => Date {
                        year: y,
                        month: m,
                        day: d,
                    },
                    Some(MonthDay::Month(m)) => Date {
                        year: y,
                        month: m,
                        day: 0,
                    },
                }
            })
    )
);

named!(and_ymd<Date>, do_parse!(tag!("/") >> d: ymd_date >> (d)));

named!(
    range<DateOrRange>,
    do_parse!(
        d1: ymd_date
            >> d2o: opt!(complete!(and_ymd))
            >> (match d2o {
                None => DateOrRange::Single(d1),
                Some(d2) => DateOrRange::Range(d1, d2),
            })
    )
);
