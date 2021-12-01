#[macro_use]
extern crate log;

mod cite;
mod cluster;
mod xml;

pub use cite::*;
use citeproc_io::output::markup::Markup;
pub use cluster::*;
pub use xml::*;

use salsa::Durability;

pub fn safe_default(db: &mut (impl cite::CiteDatabase + xml::LocaleDatabase + xml::StyleDatabase)) {
    use std::sync::Arc;
    db.set_style_with_durability(Default::default(), Durability::HIGH);
    db.set_formatter_with_durability(Markup::html(), Durability::HIGH);
    db.set_all_keys_with_durability(Default::default(), Durability::MEDIUM);
    db.set_all_uncited(Default::default());
    db.set_all_cluster_ids(Arc::new(Default::default()));
    db.set_clusters_ordered(Arc::new(Default::default()));
    db.set_locale_input_langs_with_durability(Default::default(), Durability::HIGH);
    db.set_default_lang_override_with_durability(Default::default(), Durability::HIGH);
}
