// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use crate::attr::*;
use crate::error::{InvalidCsl, PartitionResults, StyleError};
use crate::style::{DateForm, DatePart, Delimiter, Formatting, TextCase};
use crate::terms::*;
use crate::{FromNode, FromNodeResult, ParseInfo};
use fnv::FnvHashMap;
use roxmltree::{Document, Node};
use std::str::FromStr;

mod lang;
pub use self::lang::{IsoCountry, IsoLang, Lang, LocaleSource};

#[derive(Default, Debug, Clone, Hash, Eq, PartialEq)]
pub struct LocaleOptionsNode {
    pub limit_ordinals_to_day_1: Option<bool>,
    pub punctuation_in_quote: Option<bool>,
}

impl LocaleOptionsNode {
    fn merge(&mut self, other: &Self) {
        self.limit_ordinals_to_day_1 = other
            .limit_ordinals_to_day_1
            .or(self.limit_ordinals_to_day_1);
        self.punctuation_in_quote = other.punctuation_in_quote.or(self.punctuation_in_quote);
    }
}
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct LocaleOptions {
    pub limit_ordinals_to_day_1: bool,
    pub punctuation_in_quote: bool,
}

impl LocaleOptions {
    pub fn from_merged(node: &LocaleOptionsNode) -> Self {
        let mut this = Self::default();
        if let Some(x) = node.limit_ordinals_to_day_1 {
            this.limit_ordinals_to_day_1 = x;
        }
        if let Some(x) = node.punctuation_in_quote {
            this.punctuation_in_quote = x;
        }
        this
    }
}

impl Default for LocaleOptions {
    fn default() -> Self {
        LocaleOptions {
            limit_ordinals_to_day_1: false,
            punctuation_in_quote: false,
        }
    }
}

pub type DateMapping = FnvHashMap<DateForm, LocaleDate>;

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Locale {
    pub version: String,
    pub lang: Option<Lang>,
    pub options_node: LocaleOptionsNode,
    pub simple_terms: FnvHashMap<SimpleTermSelector, TermPlurality>,
    pub gendered_terms: FnvHashMap<GenderedTermSelector, GenderedTerm>,
    pub ordinal_terms: FnvHashMap<OrdinalTermSelector, String>,
    pub role_terms: FnvHashMap<RoleTermSelector, TermPlurality>,
    pub dates: DateMapping,
}

impl FromStr for Locale {
    type Err = StyleError;
    fn from_str(xml: &str) -> Result<Self, Self::Err> {
        let doc = Document::parse(&xml)?;
        let info = ParseInfo::default();
        let locale = Locale::from_node(&doc.root_element(), &info)?;
        Ok(locale)
    }
}

/// This is always bound to the prefix "xml:"
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
// const CSL_NAMESPACE: &str = "http://purl.org/net/xbiblio/csl";

impl FromNode for Locale {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let lang = attribute_option(node, (XML_NAMESPACE, "lang"), info)?;

        // TODO: one slot for each date form, avoid allocations?
        let dates_vec = node
            .children()
            .filter(|el| el.has_tag_name("date"))
            .map(|el| LocaleDate::from_node(&el, info))
            .partition_results()?;

        let mut dates = FnvHashMap::default();
        for date in dates_vec.into_iter() {
            dates.insert(date.form, date);
        }

        let mut simple_terms = FnvHashMap::default();
        let mut gendered_terms = FnvHashMap::default();
        let mut ordinal_terms = FnvHashMap::default();
        let mut role_terms = FnvHashMap::default();

        let options_node = node
            .children()
            .filter(|el| el.has_tag_name("style-options"))
            .nth(0)
            .map(|o_node| LocaleOptionsNode::from_node(&o_node, info))
            .unwrap_or_else(|| Ok(LocaleOptionsNode::default()))?;

        let terms_node = node.children().filter(|el| el.has_tag_name("terms")).nth(0);
        if let Some(tn) = terms_node {
            for n in tn.children().filter(|el| el.has_tag_name("term")) {
                match TermEl::from_node(&n, info)? {
                    TermEl::Simple(sel, con) => {
                        simple_terms.insert(sel, con);
                    }
                    TermEl::Gendered(sel, con) => {
                        gendered_terms.insert(sel.normalise(), con);
                    }
                    TermEl::Ordinal(sel, con) => {
                        ordinal_terms.insert(sel, con);
                    }
                    TermEl::Role(sel, con) => {
                        role_terms.insert(sel, con);
                    }
                }
            }
        }

        Ok(Locale {
            version: "1.0".into(),
            lang,
            options_node,
            simple_terms,
            gendered_terms,
            ordinal_terms,
            role_terms,
            dates,
        })
    }
}

