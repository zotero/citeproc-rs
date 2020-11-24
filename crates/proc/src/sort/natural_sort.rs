// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2020 Corporation for Digital Scholarship

//! Spec:
//!
//! > Date variables called via the variable attribute are returned in the YYYYMMDD format,
//! with zeros substituted for any missing date-parts (e.g. 20001200 for December 2000). As a
//! result, less specific dates precede more specific dates in ascending sorts, e.g. “2000, May
//! 2000, May 1st 2000”. Negative years are sorted inversely, e.g. “100BC, 50BC, 50AD, 100AD”.
//! Seasons are ignored for sorting, as the chronological order of the seasons differs between the
//! northern and southern hemispheres. In the case of date ranges, the start date is used for the
//! primary sort, and the end date is used for a secondary sort, e.g. “2000–2001, 2000–2005,
//! 2002–2003, 2002–2009”. Date ranges are placed after single dates when they share the same
//! (start) date, e.g. “2000, 2000–2002”. ...
//!
//! Basically, everything would be very easy without the BC/AD sorting and the ranges coming later
//! parts. But given these, we have to parse dates again.
//!
//! The approach of this section is to write dates and numbers into a string, with special unicode
//! characters delimiting them and a known format between those special characters, so the string
//! can be parsed into runs of string-number-string-date (etc).

// From the BMP(0) unicode private use area
// Delimits a date so it can be parsed when doing a natural sort comparison
pub const DATE_START: char = '\u{E000}';
pub const DATE_START_STR: &str = "\u{E000}";
pub const DATE_END: char = '\u{E001}';
pub const DATE_END_STR: &str = "\u{E001}";

// Delimits a number so it can be compared
pub const NUM_START: char = '\u{E002}';
pub const NUM_START_STR: &str = "\u{E002}";
pub const NUM_END: char = '\u{E003}';
pub const NUM_END_STR: &str = "\u{E003}";

// Delimits a citation number so it can be recognised as such
pub const CITATION_NUM_START: char = '\u{E004}';
pub const CITATION_NUM_START_STR: &str = "\u{E004}";
pub const CITATION_NUM_END: char = '\u{E005}';
pub const CITATION_NUM_END_STR: &str = "\u{E005}";

pub fn date_affixes() -> Affixes {
    Affixes {
        prefix: DATE_START_STR.into(),
        suffix: DATE_END_STR.into(),
    }
}

pub fn num_affixes() -> Affixes {
    Affixes {
        prefix: NUM_START_STR.into(),
        suffix: NUM_END_STR.into(),
    }
}

pub fn citation_number_affixes() -> Affixes {
    Affixes {
        prefix: CITATION_NUM_START_STR.into(),
        suffix: CITATION_NUM_END_STR.into(),
    }
}

pub fn extract_citation_number(s: &str) -> Option<u32> {
    let mut iter = TokenIterator { remain: s };
    if let Some(Token::CitationNumber(cnum)) = iter.next() {
        return Some(cnum);
    }
    None
}

#[test]
fn extract_cnum() {
    let mut string = String::new();
    string.push_str(CITATION_NUM_START_STR);
    string.push_str("00000008");
    string.push_str(CITATION_NUM_END_STR);
    assert_eq!(extract_citation_number(&string), Some(8));
}

#[derive(PartialEq, Eq, Debug)]
struct CmpDate<'a> {
    year: Option<i32>,
    rest: &'a str,
}

impl<'a> Ord for CmpDate<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.year
            .cmp(&other.year)
            .then_with(|| self.rest.cmp(other.rest))
    }
}

impl<'a> PartialOrd for CmpDate<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(
            self.year
                .cmp(&other.year)
                .then_with(|| self.rest.cmp(other.rest)),
        )
    }
}

#[derive(PartialEq, Eq, Debug)]
enum CmpRange<'a> {
    Single(CmpDate<'a>),
    Range(CmpDate<'a>, CmpDate<'a>),
}

