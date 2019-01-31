use std::sync::Arc;

use csl::style::{Name, Style};

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
