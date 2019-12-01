// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

#![feature(custom_test_frameworks)]
#![test_runner(datatest::runner)]

use test_utils::{humans::parse_human_test, yaml::parse_yaml_test};

use lazy_static::lazy_static;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use std::fmt;
use std::fs::read_to_string;
use std::path::Path;

/// See https://github.com/colin-kiegel/rust-pretty-assertions/issues/24
///
/// Wrapper around string slice that makes debug output `{:?}` to print string same way as `{}`.
/// Used in different `assert*!` macros in combination with `pretty_assertions` crate to make
/// test failures to show nice diffs.
#[derive(PartialEq, Eq)]
#[doc(hidden)]
pub struct PrettyString<'a>(pub &'a str);

/// Make diff to display string as multi-line string
impl<'a> fmt::Debug for PrettyString<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.0)
    }
}

lazy_static! {
    static ref IGNORES: HashSet<String> = {
        let mut m = HashSet::new();
        // cargo test -- 2>/dev/null | rg 'bib tests' |  rg suite | cut -d' ' -f2 | cut -d: -f3 | cut -d\' -f1 > bibtests.txt
        let ignores = include_str!("./data/ignore.txt");
        for name in ignores.lines() {
            // Comments
            if name.trim_start().starts_with("#") {
                continue;
            }
            m.insert(name.to_string());
        }
        m
    };
}

fn is_ignore(path: &Path) -> bool {
    let fname = path.file_name().unwrap().to_string_lossy();
    IGNORES.contains(&fname.into_owned())
}

use std::sync::Once;

static INIT: Once = Once::new();
fn setup() {
    INIT.call_once(|| {
        env_logger::init();
    });
}

#[datatest::files("tests/data/test-suite/processor-tests/humans", {
    path in r"^(.*)\.txt" if !is_ignore,
})]
fn csl_test_suite(path: &Path) {
    setup();
    let input = read_to_string(path).unwrap();
    let mut test_case = parse_human_test(&input);
    if let Some(res) = test_case.execute() {
        assert_eq!(PrettyString(&res), PrettyString(&test_case.result));
    }
}

#[datatest::files("tests/data/humans", {
    path in r"^(.*)\.yml",
})]
fn humans(path: &Path) {
    setup();
    let input = read_to_string(path).unwrap();
    let mut test_case = parse_yaml_test(&input).unwrap();
    if let Some(res) = test_case.execute() {
        assert_eq!(PrettyString(&res), PrettyString(&test_case.result));
    }
}
