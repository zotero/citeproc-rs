use citeproc_io::NumericToken::{self, *};
use citeproc_io::NumericValue;
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
) -> String {
    let mut s = String::new();
    for token in ts {
        match *token {
            NumericToken::Num(n) => {
                if !long || n == 0 || n > 10 {
                    write!(s, "{}", n).unwrap();
                }
                let term = OrdinalTerm::from_number_for_selector(n, long);
                if let Some(suffix) = locale.get_ordinal_term(OrdinalTermSelector(term, gender)) {
                    s.push_str(suffix);
                }
            }
            Affixed(ref a) => s.push_str(&a),
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
) -> String {
    debug!("{:?}", num);
    match num {
        NumericValue::Tokens(_, ts) => tokens_to_string(ts, locale, variable, prf),
        NumericValue::Str(s) => s.as_ref().to_owned(),
    }
}

fn tokens_to_string(
    ts: &[NumericToken],
    locale: &Locale,
    variable: NumberVariable,
    prf: Option<PageRangeFormat>,
) -> String {
    let mut s = String::with_capacity(ts.len());
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
            Affixed(ref a) => {
                s.push_str(&a);
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
            .all(|t| t <= 3999),
        _ => false,
    }
}

pub fn roman_lower(
    ts: &[NumericToken],
    locale: &Locale,
    variable: NumberVariable,
    _prf: Option<PageRangeFormat>,
) -> String {
    let mut s = String::with_capacity(ts.len() * 2); // estimate
    use std::convert::TryInto;
    for t in ts {
        match t {
            // TODO: ordinals, etc
            Num(i) => {
                if let Some(x) = roman::to((*i).try_into().unwrap()) {
                    s.push_str(&x);
                }
            }
            Affixed(a) => s.push_str(&a),
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
        NumericToken::Affixed("2E".into()),
    ];
    assert_eq!(
        &roman_lower(&ts[..], &Locale::default(), NumberVariable::Locator, None),
        "iii\u{2013}xi, 2E"
    );
}

#[allow(dead_code)]
mod roman {
    //! Conversion between integers and roman numerals.
    //!
    //! Duplicated because we want lowercase by default to work with text-casing.
    //! Original, 'unlicensed': https://github.com/linfir/roman.rs

    static ROMAN: &[(char, i32)] = &[
        ('i', 1),
        ('v', 5),
        ('x', 10),
        ('l', 50),
        ('c', 100),
        ('d', 500),
        ('m', 1000),
    ];
    static ROMAN_PAIRS: &[(&str, i32)] = &[
        ("m", 1000),
        ("cm", 900),
        ("d", 500),
        ("cd", 400),
        ("c", 100),
        ("xc", 90),
        ("l", 50),
        ("xl", 40),
        ("x", 10),
        ("ix", 9),
        ("v", 5),
        ("iv", 4),
        ("i", 1),
    ];

    /// The largest number representable as a roman numeral.
    pub static MAX: i32 = 3999;

    /// Converts an integer into a roman numeral.
    ///
    /// Works for integer between 1 and 3999 inclusive, returns None otherwise.
    ///
    ///
    pub fn to(n: i32) -> Option<String> {
        if n <= 0 || n > MAX {
            return None;
        }
        let mut out = String::new();
        let mut n = n;
        for &(name, value) in ROMAN_PAIRS.iter() {
            while n >= value {
                n -= value;
                out.push_str(name);
            }
        }
        assert!(n == 0);
        Some(out)
    }

    #[test]
    fn test_to_roman() {
        let roman =
            "i ii iii iv v vi vii viii ix x xi xii xiii xiv xv xvi xvii xviii xix xx xxi xxii"
                .split(' ');
        for (i, x) in roman.enumerate() {
            let n = (i + 1) as i32;
            assert_eq!(to(n).unwrap(), x);
        }
        assert_eq!(to(1984).unwrap(), "mcmlxxxiv");
    }

    /// Converts a roman numeral to an integer.
    ///
    /// Works for integer between 1 and 3999 inclusive, returns None otherwise.
    ///
    ///
    pub fn from(txt: &str) -> Option<i32> {
        let n = match from_lax(txt) {
            Some(n) => n,
            None => return None,
        };
        match to(n) {
            Some(ref x) if *x == txt => Some(n),
            _ => None,
        }
    }

    fn from_lax(txt: &str) -> Option<i32> {
        let (mut n, mut max) = (0, 0);
        for c in txt.chars().rev() {
            let &(_, val) = ROMAN.iter().find(|x| {
                let &(ch, _) = *x;
                ch == c
            })?;
            if val < max {
                n -= val;
            } else {
                n += val;
                max = val;
            }
        }
        Some(n)
    }

    #[test]
    fn test_from() {
        assert!(from("I").is_none());
    }

    #[test]
    fn test_to_from() {
        for n in 1..MAX {
            assert_eq!(from(&to(n).unwrap()).unwrap(), n);
        }
    }
}
