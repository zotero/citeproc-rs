use std::str::FromStr;
use std::sync::Arc;
use std::io;

use csl::error::StyleError;
use csl::{
    style::{Name, Style},
    locale::{Lang, Locale, LocaleSource, LocaleOptions},
};

pub trait HasFetcher {
    fn get_fetcher(&self) -> Arc<LocaleFetcher>;
}


/// Salsa interface to a CSL style.
#[salsa::query_group(StyleDatabaseStorage)]
pub trait StyleDatabase: salsa::Database {
    #[salsa::input]
    fn style(&self) -> Arc<Style>;
    fn name_citation(&self) -> Arc<Name>;
}

fn name_citation(db: &impl StyleDatabase) -> Arc<Name> {
    let style = db.style();
    let default = Name::root_default();
    let root = &style.name_inheritance;
    let citation = &style.citation.name_inheritance;
    Arc::new(default.merge(root).merge(citation))
}

/// Salsa interface to locales, including merging.
#[salsa::query_group(LocaleDatabaseStorage)]
pub trait LocaleDatabase: salsa::Database + StyleDatabase + HasFetcher {
    /// Backed by the LocaleFetcher implementation
    fn locale_xml(&self, key: Lang) -> Option<Arc<String>>;

    /// Derived from a `Style`
    fn inline_locale(&self, key: Option<Lang>) -> Option<Arc<Locale>>;

    /// A locale object, which may be `Default::default()`
    fn locale(&self, key: LocaleSource) -> Option<Arc<Locale>>;

    /// Derives the full lang inheritance chain, and merges them into one
    fn merged_locale(&self, key: Lang) -> Arc<Locale>;

    /// Even though we already have a merged `LocaleOptionsNode` struct, all its fields are
    /// `Option`. To avoid having to unwrap each field later on, we merge whatever options did
    /// get provided into a non-`Option` defaults struct.
    fn locale_options(&self, key: Lang) -> Arc<LocaleOptions>;
}

fn locale_xml(db: &impl LocaleDatabase, key: Lang) -> Option<Arc<String>> {
    db.get_fetcher().fetch_string(&key).ok().map(Arc::new)
}

fn inline_locale(db: &impl LocaleDatabase, key: Option<Lang>) -> Option<Arc<Locale>> {
    db.style()
        .locale_overrides
        .get(&key)
        .cloned()
        .map(Arc::new)
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

fn locale_options(db: &impl LocaleDatabase, key: Lang) -> Arc<LocaleOptions> {
    let merged = &db.merged_locale(key).options_node;
    Arc::new(LocaleOptions::from_merged(merged))
}

pub trait LocaleFetcher: Send + Sync {
    fn fetch_string(&self, lang: &Lang) -> Result<String, std::io::Error>;
}

#[derive(Debug)]
pub enum LocaleFetchError {
    Io(io::Error),
    Style(StyleError),
}

impl From<io::Error> for LocaleFetchError {
    fn from(err: io::Error) -> LocaleFetchError {
        LocaleFetchError::Io(err)
    }
}

impl From<StyleError> for LocaleFetchError {
    fn from(err: StyleError) -> LocaleFetchError {
        LocaleFetchError::Style(err)
    }
}

#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
pub struct Predefined(pub HashMap<Lang, String>);

#[cfg(test)]
impl LocaleFetcher for Predefined {
    fn fetch_string(&self, lang: &Lang) -> Result<String, std::io::Error> {
        Ok(self.0.get(lang).cloned().unwrap_or_else(|| {
            String::from(
                r#"<?xml version="1.0" encoding="utf-8"?>
        <locale xmlns="http://purl.org/net/xbiblio/csl" version="1.0" xml:lang="en-US">
        </locale>"#,
            )
        }))
    }
}
