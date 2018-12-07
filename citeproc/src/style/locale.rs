use fnv::FnvHashMap;
// use crate::style::terms::{T};
use crate::style::element::{LocaleDate};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CslOption(String, String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locale {
    pub version: String,
    pub lang: String,
    pub options: Vec<CslOption>,
    pub terms: Vec<String>,
    pub dates: Vec<LocaleDate>,
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
