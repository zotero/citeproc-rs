//! Describes `<locale>` elements inline or in standalone files.

use fnv::FnvHashMap;

use crate::style::element::{DateForm, LocaleDate};
use crate::style::terms::*;

pub(crate) mod db;
mod fetcher;
mod lang;
pub use self::fetcher::{LocaleFetcher};
pub use self::lang::{Lang, IsoLang, IsoCountry};
#[cfg(test)]
mod test;

#[derive(Default, Debug, Clone, Hash, Eq, PartialEq)]
pub struct LocaleOptionsNode {
    pub limit_ordinals_to_day_1: Option<bool>,
    pub punctuation_in_quote: Option<bool>,
}

impl LocaleOptionsNode {
    fn merge(&mut self, other: &Self) {
        self.limit_ordinals_to_day_1 = other
            .limit_ordinals_to_day_1
            .or(self.limit_ordinals_to_day_1);
        self.punctuation_in_quote = other.punctuation_in_quote.or(self.punctuation_in_quote);
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct LocaleOptions {
    pub limit_ordinals_to_day_1: bool,
    pub punctuation_in_quote: bool,
}

impl LocaleOptions {
    fn from_merged(node: &LocaleOptionsNode) -> Self {
        let mut this = Self::default();
        if let Some(x) = node.limit_ordinals_to_day_1 {
            this.limit_ordinals_to_day_1 = x;
        }
        if let Some(x) = node.punctuation_in_quote {
            this.punctuation_in_quote = x;
        }
        this
    }
}

impl Default for LocaleOptions {
    fn default() -> Self {
        LocaleOptions {
            limit_ordinals_to_day_1: false,
            punctuation_in_quote: false,
        }
    }
}

pub type DateMapping = FnvHashMap<DateForm, LocaleDate>;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum LocaleSource {
    Inline(Option<Lang>),
    File(Lang),
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Locale {
    pub version: String,
    pub lang: Option<Lang>,
    pub options_node: LocaleOptionsNode,
    pub simple_terms: SimpleMapping,
    pub gendered_terms: GenderedMapping,
    pub ordinal_terms: OrdinalMapping,
    pub role_terms: RoleMapping,
    pub dates: DateMapping,
}

impl Locale {
    pub fn get_text_term<'l>(&'l self, sel: TextTermSelector, plural: bool) -> Option<&'l str> {
        use crate::style::terms::TextTermSelector::*;
        match sel {
            Simple(ref ts) => ts
                .fallback()
                .filter_map(|sel| self.simple_terms.get(&sel))
                .next()
                .and_then(|r| r.get(plural)),
            Gendered(ref ts) => ts
                .fallback()
                .filter_map(|sel| self.gendered_terms.get(&sel))
                .next()
                .and_then(|r| r.0.get(plural)),
            Role(ref ts) => ts
                .fallback()
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
        extend(&mut self.dates, &with.dates);
        // replace the whole ordinals configuration
        self.ordinal_terms = with.ordinal_terms.clone();
        self.options_node.merge(&with.options_node);
    }
}

