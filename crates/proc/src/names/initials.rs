// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use std::borrow::Cow;
/// use GivenNameToken::*;
/// "John R L" == &[Name("John"), Initial("R"), Initial("L")]
/// "Jean-Luc K" = &[Name("Jean"), HyphenSegment("Luc"), Initial("K")]
/// "R. L." = &[Initial("R"), Initial("L")]
///
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GivenNameToken<'n> {
    Name(&'n str),
    Initial(&'n str),
    HyphenSegment(&'n str),
    Other(&'n str),
}

use self::GivenNameToken::*;

pub fn initialize<'n>(
    given_name: &'n str,
    initialize: bool,
    with: Option<&str>,
    initialize_with_hyphens: bool,
) -> Cow<'n, str> {
    if let Some(with) = with {
        #[derive(Copy, Clone, PartialEq)]
        enum State {
            Start,
            AfterInitial,
            AfterName,
        }

        let mut state = State::Start;
        let mut build = String::with_capacity(given_name.len());

        let mut process_token = |token: GivenNameToken| {
            state = match token {
                Name(ref n) => {
                    if initialize {
                        if state == State::AfterName {
                            // Exactly one space please
                            build.truncate(build.trim_end().len());
                            build.push(' ');
                        }
                        // name_LongAbbreviation.txt i.e. GIven => Gi.
                        if n.chars().any(|c| c.is_lowercase()) {
                            let mut iter = n.chars();
                            let mut seen_one = false;
                            while let Some(c) = iter.next() {
                                let upper = c.is_uppercase();
                                if upper && seen_one {
                                    build.extend(c.to_lowercase());
                                    continue;
                                } else if upper {
                                    build.push(c);
                                    seen_one = true;
                                    continue;
                                } else if !seen_one {
                                    build.push(c);
                                }
                                break;
                            }
                        } else {
                            build.push(n.chars().nth(0).unwrap());
                        }
                        build.push_str(with);
                        State::AfterInitial
                    } else {
                        if state != State::Start {
                            build.truncate(build.trim_end().len());
                            build.push(' ');
                        }
                        build.push_str(n);
                        State::AfterName
                    }
                }
                Initial(ref n) => {
                    if state == State::AfterName {
                        // Exactly one space please
                        build.truncate(build.trim_end().len());
                        build.push(' ');
                    }
                    build.push_str(n);
                    build.push_str(with);
                    State::AfterInitial
                }
                HyphenSegment(ref n) => {
                    if n.chars().nth(0).map_or(true, |c| c.is_lowercase()) {
                        state
                    } else if initialize {
                        if initialize_with_hyphens {
                            // Trim trailing whitespace from the previous with, as you don't want
                            // J. -L., you want J.-L.
                            build.truncate(build.trim_end().len());
                            build.push('-');
                        }
                        build.push(n.chars().nth(0).unwrap());
                        build.push_str(with);
                        State::AfterInitial
                    } else {
                        build.push('-');
                        build.push_str(n);
                        State::AfterName
                    }
                }
                Other(ref n) => {
                    if state != State::Start {
                        // Exactly one space please
                        build.truncate(build.trim_end().len());
                        build.push(' ');
                    }
                    build.push_str(n);
                    State::AfterName
                }
            }
        };

        for token in tokenize(given_name) {
            process_token(token)
        }

        Cow::Owned(build.trim().into())
    } else {
        Cow::Borrowed(given_name)
    }
}

use nom::{
    branch::alt,
    bytes::complete::{take_while, take_while1, take_while_m_n},
    character::complete::char as nom_char,
    combinator::{map, opt, recognize, rest},
    sequence::{preceded, terminated, tuple},
    IResult,
};

// Need:
// Ph.  => [Initial("Ph")]
// MA.  => [Initial("M"), Initial("A")]
// M.A. => [Initial("M"), Initial("A")]
// MA   => [Initial("M"), Initial("A")]
// Ma   => [Name("Ma")]
// aa   => [Other("aa")]

fn uppercase_char(inp: &str) -> IResult<&str, &str> {
    take_while_m_n(1, 1, |c: char| c.is_uppercase())(inp)
}

// Don't need to be certain there's a dot on the end, as the whole-string-no-dots case is
// already handled by name().
// "M.Ph" => ["M", "Ph"]
// "Ph."  => ["Ph"]
fn without_dot(inp: &str) -> IResult<&str, &str> {
    alt((
        recognize(tuple((uppercase_char, take_while(|c: char| c != '.')))),
        uppercase_char,
    ))(inp)
}

// "P" => "P"
// "Ph" => "Ph"
// "Ph." => "Ph"
fn initial_maybe_dot(inp: &str) -> IResult<&str, GivenNameToken<'_>> {
    map(
        terminated(without_dot, opt(nom_char('.'))),
        GivenNameToken::Initial,
    )(inp)
}

