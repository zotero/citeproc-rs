use salsa::Database;

use crate::style::db::*;
use crate::style::locale::{
    LocaleFetcher,
    Lang,
    db::*,
};

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
        db.query_mut(StyleTextQuery).set((), Default::default());
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
        impl StyleDatabase {
            fn style_text() for StyleTextQuery;
            fn style() for StyleQuery;
            fn inline_locale() for InlineLocaleQuery;
        }
        impl LocaleDatabase {
            fn locale_xml() for LocaleXmlQuery;
            fn locale() for LocaleQuery;
            fn merged_locale() for MergedLocaleQuery;
            fn locale_options() for LocaleOptionsQuery;
        }
    }
}