impl<'a> Ord for CmpRange<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (CmpRange::Single(a), CmpRange::Single(b)) => a.cmp(b),
            (CmpRange::Single(a), CmpRange::Range(b, _c)) => a.cmp(b),
            (CmpRange::Range(a, _b), CmpRange::Single(c)) => a.cmp(c),
            (CmpRange::Range(a, b), CmpRange::Range(c, d)) => a.cmp(c).then_with(|| b.cmp(d)),
        }
    }
}

impl<'a> PartialOrd for CmpRange<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

use csl::Affixes;
use nom::{
    branch::alt,
    bytes::complete::{take_while, take_while1, take_while_m_n},
    character::complete::char,
    combinator::{map, opt},
    sequence::delimited,
    IResult,
};
use std::cmp::Ordering;
use std::str::FromStr;

fn to_u32(s: &str) -> u32 {
    FromStr::from_str(s).unwrap()
}

fn to_i32(s: &str) -> i32 {
    FromStr::from_str(s).unwrap()
}

fn take_8_digits(inp: &str) -> IResult<&str, &str> {
    take_while_m_n(1, 8, |c: char| c.is_ascii_digit())(inp)
}

fn year_prefix(inp: &str) -> IResult<&str, char> {
    alt((char('+'), char('-')))(inp)
}

fn year(inp: &str) -> IResult<&str, i32> {
    let (rem1, pref) = opt(year_prefix)(inp)?;
    let (rem2, y) = take_while1(|c: char| c.is_ascii_digit())(rem1)?;
    let (rem3, _) = char('_')(rem2)?;
    Ok((
        rem3,
        match pref {
            Some('-') => -to_i32(y),
            _ => to_i32(y),
        },
    ))
}

fn date(inp: &str) -> IResult<&str, CmpDate> {
    let (rem1, year) = opt(year)(inp)?;
    fn still_date(c: char) -> bool {
        c != DATE_END && c != '/'
    }
    let (rem2, rest) = take_while(still_date)(rem1)?;
    Ok((rem2, CmpDate { year, rest }))
}

fn range(inp: &str) -> IResult<&str, Token> {
    let (rem1, _) = char(DATE_START)(inp)?;
    let (rem2, first) = date(rem1)?;
    fn and_ymd(inp: &str) -> IResult<&str, CmpDate> {
        let (rem1, _) = char('/')(inp)?;
        Ok(date(rem1)?)
    }
    let (rem3, d2) = opt(and_ymd)(rem2)?;
    let (rem4, _) = char(DATE_END)(rem3)?;
    Ok((
        rem4,
        Token::Date(match d2 {
            None => CmpRange::Single(first),
            Some(d) => CmpRange::Range(first, d),
        }),
    ))
}

fn citation_number(inp: &str) -> IResult<&str, Token> {
    delimited(
        char(CITATION_NUM_START),
        map(take_8_digits, |x| Token::CitationNumber(to_u32(x))),
        char(CITATION_NUM_END),
    )(inp)
}

fn num(inp: &str) -> IResult<&str, Token> {
    delimited(
        char(NUM_START),
        map(take_8_digits, |x| Token::Num(to_u32(x))),
        char(NUM_END),
    )(inp)
}

fn str_token(inp: &str) -> IResult<&str, Token> {
    fn normal(c: char) -> bool {
        !(c == DATE_START || c == NUM_START || c == CITATION_NUM_START)
    }
    map(take_while1(normal), Token::Str)(inp)
}

fn token(inp: &str) -> IResult<&str, Token> {
    alt((str_token, citation_number, num, range))(inp)
}

struct TokenIterator<'a> {
    remain: &'a str,
}

#[derive(PartialEq, Debug)]
enum Token<'a> {
    Str(&'a str),
    Num(u32),
    CitationNumber(u32),
    Date(CmpRange<'a>),
}

impl<'a> PartialOrd for Token<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        use super::Natural;
        match (self, other) {
            (Token::Str(a), Token::Str(b)) => Natural::new(a).partial_cmp(&Natural::new(b)),
            (Token::Date(a), Token::Date(b)) => a.partial_cmp(b),
            (Token::Num(a), Token::Num(b)) => a.partial_cmp(b),
            // Don't compare cnums here. If we've extracted it and it goes first, then it's already
            // been compared.
            (Token::CitationNumber(_), Token::CitationNumber(_)) => None,
            _ => None,
        }
    }
}

