use std::fmt;

use crate::style::element::LocaleDate;
use crate::style::terms::*;

mod fetcher;
pub use self::fetcher::{Filesystem, LocaleFetcher};
use fnv::FnvHashMap;
use std::collections::HashSet;

use crate::style::Style;
use salsa::Database;
use std::sync::Arc;

use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CslOption(String, String);

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Locale {
    pub version: String,
    pub lang: Option<Lang>,
    pub options: Vec<CslOption>,
    pub simple_terms: SimpleMapping,
    pub gendered_terms: GenderedMapping,
    pub ordinal_terms: OrdinalMapping,
    pub role_terms: RoleMapping,
    pub dates: Vec<LocaleDate>,
}

impl Locale {
    pub fn get_text_term<'l>(&'l self, sel: &TextTermSelector, plural: bool) -> Option<&'l str> {
        use crate::style::terms::TextTermSelector::*;
        match *sel {
            Simple(ref ts) => ts.fallback()
                .filter_map(|sel| self.simple_terms.get(&sel))
                .next()
                .and_then(|r| r.get(plural)),
            Gendered(ref ts) => ts.fallback()
                .filter_map(|sel| self.gendered_terms.get(&sel))
                .next()
                .and_then(|r| r.0.get(plural)),
            Role(ref ts) => ts.fallback()
                .filter_map(|sel| self.role_terms.get(&sel))
                .next()
                .and_then(|r| r.get(plural)),
        }
    }

    fn merge(&mut self, with: &Self) {
        fn extend<K: Clone + Eq + std::hash::Hash, V: Clone>(
            map: &mut FnvHashMap<K, V>,
            other: &FnvHashMap<K, V>,
        ) {
            map.extend(other.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        self.lang = with.lang.clone();
        extend(&mut self.simple_terms, &with.simple_terms);
        extend(&mut self.gendered_terms, &with.gendered_terms);
        extend(&mut self.role_terms, &with.role_terms);
        // replace the whole ordinals configuration
        self.ordinal_terms = with.ordinal_terms.clone();
        self.dates = with.dates.clone();
    }
}

#[salsa::query_group]
trait StyleDatabase: salsa::Database {
    #[salsa::input]
    fn style_text(&self, key: ()) -> Arc<String>;
    #[salsa::input]
    fn style(&self, key: ()) -> Arc<Style>;
    fn inline_locale(&self, key: Option<Lang>) -> Option<Arc<Locale>>;
}

fn inline_locale(db: &impl StyleDatabase, key: Option<Lang>) -> Option<Arc<Locale>> {
    db.style(())
        .locale_overrides
        .get(&key)
        .cloned()
        .map(Arc::new)
}

#[salsa::query_group]
trait LocaleDatabase: salsa::Database + StyleDatabase + LocaleFetcher {
    fn locale_xml(&self, key: Lang) -> Option<Arc<String>>;
    fn locale(&self, key: LocaleSource) -> Option<Arc<Locale>>;
    fn merged_locale(&self, key: Lang) -> Arc<Locale>;
}

fn locale_xml(db: &impl LocaleDatabase, key: Lang) -> Option<Arc<String>> {
    // let fsf = Filesystem::new("/Users/cormac/git/locales");
    db.fetch_string(&key).ok().map(Arc::new)
}

fn locale(db: &impl LocaleDatabase, key: LocaleSource) -> Option<Arc<Locale>> {
    match key {
        LocaleSource::File(ref lang) => {
            let string = db.locale_xml(lang.clone());
            string.and_then(|s| Locale::from_str(&s).ok()).map(Arc::new)
        }
        LocaleSource::Inline(ref lang) => db.inline_locale(lang.clone()),
    }
}

fn merged_locale(db: &impl LocaleDatabase, key: Lang) -> Arc<Locale> {
    let locales: Vec<_> = key.iter().filter_map(|ls| db.locale(ls)).collect();
    if locales.len() >= 1 {
        // could fold, but we only need to clone the base
        let mut base = (*locales[locales.len() - 1]).clone();
        for nxt in locales.into_iter().rev().skip(1) {
            base.merge(&nxt);
        }
        Arc::new(base)
    } else {
        Arc::new(Locale::default())
    }
}

struct LocaleDbImpl {
    runtime: salsa::Runtime<LocaleDbImpl>,
    fetcher: Box<LocaleFetcher>
}

impl LocaleDbImpl {
    fn new(fetcher: Box<LocaleFetcher>) -> Self {
        let mut db = LocaleDbImpl {
            runtime: Default::default(),
            fetcher,
        };
        db.query_mut(StyleTextQuery).set((), Default::default());
        db.query_mut(StyleQuery).set((), Default::default());
        db
    }
}

impl LocaleFetcher for LocaleDbImpl {
    #[inline]
    fn fetch_string(&self, lang: &Lang) -> Result<String, std::io::Error> {
        self.fetcher.fetch_string(lang)
    }
}

/// This impl tells salsa where to find the salsa runtime.
impl salsa::Database for LocaleDbImpl {
    fn salsa_runtime(&self) -> &salsa::Runtime<LocaleDbImpl> {
        &self.runtime
    }
}

salsa::database_storage! {
    pub struct DatabaseImplStorage for LocaleDbImpl {
        impl StyleDatabase {
            fn style_text() for StyleTextQuery;
            fn style() for StyleQuery;
            fn inline_locale() for InlineLocaleQuery;
        }
        impl LocaleDatabase {
            fn locale_xml() for LocaleXmlQuery;
            fn locale() for LocaleQuery;
            fn merged_locale() for MergedLocaleQuery;
        }
    }
}

// #[allow(dead_code)]
// fn has_ordinals(ls: Vec<Locale>) -> bool {
//     ls.iter().any(|locale| {
//         locale.terms.iter().any(|term| term.name.contains("ordinal"))
//     })
// }

// #[allow(dead_code)]
// fn remove_ordinals() {}

// The 3-character codes are ISO 693-3.
#[derive(Debug, Clone, Eq, PartialEq, Hash, EnumString)]
pub enum IsoLang {
    #[strum(serialize = "en", serialize = "eng")]
    English,
    #[strum(serialize = "de", serialize = "deu")]
    Deutsch,
    #[strum(serialize = "pt", serialize = "por")]
    Portuguese,
    #[strum(serialize = "zh", serialize = "zho")]
    Chinese,
    #[strum(serialize = "fr", serialize = "fra")]
    French,
    #[strum(serialize = "es", serialize = "esp")]
    Spanish,
    #[strum(serialize = "ja", serialize = "jpn")]
    Japanese,
    #[strum(serialize = "ar", serialize = "ara")]
    Arabic,
    /// The rest are not part of the fallback relation, so just treat them as strings.
    ///
    /// Also we save allocations for some popular languages!
    #[strum(default = "true")]
    Other(String),
}

impl fmt::Display for IsoLang {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match *self {
            IsoLang::English => "en",
            IsoLang::Deutsch => "de",
            IsoLang::Portuguese => "pt",
            IsoLang::Spanish => "es",
            IsoLang::French => "fr",
            IsoLang::Chinese => "zh",
            IsoLang::Japanese => "ja",
            IsoLang::Arabic => "ar",
            IsoLang::Other(ref o) => &o,
        };
        write!(f, "{}", s)
    }
}

