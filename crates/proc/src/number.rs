use crate::prelude::*;
use citeproc_io::NumericToken::{self, *};
use citeproc_io::{NumericValue, roman};
use csl::{
    Gender, Locale, MiscTerm, NumberVariable, OrdinalTerm, OrdinalTermSelector, PageRangeFormat,
    SimpleTermSelector, TermFormExtended,
};
use std::fmt::Write;

pub fn render_ordinal(
    ts: &[NumericToken],
    locale: &Locale,
    variable: NumberVariable,
    _prf: Option<PageRangeFormat>,
    gender: Gender,
    long: bool,
) -> SmartString {
    let mut s = SmartString::new();
    for token in ts {
        match *token {
            Num(n) | Roman(n, _) => {
                if !long || n == 0 || n > 10 {
                    write!(s, "{}", n).unwrap();
                }
                let term = OrdinalTerm::from_number_for_selector(n, long);
                if let Some(suffix) = locale.get_ordinal_term(OrdinalTermSelector(term, gender)) {
                    s.push_str(suffix);
                }
            }
            Affixed(ref pre, num, ref suf) => {
                write!(s, "{}{}{}", pre, num, suf).unwrap();
            }
            Str(ref str) => {
                s.push_str(&str);
            }
            Comma => s.push_str(", "),
            // en-dash
            Hyphen => s.push_str(get_hyphen(locale, variable)),
            Ampersand => {
                s.push(' ');
                s.push_str(get_ampersand(locale));
                s.push(' ');
            }
            And | CommaAnd => {
                if *token == CommaAnd {
                    s.push(',');
                }
                s.push(' ');
                s.push_str(locale.and_term(None).unwrap_or("and"));
                s.push(' ');
            }
        }
    }
    s
}

fn get_ampersand(locale: &Locale) -> &str {
    let sel = SimpleTermSelector::Misc(MiscTerm::And, TermFormExtended::Symbol);
    // NO fallback; only want the symbol
    if let Some(amp) = locale.simple_terms.get(&sel) {
        amp.singular().trim()
    } else {
        "&"
    }
}

pub fn get_hyphen(locale: &Locale, variable: NumberVariable) -> &str {
    // A few more than the spec's list of en-dashable variables
    // https://github.com/Juris-M/citeproc-js/blob/1aa49dd2ab9a1c85d3060073780d65c86754a438/src/util_number.js#L584
    let get = |term: MiscTerm| {
        let sel = SimpleTermSelector::Misc(term, TermFormExtended::Symbol);
        locale
            .get_simple_term(sel)
            .map(|amp| amp.singular().trim())
            .unwrap_or("\u{2013}")
    };
    match variable {
        NumberVariable::Page
        | NumberVariable::Locator
        | NumberVariable::Issue
        | NumberVariable::Volume
        | NumberVariable::Edition
        | NumberVariable::Number => get(MiscTerm::PageRangeDelimiter),
        NumberVariable::CollectionNumber => get(MiscTerm::YearRangeDelimiter),
        _ => "-",
    }
}

#[test]
fn test_get_hyphen() {
    let loc = &Locale::default();
    assert_eq!(get_hyphen(loc, NumberVariable::Locator), "\u{2013}");
}

pub fn arabic_number(
    num: &NumericValue,
    locale: &Locale,
    variable: NumberVariable,
    prf: Option<PageRangeFormat>,
) -> SmartString {
    debug!("arabic_number {:?}", num);
    match num {
        NumericValue::Tokens(_, ts) => tokens_to_string(ts, locale, variable, prf),
        NumericValue::Str(s) => s.as_ref().into(),
    }
}

fn tokens_to_string(
    ts: &[NumericToken],
    locale: &Locale,
    variable: NumberVariable,
    prf: Option<PageRangeFormat>,
) -> SmartString {
    let mut s = SmartString::new();
    #[derive(Copy, Clone)]
    enum NumBefore {
        SeenNum(u32),
        SeenNumHyphen(u32),
    }
    let mut state = None;
    let mut iter = ts.iter().peekable();
    while let Some(t) = iter.next() {
        state = match *t {
            // TODO: ordinals, etc
            Num(i) => {
                let (cropped, newstate) = match (prf, state) {
                    (Some(prf), Some(NumBefore::SeenNumHyphen(prev))) => {
                        (crate::page_range::truncate_prf(prf, prev, i), None)
                    }
                    _ => (i, Some(NumBefore::SeenNum(i))),
                };
                write!(s, "{}", cropped).unwrap();
                newstate
            }
            Roman(i, upper) => {
                // Prefer to print roman as roman, because usually roman means a preamble page.
                if let Some(x) = roman::to(i) {
                    if upper {
                        s.push_str(&x.to_ascii_uppercase());
                    } else {
                        s.push_str(&x);
                    }
                }
                None
            }
            Affixed(ref pre, num, ref suf) => {
                write!(s, "{}{}{}", pre, num, suf).unwrap();
                None
            }
            Str(ref str) => {
                s.push_str(&str);
                None
            }
            Comma => {
                s.push_str(", ");
                None
            }
            Hyphen => {
                let hyphen = get_hyphen(locale, variable);
                s.push_str(hyphen);
                match state {
                    Some(NumBefore::SeenNum(i)) => Some(NumBefore::SeenNumHyphen(i)),
                    _ => None,
                }
            }
            Ampersand => {
                s.push(' ');
                s.push_str(get_ampersand(locale));
                s.push(' ');
                None
            }
            And | CommaAnd => {
                if *t == CommaAnd {
                    s.push(',');
                }
                s.push(' ');
                s.push_str(locale.and_term(None).unwrap_or("and"));
                s.push(' ');
                None
            }
        }
    }
    s
}

/// Numbers bigger than 3999 are too cumbersome anyway
pub fn roman_representable(val: &NumericValue) -> bool {
    match val {
        NumericValue::Tokens(_, ts) => ts
            .iter()
            .filter_map(NumericToken::get_num)
            .all(|t| t <= roman::MAX),
        _ => false,
    }
}

pub fn roman_lower(
    ts: &[NumericToken],
    locale: &Locale,
    variable: NumberVariable,
    _prf: Option<PageRangeFormat>,
) -> SmartString {
    let mut s = SmartString::new();
    use std::convert::TryInto;
    for t in ts {
        match t {
            Roman(i, _) | Num(i) => {
                if let Some(x) = roman::to(*i) {
                    s.push_str(&x);
                }
            }
            Affixed(ref pre, num, ref suf) => {
                write!(s, "{}{}{}", pre, num, suf).unwrap();
            }
            Str(ref str) => {
                s.push_str(&str)
            }
            Comma => s.push_str(", "),
            // en-dash
            Hyphen => s.push_str(get_hyphen(locale, variable)),
            Ampersand => {
                s.push(' ');
                s.push_str(get_ampersand(locale));
                s.push(' ');
            }
            And | CommaAnd => {
                if *t == CommaAnd {
                    s.push(',');
                }
                s.push(' ');
                s.push_str(locale.and_term(None).unwrap_or("and"));
                s.push(' ');
            }
        }
    }
    s
}

#[test]
fn test_roman_lower() {
    let ts = &[
        NumericToken::Num(3),
        NumericToken::Hyphen,
        NumericToken::Num(11),
        NumericToken::Comma,
        NumericToken::Affixed("".into(), 2, "E".into()),
        NumericToken::Roman(3, false),
        NumericToken::Roman(3, true),
    ];
    assert_eq!(
        &roman_lower(&ts[..], &Locale::default(), NumberVariable::Locator, None),
        "iii\u{2013}xi, 2E, iii, iii"
    );
}

