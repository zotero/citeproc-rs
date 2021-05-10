use serde_json::Value;
use std::sync::Once;
mod var {
    pub use csl::DateVariable::Issued;
    pub use csl::Variable::{Title, TitleShort};
}
use var::*;
use citeproc_io::*;

static INIT: Once = Once::new();
fn setup() {
    INIT.call_once(|| env_logger::init());
}

macro_rules! test_parse {
    ($name:ident, $input:expr, $check:expr) => {
        #[allow(non_snake_case)]
        #[test]
        fn $name () {
            use ::citeproc_io::Reference;
            setup();
            let input = $input;
            let check = $check;
            let refr: Reference = ::serde_json::from_str(input).expect("test value did not parse as a Reference");
            let _: () = check(refr);
        }
    };
}

macro_rules! test_equiv {
    ($name:ident, $input:expr => $expected:expr) => {
        #[allow(non_snake_case)]
        #[test]
        fn $name () {
            use ::citeproc_io::Reference;
            setup();
            let input = $input;
            let expected = $expected;
            let refr: Reference = ::serde_json::from_str(input).expect("test value did not parse as a Reference");
            let exp: Reference = ::serde_json::from_str(expected).expect("expected value did not parse as a Reference");
            assert_eq!(refr, exp);
        }
    };
}
macro_rules! test_equiv_all {
    ($name:ident, $input:expr) => {
        #[allow(non_snake_case)]
        #[test]
        fn $name () {
            use ::citeproc_io::Reference;
            setup();
            let input = $input;
            let references: Vec<Reference> = serde_json::from_str(input).expect("test case did not parse as a Vec<Reference>");
            let mut iter = references.into_iter();
            let first = iter.next().unwrap();
            assert!(iter.all(|r| r == first));
        }
    };
}


macro_rules! assert_key {
    ($hashmap:expr, $a:expr, $b:expr) => {
        let h = $hashmap; let a = $a; let b = $b;
        let aval = h.get(&a);
        let bval = b.as_ref();
        assert_eq!(aval, bval);
    };
}

macro_rules! assert_key_deref {
    ($hashmap:expr, $a:expr, $b:expr) => {
        use core::ops::Deref;
        let h = $hashmap; let a = $a; let b = $b;
        let aval = h.get(&a).map(|x| x.deref());
        let bval = b.as_deref();
        assert_eq!(aval, bval);
    };
}

const EMPTY: &'static str = r#"{"id": 1}"#;

test_parse!(year_keyed_dates, r#" { "id": 1, "issued": { "year": 2000 } } "#, |r: Reference| {
    assert_key!(r.date, Issued, Some(DateOrRange::Single(Date::new(2000, 0, 0))));
});
test_parse!(parse_neg_year, r#" { "id": 1, "issued": { "year": -1000 } } "#, |r: Reference| {
    assert_key!(r.date, Issued, Some(DateOrRange::Single(Date::new(-1000, 0, 0))));
});

test_parse!(title_short, r#" { "id": 1, "title-short": "title" } "#, |r: Reference| {
    assert_key_deref!(r.ordinary, TitleShort, Some("title"));
});
test_equiv_all!(title_short_shortTitle, r#"[
    { "id": 1, "title-short": "title" },
    { "id": 1, "shortTitle": "title" }
]"#);

test_equiv_all!(url, r#"[
    { "id": 1, "URL": "https://example.com" },
    { "id": 1, "url": "https://example.com" }
]"#);

test_equiv!(ignore_unknown_keys, r#" { "id": 1, "will_never_be_added_to_csl_unknown": "title" } "# => EMPTY);
test_equiv!(ignore_unknown_weird_keys, r#" { "id": 1, "with\"quote": "title" } "# => EMPTY);
test_equiv!(ignore_unknown_keys_weird_data, r#" { "id": 1, "asdklfjhhjkl": { "completely": "unrecognizable" } } "# => EMPTY);
test_equiv!(ignore_unknown_weird_keys_weird_data, r#" { "id": 1, "\"\"\"": { "completely": -0.9999 } } "# => EMPTY);

