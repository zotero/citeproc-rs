use once_cell::sync::OnceCell;
use serde_json::Value;
use std::collections::HashSet;
use std::str::FromStr;
use std::{
    collections::{BTreeSet, HashMap},
    sync::Once,
};

mod csl_data;
use csl_data::CslDataSchema;
pub(self) mod schema;
use schema::{Primitive, SchemaNode};
use serde_json::json;
mod var {
    pub use csl::DateVariable::Issued;
    pub use csl::Variable::{Title, TitleShort};
    pub use csl::{DateVariable, NameVariable, NumberVariable, Variable};
}

use citeproc_io::*;
use var::*;

static INIT: Once = Once::new();
fn setup() {
    INIT.call_once(|| {
        use log::LevelFilter;
        let _ = env_logger::builder()
            .is_test(true)
            .filter_module("citeproc_io", LevelFilter::Debug)
            .filter_module("integration::json", LevelFilter::Debug)
            .try_init();
    });
}

const CSL_DATA_SCHEMA: &str = include_str!("./csl-data.json");

static SCHEMA: OnceCell<CslDataSchema> = OnceCell::new();
fn get_schema() -> &'static CslDataSchema {
    SCHEMA.get_or_init(|| {
        let jd = &mut serde_json::Deserializer::from_str(CSL_DATA_SCHEMA);
        let result: Result<CslDataSchema, _> = serde_path_to_error::deserialize(jd);
        result.expect("could not parse CslDataSchema")
    })
}

/// Test that this JSON parses as a Reference, and run check function on it.
/// Also checks internally that parsing the string to a serde_json::Value first produces an
/// equivalent result.
macro_rules! test_parse {
    ($name:ident, $input:expr, $check:expr) => {
        #[allow(non_snake_case)]
        #[test]
        fn $name() {
            use ::citeproc_io::Reference;
            setup();
            let input = $input;
            let check = $check;
            let value_dupe: Value =
                ::serde_json::from_str(input).expect("did not parse as serde_json::Value");
            let refr: Reference = ::serde_json::from_str(input)
                .expect("test value did not parse &str as a Reference");
            let val_refr: Reference = ::serde_json::from_value(value_dupe)
                .expect("did not parse Reference from serde_json::Value");
            assert_eq!(refr, val_refr);
            let _: () = check(refr);
        }
    };
}

macro_rules! test_equiv {
    ($name:ident, $input:expr => $expected:expr) => {
        #[allow(non_snake_case)]
        #[test]
        fn $name() {
            use ::citeproc_io::Reference;
            setup();
            let input = $input;
            let expected = $expected;
            let refr: Reference =
                ::serde_json::from_str(input).expect("test value did not parse as a Reference");
            let exp: Reference = ::serde_json::from_str(expected)
                .expect("expected value did not parse as a Reference");
            assert_eq!(refr, exp);
        }
    };
}
macro_rules! test_equiv_all {
    ($name:ident, $input:expr) => {
        #[allow(non_snake_case)]
        #[test]
        fn $name() {
            use ::citeproc_io::Reference;
            setup();
            let input = $input;
            let references: Vec<Reference> =
                serde_json::from_str(input).expect("test case did not parse as a Vec<Reference>");
            let mut iter = references.into_iter();
            let first = iter.next().unwrap();
            assert!(iter.all(|r| r == first));
        }
    };
}

macro_rules! assert_key {
    ($hashmap:expr, $a:expr, $b:expr) => {
        let h = $hashmap;
        let a = $a;
        let b = $b;
        let aval = h.get(&a);
        let bval = b.as_ref();
        assert_eq!(aval, bval);
    };
}

macro_rules! assert_key_deref {
    ($hashmap:expr, $a:expr, $b:expr) => {
        use core::ops::Deref;
        let h = $hashmap;
        let a = $a;
        let b = $b;
        let aval = h.get(&a).map(|x| x.deref());
        let bval = b.as_deref();
        assert_eq!(aval, bval);
    };
}

const EMPTY: &'static str = r#"{"id": 1}"#;

// https://github.com/zotero/citeproc-rs/issues/99
test_parse!(
    year_keyed_dates,
    r#" { "id": 1, "issued": { "year": 2000 } } "#,
    |r: Reference| {
        assert_key!(
            r.date,
            Issued,
            Some(DateOrRange::Single(Date::new(2000, 0, 0)))
        );
    }
);
test_parse!(
    unknown_keyed_dates,
    r#" { "id": 1, "issued": { "aslkdjfhaslkdjhflkasjhdf": 2000 } } "#,
    |r: Reference| {
        assert_key!(r.date, Issued, None);
    }
);
test_parse!(
    parse_neg_year,
    r#" { "id": 1, "issued": { "year": -1000 } } "#,
    |r: Reference| {
        assert_key!(
            r.date,
            Issued,
            Some(DateOrRange::Single(Date::new(-1000, 0, 0)))
        );
    }
);

