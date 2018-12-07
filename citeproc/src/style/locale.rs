use crate::style::element::LocaleDate;
use crate::style::terms::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CslOption(String, String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locale {
    pub version: String,
    pub lang: String,
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