/// These countries are used to do dialect fallback. Countries not used in that can be represented
/// as `IsoCountry::Other`. If a country is in the list, you don't need to allocate to refer to it,
/// so there are some non-participating countries in the list simply because it's faster.
#[derive(Debug, Clone, Eq, PartialEq, Hash, EnumString)]
pub enum IsoCountry {
    /// United States
    US,
    /// Great Britain
    GB,
    /// Australia
    AU,
    /// Deutschland
    DE,
    /// Austria
    AT,
    /// Switzerland
    CH,
    /// China
    CN,
    /// Taiwan
    TW,
    /// Portugal
    PT,
    /// Brazil
    BR,
    /// Japan
    JP,
    /// Spain
    ES,
    /// France
    FR,
    /// Canada
    CA,
    #[strum(default = "true")]
    Other(String),
}

impl fmt::Display for IsoCountry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            IsoCountry::Other(ref o) => write!(f, "{}", o),
            _ => write!(f, "{:?}", self),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum LocaleSource {
    Inline(Option<Lang>),
    File(Lang),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Lang {
    Iso(IsoLang, Option<IsoCountry>),
    Iana(String),
    Unofficial(String),
}

impl Lang {
    pub fn en_us() -> Self {
        Lang::Iso(IsoLang::English, Some(IsoCountry::US))
    }
    pub fn en_au() -> Self {
        Lang::Iso(IsoLang::English, Some(IsoCountry::AU))
    }
    pub fn iter(&self) -> impl Iterator<Item = LocaleSource> {
        use std::iter::once;
        self.inline_iter()
            .map(Some)
            .chain(once(None))
            .map(LocaleSource::Inline)
            .chain(self.file_iter().map(LocaleSource::File))
    }
    fn file_iter(&self) -> FileIter {
        FileIter {
            current: Some(self.clone()),
        }
    }
    fn inline_iter(&self) -> InlineIter {
        InlineIter {
            current: Some(self.clone()),
        }
    }
    pub fn is_english(&self) -> bool {
        match self {
            Lang::Iso(IsoLang::English, _) => true,
            _ => false,
        }
    }
}

impl fmt::Display for Lang {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Lang::Iso(l, None) => write!(f, "{}", l),
            Lang::Iso(l, Some(c)) => write!(f, "{}-{}", l, c),
            Lang::Iana(u) => write!(f, "i-{}", u),
            Lang::Unofficial(u) => write!(f, "x-{}", u),
        }
    }
}

