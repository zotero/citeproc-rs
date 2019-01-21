use salsa::Database;
use serde_json;
use std::collections::HashSet;
use std::sync::Arc;

use crate::db::*;
use crate::input::Reference;
use crate::locale::{db::*, Lang, LocaleFetcher};
use crate::style::db::*;
use crate::Atom;

pub struct RootDatabase {
    runtime: salsa::Runtime<Self>,
    fetcher: Box<LocaleFetcher>,
}

impl RootDatabase {
    pub fn new(fetcher: Box<LocaleFetcher>) -> Self {
        let mut db = RootDatabase {
            runtime: Default::default(),
            fetcher,
        };
        db.query_mut(StyleQuery).set((), Default::default());
        db
    }
}

impl LocaleFetcher for RootDatabase {
    #[inline]
    fn fetch_string(&self, lang: &Lang) -> Result<String, std::io::Error> {
        self.fetcher.fetch_string(lang)
    }
}

/// This impl tells salsa where to find the salsa runtime.
impl salsa::Database for RootDatabase {
    fn salsa_runtime(&self) -> &salsa::Runtime<RootDatabase> {
        &self.runtime
    }
}

salsa::database_storage! {
    pub struct DatabaseImplStorage for RootDatabase {
        impl ReferenceDatabase {
            fn reference_input() for ReferenceInputQuery;
            fn citekeys() for CitekeysQuery;
            fn reference() for ReferenceQuery;
        }
        impl StyleDatabase {
            fn style() for StyleQuery;
        }
        impl LocaleDatabase {
            fn locale_xml() for LocaleXmlQuery;
            fn inline_locale() for InlineLocaleQuery;
            fn locale() for LocaleQuery;
            fn merged_locale() for MergedLocaleQuery;
            fn locale_options() for LocaleOptionsQuery;
        }
    }
}

impl RootDatabase {
    pub fn add_references(&mut self, json_str: &str) -> Result<(), serde_json::error::Error> {
        let refs: Vec<Reference> = serde_json::from_str(json_str)?;
        let keys: HashSet<Atom> = refs.iter().map(|r| r.id.clone()).collect();
        for r in refs {
            self.query_mut(ReferenceInputQuery)
                .set(r.id.clone(), Arc::new(r));
        }
        self.query_mut(CitekeysQuery).set((), Arc::new(keys));
        Ok(())
    }
}
