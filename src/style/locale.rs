use crate::style::element::{ Form, Date };

#[derive(Debug, PartialEq, Eq)]
pub struct CslOption(String, String);

#[derive(Debug, PartialEq, Eq)]
pub struct Term {
    pub name: String,
    pub form: Form,
    pub gender: Gender,
    pub singular: String,
    pub plural: String,
    pub ordinal_match: OrdinalMatch,
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum OrdinalMatch {
    LastTwoDigits,
    WholeNumber,
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum Gender {
    Masculine,
    Feminine,
    Neuter,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Locale {
    pub version: String,
    pub lang: String,
    pub options: Vec<CslOption>,
    pub terms: Vec<Term>,
    pub date: Vec<Date>,
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

