use super::{Lang, Locale, LocaleSource};
use crate::style::error::StyleError;
use crate::style::FromNode;

use roxmltree::Document;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

pub trait LocaleFetcher {
    fn fetch_string(&self, lang: &Lang) -> Result<String, std::io::Error>;
    /// This is mut so that caching fetchers can still implement it, and they are the only useful
    /// ones.
    fn fetch(&mut self, lang: &Lang) -> Option<Locale> {
        Locale::from_str(&self.fetch_string(lang).ok()?).ok()
    }
    fn fetch_cli(&mut self, lang: &Lang) -> Option<Locale> {
        let string = self.fetch_string(lang).ok()?;
        let with_errors = |s: &str| Ok(Locale::from_str(s)?);
        match with_errors(&string) {
            Ok(l) => Some(l),
            Err(e) => {
                crate::style::error::file_diagnostics(&e, "input", &string);
                None
            }
        }
    }
}

use std::io;

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

type LocaleFetchResult = Result<Locale, LocaleFetchError>;

pub struct Filesystem {
    root: PathBuf,
}

impl FromStr for Locale {
    type Err = StyleError;
    fn from_str(xml: &str) -> Result<Self, Self::Err> {
        let doc = Document::parse(&xml)?;
        let locale = Locale::from_node(&doc.root_element())?;
        Ok(locale)
    }
}

impl Filesystem {
    pub fn new(repo_dir: impl Into<PathBuf>) -> Self {
        Filesystem {
            root: repo_dir.into(),
        }
    }
}

impl LocaleFetcher for Filesystem {
    fn fetch_string(&self, lang: &Lang) -> Result<String, std::io::Error> {
        let mut path = self.root.clone();
        path.push(&format!("locales-{}.xml", lang));
        fs::read_to_string(path)
    }
}

impl LocaleSource {
    pub fn get<'a, F: LocaleFetcher>(
        &self,
        inlines: &'a HashMap<Option<Lang>, Locale>,
        cache: &mut LocaleCache<F>,
    ) -> Option<Locale> {
        match self {
            LocaleSource::Inline(ref key) => inlines.get(key).cloned(),
            LocaleSource::File(ref key) => cache.fetch(key),
        }
    }
}

pub struct LocaleCache<F: LocaleFetcher> {
    cache: HashMap<Lang, Locale>,
    inner: Box<F>,
}

impl<F: LocaleFetcher> LocaleCache<F> {
    pub fn new(inner_fetcher: F) -> Self {
        LocaleCache {
            cache: HashMap::default(),
            inner: Box::new(inner_fetcher),
        }
    }
}

impl<F: LocaleFetcher> LocaleFetcher for LocaleCache<F> {
    fn fetch_string(&self, lang: &Lang) -> Result<String, io::Error> {
        Ok(String::new(), )
    }
    fn fetch(&mut self, lang: &Lang) -> Option<Locale> {
        let got = self.inner.fetch(lang);
        if let Some(ref locale) = &got {
            self.cache.insert(lang.clone(), locale.clone());
        } else {
            self.cache.remove(lang);
        }
        got
    }
}
