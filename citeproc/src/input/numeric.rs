use std::borrow::Cow;
use nom::types::CompleteStr;
use nom::*;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum NumericToken {
    Num(u32),
    Affixed(String),
    Comma,
    Hyphen,
    Ampersand,
}

use self::NumericToken::*;

impl NumericToken {
    fn get_num(&self) -> Option<u32> {
        match *self {
            Num(u) => Some(u),
            _ => None,
        }
    }
}

fn tokens_to_string(ts: &[NumericToken]) -> String {
    let mut s = String::with_capacity(ts.len());
    for t in ts {
        match t {
            // TODO: ordinals, etc
            Num(i) => s.push_str(&format!("{}", i)),
            Affixed(a) => s.push_str(&a),
            Comma => s.push_str(", "),
            // en-dash
            Hyphen => s.push_str("\u{2013}"),
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

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum NumericValue {
    Tokens(String, Vec<NumericToken>),
    /// For values that could not be parsed.
    Str(String),
}

impl NumericValue {
    pub fn num(i: u32) -> Self {
        NumericValue::Tokens(format!("{}", i), vec![Num(i)])
    }
    pub fn page_first(&self) -> Option<NumericValue> {
        self.first_num().map(|n| NumericValue::num(n))
    }
    fn first_num(&self) -> Option<u32> {
        match *self {
            NumericValue::Tokens(_, ref ts) => ts.iter().nth(0).and_then(|token| token.get_num()),
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
            NumericValue::Tokens(_, ref ts) => ts.len() > 1,

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
    pub fn verbatim(&self, replace_hyphens: bool) -> String {
        let s = match self {
            NumericValue::Tokens(verb, _) => verb,
            NumericValue::Str(s) => s,
        };
        if replace_hyphens {
            s.replace('-', "\u{2013}")
        } else {
            s.clone()
        }
    }

    pub fn as_number(&self, replace_hyphens: bool) -> String {
        match self {
            NumericValue::Tokens(_, ts) => tokens_to_string(ts),
            NumericValue::Str(s) => {
                if replace_hyphens {
                    s.replace('-', "\u{2013}")
                } else {
                    s.clone()
                }
            }
        }
    }
}

fn from_digits(input: CompleteStr) -> Result<u32, std::num::ParseIntError> {
    input.parse()
}

fn to_affixed(input: CompleteStr) -> NumericToken {
    NumericToken::Affixed(input.0.to_string())
}

fn sep_from<'s>(input: char) -> Result<NumericToken, ()> {
    match input {
        ',' => Ok(Comma),
        '-' => Ok(Hyphen),
        '&' => Ok(Ampersand),
        _ => Err(()),
    }
}

named!(int<CompleteStr, u32>, map_res!(call!(digit1), from_digits));
named!(num<CompleteStr, NumericToken>, map!(call!(int), NumericToken::Num));

// Try to parse affixed versions first, because
// 2b => Affixed("2b")
// not   Num(2), Err("b")

named!(num_pre<CompleteStr, CompleteStr>, is_not!(" ,&-01234567890"));
named!(num_suf<CompleteStr, CompleteStr>, is_not!(" ,&-"));

named!(prefix1<CompleteStr, NumericToken>,
    map!(
        recognize!(tuple!(many1!(call!(num_pre)), call!(digit1), many0!(call!(num_suf)))),
        to_affixed
    )
);

named!(suffix1<CompleteStr, NumericToken>,
    map!(
        recognize!(tuple!(many0!(call!(num_pre)), call!(digit1), many1!(call!(num_suf)))),
        to_affixed
    )
);

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
    assert_eq!(
        num_ish(CompleteStr("2b")),
        Ok((CompleteStr(""), NumericToken::Affixed("2b".to_string())))
    );
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
        Ok((
            CompleteStr(", edition"),
            vec![Num(2), Hyphen, Num(5), Comma, Num(9)]
        ))
    );
}

impl<'r> From<Cow<'r, str>> for NumericValue {
    fn from(input: Cow<'r, str>) -> Self {
        if let Ok((remainder, parsed)) = num_tokens(CompleteStr(&input)) {
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
        NumericValue::from(String::from("2-5, 9")),
        NumericValue::Tokens(
            String::from("2-5, 9"),
            vec![Num(2), Hyphen, Num(5), Comma, Num(9)]
        )
    );
    assert_eq!(
        NumericValue::from(String::from("2 - 5, 9, edition")),
        NumericValue::Str("2 - 5, 9, edition".into())
    );
    assert_eq!(
        NumericValue::from(String::from("[1.2.3]")),
        NumericValue::Tokens(
            String::from("[1.2.3]"),
            vec![Affixed("[1.2.3]".to_string())]
        )
    );
    assert_eq!(
        NumericValue::from(String::from("[3], (5), [17.1.89(4(1))(2)(a)(i)]")),
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
        NumericValue::from(String::from("2-5, 9"))
            .page_first()
            .unwrap(),
        NumericValue::num(2)
    );
}
