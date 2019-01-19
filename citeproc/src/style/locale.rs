use std::fmt;

use crate::style::element::LocaleDate;
use crate::style::terms::*;

mod fetcher;
pub use self::fetcher::{Filesystem, LocaleFetcher};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CslOption(String, String);

#[derive(Debug, Clone, PartialEq, Eq)]
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
            Simple(ref ts) => self.simple_terms.get(ts).and_then(|r| r.get(plural)),
            Gendered(ref ts) => self.gendered_terms.get(ts).and_then(|r| r.0.get(plural)),
            Role(ref ts) => self.role_terms.get(ts).and_then(|r| r.get(plural)),
        }
    }
}

// pub fn merge_locales<'d, 'l: 'd>(_base: Locale<'d>, locales: Vec<Locale<'l>>) -> Vec<Locale<'l>> {
//     locales
// }

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

#[test]
fn test_file_iter() {
    let de_at = Lang::Iso(IsoLang::Deutsch, Some(IsoCountry::AT));
    let de_de = Lang::Iso(IsoLang::Deutsch, Some(IsoCountry::DE));
    let en_us = Lang::Iso(IsoLang::English, Some(IsoCountry::US));
    assert_eq!(
        de_at.file_iter().collect::<Vec<_>>(),
        &[de_at, de_de, en_us]
    );
}

#[test]
fn test_inline_iter() {
    let de_at = Lang::Iso(IsoLang::Deutsch, Some(IsoCountry::AT));
    let de = Lang::Iso(IsoLang::Deutsch, None);
    assert_eq!(de_at.inline_iter().collect::<Vec<_>>(), &[de_at, de]);
}

use std::str::FromStr;

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

#[test]
fn test_lang_from_str() {
    let de_at = Lang::Iso(IsoLang::Deutsch, Some(IsoCountry::AT));
    let de = Lang::Iso(IsoLang::Deutsch, None);
    let iana = Lang::Iana("Navajo".to_string());
    let unofficial = Lang::Unofficial("Newspeak".to_string());
    assert_eq!(Lang::from_str("de-AT"), Ok(de_at));
    assert_eq!(Lang::from_str("de"), Ok(de));
    assert_eq!(Lang::from_str("i-Navajo"), Ok(iana));
    assert_eq!(Lang::from_str("x-Newspeak"), Ok(unofficial));
}

use nom::types::CompleteStr;
use nom::*;

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
