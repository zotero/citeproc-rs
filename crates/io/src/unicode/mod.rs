#[allow(clippy::all)]
#[rustfmt::skip]
pub(crate) mod sup_sub;

#[allow(clippy::all)]
#[rustfmt::skip]
#[allow(dead_code)]
mod latin_cyrillic;
use self::latin_cyrillic::{COMMON, CYRILLIC, GREEK, LATIN};

pub fn is_latin_cyrillic(s: &str) -> bool {
    s.chars().all(|c| {
        LATIN.contains_char(c)
            || COMMON.contains_char(c)
            || CYRILLIC.contains_char(c)
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
