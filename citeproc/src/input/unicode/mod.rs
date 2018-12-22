#[allow(dead_code)]
mod script;
use self::script::{COMMON, CYRILLIC, LATIN};

pub fn is_latin_cyrillic(s: &str) -> bool {
    s.chars()
        .all(|c| LATIN.contains_char(c) || CYRILLIC.contains_char(c) || COMMON.contains_char(c))
}

#[test]
fn test_is_latin_cyrillic() {
    assert!(is_latin_cyrillic(" @")); // Common only
    assert!(is_latin_cyrillic("ÀÖ hello world")); // Latin only
    assert!(is_latin_cyrillic("'҇ԯ")); // Cyrillic only
    assert!(is_latin_cyrillic("ÀÖ '҇ԯ@ ")); // Latin and Cyrillic and Common
    assert!(!is_latin_cyrillic("ἀἕἘ")); // Greek
    assert!(!is_latin_cyrillic("⺙⺛⻳")); // Han
    assert!(!is_latin_cyrillic("⺙.⺛⻳")); // Han with common
}
