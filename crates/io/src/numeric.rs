// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use std::borrow::Cow;

#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub enum NumericValue {
    Tokens(String, Vec<NumericToken>),
    /// For values that could not be parsed.
    Str(String),
}

#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub enum NumericToken {
    Num(u32),
    Affixed(String),
    Comma,
    Hyphen,
    Ampersand,
}

use self::NumericToken::*;

impl NumericToken {
    pub fn get_num(&self) -> Option<u32> {
        match *self {
            Num(u) => Some(u),
            _ => None,
        }
    }
}

/// Either a parsed vector of numeric tokens, or the raw string input.
///
/// Relevant parts of the Spec:
///
/// * [`<choose is-numeric="...">`](https://docs.citationstyles.org/en/stable/specification.html#choose)
/// * [`<number>`](https://docs.citationstyles.org/en/stable/specification.html#number)
///
/// We parse:
///
/// ```text
/// "2, 4"         => Tokens([Num(2), Comma, Num(4)])
/// "2-4, 5"       => Tokens([Num(2), Hyphen, Num(4), Comma, Num(5)])
/// "2 -4    , 5"  => Tokens([Num(2), Hyphen, Num(4), Comma, Num(5)])
/// "2nd"          => Tokens([Affixed("2nd")])
/// "L2"           => Tokens([Affixed("L2")])
/// "L2tp"         => Tokens([Affixed("L2tp")])
/// "2nd-4th"      => Tokens([Affixed("2nd"), Hyphen, Affixed("4th")])
/// ```
///
/// We don't parse:
///
/// ```text
/// "2nd edition"  => Err("edition") -> not numeric -> Str("2nd edition")
/// "-5"           => Err("-5") -> not numeric -> Str("-5")
/// "5,,7"         => Err(",7") -> not numeric -> Str("5,,7")
/// "5 7 9 11"     => Err("7 9 11") -> not numeric -> Str("5 7 9 11")
/// "5,"           => Err("") -> not numeric -> Str("5,")
/// ```
///
/// It's a number, then a { comma|hyphen|ampersand } with any whitespace, then another number, and
/// so on. All numbers are unsigned.

impl NumericValue {
    pub fn num(i: u32) -> Self {
        NumericValue::Tokens(format!("{}", i), vec![Num(i)])
    }
    pub fn page_first(&self) -> Option<NumericValue> {
        self.first_num().map(NumericValue::num)
    }
    fn first_num(&self) -> Option<u32> {
        match *self {
            NumericValue::Tokens(_, ref ts) => ts.get(0).and_then(|token| token.get_num()),
            NumericValue::Str(_) => None,
        }
    }
    pub fn is_numeric(&self) -> bool {
        match *self {
            NumericValue::Tokens(_, _) => true,
            NumericValue::Str(_) => false,
        }
    }
    pub fn is_multiple(&self) -> bool {
        match *self {
            NumericValue::Tokens(_, ref ts) => {
                match ts.len() {
                    0 => false,
                    1 => if let Some(NumericToken::Num(i)) = ts.get(0) {
                        *i != 1
                    } else {
                        false
                    }
                    _ => true
                }
            },

            // TODO: fallback interpretation of "multiple" to include unparsed numerics that have
            // multiple numbers etc
            //
            // “contextual” - (default), the term plurality matches that of the variable content.
            // Content is considered plural when it contains multiple numbers (e.g. “page 1”,
            // “pages 1-3”, “volume 2”, “volumes 2 & 4”), or, in the case of the “number-of-pages”
            // and “number-of-volumes” variables, when the number is higher than 1 (“1 volume” and
            // “3 volumes”).
            NumericValue::Str(_) => false,
        }
    }
    pub fn verbatim(&self) -> &str {
        match self {
            NumericValue::Tokens(verb, _) => verb.as_str(),
            NumericValue::Str(s) => s.as_str(),
        }
    }
}

// Ordering

use std::cmp::{Ord, Ordering, PartialOrd};
impl Ord for NumericToken {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Num(a), Num(b)) => a.cmp(b),
            (Affixed(a), Affixed(b)) => a.cmp(b),
            (Hyphen, Hyphen) => Ordering::Equal,
            (Comma, Comma) => Ordering::Equal,
            (Ampersand, Ampersand) => Ordering::Equal,
            _ => Ordering::Equal,
        }
    }
}

