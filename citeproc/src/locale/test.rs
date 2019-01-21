use std::collections::HashMap;
use std::str::FromStr;

use super::fetcher::Predefined;
use super::*;
use crate::db_impl::RootDatabase;
use crate::locale::db::LocaleDatabase;

fn terms(xml: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
        <locale xmlns="http://purl.org/net/xbiblio/csl" version="1.0" xml:lang="en-US">
        <terms>{}</terms></locale>"#,
        xml
    )
}

fn predefined_xml(pairs: &[(Lang, &str)]) -> Predefined {
    let mut map = HashMap::new();
    for (lang, ts) in pairs {
        map.insert(lang.clone(), terms(ts));
    }
    Predefined(map)
}

fn term_and(form: TermFormExtended) -> SimpleTermSelector {
    SimpleTermSelector::Misc(MiscTerm::And, form)
}

fn test_simple_term(term: SimpleTermSelector, langs: &[(Lang, &str)], expect: Option<&str>) {
    let db = RootDatabase::new(Box::new(predefined_xml(langs)));
    // use en-AU so it has to do fallback to en-US
    let locale = db.merged_locale(Lang::en_au());
    assert_eq!(
        locale.get_text_term(TextTermSelector::Simple(term), false),
        expect
    )
}

#[test]
fn term_override() {
    test_simple_term(
        term_and(TermFormExtended::Long),
        &[
            (Lang::en_us(), r#"<term name="and">USA</term>"#),
            (Lang::en_au(), r#"<term name="and">Australia</term>"#),
        ],
        Some("Australia"),
    )
}

#[test]
fn term_form_refine() {
    test_simple_term(
        term_and(TermFormExtended::Long),
        &[
            (Lang::en_us(), r#"<term name="and">USA</term>"#),
            (
                Lang::en_au(),
                r#"<term name="and" form="short">Australia</term>"#,
            ),
        ],
        Some("USA"),
    );
    test_simple_term(
        term_and(TermFormExtended::Short),
        &[
            (Lang::en_us(), r#"<term name="and">USA</term>"#),
            (
                Lang::en_au(),
                r#"<term name="and" form="short">Australia</term>"#,
            ),
        ],
        Some("Australia"),
    );
}

#[test]
fn term_form_fallback() {
    test_simple_term(
        term_and(TermFormExtended::Short),
        &[
            (Lang::en_us(), r#"<term name="and">USA</term>"#),
            (Lang::en_au(), r#"<term name="and">Australia</term>"#),
        ],
        Some("Australia"),
    );
    test_simple_term(
        // short falls back to long and skips the "symbol" in a later locale
        term_and(TermFormExtended::Short),
        &[
            (Lang::en_us(), r#"<term name="and">USA</term>"#),
            (
                Lang::en_au(),
                r#"<term name="and" form="symbol">Australia</term>"#,
            ),
        ],
        Some("USA"),
    );
    test_simple_term(
        term_and(TermFormExtended::VerbShort),
        &[
            (Lang::en_us(), r#"<term name="and" form="long">USA</term>"#),
            (
                Lang::en_au(),
                r#"<term name="and" form="symbol">Australia</term>"#,
            ),
        ],
        Some("USA"),
    );
}

#[test]
fn term_locale_fallback() {
    test_simple_term(
        term_and(TermFormExtended::Long),
        &[
            (Lang::en_us(), r#"<term name="and">USA</term>"#),
            (Lang::en_au(), r#""#),
        ],
        Some("USA"),
    )
}

#[test]
fn lang_from_str() {
    let de_at = Lang::Iso(IsoLang::Deutsch, Some(IsoCountry::AT));
    let de = Lang::Iso(IsoLang::Deutsch, None);
    let iana = Lang::Iana("Navajo".to_string());
    let unofficial = Lang::Unofficial("Newspeak".to_string());
    assert_eq!(Lang::from_str("de-AT"), Ok(de_at));
    assert_eq!(Lang::from_str("de"), Ok(de));
    assert_eq!(Lang::from_str("i-Navajo"), Ok(iana));
    assert_eq!(Lang::from_str("x-Newspeak"), Ok(unofficial));
}
