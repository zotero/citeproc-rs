// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use crate::error::{InvalidCsl, PartitionResults, StyleError};
use crate::style::{DateForm, DatePart, Formatting, TextCase};
use crate::terms::*;
use crate::variables::NumberVariable;
use crate::{attr::*, CslError, Severity};
use crate::{AttrChecker, FromNode, FromNodeResult, ParseInfo, SmartString};
use fnv::FnvHashMap;
use roxmltree::{Document, Node};
use std::str::FromStr;

mod lang;
pub use self::lang::{IsoCountry, IsoLang, Lang, LocaleSource};

pub const EN_US: &str = include_str!("locales-en-US.xml");

#[derive(Default, Debug, Clone, Hash, Eq, PartialEq)]
pub struct LocaleOptionsNode {
    pub limit_day_ordinals_to_day_1: Option<bool>,
    pub punctuation_in_quote: Option<bool>,
}

impl LocaleOptionsNode {
    fn merge(&mut self, other: &Self) {
        self.limit_day_ordinals_to_day_1 = other
            .limit_day_ordinals_to_day_1
            .or(self.limit_day_ordinals_to_day_1);
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
        if let Some(x) = node.limit_day_ordinals_to_day_1 {
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
        Locale::parse(xml)
    }
}

impl Locale {
    pub fn parse(xml: &str) -> Result<Self, StyleError> {
        let doc = Document::parse(&xml)?;
        let info = ParseInfo::default();
        let locale = Locale::from_node(&doc.root_element(), &info)?;
        Ok(locale)
    }
}

/// This is always bound to the prefix "xml:"
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
// const CSL_NAMESPACE: &str = "http://purl.org/net/xbiblio/csl";

pub(crate) const LANG_ATTR: (&str, &str) = (XML_NAMESPACE, "lang");

impl FromNode for Lang {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let lang = attribute_required(node, (XML_NAMESPACE, "lang"), info)?;
        Ok(lang)
    }
}

impl AttrChecker for Lang {
    fn filter_attribute(_attr: &str) -> bool {
        // unreachable
        false
    }
    fn filter_attribute_full(a: &roxmltree::Attribute) -> bool {
        a.name() == "lang" && a.namespace() == Some(XML_NAMESPACE)
    }
}

impl FromNode for Locale {
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("locale")
    }
    const CHILD_DESC: &'static str = "locale";
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let lang: Option<Lang> = FromNode::from_node(node, info)?;

        if node.tag_name().name() != "locale" {
            return Err(CslError(vec![InvalidCsl {
                severity: Severity::Error,
                range: node.range(),
                message: format!(
                    "root node must be a `<locale>` node, was `<{}>` instead",
                    node.tag_name().name()
                ),
                hint: "".into(),
            }]));
        }

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
    pub delimiter: Option<SmartString>,
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
            delimiter: attribute_option(node, "delimiter", info)?,
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
            Season(t) => Ok(TermEl::Gendered(
                GenderedTermSelector::Season(t, TermForm::from_node(node, info)?),
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
            Category(t) => Ok(TermEl::Simple(
                SimpleTermSelector::Category(t, TermForm::from_node(node, info)?),
                content,
            )),
            Quote(t) => Ok(TermEl::Simple(SimpleTermSelector::Quote(t), content)),
            Role(t) => Ok(TermEl::Role(
                RoleTermSelector(t, TermFormExtended::from_node(node, info)?),
                content,
            )),
            Ordinal(mut t) => match content {
                TermPlurality::Invariant(a) => {
                    if let OrdinalTerm::Mod100(_, ref mut m) = t {
                        if let Some(overrider) = attribute_option(node, "match", info)? {
                            *m = overrider;
                        }
                    }
                    Ok(TermEl::Ordinal(
                        OrdinalTermSelector(t, attribute_optional(node, "gender-form", info)?),
                        a,
                    ))
                }
                _ => Err(InvalidCsl::new(node, "ordinal terms cannot be pluralized").into()),
            },
        }
    }
}

impl FromNode for LocaleOptionsNode {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(LocaleOptionsNode {
            limit_day_ordinals_to_day_1: attribute_option(
                node,
                "limit-day-ordinals-to-day-1",
                info,
            )?,
            punctuation_in_quote: attribute_option(node, "punctuation-in-quote", info)?,
        })
    }
}

impl Locale {
    /// May return Some("") if the term is defined but empty. Not all code renders None in that
    /// case, so each call site should decide whether to slap .filter(|x| !x.is_empty()) after
    /// .get_text_term().
    pub fn get_text_term(&self, sel: TextTermSelector, plural: bool) -> Option<&str> {
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

    pub fn get_ordinal_term(&self, selector: OrdinalTermSelector) -> Option<&str> {
        let mut found = None;
        for sel in selector.fallback() {
            debug!("{:?}", sel);
            if let f @ Some(_) = self.ordinal_terms.get(&sel) {
                debug!("{:?}", f);
                found = f.map(|s| s.as_str());
                break;
            }
        }
        found
    }

    pub fn get_gendered_term(&self, selector: GenderedTermSelector) -> Option<&GenderedTerm> {
        let mut found = None;
        for sel in selector.fallback() {
            if let f @ Some(_) = self.gendered_terms.get(&sel) {
                found = f;
                break;
            }
        }
        found
    }

    pub fn get_simple_term(&self, selector: SimpleTermSelector) -> Option<&TermPlurality> {
        let mut found = None;
        for sel in selector.fallback() {
            if let f @ Some(_) = self.simple_terms.get(&sel) {
                found = f;
                break;
            }
        }
        found
    }

    pub fn and_term(&self, form: Option<TermFormExtended>) -> Option<&str> {
        let form = form.unwrap_or(TermFormExtended::Long);
        self.get_simple_term(SimpleTermSelector::Misc(MiscTerm::And, form))
            .map(|term_plurality| term_plurality.singular())
    }

    pub fn et_al_term(
        &self,
        element: Option<&crate::NameEtAl>,
    ) -> Option<(String, Option<Formatting>)> {
        let mut term = MiscTerm::EtAl;
        let mut default = "et al";
        let mut formatting = None;
        if let Some(el) = element {
            if el.term == "and others" {
                term = MiscTerm::AndOthers;
                default = "and others";
            }
            formatting = el.formatting;
        }
        let txt = self
            .get_text_term(
                TextTermSelector::Simple(SimpleTermSelector::Misc(term, TermFormExtended::Long)),
                false,
            )
            .unwrap_or(default);
        if txt.is_empty() {
            return None;
        }
        Some((txt.into(), formatting))
    }

    pub fn get_month_gender(&self, month: MonthTerm) -> Gender {
        let selector = GenderedTermSelector::Month(month, TermForm::Long);
        // Don't use fallback, just the long form
        if let Some(GenderedTerm(_, gender)) = self.gendered_terms.get(&selector) {
            *gender
        } else {
            Gender::Neuter
        }
    }

    pub fn get_num_gender(&self, var: NumberVariable, locator_type: LocatorType) -> Gender {
        let selector = if var == NumberVariable::Locator {
            GenderedTermSelector::Locator(locator_type, TermForm::Long)
        } else {
            GenderedTermSelector::Number(var, TermForm::Long).normalise()
        };
        // Don't use fallback, just the long form
        if let Some(GenderedTerm(_, gender)) = self.gendered_terms.get(&selector) {
            *gender
        } else {
            Gender::Neuter
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
        // replace the whole ordinals configuration if any of them are specified
        if !with.ordinal_terms.is_empty() {
            self.ordinal_terms = with.ordinal_terms.clone();
        }
        self.options_node.merge(&with.options_node);
    }
}
