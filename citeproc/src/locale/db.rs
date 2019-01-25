use std::str::FromStr;
use std::sync::Arc;

use super::lang::LocaleSource;
use super::*;
use crate::style::db::*;

pub trait HasFetcher {
    fn get_fetcher(&self) -> Arc<LocaleFetcher>;
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
    db.style(())
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