test_parse!(
    title_short,
    r#" { "id": 1, "title-short": "title" } "#,
    |r: Reference| {
        assert_key_deref!(r.ordinary, TitleShort, Some("title"));
    }
);
test_equiv_all!(
    title_short_shortTitle,
    r#"[
    { "id": 1, "title-short": "title" },
    { "id": 1, "shortTitle": "title" }
]"#
);

test_equiv_all!(
    url,
    r#"[
    { "id": 1, "URL": "https://example.com" },
    { "id": 1, "url": "https://example.com" }
]"#
);

test_equiv!(ignore_unknown_keys, r#" { "id": 1, "will_never_be_added_to_csl_unknown": "title" } "# => EMPTY);
test_equiv!(ignore_unknown_weird_keys, r#" { "id": 1, "with\"quote": "title" } "# => EMPTY);
test_equiv!(ignore_unknown_keys_weird_data, r#" { "id": 1, "asdklfjhhjkl": { "completely": "unrecognizable" } } "# => EMPTY);
test_equiv!(ignore_unknown_weird_keys_weird_data, r#" { "id": 1, "\"\"\"": { "completely": -0.9999 } } "# => EMPTY);

test_parse!(
    duplicate_keys_ok,
    r#" { "id": 1, "title": "first", "title": "second"} "#,
    |r: Reference| {
        // It parsed. that's already a win. May as well also check it has the title.
        assert!(r.ordinary.contains_key(&Title));
        // You generally shouldn't make any assertions about which one will win, at least not in a
        // spec. That's just undefined JSON behaviour.
        // But we want to match what happens when you parse it as a serde_json::Value first.
        assert_key_deref!(r.ordinary, Title, Some("second"));
    }
);

// From the spec

fn parse_errors<'a, T: FromStr>(varnames: &[&'a str]) -> Vec<&'a str> {
    varnames
        .iter()
        .map(|s| (s, s.parse::<T>()))
        .filter_map(|(s, r)| r.err().map(|_x| *s))
        .collect::<Vec<&str>>()
}

#[test]
fn parse_schema_date_variables() {
    setup();
    let vars = get_schema().date_variables();
    let errors = parse_errors::<DateVariable>(&vars);
    assert_eq!(&errors, &[] as &[&str]);
}

#[test]
fn parse_schema_name_variables() {
    setup();
    let vars = get_schema().name_variables();
    let errors = parse_errors::<NameVariable>(&vars);
    assert_eq!(&errors, &[] as &[&str]);
}

#[test]
fn parse_schema_ordinary_variables() {
    setup();
    let vars = get_schema().string_variables();
    let errors = parse_errors::<Variable>(&vars);
    assert_eq!(&errors, &[] as &[&str]);
}

#[test]
fn parse_schema_number_variables() {
    setup();
    let vars = get_schema().number_variables();
    let errors = parse_errors::<NumberVariable>(&vars);
    assert_eq!(&errors, &[] as &[&str]);
}

#[test]
fn variable_test_exhaustive() {
    setup();
    let schema = get_schema();
    let all = schema.all_variables();

    let mut covered = schema.string_variables();
    covered.append(&mut schema.date_variables());
    covered.append(&mut schema.name_variables());
    covered.append(&mut schema.number_variables());

    let covered = covered.into_iter().collect::<HashSet<&str>>();
    let ignored: HashSet<&str> = CslDataSchema::IGNORED_VARIABLES.iter().cloned().collect();
    let without_covered = all.difference(&covered).cloned().collect::<HashSet<&str>>();
    let diff: HashSet<_> = without_covered.difference(&ignored).collect();

    assert_eq!(diff, HashSet::default());
}

#[test]
fn each_date() {
    setup();
    type Map = serde_json::Map<String, Value>;
    type Generator<'a> = (&'a str, Option<Primitive>, &'a dyn Fn() -> Value);
    let generators: &[Generator<'static>] = &[
        // citeproc-rs only handles 1,2,3,4,"1","2","3","4".
        // but for the rest, at least it has to silently ignore them.
        ("season", Some(Primitive::String), &|| json!("")),
        ("season", Some(Primitive::String), &|| json!("1")),
        ("season", Some(Primitive::String), &|| json!("summer")),
        ("season", Some(Primitive::String), &|| json!("other")),
        // this one appears in the csl test suite, for whatever reason someone parsed a time into
        // the season field.
        ("season", Some(Primitive::String), &|| json!("22:38:19")),
        ("season", Some(Primitive::Number), &|| json!(1)),
        ("season", Some(Primitive::Number), &|| json!(21)),
        ("season", Some(Primitive::Number), &|| json!(1234)),
        // we are only interested in establishing JavaScript truthiness here.
        ("circa", Some(Primitive::String), &|| json!("")),
        ("circa", Some(Primitive::String), &|| json!("1995")),
        ("circa", Some(Primitive::String), &|| json!("non_numeric")),
        ("circa", Some(Primitive::Number), &|| json!(0)),
        ("circa", Some(Primitive::Number), &|| json!(1234)),
        ("circa", Some(Primitive::Number), &|| {
            json!(2u64.pow(32) + 5)
        }),
        ("circa", Some(Primitive::Number), &|| json!(-1i32)),
        ("circa", Some(Primitive::Number), &|| {
            json!(0 - 2i64.pow(32) - 5)
        }),
        ("circa", Some(Primitive::Boolean), &|| json!(true)),
        ("circa", Some(Primitive::Boolean), &|| json!(false)),
        ("literal", Some(Primitive::String), &|| json!("")),
        ("literal", Some(Primitive::Boolean), &|| json!("1995-08-07")),
        ("literal", Some(Primitive::Boolean), &|| {
            json!("the 21st of September, 1991")
        }),
        ("literal", Some(Primitive::Boolean), &|| {
            json!("-1888lit^%(&@(*&")
        }),
        // we only support some of the raw format, and we test it  more comprehensively over at DateOrRange::from_str
        ("raw", Some(Primitive::String), &|| json!("")),
        ("raw", Some(Primitive::String), &|| json!("1995-08")),
        ("raw", Some(Primitive::String), &|| json!("1995-08-07")),
        ("raw", Some(Primitive::String), &|| {
            json!("1995-08-07/1996-09-15")
        }),
        // the un-specced year.
        ("year", Some(Primitive::String), &|| json!(1995)),
        ("year", Some(Primitive::String), &|| json!(-1)),
        ("year", Some(Primitive::String), &|| json!(2i64.pow(32) + 5)),
        ("year", Some(Primitive::String), &|| json!("")),
        ("year", Some(Primitive::String), &|| json!("1995")),
        ("year", Some(Primitive::String), &|| json!("1995-08-08")), // invalid, ignore
        // single
        ("date-parts", None, &|| json!([[1995, 08, 07]])),
        // too many parts
        ("date-parts", None, &|| json!([[1995, 08, 07, 1234, 12345]])),
        ("date-parts", None, &|| json!([[1995, 08, "07"]])),
        ("date-parts", None, &|| json!([[1995, 08]])),
        ("date-parts", None, &|| json!([[1995]])),
        ("date-parts", None, &|| json!([["1995", "08", "07"]])),
        ("date-parts", None, &|| json!([["1995", "08"]])),
        ("date-parts", None, &|| json!([["1995"]])),
        // json! - invalid but ignore
        ("date-parts", None, &|| json!([["1995", "12345"]])),
        ("date-parts", None, &|| json!([["1995", 12345678]])),
        // json! - integer overflow
        ("date-parts", None, &|| json!([["1995", 2u64.pow(32) + 5]])),
        ("date-parts", None, &|| json!([["1995", -1i32]])),
        ("date-parts", None, &|| {
            json!([["1995", 0 - 2i64.pow(32) - 5]])
        }),
        // json! mixed
        ("date-parts", None, &|| json!([["1995", 8, 7]])),
        ("date-parts", None, &|| json!([["1995", 8]])),
        // range
        ("date-parts", None, &|| json!([[1995, 8, 9], [2000]])),
        ("date-parts", None, &|| json!([[1995], [2000, 10, 5]])),
    ];
    let get_gens_for = |s: &str, p: Option<Primitive>| {
        let s = s.to_owned();
        generators
            .iter()
            .filter(move |x| x.0 == s && x.1 == p)
            .map(|x| (x.2)())
    };

    let properties = get_schema().date_schemas_properties();
    let keys = properties.keys().map(String::as_str).collect::<Vec<_>>();

    // first test each of the keys on their own.
    let mut map = Map::new();
    let mut buffer = vec![0u8; 0];
    for &key in keys.iter() {
        let gens: Box<dyn Iterator<Item = Value>> = match &properties[key] {
            SchemaNode::String { .. } => Box::new(get_gens_for(key, Some(Primitive::String))),
            SchemaNode::Multi { types, .. } => {
                Box::new(types.iter().flat_map(|&prim| get_gens_for(key, Some(prim))))
            }
            SchemaNode::Ref { pointer, .. } if pointer == CslDataSchema::EDTF_DATATYPE => {
                Box::new(get_gens_for(key, None))
            }
            SchemaNode::Array { .. } if key == "date-parts" => Box::new(get_gens_for(key, None)),
            _ => todo!(),
        };
        map.clear();
        buffer.clear();
        for value in gens {
            map.insert(key.into(), value);
            buffer.clear();
            buffer.extend_from_slice(br#"{ "id": 1, "issued": "#);
            serde_json::to_writer(&mut buffer, &map).unwrap();
            buffer.extend_from_slice(b" }");
            let utf8 = std::str::from_utf8(&buffer[..]).unwrap();
            log::info!("testing: {}", utf8);
            let _parsed: Reference = serde_json::from_reader(&buffer[..])
                .unwrap_or_else(|e| panic!("did not parse {} -> {}", utf8, e));
        }
    }
}

#[test]
fn test_date_combos() {
    setup();
    use itertools::Itertools;
    type Map = serde_json::Map<String, Value>;
    let properties = get_schema().date_schemas_properties();
    let keys = properties.keys().map(String::as_str).collect::<Vec<_>>();

    let literal = "January 17, 2019";
    let raw = "1970";
    let date_parts = &[1995, 8, 1];
    let date_parts_exact = DateOrRange::Single(Date::from_parts(date_parts).unwrap());
    let edtf = "2020-01";
    let useful_values: &[(&str, Value)] = &[
        ("literal", json!(literal)),
        ("raw", json!(raw)),
        ("season", json!(1)),
        ("circa", json!(true)),
        ("date-parts", json!([date_parts])),
        ("year", json!(1967)),
        ("edtf", json!(edtf)),
    ];
    fn mk_set<'a>(strings: &[&'a str]) -> BTreeSet<&'a str> {
        strings.iter().cloned().collect()
    }

    // none represents not being parsed as a date at all, flat out ignored
    // we won't test the interaction of year/literal/raw/date-parts/edtf, because that's just
    // last-write-wins.
    let mut tests: HashMap<BTreeSet<&'static str>, Option<DateOrRange>> = HashMap::new();
    tests.insert(
        mk_set(&["literal", "raw"]),
        Some(DateOrRange::Literal {
            literal: literal.into(),
            circa: false,
        }),
    );
    // raw is only 1970, so we get 1970 + season-01
    tests.insert(
        mk_set(&["raw", "season"]),
        Some(Date::new(1970, 13, 0).into()),
    );
    // date-parts is a full date. what do we do here? I suppose if there's a season, just ignore it.
    tests.insert(
        mk_set(&["date-parts", "season"]),
        Some(date_parts_exact.clone()),
    );
    tests.insert(
        mk_set(&["date-parts", "circa", "season"]),
        Some(date_parts_exact.clone().with_circa(true)),
    );
    tests.insert(
        mk_set(&["date-parts", "circa", "season"]),
        Some(date_parts_exact.clone().with_circa(true)),
    );
    tests.insert(
        mk_set(&["raw", "circa", "season"]),
        Some(DateOrRange::from(Date::new(1970, 13, 0)).with_circa(true)),
    );
    tests.insert(
        mk_set(&["year", "circa", "season"]),
        Some(DateOrRange::from(Date::new(1967, 13, 0)).with_circa(true)),
    );

    let get_useful = |s: &str| -> Value {
        useful_values
            .iter()
            .find(|x| x.0 == s)
            .map(|x| x.1.clone())
            .unwrap_or_else(|| panic!("didn't know what to do with date key {:?}", s))
    };

    // first, all combos of keys. No replacement, keys being specified twice is fine, we just use
    // the later one. (But should test separately.)
    let mut buffer = vec![0u8; 0];
    let mut map = Map::new();
    for i in 0..keys.len() {
        for combo in Itertools::combinations(keys.iter().cloned(), i) {
            map.clear();
            buffer.clear();
            buffer.extend_from_slice(br#"{ "id": 1, "issued": "#);
            log::info!("testing combo: {:?}", combo);
            for &key in &combo {
                map.insert(key.into(), get_useful(key));
            }

            serde_json::to_writer(&mut buffer, &map).unwrap();
            buffer.extend_from_slice(b" }");
            let utf8 = std::str::from_utf8(&buffer[..]).unwrap();
            log::info!("combo got: {}", utf8);
            let parsed: Reference = serde_json::from_reader(&buffer[..])
                .unwrap_or_else(|e| panic!("did not parse {} -> {}", utf8, e));

            let keys = mk_set(&combo);
            if let Some(test) = tests.get(&keys) {
                assert_eq!(parsed.date.get(&Issued), test.as_ref())
            }
        }
    }
}

#[test]
#[ignore = "EDTF dates not implemented yet"]
fn test_edtf_date() {
    setup();
    let _edtf_schema = get_schema().named_schema(CslDataSchema::EDTF_DATATYPE);
    let doc = json!({ "id": 1, "issued": "199X" });
    let _refr: Reference = serde_json::from_value(doc).unwrap();
}