// "P" => "P"
// "Ph" => "Ph"
// "Ph." => "Ph"
// "PA" => "P" with remaining "A"
fn initial_with_dot(inp: &str) -> IResult<&str, GivenNameToken<'_>> {
    map(
        terminated(without_dot, nom_char('.')),
        GivenNameToken::Initial,
    )(inp)
}

fn normal(c: char) -> bool {
    !(c == '.' || c == '-')
}

// Anything starting with uppercase and no dots in it.
fn name(inp: &str) -> IResult<&str, GivenNameToken<'_>> {
    map(
        recognize(tuple((uppercase_char, take_while1(normal)))),
        GivenNameToken::Name,
    )(inp)
}

fn hyphen(inp: &str) -> IResult<&str, GivenNameToken<'_>> {
    map(
        preceded(nom_char('-'), take_while1(normal)),
        GivenNameToken::HyphenSegment,
    )(inp)
}

// E.g. "de"
fn other(inp: &str) -> IResult<&str, GivenNameToken<'_>> {
    map(rest, GivenNameToken::Other)(inp)
}

fn token(inp: &str, state: IterState) -> IResult<&str, GivenNameToken<'_>> {
    match state {
        IterState::Start => alt((hyphen, initial_with_dot, name, initial_maybe_dot, other))(inp),
        IterState::TriedFull => alt((hyphen, initial_maybe_dot, other))(inp),
    }
}

#[derive(Copy, Clone)]
enum IterState {
    Start,
    TriedFull,
}

fn tokenize<'a>(given_name: &'a str) -> impl Iterator<Item = GivenNameToken<'a>> {
    struct TokenIter<'a> {
        state: IterState,
        remain: &'a str,
    }

    impl<'a> Iterator for TokenIter<'a> {
        type Item = GivenNameToken<'a>;
        fn next(&mut self) -> Option<Self::Item> {
            if self.remain.is_empty() {
                None
            } else if let Ok((remainder, token)) = token(self.remain, self.state) {
                self.state = IterState::TriedFull;
                self.remain = remainder;
                Some(token)
            } else {
                None
            }
        }
    }

    given_name
        .split(' ')
        .filter(|x| !x.is_empty())
        .flat_map(|word| TokenIter {
            state: IterState::Start,
            remain: word,
        })
}

#[test]
fn test_tokenize() {
    fn tok(inp: &str) -> Vec<GivenNameToken<'_>> {
        tokenize(inp).collect()
    }
    assert_eq!(
        &tok("Ph. M.E.")[..],
        &[
            GivenNameToken::Initial("Ph"),
            GivenNameToken::Initial("M"),
            GivenNameToken::Initial("E"),
        ][..]
    );
    assert_eq!(&tok("ME")[..], &[GivenNameToken::Name("ME")][..]);
    assert_eq!(&tok("ME.")[..], &[GivenNameToken::Initial("ME")][..]);
    assert_eq!(
        &tok("A. Alan")[..],
        &[GivenNameToken::Initial("A"), GivenNameToken::Name("Alan")][..]
    );
}

