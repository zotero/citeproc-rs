use nom::types::CompleteStr;
use nom::{self, digit1, alpha0, alpha1};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum NumericToken<'r> {
    Num(u32),
    Affixed(&'r str),
    Comma,
    Hyphen,
    Ampersand,
}

use self::NumericToken::*;

fn tokens_to_string(ts: &[NumericToken]) -> String {
    let mut s = String::with_capacity(ts.len());
    for t in ts {
        match t {
            // TODO: ordinals, etc
            Num(i) => s.push_str(&format!("{}", i)),
            Affixed(a) => s.push_str(a),
            Comma => s.push_str(", "),
            // TODO: en-dash? from locale. yeah.
            Hyphen => s.push_str("-"),
            Ampersand => s.push_str(" & "),
        }
    }
    s
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
/// ```
/// "2, 4"         => Tokens([Num(2), Comma, Num(4)])
/// "2-4, 5"       => Tokens([Num(2), Hyphen, Num(4), Comma, Num(5)])
/// "2 -4    , 5"  => Tokens([Num(2), Hyphen, Num(4), Comma, Num(5)])
/// "2nd"          => Tokens([Aff("",  2, "nd")])
/// "L2"           => Tokens([Aff("L", 2, "")])
/// "L2tp"         => Tokens([Aff("L", 2, "tp")])
/// "2nd-4th"      => Tokens([Aff("",  2, "nd"), Hyphen, Aff("", 4, "th")])
/// ```
///
/// We don't parse:
///
/// ```
/// "2nd edition"  => Err("edition") -> not numeric -> Str("2nd edition")
/// "-5"           => Err("-5") -> not numeric -> Str("-5")
/// "5,,7"         => Err(",7") -> not numeric -> Str("5,,7")
/// "5 7 9 11"     => Err("7 9 11") -> not numeric -> Str("5 7 9 11")
/// "5,"           => Err("") -> not numeric -> Str("5,")
/// ```
///
/// It's a number, then a { comma|hyphen|ampersand } with any whitespace, then another number, and
/// so on. All numbers are unsigned.

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum NumericValue<'r> {
    // for values arriving as actual integers
    Tokens(Vec<NumericToken<'r>>),
    // for values that were originally strings, and maybe got parsed into numbers as an alternative
    Str(&'r str),
}

impl<'r> NumericValue<'r> {
    pub fn num(i: u32) -> Self {
        NumericValue::Tokens(vec![Num(i)])
    }
    pub fn is_numeric(&self) -> bool {
        match *self {
            NumericValue::Tokens(_) => true,
            NumericValue::Str(_) => false,
        }
    }
    pub fn to_string(&self) -> String {
        match self {
            NumericValue::Tokens(ts) => tokens_to_string(ts),
            NumericValue::Str(s) => (*s).into(),
        }
    }
}

fn from_digits(input: CompleteStr) -> Result<u32, std::num::ParseIntError> {
    input.parse()
}

fn to_affixed<'s>(input: CompleteStr<'s>) -> NumericToken<'s> {
    NumericToken::Affixed(input.0)
}

fn sep_from<'s>(input: char) -> Result<NumericToken<'s>, ()> {
    match input {
        ',' => Ok(Comma),
        '-' => Ok(Hyphen),
        '&' => Ok(Ampersand),
        _ => Err(())
    }
}

named!(int<CompleteStr, u32>, map_res!(call!(digit1), from_digits));
named!(num<CompleteStr, NumericToken>, map!(call!(int), NumericToken::Num));

named!(prefix1<CompleteStr, NumericToken>,
    map!(
        recognize!(tuple!(call!(alpha1), call!(digit1), call!(alpha0))),
        to_affixed
    )
);
named!(suffix1<CompleteStr, NumericToken>,
    map!(
        recognize!(tuple!(call!(alpha0), call!(digit1), call!(alpha1))),
        to_affixed
    )
);

// Try to parse affixed versions first, because
// 2b => Aff("", 2, "b")
// not   Num(2), Err("b")
named!(num_ish<CompleteStr, NumericToken>,
       alt!(call!(prefix1) | call!(suffix1) | call!(num)));


named!(
    sep<CompleteStr, NumericToken>,
    map_res!(delimited!(
        many0!(char!(' ')),
        one_of!(",&-"),
        many0!(char!(' '))
    ), sep_from)
);

named!(
    num_tokens<CompleteStr, Vec<NumericToken> >,
    map!(tuple!(
        call!(num_ish), 
        many0!(tuple!( call!(sep), call!(num_ish) ))
    ), |(n, rest)| {
        let mut new = Vec::with_capacity(rest.len() * 2);
        new.push(n);
        rest.into_iter()
            .fold(new, |mut acc, p| { acc.push(p.0); acc.push(p.1); acc })
    })
);

#[test]
fn test_num_token_parser() {
    assert_eq!(num_ish(CompleteStr("2")), Ok((CompleteStr(""), Num(2))));
    assert_eq!(num_ish(CompleteStr("2b")), Ok((CompleteStr(""), NumericToken::Affixed("2b"))));
    assert_eq!(sep(CompleteStr("- ")), Ok((CompleteStr(""), Hyphen)));
    assert_eq!(sep(CompleteStr(", ")), Ok((CompleteStr(""), Comma)));
    assert_eq!(
        num_tokens(CompleteStr("2, 3")),
        Ok((CompleteStr(""), vec![Num(2), Comma, Num(3)]))
    );
    assert_eq!(
        num_tokens(CompleteStr("2 - 5, 9")),
        Ok((CompleteStr(""), vec![Num(2), Hyphen, Num(5), Comma, Num(9)]))
    );
    assert_eq!(
        num_tokens(CompleteStr("2 - 5, 9, edition")),
        Ok((CompleteStr(", edition"), vec![Num(2), Hyphen, Num(5), Comma, Num(9)]))
    );
}


impl<'r> From<&'r str> for NumericValue<'r> {
    fn from(input: &'r str) -> Self {
        if let Ok((remainder, parsed)) = num_tokens(CompleteStr(input)) {
            if remainder.is_empty() {
                NumericValue::Tokens(parsed)
            } else {
                NumericValue::Str(input)
            }
        } else {
            NumericValue::Str(input)
        }
    }
}

#[test]
fn test_numeric_value() {
    assert_eq!(
        NumericValue::from("2-5, 9"),
        NumericValue::Tokens(vec![Num(2), Hyphen, Num(5), Comma, Num(9)])
    );
    assert_eq!(
        NumericValue::from("2 - 5, 9, edition"),
        NumericValue::Str("2 - 5, 9, edition")
    );
}


