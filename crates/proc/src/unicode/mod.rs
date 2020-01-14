// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

#[allow(dead_code)]
#[allow(clippy::all)]
#[rustfmt::skip]
mod script;
use self::script::{COMMON, CYRILLIC, GREEK, LATIN};

pub fn is_latin_cyrillic(s: &str) -> bool {
    s.chars().all(|c| {
        LATIN.contains_char(c)
            || CYRILLIC.contains_char(c)
            || COMMON.contains_char(c)
            || GREEK.contains_char(c)
    })
}

#[test]
fn test_is_latin_cyrillic() {
    assert!(is_latin_cyrillic(" @")); // Common only
    assert!(is_latin_cyrillic("ÀÖ hello world")); // Latin only
    assert!(is_latin_cyrillic("'҇ԯ")); // Cyrillic only
    assert!(is_latin_cyrillic("ÀÖ '҇ԯ@ ")); // Latin and Cyrillic and Common
    assert!(!is_latin_cyrillic("⺙⺛⻳")); // Han
    assert!(!is_latin_cyrillic("⺙.⺛⻳")); // Han with common

    // Extras
    assert!(is_latin_cyrillic("ἀἕἘ")); // Greek
    assert!(is_latin_cyrillic("Άγρας")) // Greek
}