impl crate::style::get_attribute::GetAttribute for Lang {
    fn get_attr(
        s: &str,
        _: crate::style::version::CslVariant,
    ) -> Result<Self, crate::style::error::UnknownAttributeValue> {
        match Lang::from_str(s) {
            Ok(a) => Ok(a),
            Err(_) => Err(crate::style::error::UnknownAttributeValue::new(s)),
        }
    }
}

struct FileIter {
    current: Option<Lang>,
}

struct InlineIter {
    current: Option<Lang>,
}

use std::mem;

impl Iterator for FileIter {
    type Item = Lang;
    fn next(&mut self) -> Option<Lang> {
        use self::IsoCountry::*;
        use self::IsoLang::*;
        use self::Lang::*;
        let next = self.current.as_ref().and_then(|curr| match curr {
            // Technically speaking most countries' English dialects are closer to en-GB than en-US,
            // but predictably implementing the spec is more important.
            Iso(English, Some(co)) if *co != US => Some(Iso(English, Some(US))),
            Iso(English, Some(US)) => None,
            Iso(Deutsch, Some(co)) if *co != DE => Some(Iso(Deutsch, Some(DE))),
            Iso(French, Some(co)) if *co != FR => Some(Iso(French, Some(FR))),
            Iso(Portuguese, Some(co)) if *co != PT => Some(Iso(Portuguese, Some(PT))),
            Iso(Chinese, Some(TW)) => Some(Iso(Chinese, Some(CN))),
            _ => Some(Iso(English, Some(US))),
        });
        mem::replace(&mut self.current, next)
    }
}