#[test]
fn test_initialize_true_empty() {
    fn init(given_name: &str) -> Cow<'_, str> {
        initialize(given_name, true, Some(""), false)
    }
    assert_eq!(init("ME"), "M");
    assert_eq!(init("ME."), "ME");
    assert_eq!(init("A. Alan"), "AA");
    assert_eq!(init("John R L"), "JRL");
    assert_eq!(init("Jean-Luc K"), "JLK");
    assert_eq!(init("R. L."), "RL");
    assert_eq!(init("R L"), "RL");
    assert_eq!(init("John R.L."), "JRL");
    assert_eq!(init("John R L de Bortoli"), "JRL de B");
}

#[test]
fn test_initialize_true_period() {
    fn init(given_name: &str) -> Cow<'_, str> {
        initialize(given_name, true, Some("."), true)
    }
    assert_eq!(init("ME"), "M.");
    assert_eq!(init("ME."), "ME.");
    assert_eq!(init("A. Alan"), "A.A.");
    assert_eq!(init("John R L"), "J.R.L.");
    assert_eq!(init("Jean-Luc K"), "J.-L.K.");
    assert_eq!(init("R. L."), "R.L.");
    assert_eq!(init("R L"), "R.L.");
    assert_eq!(init("John R.L."), "J.R.L.");
    assert_eq!(init("John R L de Bortoli"), "J.R.L. de B.");
    assert_eq!(init("好 好"), "好 好");
}

#[test]
fn test_initialize_true_period_space() {
    fn init(given_name: &str) -> Cow<'_, str> {
        initialize(given_name, true, Some(". "), true)
    }
    assert_eq!(init("ME"), "M.");
    assert_eq!(init("ME."), "ME.");
    assert_eq!(init("A. Alan"), "A. A.");
    assert_eq!(init("John R L"), "J. R. L.");
    assert_eq!(init("Jean-Luc K"), "J.-L. K.");
    assert_eq!(init("R. L."), "R. L.");
    assert_eq!(init("R L"), "R. L.");
    assert_eq!(init("John R.L."), "J. R. L.");
    assert_eq!(init("John R L de Bortoli"), "J. R. L. de B.");
    assert_eq!(init("好 好"), "好 好");
}

#[test]
fn test_initialize_false_period() {
    fn init(given_name: &str) -> Cow<'_, str> {
        initialize(given_name, false, Some("."), true)
    }
    assert_eq!(init("ME"), "ME");
    assert_eq!(init("ME."), "ME.");
    assert_eq!(init("A. Alan"), "A. Alan");
    assert_eq!(init("John R L"), "John R.L.");
    assert_eq!(init("Jean-Luc K"), "Jean-Luc K.");
    assert_eq!(init("R. L."), "R.L.");
    assert_eq!(init("R L"), "R.L.");
    assert_eq!(init("John R.L."), "John R.L.");
    assert_eq!(init("John R L de Bortoli"), "John R.L. de Bortoli");
    assert_eq!(init("好 好"), "好 好");
    assert_eq!(init("Immel, Ph. M.E."), "Immel, Ph.M.E.")
}

#[test]
fn test_initialize_false_period_space() {
    fn init(given_name: &str) -> Cow<'_, str> {
        initialize(given_name, false, Some(". "), true)
    }
    assert_eq!(init("ME"), "ME");
    assert_eq!(init("ME."), "ME.");
    assert_eq!(init("A. Alan"), "A. Alan");
    assert_eq!(init("John R L"), "John R. L.");
    assert_eq!(init("Jean-Luc K"), "Jean-Luc K.");
    assert_eq!(init("R. L."), "R. L.");
    assert_eq!(init("R L"), "R. L.");
    assert_eq!(init("John R.L."), "John R. L.");
    assert_eq!(init("John R L de Bortoli"), "John R. L. de Bortoli");
    assert_eq!(init("好 好"), "好 好");
    assert_eq!(init("Immel, Ph. M.E."), "Immel, Ph. M. E.")
}
