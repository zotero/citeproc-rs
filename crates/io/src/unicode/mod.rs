#[allow(clippy::all)]
#[rustfmt::skip]
pub(crate) mod sup_sub;

#[allow(clippy::all)]
#[rustfmt::skip]
#[allow(dead_code)]
mod latin_cyrillic;
use self::latin_cyrillic::{ARABIC, COMMON, CYRILLIC, GREEK, LATIN};

pub fn char_is_latin_cyrillic(c: char) -> bool {
    LATIN.contains_char(c)
        || COMMON.contains_char(c)
        || CYRILLIC.contains_char(c)
        || GREEK.contains_char(c)
        // bugreports_ArabicLocale.txt -- apparently Arabic should be on this list.
        // Citeproc-js uses
        // STARTSWITH_ROMANESQUE_REGEXP:
        // /^[&a-zA-Z\u0e01-\u0e5b\u00c0-\u017f\u0370-\u03ff\u0400-\u052f\u0590-\u05d4\u05d6-\u05ff
        //    ... \u1f00-\u1fff\u0600-\u06ff\u200c\u200d\u200e\u0218\u0219\u021a\u021b\u202a-\u202e]/,
        // which tests true for the Arabic locale's "et-al" term (tested below)
        || ARABIC.contains_char(c)
}

pub fn is_latin_cyrillic(s: &str) -> bool {
    s.chars().all(|c| char_is_latin_cyrillic(c))
}

#[test]
fn test_is_latin_cyrillic() {
    assert!(is_latin_cyrillic(" @")); // Common only
    assert!(is_latin_cyrillic("ÀÖ hello world")); // Latin only
    assert!(is_latin_cyrillic("'҇ԯ")); // Cyrillic only
    assert!(is_latin_cyrillic("ÀÖ '҇ԯ@ ")); // Latin and Cyrillic and Common

    // Extras that should be latin
    assert!(is_latin_cyrillic("ἀἕἘ")); // Greek
    assert!(is_latin_cyrillic("Άγρας")); // Greek
    assert!(is_latin_cyrillic("وآخرون")); // Arabic

    // Non-Latin
    assert!(!is_latin_cyrillic("⺙⺛⻳")); // Han
    assert!(!is_latin_cyrillic("⺙.⺛⻳")); // Han with common
    assert!(!is_latin_cyrillic("휴전 상태를 유지해야 한다")); // Hangeul with common
}
