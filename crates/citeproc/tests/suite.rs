// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

#![feature(custom_test_frameworks)]
#![test_runner(datatest::runner)]

use cfg_if::cfg_if;
cfg_if! {
    if #[cfg(all(feature="test-allocator-jemalloc", feature="jemallocator"))] {
        use jemallocator::Jemalloc;
        #[global_allocator]
        static A: Jemalloc = Jemalloc;
    } else if #[cfg(all(feature="test-allocator-dlmalloc", feature="dlmalloc"))] {
        #[global_allocator]
        static A: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;
    } else {
        use std::alloc::System;
        #[global_allocator]
        static A: System = System;
    }
}

mod test_format;
use test_format::{humans::parse_human_test, yaml::parse_yaml_test};

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

fn ignore_file(file: &str) -> HashSet<String> {
    let mut m = HashSet::new();
    for name in file.lines() {
        // Comments
        if name.trim_start().starts_with("#") || name.trim().is_empty() {
            continue;
        }
        m.insert(name.to_string());
    }
    m
}

lazy_static! {
    static ref IGNORES: HashSet<String> = {
        // cargo test -- 2>/dev/null | rg 'bib tests' |  rg suite | cut -d' ' -f2 | cut -d: -f3 | cut -d\' -f1 > bibtests.txt
        let ignores = read_to_string("./tests/data/ignore.txt").unwrap();
        // let ignores = include_str!("./data/ignore.txt");
        ignore_file(&ignores)
    };
}

lazy_static! {
    static ref SNAPSHOTS: HashSet<String> = {
        let snapshot = include_str!("./data/snapshot.txt");
        ignore_file(snapshot)
    };
}

fn is_ignore(path: &Path) -> bool {
    let fname = path.file_name().unwrap().to_string_lossy();
    IGNORES.contains(&fname.into_owned())
}

fn is_snapshot(path: &Path) -> bool {
    let fname = path.file_name().unwrap().to_string_lossy();
    SNAPSHOTS.contains(&fname.into_owned())
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
    let mut test_case = parse_human_test(&input, None);
    if let Some(res) = test_case.execute() {
        let pass = res == test_case.result;
        if !pass && is_snapshot(path) {
            let name = path.file_name().unwrap().to_string_lossy();
            insta::assert_snapshot!(name.as_ref(), res);
        } else {
            assert_eq!(PrettyString(&res), PrettyString(&test_case.result));
        }
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

#[datatest::files("tests/data/fixtures-local", {
    path in r"^(.*)\.txt" if !is_ignore,
})]
fn fixtures_local(path: &Path) {
    setup();
    let input = read_to_string(path).unwrap();
    let mut test_case = parse_human_test(
        &input,
        Some(csl::Features {
            custom_intext: true,
            ..Default::default()
        }),
    );
    if let Some(res) = test_case.execute() {
        let pass = res == test_case.result;
        if !pass && is_snapshot(path) {
            let name = path.file_name().unwrap().to_string_lossy();
            insta::assert_snapshot!(name.as_ref(), res);
        } else {
            assert_eq!(PrettyString(&res), PrettyString(&test_case.result));
        }
    }
}
