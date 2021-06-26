use crate::prelude::*;
use citeproc_io::NumericToken::{self, *};
use citeproc_io::{roman, NumericValue};
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
        NumericValue::Tokens(_, ts, _) => tokens_to_string(ts, locale, variable, prf),
        NumericValue::Str(s) => s.as_ref().into(),
    }
}

#[derive(Debug, Copy, Clone)]
enum NumBefore {
    SeenNum(u32),
    SeenNumHyphen(u32),
    SeenRoman(u32),
    SeenRomanHyphen(u32),
}
impl NumBefore {
    fn matching_for_crop(&self, _is_roman: bool) -> Option<u32> {
        match *self {
            NumBefore::SeenNumHyphen(n) => Some(n),
            NumBefore::SeenRomanHyphen(n) => Some(n),
            _ => None,
        }
    }
    fn see_num(num: u32, is_roman: bool) -> Self {
        if is_roman {
            NumBefore::SeenRoman(num)
        } else {
            NumBefore::SeenNum(num)
        }
    }
}
#[derive(Debug, Copy, Clone)]
enum State<'a> {
    Normal,
    Hyphenating { prefix: &'a str, last: NumBefore },
}
#[derive(Debug, Copy, Clone)]
enum HyphenInsert {
    None,
    Simple,
    Locale,
}
impl<'a> State<'a> {
    fn crop(
        &self,
        prf: Option<PageRangeFormat>,
        num: u32,
        is_roman: bool,
        pfx: &'a str,
        sfx: &'a str,
    ) -> (&'a str, u32, HyphenInsert, Self) {
        use crate::page_range::truncate_prf;
        match *self {
            State::Normal if sfx.is_empty() => (
                pfx,
                num,
                HyphenInsert::None,
                State::Hyphenating {
                    prefix: pfx,
                    last: NumBefore::see_num(num, is_roman),
                },
            ),
            State::Normal => (pfx, num, HyphenInsert::None, State::Normal),
            State::Hyphenating { prefix, last } if pfx == prefix => {
                // Prefixes match, we're going to crop it
                if let Some(last_num) = last.matching_for_crop(is_roman) {
                    if let Some(prf) = prf {
                        let cropped = truncate_prf(prf, last_num, num);
                        let crop_prefix = match prf {
                            PageRangeFormat::Expanded => pfx,
                            _ => "",
                        };
                        (crop_prefix, cropped, HyphenInsert::Locale, State::Normal)
                    } else {
                        // The spec says this should be HyphenInsert::Simple, but it breaks
                        // ~ a hundred CSL tests, so the spec is to be ignored...
                        (pfx, num, HyphenInsert::Locale, State::Normal)
                    }
                } else {
                    (pfx, num, HyphenInsert::Simple, State::Normal)
                }
            }
            State::Hyphenating { prefix: _, last: _ } => {
                (pfx, num, HyphenInsert::Simple, State::Normal)
            }
        }
    }
    fn see_hyphen(&self) -> Self {
        match self {
            State::Normal => State::Normal,
            State::Hyphenating { last, prefix } => {
                let neu = match *last {
                    NumBefore::SeenNum(n) => NumBefore::SeenNumHyphen(n),
                    NumBefore::SeenRoman(n) => NumBefore::SeenRomanHyphen(n),
                    a => a,
                };
                State::Hyphenating { last: neu, prefix }
            }
        }
    }
    fn non_num_should_push_hyphen(&self) -> HyphenInsert {
        match self {
            State::Hyphenating { last, .. } => match last {
                NumBefore::SeenNumHyphen(_) | NumBefore::SeenRomanHyphen(_) => HyphenInsert::Simple,
                _ => HyphenInsert::None,
            },
            _ => HyphenInsert::None,
        }
    }
}

impl HyphenInsert {
    fn write(&self, s: &mut SmartString, locale: &Locale, variable: NumberVariable) {
        // eprintln!(" => {:?}", self);
        match self {
            HyphenInsert::Locale => {
                let hyphen = get_hyphen(locale, variable);
                s.push_str(hyphen);
            }
            HyphenInsert::Simple => {
                s.push('-');
            }
            HyphenInsert::None => {}
        }
    }
}

fn tokens_to_string(
    ts: &[NumericToken],
    locale: &Locale,
    variable: NumberVariable,
    prf: Option<PageRangeFormat>,
) -> SmartString {
    let mut s = SmartString::new();
    let mut state = State::Normal;
    let mut iter = ts.iter().peekable();
    while let Some(t) = iter.next() {
        // eprintln!("{:?}\n -  {:?}", state, t);
        state = match *t {
            Hyphen => state.see_hyphen(),
            Num(i) => {
                let (_, cropped, hyphen, newstate) = state.crop(prf, i, false, "", "");
                hyphen.write(&mut s, locale, variable);
                write!(s, "{}", cropped).unwrap();
                newstate
            }
            Affixed(ref pre, num, ref suf) => {
                let (prefix, cropped, hyphen, newstate) = state.crop(prf, num, false, pre, suf);
                hyphen.write(&mut s, locale, variable);
                write!(s, "{}{}{}", prefix, cropped, suf).unwrap();
                newstate
            }
            Roman(i, upper) => {
                let (_, _, hyphen, newstate) = state.crop(prf, i, true, "", "");
                hyphen.write(&mut s, locale, variable);
                // Prefer to print roman as roman, because usually roman means a preamble page.
                if let Some(x) = roman::to(i) {
                    if upper {
                        s.push_str(&x.to_ascii_uppercase());
                    } else {
                        s.push_str(&x);
                    }
                }
                newstate
            }
            Str(ref str) => {
                state
                    .non_num_should_push_hyphen()
                    .write(&mut s, locale, variable);
                s.push_str(&str);
                State::Normal
            }
            Comma => {
                state
                    .non_num_should_push_hyphen()
                    .write(&mut s, locale, variable);
                s.push_str(", ");
                State::Normal
            }
            Ampersand => {
                state
                    .non_num_should_push_hyphen()
                    .write(&mut s, locale, variable);
                s.push(' ');
                s.push_str(get_ampersand(locale));
                s.push(' ');
                State::Normal
            }
            And | CommaAnd => {
                state
                    .non_num_should_push_hyphen()
                    .write(&mut s, locale, variable);
                if *t == CommaAnd {
                    s.push(',');
                }
                s.push(' ');
                s.push_str(locale.and_term(None).unwrap_or("and"));
                s.push(' ');
                State::Normal
            }
        }
    }
    // eprintln!("... Final state: {:?}   =>   {:?}", state, s);
    s
}

/// Numbers bigger than 3999 are too cumbersome anyway
pub fn roman_representable(val: &NumericValue) -> bool {
    match val {
        NumericValue::Tokens(_, ts, _) => ts
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
            Str(ref str) => s.push_str(&str),
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
        NumericToken::Comma,
        NumericToken::Roman(3, false),
        NumericToken::Comma,
        NumericToken::Roman(3, true),
    ];
    assert_eq!(
        &roman_lower(&ts[..], &Locale::default(), NumberVariable::Locator, None),
        "iii\u{2013}xi, 2E, iii, iii"
    );
}
