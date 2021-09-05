// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::NumberLike;
use crate::String;
use std::borrow::Cow;

pub mod roman;

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum NumericValue<'a> {
    Tokens(
        Cow<'a, str>,
        Vec<NumericToken>,
        /* is_num i.e. parsed perfectly */ bool,
    ),
    /// For values that could not be parsed.
    Str(Cow<'a, str>),
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum NumericValueOwned {
    Tokens(Vec<NumericToken>),
    Str(String),
}
impl From<NumericValue<'_>> for NumericValueOwned {
    fn from(other: NumericValue) -> Self {
        match other {
            NumericValue::Tokens(_, tk, _) => NumericValueOwned::Tokens(tk.clone()),
            NumericValue::Str(s) => NumericValueOwned::Str(String::from(s.as_ref())),
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum NumericToken {
    Num(u32),
    Roman(u32, /* uppercase */ bool),
    Affixed(String, u32, String),
    Str(String),
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
            Roman(u, _) => Some(u),
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
        NumericValue::Tokens(format!("{}", i).into(), vec![Num(i)], true)
    }
    pub fn page_first(&self) -> Option<Self> {
        self.first_num().map(NumericValue::num)
    }
    fn first_num(&self) -> Option<u32> {
        match *self {
            NumericValue::Tokens(_, ref ts, _) => ts.get(0).and_then(|token| token.get_num()),
            NumericValue::Str(_) => None,
        }
    }
    pub fn is_numeric(&self) -> bool {
        match *self {
            NumericValue::Tokens(_, _, is_numeric) => is_numeric,
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
            NumericValue::Tokens(_, ref ts, _isnum) => {
                if var.is_quantity() {
                    match ts.len() {
                        0 => true, // doesn't matter
                        1 => {
                            if let Some(Num(i)) = ts.get(0) {
                                *i != 1
                            } else {
                                false
                            }
                        }
                        _ => true,
                    }
                } else {
                    ts.len() > 1
                }
            }

            NumericValue::Str(_) => false,
        }
    }
    pub fn verbatim(&self) -> &str {
        match self {
            NumericValue::Tokens(verb, _, _isnum) => verb,
            NumericValue::Str(s) => s,
        }
    }
}

// Parsing

impl<'a> NumericValue<'a> {
    fn parse_full(input: &'a str, and_term: &'a str) -> Self {
        if let Ok((remainder, mut parsed)) = num_tokens(and_term)(input) {
            if remainder.is_empty() {
                if parsed
                    .iter()
                    .any(|x| matches!(x, Num(_) | Roman(..) | Affixed(..)))
                {
                    NumericValue::Tokens(input.into(), parsed, true)
                } else {
                    NumericValue::Str(input.into())
                }
            } else if !parsed.is_empty() {
                // Use as much as we can, slap the rest on as a block.
                // Also, cement that it can't be numeric even if it was partially parsed.
                parsed.push(Str(remainder.into()));
                NumericValue::Tokens(input.into(), parsed, false)
            } else {
                NumericValue::Str(input.into())
            }
        } else {
            NumericValue::Str(input.into())
        }
    }
    #[cfg(test)]
    fn parse(input: &'a str) -> Self {
        NumericValue::parse_full(input, "and")
    }
    pub fn from_localized(and_term: &'a str) -> impl Fn(&'a NumberLike) -> NumericValue<'a> + 'a {
        move |like| match like {
            // locator_WithLeadingSpace
            NumberLike::Str(input) => NumericValue::parse_full(input.trim(), and_term),
            NumberLike::Num(n) => NumericValue::num(*n),
        }
    }
}

use nom::{
    branch::alt,
    bytes::complete::{escaped, is_not, tag},
    character::complete::{char, digit1, one_of},
    combinator::{map, map_parser, opt},
    multi::{fold_many1, many0, many0_count},
    sequence::{delimited, tuple},
    IResult,
};

fn sep_and<'a>(and_term: &'a str) -> impl Fn(&'a str) -> IResult<&'a str, NumericToken> + 'a {
    move |inp| {
        let (inp, comma) = opt(tag(", "))(inp)?;
        let (inp, _and) = alt((tag(and_term), tag("and")))(inp)?;
        Ok((inp, if comma.is_some() { CommaAnd } else { And }))
    }
}

fn sep_or_and<'a>(and_term: &'a str) -> impl Fn(&'a str) -> IResult<&'a str, NumericToken> + 'a {
    fn sep_from(input: char) -> NumericToken {
        match input {
            ',' => Comma,
            '-' | '\u{2013}' => Hyphen,
            '&' => Ampersand,
            _ => unreachable!(),
        }
    }
    move |inp| alt((sep_and(and_term), map(one_of(",&-\u{2013}"), sep_from)))(inp)
}

