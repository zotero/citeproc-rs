// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use std::borrow::Cow;
use crate::NumberLike;

#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub enum NumericValue<'a> {
    Tokens(Cow<'a, str>, Vec<NumericToken>),
    /// For values that could not be parsed.
    Str(Cow<'a, str>),
}

#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub enum NumericToken {
    Num(u32),
    Affixed(String),
    Comma,
    Hyphen,
    Ampersand,
    And,
    CommaAnd,
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

impl<'a> NumericValue<'a> {
    pub fn num(i: u32) -> Self {
        NumericValue::Tokens(format!("{}", i).into(), vec![Num(i)])
    }
    pub fn page_first(&self) -> Option<Self> {
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
    pub fn is_multiple(&self, var: csl::NumberVariable) -> bool {
        match *self {
            // “contextual” - (default), the term plurality matches that of the variable
            // content. Content is considered plural when it contains multiple numbers (e.g.
            // “page 1”, “pages 1-3”, “volume 2”, “volumes 2 & 4”), or, in the case of the
            // “number-of-pages” and “number-of-volumes” variables, when the number is higher
            // than 1 (“1 volume” and “3 volumes”).
            NumericValue::Tokens(_, ref ts) => {
                if var.is_quantity() {
                    match ts.len() {
                        0 => true, // doesn't matter
                        1 => if let Some(NumericToken::Num(i)) = ts.get(0) {
                            *i != 1
                        } else {
                            false
                        }
                        _ => true
                    }
                } else {
                    ts.len() > 1
                }
            },

            NumericValue::Str(_) => false,
        }
    }
    pub fn verbatim(&self) -> &str {
        match self {
            NumericValue::Tokens(verb, _) => verb,
            NumericValue::Str(s) => s,
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
            (And, And) => Ordering::Equal,
            _ => Ordering::Equal,
        }
    }
}

impl Ord for NumericValue<'_> {
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

impl PartialOrd for NumericValue<'_> {
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

use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{char, digit1, one_of},
    combinator::{map, map_res, recognize, opt},
    multi::{many0, many1},
    sequence::{delimited, tuple},
    IResult,
};

fn sep_from(input: char) -> NumericToken {
    match input {
        ',' => Comma,
        '-' => Hyphen,
        '&' => Ampersand,
        _ => unreachable!()
    }
}

fn sep_and<'a>(and_term: &'a str) -> impl Fn(&'a str) -> IResult<&'a str, NumericToken> + 'a {
    move |inp| {
        let (inp, comma) = opt(tag(", "))(inp)?;
        let (inp, and) = alt((tag(and_term), tag("and")))(inp)?;
        Ok((inp, if comma.is_some() {
            CommaAnd
        } else {
            And
        }))
    }
}

fn sep_or_and<'a>(and_term: &'a str) -> impl Fn(&'a str) -> IResult<&'a str, NumericToken> + 'a {
    move |inp| {
        alt((sep_and(and_term), map(one_of(",&-"), sep_from)))(inp)
    }
}

fn sep<'a>(and_term: &'a str) -> impl Fn(&'a str) -> IResult<&'a str, NumericToken> + 'a {
    move |inp| delimited(many0(char(' ')), sep_or_and(and_term), many0(char(' '))) (inp)
}

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

fn num_tokens<'a>(and_term: &'a str) -> impl Fn(&'a str) -> IResult<&'a str, Vec<NumericToken>> + 'a {
    move |inp| map(
        tuple((num_ish, many0(tuple((sep(and_term), num_ish))))),
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
    assert_eq!(sep("et")("- "), Ok(("", Hyphen)));
    assert_eq!(sep("et")(", "), Ok(("", Comma)));
    assert_eq!(num_tokens("et")("2, 3"), Ok(("", vec![Num(2), Comma, Num(3)])));
    assert_eq!(
        num_tokens("et")("2 - 5, 9"),
        Ok(("", vec![Num(2), Hyphen, Num(5), Comma, Num(9)]))
    );
    assert_eq!(
        num_tokens("et")("2 - 5, 9, edition"),
        Ok((", edition", vec![Num(2), Hyphen, Num(5), Comma, Num(9)]))
    );
}

impl<'a> NumericValue<'a> {
    fn parse_full(input: &'a str, and_term: &'a str) -> Self {
        if let Ok((remainder, parsed)) = num_tokens(and_term)(input) {
            if remainder.is_empty() {
                NumericValue::Tokens(input.into(), parsed)
            } else {
                NumericValue::Str(input.into())
            }
        } else {
            NumericValue::Str(input.into())
        }
    }
    fn parse(input: &'a str) -> Self {
        NumericValue::parse_full(input, "and")
    }
    pub fn from_localized(and_term: &'a str) -> impl Fn(&'a NumberLike) -> NumericValue<'a> + 'a {
        move |like| match like {
            NumberLike::Str(input) => NumericValue::parse_full(input, and_term),
            NumberLike::Num(n) => NumericValue::num(*n),
        }
    }
}

#[test]
fn test_numeric_value() {
    assert_eq!(
        NumericValue::parse("2-5, 9"),
        NumericValue::Tokens(
            "2-5, 9".into(),
            vec![Num(2), Hyphen, Num(5), Comma, Num(9)]
        )
    );
    assert_eq!(
        NumericValue::parse("2 - 5, 9, edition"),
        NumericValue::Str("2 - 5, 9, edition".into())
    );
    assert_eq!(
        NumericValue::parse("[1.2.3]"),
        NumericValue::Tokens(
            "[1.2.3]".into(),
            vec![Affixed("[1.2.3]".to_string())]
        )
    );
    assert_eq!(
        NumericValue::parse("[3], (5), [17.1.89(4(1))(2)(a)(i)]"),
        NumericValue::Tokens(
            "[3], (5), [17.1.89(4(1))(2)(a)(i)]".into(),
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
        NumericValue::parse("2-5, 9")
            .page_first()
            .unwrap(),
        NumericValue::num(2)
    );
}