/// A date element defined inside a `<cs:locale>`
#[derive(Debug, Eq, Clone, PartialEq)]
pub struct LocaleDate {
    pub form: DateForm,
    pub date_parts: Vec<DatePart>,
    pub delimiter: Delimiter,
    pub text_case: TextCase,
    pub formatting: Option<Formatting>,
}

impl FromNode for LocaleDate {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let elements = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("date-part"))
            .map(|el| DatePart::from_node_dp(&el, true, info))
            .partition_results()?;
        Ok(LocaleDate {
            form: attribute_required(node, "form", info)?,
            date_parts: elements,
            formatting: Option::from_node(node, info)?,
            delimiter: Delimiter::from_node(node, info)?,
            text_case: TextCase::from_node(node, info)?,
        })
    }
}

// Intermediate type for transforming a list of many terms into 4 different hashmaps
enum TermEl {
    Simple(SimpleTermSelector, TermPlurality),
    Gendered(GenderedTermSelector, GenderedTerm),
    Ordinal(OrdinalTermSelector, String),
    Role(RoleTermSelector, TermPlurality),
}

impl FromNode for TermEl {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        use crate::terms::AnyTermName::*;
        let name: AnyTermName = attribute_required(node, "name", info)?;
        let content = TermPlurality::from_node(node, info)?;
        match name {
            Number(v) => Ok(TermEl::Gendered(
                GenderedTermSelector::Number(v, TermForm::from_node(node, info)?),
                GenderedTerm(content, attribute_optional(node, "gender", info)?),
            )),
            Month(mt) => Ok(TermEl::Gendered(
                GenderedTermSelector::Month(mt, TermForm::from_node(node, info)?),
                GenderedTerm(content, attribute_optional(node, "gender", info)?),
            )),
            Loc(lt) => Ok(TermEl::Gendered(
                GenderedTermSelector::Locator(lt, TermForm::from_node(node, info)?),
                GenderedTerm(content, attribute_optional(node, "gender", info)?),
            )),
            Misc(t) => Ok(TermEl::Simple(
                SimpleTermSelector::Misc(t, TermFormExtended::from_node(node, info)?),
                content,
            )),
            Season(t) => Ok(TermEl::Simple(
                SimpleTermSelector::Season(t, TermForm::from_node(node, info)?),
                content,
            )),
            Quote(t) => Ok(TermEl::Simple(
                SimpleTermSelector::Quote(t, TermForm::from_node(node, info)?),
                content,
            )),
            Role(t) => Ok(TermEl::Role(
                RoleTermSelector(t, TermFormExtended::from_node(node, info)?),
                content,
            )),
            Ordinal(t) => match content {
                TermPlurality::Invariant(a) => Ok(TermEl::Ordinal(
                    OrdinalTermSelector(
                        t,
                        attribute_optional(node, "gender-form", info)?,
                        OrdinalMatch::from_node(node, info)?,
                    ),
                    a,
                )),
                _ => Err(InvalidCsl::new(node, "ordinal terms cannot be pluralized").into()),
            },
        }
    }
}

impl FromNode for LocaleOptionsNode {
    fn from_node(node: &Node, _info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(LocaleOptionsNode {
            limit_ordinals_to_day_1: attribute_option_bool(node, "limit-ordinals-to-day-1")?,
            punctuation_in_quote: attribute_option_bool(node, "punctuation-in-quote")?,
        })
    }
}

impl Locale {
    pub fn get_text_term<'l>(&'l self, sel: TextTermSelector, plural: bool) -> Option<&'l str> {
        use crate::terms::TextTermSelector::*;
        match sel {
            Simple(ref ts) => ts
                .fallback()
                .filter_map(|sel| self.simple_terms.get(&sel))
                .next()
                .map(|r| r.get(plural)),
            Gendered(ref ts) => ts
                .normalise()
                .fallback()
                .filter_map(|sel| self.gendered_terms.get(&sel))
                .next()
                .map(|r| r.0.get(plural)),
            Role(ref ts) => ts
                .fallback()
                .filter_map(|sel| self.role_terms.get(&sel))
                .next()
                .map(|r| r.get(plural)),
        }
    }

    pub fn merge(&mut self, with: &Self) {
        fn extend<K: Clone + Eq + std::hash::Hash, V: Clone>(
            map: &mut FnvHashMap<K, V>,
            other: &FnvHashMap<K, V>,
        ) {
            map.extend(other.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        self.lang = with.lang.clone();
        extend(&mut self.simple_terms, &with.simple_terms);
        extend(&mut self.gendered_terms, &with.gendered_terms);
        extend(&mut self.role_terms, &with.role_terms);
        extend(&mut self.dates, &with.dates);
        // replace the whole ordinals configuration
        self.ordinal_terms = with.ordinal_terms.clone();
        self.options_node.merge(&with.options_node);
    }
}