fn sep<'a>(and_term: &'a str) -> impl Fn(&'a str) -> IResult<&'a str, NumericToken> + 'a {
    move |inp| {
        delimited(
            many0_count(char(' ')),
            sep_or_and(and_term),
            many0_count(char(' ')),
        )(inp)
    }
}

/// Parses and counts leading zeros
fn from_digits(input: &str) -> IResult<&str, u32> {
    nom::parse_to!(input, u32)
}

fn int(inp: &str) -> IResult<&str, NumericToken> {
    let (rem, digits) = digit1(inp)?;
    let (_empty, parsed) = from_digits(digits)?;
    Ok((rem, NumericToken::Num(parsed)))
}

// Try to parse affixed versions first, because
// 2b => Affixed("2b")
// not   Num(2), Err("b")

// Let people use \- etc to avoid making a separator
fn esc<'a>(
    f: impl Fn(&'a str) -> IResult<&'a str, &'a str>,
) -> impl FnOnce(&'a str) -> IResult<&'a str, &'a str> {
    move |i| {
        // For whatever reason, escaped() accepts "" even if the inner parser does not.
        // So we have to guard it
        if i.len() == 0 {
            return Err(nom::Err::Error(nom::error::Error::new(
                i,
                nom::error::ErrorKind::Escaped,
            )));
        }
        escaped(f, '\\', one_of(r#"\ ,&-\u{2013}"#))(i)
    }
}

/// Undo the backslash escaping
/// Any unrecognized escapes are not unescaped. So `\& => &` but `\a => \a`.
fn unescape(inp: &str) -> String {
    let mut out = String::new();
    let mut after_backslash = false;
    for (i, ch) in inp.char_indices() {
        if ch == '\\'
            && !after_backslash
            && inp[i..].chars().nth(0).map_or(false, |c| {
                c == ' ' || c == ',' || c == '-' || c == '&' || c == '\\' || c == '\u{2013}'
            })
        {
            after_backslash = true;
        } else {
            out.push(ch);
            after_backslash = false;
        }
    }
    out
}

fn num_pre(inp: &str) -> IResult<&str, &str> {
    // Note! Does not exclude zero. So this will pick up a leading zero prefix.
    esc(is_not("\\ ,&-\u{2013}123456789"))(inp)
}

fn non_sep(inp: &str) -> IResult<&str, &str> {
    esc(is_not(" ,&-\u{2013}"))(inp)
}
use nom::error::Error as NomError;

fn num_alpha_num(inp: &str) -> IResult<&str, NumericToken> {
    #[derive(Debug, PartialEq)]
    enum Blk {
        Num(usize, u32),
        Alpha(usize),
    }
    impl Blk {
        fn num(s: &str) -> IResult<&str, Self> {
            let (rem, n) = from_digits(s)?;
            Ok((rem, Blk::Num(s.len(), n)))
        }
        fn alpha(s: &str) -> Self {
            Blk::Alpha(s.len())
        }
    }

    #[derive(Clone, Debug)]
    enum Acc {
        Len(usize),
        LenNum {
            prefix: usize,
            num_len: usize,
            num: u32,
            post_num_len: usize,
        },
    }
    // Split it into a sequence of numeric / non-numerics, and save the last numeric one
    let (rem, res) = fold_many1(
        alt((map(num_pre, Blk::alpha), map_parser(digit1, Blk::num))),
        Acc::Len(0),
        |acc, neu| match neu {
            Blk::Num(new_num_len, new_num) => match acc {
                Acc::Len(prefix) => Acc::LenNum {
                    prefix,
                    num_len: new_num_len,
                    num: new_num,
                    post_num_len: 0,
                },
                Acc::LenNum {
                    prefix,
                    num_len: old_num_len,
                    num: _,
                    post_num_len,
                } => Acc::LenNum {
                    prefix: prefix + old_num_len + post_num_len,
                    num_len: new_num_len,
                    num: new_num,
                    post_num_len: 0,
                },
            },
            Blk::Alpha(len) => match acc {
                Acc::Len(prefix) => Acc::Len(prefix + len),
                Acc::LenNum {
                    prefix,
                    num_len,
                    num,
                    post_num_len,
                } => Acc::LenNum {
                    prefix,
                    num_len,
                    num,
                    post_num_len: post_num_len + len,
                },
            },
        },
    )(inp)?;
    let token = match res {
        Acc::Len(_len) => {
            return Err(nom::Err::Error(NomError::new(
                inp,
                nom::error::ErrorKind::Many1,
            )))
        }
        Acc::LenNum {
            prefix,
            num_len,
            num,
            post_num_len,
        } => {
            if prefix == 0 && post_num_len == 0 {
                Num(num)
            } else {
                let pre = unescape(&inp[..prefix]);
                let suf = unescape(&inp[prefix + num_len..prefix + num_len + post_num_len]);
                Affixed(pre, num, suf)
            }
        }
    };
    Ok((rem, token))
}

#[test]
fn test_affixed() {
    assert_eq!(num_alpha_num("123"), Ok(("", nn(123))));
    assert_eq!(num_alpha_num("123n110"), Ok(("", afxd("123n", 110, ""))));
}

fn roman_numeral(inp: &str) -> IResult<&str, NumericToken> {
    let (rest, potential) = non_sep(inp)?;
    if let Some(rom) = roman::from(potential) {
        return Ok((
            rest,
            NumericToken::Roman(
                rom as u32,
                potential.chars().nth(0).map_or(false, |x| x.is_uppercase()),
            ),
        ));
    }
    Err(nom::Err::Error(nom::error::Error::new(
        inp,
        nom::error::ErrorKind::ParseTo,
    )))
}

fn num_ish(inp: &str) -> IResult<&str, NumericToken> {
    alt((roman_numeral, num_alpha_num, int))(inp)
}

fn num_tokens<'a>(
    and_term: &'a str,
) -> impl Fn(&'a str) -> IResult<&'a str, Vec<NumericToken>> + 'a {
    move |inp| {
        map(
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
}

#[test]
fn test_num_token_parser() {
    assert_eq!(num_ish("2"), Ok(("", nn(2))));
    assert_eq!(num_ish("2b"), Ok(("", afxd("", 2, "b"))));
    assert_eq!(sep("et")("- "), Ok(("", Hyphen)));
    assert_eq!(sep("et")(", "), Ok(("", Comma)));
    assert_eq!(
        num_tokens("et")("2, 3"),
        Ok(("", vec![nn(2), Comma, nn(3)]))
    );
    assert_eq!(
        num_tokens("et")("2 - 5, 9"),
        Ok(("", vec![nn(2), Hyphen, nn(5), Comma, nn(9)]))
    );
    assert_eq!(
        num_tokens("et")("2 - 5, 9, edition"),
        Ok((", edition", vec![nn(2), Hyphen, nn(5), Comma, nn(9),]))
    );
}

#[cfg(test)]
fn afxd(p: &str, n: u32, s: &str) -> NumericToken {
    Affixed(p.into(), n, s.into())
}

#[cfg(test)]
fn nn(n: u32) -> NumericToken {
    Num(n)
}

#[cfg(test)]
macro_rules! test_parse {
    ($inp:literal, @noparse) => {
        assert_eq!(
            NumericValue::parse($inp),
            NumericValue::Str($inp.into()),
        )
    };
    ($inp:literal, [ $($x:expr),+ ]) => {
        assert_eq!(
            NumericValue::parse($inp),
            NumericValue::Tokens($inp.into(), vec![ $($x),* ], true)
        )
    };
    ($inp:literal, [ $($x:expr),+ ], false) => {
        assert_eq!(
            NumericValue::parse($inp),
            NumericValue::Tokens($inp.into(), vec![ $($x),* ], false)
        )
    };
}

#[test]
fn test_numeric_value() {
    test_parse!("2-5, 9", [nn(2), Hyphen, nn(5), Comma, nn(9)]);
    test_parse!("[1.2.3]", [afxd("[1.2.", 3, "]")]);
    test_parse!(
        "[3], (5), [17.1.89(4(1))(2)(a)(i)]",
        [
            afxd("[", 3, "]"),
            Comma,
            afxd("(", 5, ")"),
            Comma,
            afxd("[17.1.89(4(1))(", 2, ")(a)(i)]")
        ]
    );
    test_parse!("1998-VIII", [nn(1998), Hyphen, Roman(8, true)]);
    // Random extra text is parsed into units
    test_parse!(
        "2 - 5, 9, edition, iv",
        [
            nn(2),
            Hyphen,
            nn(5),
            Comma,
            nn(9),
            Str(", edition, iv".into())
        ],
        false
    );
}

#[test]
fn test_weird_affixes() {
    test_parse!("123N110", [afxd("123N", 110, "")]);
    // The leading zeroes should be included in the prefix
    test_parse!("0110", [afxd("0", 110, "")]);
    test_parse!("N0110", [afxd("N0", 110, "")]);
}

#[test]
fn test_numeric_escape() {
    test_parse!("3\\-B", [afxd("", 3, "-B")]);
}

#[test]
fn test_integer_overflow() {
    // https://github.com/cormacrelf/citeproc-rs/issues/93
    // This shouldn't panic when parsing an overflowing integer,
    // it should just see it as a string.
    test_parse!("071124114012001-???", @noparse);
}

#[test]
fn test_page_first() {
    assert_eq!(
        NumericValue::parse("2-5, 9").page_first().unwrap(),
        NumericValue::num(2)
    );
}
