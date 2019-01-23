use super::{Lang, Locale};
use crate::style::error::StyleError;
use crate::style::FromNode;

use roxmltree::Document;
use std::str::FromStr;

pub trait LocaleFetcher: Send + Sync {
    fn fetch_string(&self, lang: &Lang) -> Result<String, std::io::Error>;

    fn fetch_cli(&self, lang: &Lang) -> Option<Locale> {
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

impl FromStr for Locale {
    type Err = StyleError;
    fn from_str(xml: &str) -> Result<Self, Self::Err> {
        let doc = Document::parse(&xml)?;
        let locale = Locale::from_node(&doc.root_element())?;
        Ok(locale)
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