impl Iterator for InlineIter {
    type Item = Lang;
    fn next(&mut self) -> Option<Lang> {
        use self::Lang::*;
        let next = self.current.as_ref().and_then(|curr| match curr {
            Iso(lang, Some(_)) => Some(Iso(lang.clone(), None)),
            _ => None,
        });
        mem::replace(&mut self.current, next)
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use super::fetcher::Predefined;
    use crate::style::terms::*;
    use std::collections::HashMap;

    fn terms(xml: &str) -> String {
        format!(r#"<?xml version="1.0" encoding="utf-8"?>
        <locale xmlns="http://purl.org/net/xbiblio/csl" version="1.0" xml:lang="en-US">
        <terms>{}</terms></locale>"#, xml)
    }

    fn predef_terms(pairs: &[(Lang, &str)]) -> Predefined {
        let mut map = HashMap::new();
        for (lang, ts) in pairs {
            map.insert(lang.clone(), terms(ts));
        }
        Predefined(map)
    }

    fn term_and(form: TermFormExtended) -> SimpleTermSelector {
        SimpleTermSelector::Misc(
            MiscTerm::And,
            form
        )
    }

    fn test_simple_term(
        term: SimpleTermSelector,
        langs: &[(Lang, &str)],
        expect: Option<&TermPlurality>
    ) {
        let db = LocaleDbImpl::new(Box::new(predef_terms(langs)));
        // use en-AU so it has to do fallback to en-US
        let locale = db.merged_locale(Lang::en_au());
        assert_eq!(
            locale.simple_terms.get(&term),
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
            Some(&TermPlurality::Invariant("Australia".into()))
        )
    }

    #[test]
    fn term_form_refine() {
        test_simple_term(
            term_and(TermFormExtended::Long),
            &[
                (Lang::en_us(), r#"<term name="and">USA</term>"#),
                (Lang::en_au(), r#"<term name="and" form="short">Australia</term>"#),
            ],
            Some(&TermPlurality::Invariant("USA".into()))
        );
        test_simple_term(
            term_and(TermFormExtended::Short),
            &[
                (Lang::en_us(), r#"<term name="and">USA</term>"#),
                (Lang::en_au(), r#"<term name="and" form="short">Australia</term>"#),
            ],
            Some(&TermPlurality::Invariant("Australia".into()))
        );
    }

    #[test]
    fn term_fallback() {
        test_simple_term(
            term_and(TermFormExtended::Long),
            &[
                (Lang::en_us(), r#"<term name="and">USA</term>"#),
                (Lang::en_au(), r#""#),
            ],
            Some(&TermPlurality::Invariant("USA".into()))
        )
    }

    #[test]
    fn test_inline_iter() {
        let de_at = Lang::Iso(IsoLang::Deutsch, Some(IsoCountry::AT));
        let de = Lang::Iso(IsoLang::Deutsch, None);
        assert_eq!(de_at.inline_iter().collect::<Vec<_>>(), &[de_at, de]);
    }

    #[test]
    fn file_iter() {
        let de_at = Lang::Iso(IsoLang::Deutsch, Some(IsoCountry::AT));
        let de_de = Lang::Iso(IsoLang::Deutsch, Some(IsoCountry::DE));
        let en_us = Lang::Iso(IsoLang::English, Some(IsoCountry::US));
        assert_eq!(
            de_at.file_iter().collect::<Vec<_>>(),
            &[de_at, de_de, en_us]
        );
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

}

use nom::types::CompleteStr;
use nom::*;

impl FromStr for Lang {
    type Err = ();
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if let Ok((remainder, parsed)) = parse_lang(CompleteStr(&input)) {
            if remainder.is_empty() {
                Ok(parsed)
            } else {
                Err(())
            }
        } else {
            Err(())
        }
    }
}

named!(
    iso_lang<CompleteStr, IsoLang>,
    map!(take_while_m_n!(2, 3, char::is_alphabetic), |lang| {
        // You can unwrap because codegen has a default case with no Err output
        IsoLang::from_str(&lang).unwrap()
    })
);

named!(
    iso_country<CompleteStr, IsoCountry>,
    map!(preceded!(tag!("-"), take_while_m_n!(2, 2, char::is_alphabetic)), |country| {
        // You can unwrap because codegen has a default case with no Err output
        IsoCountry::from_str(&country).unwrap()
    })
);

named!(
    parse_iana<CompleteStr, Lang>,
    map!(preceded!(
        tag!("i-"),
        take_while!(|_| true)
    ), |lang| {
        Lang::Iana(lang.to_string())
    })
);

named!(
    parse_unofficial<CompleteStr, Lang>,
    map!(preceded!(
        tag!("x-"),
        take_while_m_n!(1, 8, char::is_alphanumeric)
    ), |lang| {
        Lang::Unofficial(lang.to_string())
    })
);

named!(
    parse_iso<CompleteStr, Lang>,
    map!(tuple!(
        call!(iso_lang),
        opt!(call!(iso_country))
    ), |(lang, country)| {
        Lang::Iso(lang, country)
    })
);

named!(
    parse_lang<CompleteStr, Lang>,
    alt!(call!(parse_unofficial) | call!(parse_iana) | call!(parse_iso))
);
