use citeproc_db::{LocaleFetcher, PredefinedLocales, StyleDatabase};
use csl::locale::Lang;
use std::collections::HashMap;
use std::sync::Arc;

#[salsa::database(
    citeproc_db::StyleDatabaseStorage,
    citeproc_db::LocaleDatabaseStorage,
    citeproc_db::CiteDatabaseStorage,
    crate::db::IrDatabaseStorage
)]
pub struct MockProcessor {
    runtime: salsa::Runtime<Self>,
    fetcher: Arc<dyn LocaleFetcher>,
}

impl salsa::Database for MockProcessor {
    fn salsa_runtime(&self) -> &salsa::Runtime<MockProcessor> {
        &self.runtime
    }
}

impl citeproc_db::HasFetcher for MockProcessor {
    fn get_fetcher(&self) -> Arc<dyn LocaleFetcher> {
        self.fetcher.clone()
    }
}

impl MockProcessor {
    pub fn new() -> Self {
        let mut m = HashMap::new();
        m.insert(
            Lang::en_us(),
            include_str!("../../citeproc-wasm/src/locales-en-US.xml").to_string(),
        );
        let fetcher = Arc::new(PredefinedLocales(m));
        let mut db = MockProcessor {
            runtime: Default::default(),
            fetcher,
        };
        citeproc_db::safe_default(&mut db);
        db
    }

    pub fn set_style_text(&mut self, style_text: &str) {
        use csl::style::Style;
        use std::str::FromStr;
        let style = Style::from_str(style_text).unwrap();
        use salsa::Durability;
        self.set_style_with_durability(Arc::new(style), Durability::MEDIUM);
    }
}
