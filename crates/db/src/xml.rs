// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use std::io;
use std::sync::Arc;

use citeproc_io::output::markup::Markup;
use csl::{
    locale::{Lang, Locale, LocaleSource, EN_US},
    style::{Name, Style, TextElement, TextSource},
    SmartString,
};
use fnv::FnvHashSet;

pub trait HasFetcher {
    fn get_fetcher(&self) -> Arc<dyn LocaleFetcher>;
}

/// Salsa interface to a CSL style.
#[salsa::query_group(StyleDatabaseStorage)]
pub trait StyleDatabase {
    #[salsa::input]
    fn style(&self) -> Arc<Style>;

    #[salsa::input]
    fn formatter(&self) -> Markup;

    /// Grabs the Name options from `<style>` + `<citation>` elements
    /// First one is the inherited names-delimiter
    fn name_info_citation(&self) -> (Option<SmartString>, Arc<Name>);
    /// Grabs the Name options from `<style>` + `<bibliography>` elements
    /// First one is the inherited names-delimiter
    fn name_info_bibliography(&self) -> (Option<SmartString>, Arc<Name>);

    /// Lists every <names> block in the style, with each name variable it is used for
    fn name_configurations(&self) -> Arc<Vec<(NameVariable, Name)>>;
}

fn name_info_citation(db: &dyn StyleDatabase) -> (Option<SmartString>, Arc<Name>) {
    let style = db.style();
    style.name_info_citation()
}

fn name_info_bibliography(db: &dyn StyleDatabase) -> (Option<SmartString>, Arc<Name>) {
    let style = db.style();
    style.name_info_bibliography()
}

use csl::Element;
use csl::NameVariable;

fn name_configurations_inner(
    style: &Style,
    base: &Name,
    el: &Element,
    buf: &mut Vec<(NameVariable, Name)>,
) {
    match el {
        Element::Names(n) => {
            if let Some(name) = n.name.clone() {
                let name = base.merge(&name);
                for &var in &n.variables {
                    buf.push((var, name.clone()));
                }
                // recurse with the name properties as a new base
                if let Some(subs) = &n.substitute {
                    for e in &subs.0 {
                        name_configurations_inner(style, &name, e, buf);
                    }
                }
            } else {
                for &var in &n.variables {
                    buf.push((var, base.clone()));
                }
            }
        }
        // Otherwise recurse
        Element::Group(g) => {
            for e in &g.elements {
                name_configurations_inner(style, base, e, buf);
            }
        }
        Element::Choose(c) => {
            let c: &csl::style::Choose = &*c;
            let csl::style::Choose(ref iff, ref elseifs, ref elsee) = c;
            for e in &iff.1 {
                name_configurations_inner(style, base, e, buf);
            }
            for elseif in elseifs {
                for e in &elseif.1 {
                    name_configurations_inner(style, base, e, buf);
                }
            }
            for e in &elsee.0 {
                name_configurations_inner(style, base, e, buf);
            }
        }
        Element::Text(TextElement {
            source: TextSource::Macro(m),
            ..
        }) => {
            if let Some(mac) = style.macros.get(m) {
                for e in mac {
                    name_configurations_inner(style, base, e, buf);
                }
            }
        }
        _ => {}
    }
}

/// Testable variant, is all.
fn name_configurations_middle(style: &Style) -> Vec<(NameVariable, Name)> {
    let base = style.name_citation();
    let mut vec = Vec::new();
    for el in &style.citation.layout.elements {
        name_configurations_inner(style, &base, el, &mut vec);
    }
    vec
}

fn name_configurations(db: &dyn StyleDatabase) -> Arc<Vec<(NameVariable, Name)>> {
    let style = db.style();
    Arc::new(name_configurations_middle(&style))
}

#[test]
fn test_name_configurations() {
    let sty = Style::parse_for_test(
        r#"<style class="note" version="1.0">
        <macro name="blah">
            <names variable="translator"/>
        </macro>
        <citation et-al-min="10">
            <layout>
                <names variable="author editor"/>
                <group>
                    <names variable="editor">
                        <name et-al-min="11" />
                    </names>
                </group>
                <text macro="blah" />
            </layout>
        </citation>
    </style>"#,
        None,
    )
    .unwrap();
    let confs = name_configurations_middle(&sty);
    let mut conf = Name::root_default();
    conf.et_al_min = Some(10);
    let mut conf2 = Name::root_default();
    conf2.et_al_min = Some(11);
    assert_eq!(
        &confs,
        &[
            (NameVariable::Author, conf.clone()),
            (NameVariable::Editor, conf.clone()),
            (NameVariable::Editor, conf2),
            (NameVariable::Translator, conf)
        ]
    );
}

/// Salsa interface to locales, including merging.
#[salsa::query_group(LocaleDatabaseStorage)]
pub trait LocaleDatabase: StyleDatabase + HasFetcher {
    #[salsa::input]
    fn locale_input_xml(&self, key: Lang) -> Arc<String>;
    #[salsa::input]
    fn locale_input_langs(&self) -> Arc<FnvHashSet<Lang>>;
    #[salsa::input]
    fn default_lang_override(&self) -> Option<Lang>;

