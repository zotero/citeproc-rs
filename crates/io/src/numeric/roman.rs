// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2020 Corporation for Digital Scholarship

//! Conversion between integers and roman numerals.
//!
//! Duplicated because we want lowercase by default to work with text-casing.
//! Original, 'unlicensed': https://github.com/linfir/roman.rs

#![allow(dead_code)]

pub static ROMAN_ALLOWED: &'static str = "ivxlcdmIVXLCDM";

static ROMAN: &[(char, u32)] = &[
    // Lower
    ('i', 1),
    ('v', 5),
    ('x', 10),
    ('l', 50),
    ('c', 100),
    ('d', 500),
    ('m', 1000),
    // Upper
    ('I', 1),
    ('V', 5),
    ('X', 10),
    ('L', 50),
    ('C', 100),
    ('D', 500),
    ('M', 1000),
];
static ROMAN_PAIRS: &[(&str, u32)] = &[
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
pub static MAX: u32 = 3999;

/// Converts an integer into a roman numeral.
///
/// Works for integer between 1 and 3999 inclusive, returns None otherwise.
///
///
pub fn to(n: u32) -> Option<String> {
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
    let roman = "i ii iii iv v vi vii viii ix x xi xii xiii xiv xv xvi xvii xviii xix xx xxi xxii"
        .split(' ');
    for (i, x) in roman.enumerate() {
        let n = (i + 1) as u32;
        assert_eq!(to(n).unwrap(), x);
    }
    assert_eq!(to(1984).unwrap(), "mcmlxxxiv");
}

/// Converts a roman numeral to an integer.
///
/// Works for integer between 1 and 3999 inclusive, returns None otherwise.
///
///
pub fn from(txt: &str) -> Option<u32> {
    let n = match from_lax(txt) {
        Some(n) => n,
        None => return None,
    };
    match to(n) {
        Some(ref x) if x.eq_ignore_ascii_case(txt) => Some(n),
        _ => None,
    }
}

fn from_lax(txt: &str) -> Option<u32> {
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
    assert!(from("I").is_some());
}

#[test]
fn test_to_from() {
    for n in 1..=MAX {
        assert_eq!(from(&to(n).unwrap()).unwrap(), n);
    }
}

#[test]
fn test_to_from_upper() {
    for n in 1..=MAX {
        let lower = to(n).unwrap();
        let upper = lower.to_uppercase();
        assert_eq!(from(&upper).unwrap(), n);
    }
}
