#[macro_use]
extern crate log;

mod cite;
mod xml;
mod cluster;

pub use cite::*;
pub use xml::*;
pub use cluster::*;

use salsa::Durability;

pub fn safe_default(db: &mut (impl cite::CiteDatabase + xml::LocaleDatabase + xml::StyleDatabase)) {
    use std::sync::Arc;
    db.set_style_with_durability(Default::default(), Durability::HIGH);
    db.set_all_keys_with_durability(Default::default(), Durability::MEDIUM);
    db.set_all_uncited(Default::default());
    db.set_cluster_ids(Arc::new(Default::default()));
    db.set_locale_input_langs(Default::default());
    db.set_default_lang_override(Default::default());
}