impl Ord for NumericValue {
    fn cmp(&self, other: &Self) -> Ordering {
        use self::NumericValue::*;
        match (self, other) {
            (Tokens(_, a), Tokens(_, b)) => a.cmp(b),
            (Tokens(a, _), Str(b)) => a.cmp(b),
            (Str(a), Tokens(b, _)) => a.cmp(b),
            (Str(a), Str(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for NumericToken {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialOrd for NumericValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Parsing

fn from_digits(input: &str) -> Result<u32, std::num::ParseIntError> {
    input.parse()
}

fn to_affixed(input: &str) -> NumericToken {
    NumericToken::Affixed(input.to_string())
}

fn sep_from(input: char) -> Result<NumericToken, ()> {
    match input {
        ',' => Ok(Comma),
        '-' => Ok(Hyphen),
        '&' => Ok(Ampersand),
        _ => Err(()),
    }
}

use nom::{
    branch::alt,
    bytes::complete::is_not,
    character::complete::{char, digit1, one_of},
    combinator::{map, map_res, recognize},
    multi::{many0, many1},
    sequence::{delimited, tuple},
    IResult,
};

fn int(inp: &str) -> IResult<&str, u32> {
    map_res(digit1, from_digits)(inp)
}

fn num(inp: &str) -> IResult<&str, NumericToken> {
    map(int, NumericToken::Num)(inp)
}

// Try to parse affixed versions first, because
// 2b => Affixed("2b")
// not   Num(2), Err("b")

fn num_pre(inp: &str) -> IResult<&str, &str> {
    is_not(" ,&-01234567890")(inp)
}

fn num_suf(inp: &str) -> IResult<&str, &str> {
    is_not(" ,&-")(inp)
}

fn prefix1(inp: &str) -> IResult<&str, NumericToken> {
    map(
        recognize(tuple((many1(num_pre), digit1, many0(num_suf)))),
        to_affixed,
    )(inp)
}

fn suffix1(inp: &str) -> IResult<&str, NumericToken> {
    map(
        recognize(tuple((many0(num_pre), digit1, many1(num_suf)))),
        to_affixed,
    )(inp)
}

fn num_ish(inp: &str) -> IResult<&str, NumericToken> {
    alt((prefix1, suffix1, num))(inp)
}

fn sep(inp: &str) -> IResult<&str, NumericToken> {
    map_res(
        delimited(many0(char(' ')), one_of(",&-"), many0(char(' '))),
        sep_from,
    )(inp)
}

fn num_tokens(inp: &str) -> IResult<&str, Vec<NumericToken>> {
    map(
        tuple((num_ish, many0(tuple((sep, num_ish))))),
        |(n, rest)| {
            let mut new = Vec::with_capacity(rest.len() * 2);
            new.push(n);
            rest.into_iter().fold(new, |mut acc, p| {
                acc.push(p.0);
                acc.push(p.1);
                acc
            })
        },
    )(inp)
}

#[test]
fn test_num_token_parser() {
    assert_eq!(num_ish("2"), Ok(("", Num(2))));
    assert_eq!(
        num_ish("2b"),
        Ok(("", NumericToken::Affixed("2b".to_string())))
    );
    assert_eq!(sep("- "), Ok(("", Hyphen)));
    assert_eq!(sep(", "), Ok(("", Comma)));
    assert_eq!(num_tokens("2, 3"), Ok(("", vec![Num(2), Comma, Num(3)])));
    assert_eq!(
        num_tokens("2 - 5, 9"),
        Ok(("", vec![Num(2), Hyphen, Num(5), Comma, Num(9)]))
    );
    assert_eq!(
        num_tokens("2 - 5, 9, edition"),
        Ok((", edition", vec![Num(2), Hyphen, Num(5), Comma, Num(9)]))
    );
}

impl<'r> From<Cow<'r, str>> for NumericValue {
    fn from(input: Cow<'r, str>) -> Self {
        if let Ok((remainder, parsed)) = num_tokens(&input) {
            if remainder.is_empty() {
                NumericValue::Tokens(input.into_owned(), parsed)
            } else {
                NumericValue::Str(input.into_owned())
            }
        } else {
            NumericValue::Str(input.into_owned())
        }
    }
}

#[test]
fn test_numeric_value() {
    assert_eq!(
        NumericValue::from(Cow::Borrowed("2-5, 9")),
        NumericValue::Tokens(
            String::from("2-5, 9"),
            vec![Num(2), Hyphen, Num(5), Comma, Num(9)]
        )
    );
    assert_eq!(
        NumericValue::from(Cow::Borrowed("2 - 5, 9, edition")),
        NumericValue::Str("2 - 5, 9, edition".into())
    );
    assert_eq!(
        NumericValue::from(Cow::Borrowed("[1.2.3]")),
        NumericValue::Tokens(
            String::from("[1.2.3]"),
            vec![Affixed("[1.2.3]".to_string())]
        )
    );
    assert_eq!(
        NumericValue::from(Cow::Borrowed("[3], (5), [17.1.89(4(1))(2)(a)(i)]")),
        NumericValue::Tokens(
            String::from("[3], (5), [17.1.89(4(1))(2)(a)(i)]"),
            vec![
                Affixed("[3]".to_string()),
                Comma,
                Affixed("(5)".to_string()),
                Comma,
                Affixed("[17.1.89(4(1))(2)(a)(i)]".to_string())
            ]
        )
    );
}

#[test]
fn test_page_first() {
    assert_eq!(
        NumericValue::from(Cow::Borrowed("2-5, 9"))
            .page_first()
            .unwrap(),
        NumericValue::num(2)
    );
}
