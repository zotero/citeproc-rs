use salsa::{Database, ParallelDatabase, Snapshot};
use serde_json;
use std::collections::HashSet;
use std::sync::Arc;

use crate::db::*;
use crate::input::Reference;
use crate::locale::db::HasFetcher;
use crate::locale::{db::*, LocaleFetcher};
use crate::style::db::*;
use crate::Atom;

pub struct RootDatabase {
    runtime: salsa::Runtime<Self>,
    fetcher: Arc<LocaleFetcher>,
}

impl RootDatabase {
    pub fn new(fetcher: Arc<LocaleFetcher>) -> Self {
        let mut db = RootDatabase {
            runtime: Default::default(),
            fetcher,
        };
        db.query_mut(StyleQuery).set((), Default::default());
        db
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
            fn uncited() for UncitedQuery;
            fn reference() for ReferenceQuery;
            fn disamb_tokens() for DisambTokensQuery;
            fn inverted_index() for InvertedIndexQuery;
        }
        impl StyleDatabase {
            fn style() for StyleQuery;
            fn name_citation() for NameCitationQuery;
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

impl ParallelDatabase for RootDatabase {
    fn snapshot(&self) -> Snapshot<Self> {
        Snapshot::new(RootDatabase {
            runtime: self.runtime.snapshot(self),
            fetcher: self.fetcher.clone(),
        })
    }
}

impl HasFetcher for RootDatabase {
    fn get_fetcher(&self) -> Arc<LocaleFetcher> {
        self.fetcher.clone()
    }
}

impl RootDatabase {
    pub fn set_references(&mut self, json_str: &str) -> Result<(), serde_json::error::Error> {
        let refs: Vec<Reference> = serde_json::from_str(json_str)?;
        let keys: HashSet<Atom> = refs.iter().map(|r| r.id.clone()).collect();
        for r in refs {
            self.query_mut(ReferenceInputQuery)
                .set(r.id.clone(), Arc::new(r));
        }
        self.query_mut(CitekeysQuery).set((), Arc::new(keys));
        Ok(())
    }
    pub fn set_uncited(&mut self, uncited: HashSet<Atom>) {
        // make sure there are no keys we wouldn't recognise
        let merged = self.citekeys(()).intersection(&uncited).cloned().collect();
        self.query_mut(UncitedQuery).set((), Arc::new(merged));
    }
}