impl<'a> Iterator for TokenIterator<'a> {
    type Item = Token<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remain.is_empty() {
            return None;
        }
        if let Ok((remainder, token)) = token(self.remain) {
            self.remain = remainder;
            Some(token)
        } else {
            None
        }
    }
}

use citeproc_io::SmartString;

#[derive(Debug, PartialEq, Eq)]
pub struct NaturalCmp(SmartString);
impl NaturalCmp {
    pub fn new(s: SmartString) -> Option<Self> {
        if s.is_empty() {
            None
        } else {
            Some(NaturalCmp(s))
        }
    }
}
impl PartialOrd for NaturalCmp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for NaturalCmp {
    fn cmp(&self, other: &Self) -> Ordering {
        natural_cmp(&self.0, &other.0)
    }
}

fn natural_cmp(a: &str, b: &str) -> Ordering {
    let a_i = TokenIterator { remain: a };
    let b_i = TokenIterator { remain: b };
    let mut iter = a_i.zip(b_i);
    let mut o = Ordering::Equal;
    while let Some((a_t, b_t)) = iter.next() {
        if o != Ordering::Equal {
            return o;
        }
        if let Some(c) = a_t.partial_cmp(&b_t) {
            o = c;
        }
    }
    o
}

#[test]
fn natural_cmp_strings() {
    assert_eq!(natural_cmp("a", "z"), Ordering::Less, "a - z");
    assert_eq!(natural_cmp("z", "a"), Ordering::Greater, "z - a");
    assert_eq!(
        natural_cmp("a\u{E000}2009_0407\u{E001}", "a\u{E000}2008_0407\u{E001}"),
        Ordering::Greater,
        "2009 > 2008"
    );
    assert_eq!(
        natural_cmp("a\u{E000}2009_0507\u{E001}", "a\u{E000}2009_0407\u{E001}"),
        Ordering::Greater
    );
    assert_eq!(
        natural_cmp("a\u{E000}-0100_\u{E001}", "a\u{E000}0100_\u{E001}"),
        Ordering::Less,
        "100BC < 100AD"
    );

    // 2000, May 2000, May 1st 2000
    assert_eq!(
        natural_cmp("a\u{E000}2000_\u{E001}", "a\u{E000}2000_04\u{E001}"),
        Ordering::Less,
        "2000 < May 2000"
    );
    assert_eq!(
        natural_cmp("a\u{E000}2000_04\u{E001}", "a\u{E000}2000_0401\u{E001}"),
        Ordering::Less,
        "May 2000 < May 1st 2000"
    );

    assert_eq!(
        natural_cmp(
            "a\u{E000}2009_0407/0000_0000\u{E001}",
            "a\u{E000}2009_0407/2010_0509\u{E001}"
        ),
        Ordering::Less,
        "2009 < 2009/2010"
    );

    assert_eq!(
        natural_cmp(
            "\u{e000}-044_0315/0000_00\u{e001}",
            "\u{e000}-100_0713/0000_00\u{e001}"
        ),
        Ordering::Greater,
        "44BC > 100BC"
    );

    // Numbers
    assert_eq!(
        natural_cmp("\u{E002}1000\u{E003}", "\u{E002}1000\u{E003}"),
        Ordering::Equal,
        "1000 == 1000"
    );
    assert_eq!(
        natural_cmp("\u{E002}1000\u{E003}", "\u{E002}2000\u{E003}"),
        Ordering::Less,
        "1000 < 2000"
    );

    // Case insensitive only means that "a" is before "B" even though it appears later in ascii,
    // i.e. both "A" and "a" sort the same relative to "B" and "b".
    // Tie-breakers between "A" and "a" are deterministically caps-first
    // You still never get Ordering::Equal unless they are identical
    assert_eq!(natural_cmp("Aaa", "ABC"), Ordering::Less);
    assert_eq!(natural_cmp("AAA", "Aaa"), Ordering::Less);
}