    /// Backed by the LocaleFetcher implementation
    #[salsa::transparent]
    fn locale_xml(&self, key: Lang) -> Option<Arc<String>>;

    /// Derived from a `Style`
    #[salsa::transparent]
    fn inline_locale(&self, key: Option<Lang>) -> Option<Arc<Locale>>;

    /// A locale object, which may be `Default::default()`
    fn parsed_locale(&self, key: LocaleSource) -> Option<Arc<Locale>>;

    /// Derives the full lang inheritance chain, and merges them into one
    #[salsa::transparent]
    fn merged_locale(&self, key: Lang) -> Arc<Locale>;

    fn default_locale(&self) -> Arc<Locale>;

    #[salsa::transparent]
    fn default_lang(&self) -> Lang;
}

fn default_lang(db: &dyn LocaleDatabase) -> Lang {
    db.default_lang_override().unwrap_or_else(|| {
        db.style()
            .default_locale
            .clone()
            .unwrap_or_else(Default::default)
    })
}

fn default_locale(db: &dyn LocaleDatabase) -> Arc<Locale> {
    db.merged_locale(db.default_lang())
}

fn locale_xml(db: &dyn LocaleDatabase, key: Lang) -> Option<Arc<String>> {
    let stored = db.locale_input_langs();
    if stored.contains(&key) {
        return Some(db.locale_input_xml(key));
    }
    debug!("fetching locale: {:?}", key);
    match db.get_fetcher().fetch_string(&key) {
        Ok(Some(s)) => Some(Arc::new(s)),
        Ok(None) => None,
        Err(e) => {
            error!("{:?}", e);
            None
        }
    }
}

fn inline_locale(db: &dyn LocaleDatabase, key: Option<Lang>) -> Option<Arc<Locale>> {
    db.style().locale_overrides.get(&key).cloned().map(Arc::new)
}

fn parsed_locale(db: &dyn LocaleDatabase, key: LocaleSource) -> Option<Arc<Locale>> {
    match key {
        LocaleSource::File(ref lang) => {
            let string = db.locale_xml(lang.clone());
            string
                .and_then(|s| match Locale::parse(&s) {
                    Ok(l) => Some(l),
                    Err(e) => {
                        error!("failed to parse locale for lang {}: {:?}", lang, e);
                        None
                    }
                })
                .map(Arc::new)
        }
        LocaleSource::Inline(ref lang) => db.inline_locale(lang.clone()),
    }
}

fn merged_locale(db: &dyn LocaleDatabase, key: Lang) -> Arc<Locale> {
    debug!("requested locale {:?}", key);
    let locales = key
        .iter()
        .filter_map(|src| db.parsed_locale(src))
        .collect::<Vec<_>>();
    Arc::new(
        locales
            .into_iter()
            .rev()
            .fold(None, |mut acc, l| match acc {
                None => Some((*l).clone()),
                Some(ref mut base) => {
                    debug!("merging locales: {:?} <- {:?}", base.lang, l.lang);
                    base.merge(&l);
                    acc
                }
            })
            .unwrap_or_else(|| {
                warn!("Using default, empty locale");
                Locale::default()
            }),
    )
}

use std::panic::RefUnwindSafe;

cfg_if::cfg_if! {
    if #[cfg(feature = "parallel")] {
        /// Must be RefUnwindSafe because LocaleFetcher is typically stored in an Arc, and we want
        /// to have citeproc::Processor be unwind safe as well. RefUnwindSafe basically means
        /// safe to have a reference to in an Arc that crosses a catch_panic boundary.
        pub trait LocaleFetcher: Send + Sync + RefUnwindSafe {
            fn fetch_string(&self, lang: &Lang) -> Result<Option<String>, LocaleFetchError>;
            fn fetch_locale(&self, lang: &Lang) -> Option<Locale> {
                let s = self.fetch_string(lang).ok()??;
                Some(Locale::parse(&s).ok()?)
            }
        }
    } else {
        pub trait LocaleFetcher: RefUnwindSafe {
            fn fetch_string(&self, lang: &Lang) -> Result<Option<String>, LocaleFetchError>;
            fn fetch_locale(&self, lang: &Lang) -> Option<Locale> {
                let s = self.fetch_string(lang).ok()??;
                Some(Locale::parse(&s).ok()?)
            }
        }
    }
}

#[derive(Debug)]
pub enum LocaleFetchError {
    Io(io::Error),
    Other(String),
}

impl From<String> for LocaleFetchError {
    fn from(err: String) -> LocaleFetchError {
        LocaleFetchError::Other(err)
    }
}

impl From<io::Error> for LocaleFetchError {
    fn from(err: io::Error) -> LocaleFetchError {
        LocaleFetchError::Io(err)
    }
}

use std::collections::HashMap;

pub struct PredefinedLocales(pub HashMap<Lang, String>);

impl PredefinedLocales {
    pub fn bundled_en_us() -> Self {
        let mut m = HashMap::new();
        m.insert(Lang::en_us(), EN_US.to_owned());
        PredefinedLocales(m)
    }
}

impl LocaleFetcher for PredefinedLocales {
    fn fetch_string(&self, lang: &Lang) -> Result<Option<String>, LocaleFetchError> {
        Ok(self.0.get(lang).cloned())
    }
}
