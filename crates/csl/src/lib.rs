// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

//! Describes the `<style>` element and all its children, and parses it from an XML tree.

pub use smartstring::alias::String as SmartString;
pub use string_cache::DefaultAtom as Atom;

#[macro_use]
extern crate strum_macros;
#[macro_use]
extern crate log;

use std::sync::Arc;

pub mod error;

macro_rules! append_invalid_err {
    ($res:expr, $errs:expr) => {
        match $res {
            Ok(x) => Ok(x),
            Err(e) => {
                $errs.push(e);
                Err($crate::error::ChildGetterError)
            }
        }
    };
}

macro_rules! append_err {
    ($res:expr, $errs:expr) => {
        match $res {
            Ok(x) => Ok(x),
            Err(e) => {
                $errs.extend(e.0.into_iter());
                Err($crate::error::ChildGetterError)
            }
        }
    };
}

mod from_node;
use from_node::*;

#[cfg(test)]
macro_rules! assert_snapshot_parse {
    (Style, $xml:literal, $o:expr) => {
        ::insta::assert_debug_snapshot!($crate::Style::parse_for_test(
            ::indoc::indoc!($xml),
            Some($o)
        )
        .expect("should have parsed successfully"));
    };
    (Style, $xml:literal) => {
        ::insta::assert_debug_snapshot!($crate::Style::parse_for_test(::indoc::indoc!($xml), None)
            .expect("should have parsed successfully"));
    };
    ($ty:ty, $xml:literal, $options:expr) => {
        ::insta::assert_debug_snapshot!($crate::from_node::parse_as_with::<$ty>(
            ::indoc::indoc!($xml),
            Some($options)
        )
        .expect("did not parse"));
    };
    ($ty:ty, $xml:literal) => {
        ::insta::assert_debug_snapshot!($crate::from_node::parse_as_with::<$ty>(
            ::indoc::indoc!($xml),
            None
        )
        .expect("did not parse"));
    };
}

#[cfg(test)]
macro_rules! assert_snapshot_err {
    (Style, $xml:literal, $o:expr) => {
        ::insta::assert_debug_snapshot!($crate::Style::parse_for_test(
            ::indoc::indoc!($xml),
            Some($o)
        )
        .expect_err("should have failed with errors"));
    };
    (Style, $xml:literal) => {
        ::insta::assert_debug_snapshot!($crate::Style::parse_for_test(::indoc::indoc!($xml), None)
            .expect_err("should have failed with errors"));
    };
    ($ty:ty, $xml:literal, $options:expr) => {
        ::insta::assert_debug_snapshot!($crate::from_node::parse_as_with::<$ty>(
            ::indoc::indoc!($xml),
            Some($options)
        )
        .expect_err("should have failed with errors"));
    };
    ($ty:ty, $xml:literal) => {
        ::insta::assert_debug_snapshot!($crate::from_node::parse_as_with::<$ty>(
            ::indoc::indoc!($xml),
            None
        )
        .expect_err("should have failed with errors"));
    };
}

pub(crate) mod attr;
pub use self::attr::GetAttribute;
pub mod locale;
pub mod style;
pub mod terms;
pub mod variables;
pub mod version;

#[cfg(test)]
mod test;

pub use self::error::*;
pub use self::from_node::ParseOptions;
pub use self::locale::*;
pub use self::style::{dependent::*, info::*, *};
pub use self::terms::*;
pub use self::variables::*;
pub use self::version::*;

use self::attr::*;
use fnv::FnvHashMap;
use roxmltree::{Children, Node};
use semver::VersionReq;
use std::collections::HashMap;

use roxmltree::Document;
use std::str::FromStr;
impl FromStr for Style {
    type Err = StyleError;
    fn from_str(xml: &str) -> Result<Self, Self::Err> {
        Style::parse(xml)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CiteOrBib {
    Citation,
    Bibliography,
}

impl Style {
    /// Parses a style from an CSL string as XML.
    pub fn parse(xml: &str) -> Result<Self, StyleError> {
        Style::parse_with_opts(xml, ParseOptions::default())
    }
    pub fn parse_with_opts(xml: &str, options: ParseOptions) -> Result<Self, StyleError> {
        let doc = Document::parse(xml)?;
        let node = &doc.root_element();

        if node.tag_name().name() != "style" {
            return Err(StyleError::Invalid(CslError(vec![InvalidCsl {
                severity: Severity::Error,
                range: node.range(),
                message: format!(
                    "root node must be a `<style>` node, was `<{}>` instead",
                    node.tag_name().name()
                ),
                hint: "".into(),
            }])));
        }

        // We don't know which features will be enabled yet, but we get that in
        // Style::from_node_custom
        let parse_info = ParseInfo {
            options,
            ..Default::default()
        };

        // If there is actually no info block (testing only!), it can't be a dependent style.
        let mut errors = Vec::new();
        let info = if parse_info.options.allow_no_info {
            let mut throw_out = Vec::new();
            Ok(max_one_child::<Info>(node, &parse_info, &mut throw_out)
                .ok()
                .flatten()
                .unwrap_or_default())
        } else {
            exactly_one_child::<Info>(node, &parse_info, &mut errors)
        };
        if !errors.is_empty() {
            return Err(StyleError::Invalid(CslError(errors)));
        }
        let info = info.map_err(|_| StyleError::Invalid(CslError(Vec::new())))?;

        if let Some(parent_id) = info.independent_parent_id() {
            return Err(StyleError::DependentStyle {
                required_parent: parent_id,
            }
            .into());
        }

        let style = Style::from_node_custom(node, &parse_info, info)?;
        Ok(style)
    }
    #[doc(hidden)]
    pub fn parse_for_test(xml: &str, options: Option<ParseOptions>) -> Result<Self, StyleError> {
        Self::parse_with_opts(
            xml,
            ParseOptions {
                allow_no_info: true,
                ..options.unwrap_or_else(Default::default)
            },
        )
    }
    pub fn is_dependent(&self) -> bool {
        self.info.parent.is_some()
    }
    pub fn independent_parent(&self) -> Option<&ParentLink> {
        self.info.parent.as_ref()
    }
    pub fn get_layout(&self, loc: CiteOrBib) -> Option<&Layout> {
        match loc {
            CiteOrBib::Citation => Some(&self.citation.layout),
            CiteOrBib::Bibliography => self.bibliography.as_ref().map(|b| &b.layout),
        }
    }
}

impl Info {
    pub fn parse_from_style(xml: &str) -> Result<Self, StyleError> {
        let doc = Document::parse(xml)?;
        let node = &doc.root_element();
        let parse_info = ParseInfo::default();
        let mut errors = Vec::new();
        let info = exactly_one_child(node, &parse_info, &mut errors)?;
        if !errors.is_empty() {
            return Err(StyleError::Invalid(CslError(errors)));
        }
        Ok(info)
    }
    pub fn independent_parent_id(&self) -> Option<String> {
        self.parent.as_ref().map(|p| p.href.to_string())
    }
}

/// Something is Independent if what it represents is computed during processing, based on a Cite
/// and the rest of a document. That is, it is not sourced directly from a Reference.
pub trait IsIndependent {
    fn is_independent(&self) -> bool;
}

impl AttrChecker for Formatting {
    fn filter_attribute(attr: &str) -> bool {
        attr == "font-style"
            || attr == "font-variant"
            || attr == "font-weight"
            || attr == "vertical-align"
            || attr == "text-decoration"
    }
}

impl FromNode for Affixes {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(Affixes {
            prefix: SmartString::attribute_default(node, "prefix", info)?,
            suffix: SmartString::attribute_default(node, "suffix", info)?,
        })
    }
}

impl FromNode for RangeDelimiter {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(RangeDelimiter(SmartString::attribute_default_with(
            node,
            "range-delimiter",
            info,
            || "\u{2013}".into(),
        )?))
    }
}

impl AttrChecker for RangeDelimiter {
    fn filter_attribute(attr: &str) -> bool {
        attr == "range-delimiter"
    }
}

impl AttrChecker for Affixes {
    fn filter_attribute(attr: &str) -> bool {
        attr == "prefix" || attr == "suffix"
    }
}

impl FromNode for Formatting {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(Formatting {
            font_style: attribute_option(node, "font-style", info)?,
            font_variant: attribute_option(node, "font-variant", info)?,
            font_weight: attribute_option(node, "font-weight", info)?,
            text_decoration: attribute_option(node, "text-decoration", info)?,
            vertical_alignment: attribute_option(node, "vertical-align", info)?,
            // TODO: carry options from root
            // hyperlink: String::from(""),
        })
    }
}

impl FromNode for Citation {
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("citation")
    }
    const CHILD_DESC: &'static str = "citation";
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        // TODO: remove collect() using Peekable
        let layouts: Vec<_> = node
            .children()
            .filter(|n| n.has_tag_name("layout"))
            .collect();
        if layouts.len() != 1 {
            return Err(
                InvalidCsl::new(node, "<citation> must contain exactly one <layout>").into(),
            );
        }
        let layout_node = layouts[0];
        let sorts: Vec<_> = node.children().filter(|n| n.has_tag_name("sort")).collect();
        if sorts.len() > 1 {
            return Err(InvalidCsl::new(node, "<citation> can only contain one <sort>").into());
        }
        let sort = if sorts.is_empty() {
            None
        } else {
            Some(Sort::from_node(&sorts[0], info)?)
        };
        Ok(Citation {
            disambiguate_add_names: bool::attribute_default_val(
                node,
                "disambiguate-add-names",
                info,
                false,
            )?,
            disambiguate_add_givenname: bool::attribute_default_val(
                node,
                "disambiguate-add-givenname",
                info,
                false,
            )?,
            givenname_disambiguation_rule: attribute_optional(
                node,
                "givenname-disambiguation-rule",
                info,
            )?,
            disambiguate_add_year_suffix: bool::attribute_default_val(
                node,
                "disambiguate-add-year-suffix",
                info,
                false,
            )?,
            layout: Layout::from_node(&layout_node, info)?,
            name_inheritance: Name::from_node(&node, info)?,
            names_delimiter: attribute_option(node, "names-delimiter", info)?,
            near_note_distance: attribute_option_int(node, "near-note-distance")?.unwrap_or(5),
            cite_group_delimiter: attribute_option(node, "cite-group-delimiter", info)?,
            year_suffix_delimiter: attribute_option(node, "year-suffix-delimiter", info)?,
            after_collapse_delimiter: attribute_option(node, "after-collapse-delimiter", info)?,
            collapse: attribute_option(node, "collapse", info)?,
            sort,
        })
    }
}

impl FromNode for Sort {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(Sort {
            keys: node
                .children()
                .filter(|n| n.has_tag_name("key"))
                .map(|node| SortKey::from_node(&node, info))
                .partition_results()?,
        })
    }
}

impl FromNode for SortKey {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(SortKey {
            sort_source: SortSource::from_node(node, info)?,
            names_min: attribute_option_int(node, "names-min")?,
            names_use_first: attribute_option_int(node, "names-use-first")?,
            names_use_last: attribute_option(node, "names-use-last", info)?,
            direction: attribute_option(node, "sort", info)?,
        })
    }
}

impl FromNode for SortSource {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let macro_ = node.attribute("macro");
        let variable = node.attribute("variable");
        let err = "<key> must have either a `macro` or `variable` attribute";
        match (macro_, variable) {
            (Some(mac), None) => {
                let mac: SmartString = mac.into();
                if !info.macros.as_ref().map_or(false, |ms| ms.contains(&mac)) {
                    return Err(
                        InvalidCsl::new(node, format!("macro `{}` not defined", mac)).into(),
                    );
                }
                Ok(SortSource::Macro(mac))
            }
            (None, Some(_)) => Ok(SortSource::Variable(attribute_var_type(
                node,
                "variable",
                NeedVarType::Any,
                info,
            )?)),
            _ => Err(InvalidCsl::new(node, err).into()),
        }
    }
}

impl FromNode for InText {
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("intext")
    }
    const CHILD_DESC: &'static str = "intext";
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let mut errors = Vec::new();
        let layout = exactly_one_child::<Layout>(node, &info, &mut errors);
        if !errors.is_empty() {
            return Err(CslError(errors));
        }
        Ok(InText {
            layout: layout?,
            cite_group_delimiter: attribute_option(node, "cite-group-delimiter", info)?,
            and: attribute_option(node, "and", info)?,
            after_collapse_delimiter: attribute_option(node, "after-collapse-delimiter", info)?,
        })
    }
}

impl FromNode for Bibliography {
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("bibliography")
    }
    const CHILD_DESC: &'static str = "bibliography";
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        // TODO: layouts matching locales in CSL-M mode
        // TODO: make sure that all elements are under the control of a display attribute
        //       if any of them are
        let layouts: Vec<_> = node
            .children()
            .filter(|n| n.has_tag_name("layout"))
            .collect();
        if layouts.len() != 1 {
            return Err(
                InvalidCsl::new(node, "<citation> must contain exactly one <layout>").into(),
            );
        }
        let layout_node = layouts[0];
        let line_spacing = attribute_int(node, "line-spacing", 1)?;
        if line_spacing < 1 {
            return Err(InvalidCsl::new(node, "line-spacing must be >= 1").into());
        }
        let entry_spacing = attribute_int(node, "entry-spacing", 1)?;
        let sorts: Vec<_> = node.children().filter(|n| n.has_tag_name("sort")).collect();
        if sorts.len() > 1 {
            return Err(InvalidCsl::new(node, "<bibliography> can only contain one <sort>").into());
        }
        let sort = if sorts.is_empty() {
            None
        } else {
            Some(Sort::from_node(&sorts[0], info)?)
        };
        Ok(Bibliography {
            sort,
            layout: Layout::from_node(&layout_node, info)?,
            hanging_indent: bool::attribute_default_val(node, "hanging-indent", info, false)?,
            second_field_align: attribute_option(node, "second-field-align", info)?,
            line_spacing,
            entry_spacing,
            name_inheritance: Name::from_node(&node, info)?,
            subsequent_author_substitute: attribute_option(
                node,
                "subsequent-author-substitute",
                info,
            )?,
            subsequent_author_substitute_rule: attribute_optional(
                node,
                "subsequent-author-substitute-rule",
                info,
            )?,
            names_delimiter: attribute_option(node, "names-delimiter", info)?,
        })
    }
}

impl FromNode for Layout {
    const CHILD_DESC: &'static str = "layout";
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("layout")
    }
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let elements = node
            .children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el, info))
            .partition_results()?;
        Ok(Layout {
            formatting: Option::from_node(node, info)?,
            affixes: Option::from_node(node, info)?,
            delimiter: attribute_option(node, "delimiter", info)?,
            locale: attribute_array(node, "locale", info)?,
            elements,
        })
    }
}

impl TextTermSelector {
    fn from_term_and_form<E, FRT, FRTE, TO>(
        term: &AnyTermName,
        read_term_form: FRT,
        read_term_form_extended: FRTE,
        throw_ordinal: TO,
    ) -> Result<Self, E>
    where
        FRT: Fn() -> Result<TermForm, E>,
        FRTE: Fn() -> Result<TermFormExtended, E>,
        TO: Fn() -> E,
    {
        use self::terms::AnyTermName::*;
        match *term {
            Number(v) => Ok(TextTermSelector::Gendered(GenderedTermSelector::Number(
                v,
                read_term_form()?,
            ))),
            Month(t) => Ok(TextTermSelector::Gendered(GenderedTermSelector::Month(
                t,
                read_term_form()?,
            ))),
            Season(t) => Ok(TextTermSelector::Gendered(GenderedTermSelector::Season(
                t,
                read_term_form()?,
            ))),
            Loc(t) => Ok(TextTermSelector::Gendered(GenderedTermSelector::Locator(
                t,
                read_term_form()?,
            ))),
            Misc(t) => Ok(TextTermSelector::Simple(SimpleTermSelector::Misc(
                t,
                read_term_form_extended()?,
            ))),
            Category(t) => Ok(TextTermSelector::Simple(SimpleTermSelector::Category(
                t,
                read_term_form()?,
            ))),
            Quote(t) => Ok(TextTermSelector::Simple(SimpleTermSelector::Quote(t))),
            Role(t) => Ok(TextTermSelector::Role(RoleTermSelector(
                t,
                read_term_form_extended()?,
            ))),
            Ordinal(_) => Err(throw_ordinal()),
        }
    }

    pub fn from_term_form_unwrap(term: &str, form: Option<&str>, features: &Features) -> Self {
        let term =
            AnyTermName::get_attr(term, features).expect("Could not parse input term as a term.");
        let ordinal = || panic!("ordinal terms not accessible");
        if let Some(form) = form {
            let term_form = || {
                Ok(TermForm::get_attr(form, features)
                    .expect("Could not parse input term as a term."))
            };
            let term_form_extended = || {
                Ok(TermFormExtended::get_attr(form, features)
                    .expect("Could not parse input term as a term."))
            };
            TextTermSelector::from_term_and_form(&term, term_form, term_form_extended, ordinal)
                .unwrap()
        } else {
            TextTermSelector::from_term_and_form(
                &term,
                || Ok(Default::default()),
                || Ok(Default::default()),
                ordinal,
            )
            .unwrap()
        }
    }
}

impl FromNode for TextTermSelector {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        // we already know term is on there
        let term = attribute_required(node, "term", info)?;
        Ok(TextTermSelector::from_term_and_form(
            &term,
            || TermForm::from_node(node, info),
            || TermFormExtended::from_node(node, info),
            || InvalidCsl::new(node, "you cannot render an ordinal term directly").into(),
        )?)
    }
}

impl FromNode for LabelElement {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(LabelElement {
            variable: attribute_var_type(node, "variable", NeedVarType::NumberVariable, info)?,
            form: attribute_optional(node, "form", info)?,
            formatting: Option::from_node(node, info)?,
            affixes: Option::from_node(node, info)?,
            strip_periods: bool::attribute_default_val(node, "strip-periods", info, false)?,
            text_case: TextCase::from_node(node, info)?,
            plural: attribute_optional(node, "plural", info)?,
        })
    }
}

impl FromNode for TextElement {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let macro_ = node.attribute("macro");
        let value = node.attribute("value");
        let variable = node.attribute("variable");
        let term = node.attribute("term");
        let invalid = "<text> without a `variable`, `macro`, `term` or `value` is invalid";

        let source = match (macro_, value, variable, term) {
            (Some(mac), None, None, None) => {
                let mac: SmartString = mac.into();
                if !info.macros.as_ref().map_or(false, |ms| ms.contains(&mac)) {
                    return Err(
                        InvalidCsl::new(node, format!("macro `{}` not defined", mac)).into(),
                    );
                }
                TextSource::Macro(mac)
            }
            (None, Some(val), None, None) => TextSource::Value(val.into()),
            (None, None, Some(_vv), None) => TextSource::Variable(
                attribute_var_type(node, "variable", NeedVarType::TextVariable, info)?,
                attribute_optional(node, "form", info)?,
            ),
            (None, None, None, Some(_tt)) => TextSource::Term(
                TextTermSelector::from_node(node, info)?,
                bool::attribute_default_val(node, "plural", info, false)?,
            ),
            _ => return Err(InvalidCsl::new(node, invalid).into()),
        };

        let formatting = Option::from_node(node, info)?;
        let affixes = Option::from_node(node, info)?;
        let quotes = bool::attribute_default_val(node, "quotes", info, false)?;
        let strip_periods = bool::attribute_default_val(node, "strip-periods", info, false)?;
        let text_case = TextCase::from_node(node, info)?;
        let display = attribute_option(node, "display", info)?;

        Ok(TextElement {
            source,
            formatting,
            affixes,
            quotes,
            strip_periods,
            text_case,
            display,
        })
    }
}

impl FromNode for NumberElement {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(NumberElement {
            variable: attribute_var_type(node, "variable", NeedVarType::NumberVariable, info)?,
            form: attribute_optional(node, "form", info)?,
            formatting: Option::from_node(node, info)?,
            affixes: Option::from_node(node, info)?,
            text_case: attribute_optional(node, "text-case", info)?,
            display: attribute_option(node, "display", info)?,
        })
    }
}

impl FromNode for Group {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let elements = node
            .children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el, info))
            .partition_results()?;
        Ok(Group {
            elements,
            formatting: Option::from_node(node, info)?,
            delimiter: attribute_option(node, "delimiter", info)?,
            affixes: Option::from_node(node, info)?,
            display: attribute_option(node, "display", info)?,
            // TODO: CSL-M only
            is_parallel: bool::attribute_default_val(node, "is-parallel", info, false)?,
        })
    }
}

impl FromNode for Else {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let elements = node
            .children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el, info))
            .partition_results()?;
        Ok(Else(elements))
    }
}

impl FromNode for Match {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(attribute_optional(node, "match", info)?)
    }
}

#[derive(Debug)]
enum ConditionError {
    Unconditional(InvalidCsl),
    Invalid(CslError),
}

impl ConditionError {
    fn into_inner(self) -> CslError {
        match self {
            ConditionError::Unconditional(e) => CslError(vec![e]),
            ConditionError::Invalid(e) => e,
        }
    }
}

impl From<InvalidCsl> for ConditionError {
    fn from(err: InvalidCsl) -> Self {
        ConditionError::Invalid(CslError::from(err))
    }
}

impl From<CslError> for ConditionError {
    fn from(err: CslError) -> Self {
        ConditionError::Invalid(err)
    }
}

impl From<Vec<CslError>> for ConditionError {
    fn from(err: Vec<CslError>) -> Self {
        ConditionError::Invalid(CslError::from(err))
    }
}

impl ConditionParser {
    fn from_node_custom(node: &Node, info: &ParseInfo) -> Result<Self, ConditionError> {
        let (has_year_only, has_month_or_season, has_day) = if info.features.condition_date_parts {
            (
                attribute_array_var(node, "has-year-only", NeedVarType::CondDate, info)?,
                attribute_array_var(node, "has-month-or-season", NeedVarType::CondDate, info)?,
                attribute_array_var(node, "has-day", NeedVarType::CondDate, info)?,
            )
        } else {
            Default::default()
        };
        let cond = ConditionParser {
            match_type: Match::from_node(node, info)?,
            jurisdiction: attribute_option(node, "jurisdiction", info)?,
            subjurisdictions: attribute_option_int(node, "subjurisdictions")?,
            context: attribute_option(node, "context", info)?,
            disambiguate: bool::attribute_option(node, "disambiguate", info)?,
            variable: attribute_array_var(node, "variable", NeedVarType::Any, info)?,
            position: attribute_array_var(node, "position", NeedVarType::CondPosition, info)?,
            is_plural: attribute_array_var(node, "is-plural", NeedVarType::CondIsPlural, info)?,
            csl_type: attribute_array_var(node, "type", NeedVarType::CondType, info)?,
            locator: attribute_array_var(node, "locator", NeedVarType::CondLocator, info)?,
            is_uncertain_date: attribute_array_var(
                node,
                "is-uncertain-date",
                NeedVarType::CondDate,
                info,
            )?,
            is_numeric: attribute_array_var(node, "is-numeric", NeedVarType::Any, info)?,
            has_year_only,
            has_month_or_season,
            has_day,
        };
        // technically, only a match="..." on an <if> is ignored when a <conditions> block is
        // present, but that's ok
        if cond.is_empty() {
            Err(ConditionError::Unconditional(InvalidCsl::new(
                node,
                "Unconditional <choose> branch",
            )))
        } else {
            Ok(cond)
        }
    }
}

impl FromNode for Conditions {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let match_type = attribute_required(node, "match", info)?;
        let conds = node
            .children()
            .filter(|n| n.has_tag_name("condition"))
            .map(|el| CondSet::from_node_custom(&el, info).map_err(|e| e.into_inner()))
            .partition_results()?;
        if conds.is_empty() {
            Err(InvalidCsl::new(node, "Unconditional <choose> branch").into())
        } else {
            Ok(Conditions(match_type, conds))
        }
    }
}

impl CondSet {
    fn from_node_custom(node: &Node, info: &ParseInfo) -> Result<Self, ConditionError> {
        ConditionParser::from_node_custom(node, info).map(CondSet::from)
    }
}

// TODO: need context to determine if the CSL-M syntax can be used
impl FromNode for IfThen {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let tag = "if or else-if";

        // CSL 1.0.1 <if match="MMM" vvv ></if> equivalent to
        // CSL-M <if><conditions match="all"><condition match="MMM" vvv /></conditions></if>

        // these are 'inline' ones directly on an if / if-else node
        let own_conditions: Result<Conditions, ConditionError> =
            CondSet::from_node_custom(node, info).map(|c| Conditions(Match::All, vec![c]));

        // these are child nodes
        let sub_conditions: Result<Option<Conditions>, CslError> = if info.features.conditions {
            // TODO: only accept <conditions> in head position
            max1_child(tag, "conditions", node.children(), info)
        } else if let Some(invalid) = node
            .children()
            .filter(|n| n.has_tag_name("conditions"))
            .nth(0)
        {
            Err(InvalidCsl::new(
                &invalid,
                "You must opt-in to the `conditions` feature to use <conditions>",
            )
            .into())
        } else {
            Ok(None)
        };

        use self::ConditionError::*;

        let conditions: Conditions = (match (own_conditions, sub_conditions) {
            // just an if block
            (Ok(own), Ok(None)) => Ok(own),
            // just an if block, that failed
            (Err(e), Ok(None)) => Err(e.into_inner()),
            // no conds on if block, but error in <conditions>
            (Err(Unconditional(_)), Err(esub)) => Err(esub),
            // no conds on if block, but <conditions> present
            (Err(Unconditional(_)), Ok(Some(sub))) => Ok(sub),
            // if block has conditions, and <conditions> was also present
            (Err(Invalid(_)), Ok(Some(_)))
            | (Err(Invalid(_)), Err(_))
            | (Ok(_), Ok(Some(_)))
            | (Ok(_), Err(_)) => {
                return Err(InvalidCsl::new(
                    node,
                    &format!(
                        "{} can only have its own conditions OR a <conditions> block",
                        tag
                    ),
                )
                .into())
            }
        })?;
        let elements = node
            .children()
            .filter(|n| n.is_element() && !n.has_tag_name("conditions"))
            .map(|el| Element::from_node(&el, info))
            .partition_results()?;
        Ok(IfThen(conditions, elements))
    }
}

fn choose_el(node: &Node, info: &ParseInfo) -> Result<Element, CslError> {
    let mut if_block: Option<IfThen> = None;
    let mut elseifs = vec![];
    let mut else_block = Else(vec![]);
    let mut seen_if = false;
    let mut seen_else = false;

    let unrecognised = |el, tag| {
        if tag == "if" || tag == "else-if" || tag == "else" {
            return Err(InvalidCsl::new(
                el,
                &format!(
                    "<choose> elements out of order; found <{}> in wrong position",
                    tag
                ),
            )
            .into());
        }
        Err(InvalidCsl::new(el, &format!("Unrecognised element {} in <choose>", tag)).into())
    };

    for el in node.children().filter(|n| n.is_element()) {
        // TODO: figure out why doing this without a clone causes 'borrowed value does not
        // live long enough' problems.
        let tn = el.tag_name();
        let tag = tn.name().to_owned();
        if !seen_if {
            if tag == "if" {
                seen_if = true;
                if_block = Some(IfThen::from_node(&el, info)?);
            } else {
                return Err(InvalidCsl::new(&el, "<choose> blocks must begin with an <if>").into());
            }
        } else if !seen_else {
            if tag == "else-if" {
                elseifs.push(IfThen::from_node(&el, info)?);
            } else if tag == "else" {
                seen_else = true;
                else_block = Else::from_node(&el, info)?;
            } else {
                return unrecognised(&el, tag);
            }
        } else {
            return unrecognised(&el, tag);
        }
    }

    let _if = if_block.ok_or_else(|| InvalidCsl::new(node, "<choose> blocks must have an <if>"))?;

    Ok(Element::Choose(Arc::new(Choose(_if, elseifs, else_block))))
}

pub(crate) fn max1_child<T: FromNode>(
    parent_tag: &str,
    child_tag: &str,
    els: Children,
    info: &ParseInfo,
) -> Result<Option<T>, CslError> {
    // TODO: remove the allocation here, with a cloned iterator / peekable
    let subst_els: Vec<_> = els.filter(|n| n.has_tag_name(child_tag)).collect();
    if subst_els.len() > 1 {
        return Err(InvalidCsl::new(
            &subst_els[1],
            &format!(
                "There can only be one <{}> in a <{}> block.",
                child_tag, parent_tag
            ),
        )
        .into());
    }
    let substs = subst_els
        .iter()
        .map(|el| T::from_node(&el, info))
        .partition_results()?;
    let substitute = substs.into_iter().nth(0);
    Ok(substitute)
}

impl AttrChecker for TextCase {
    fn filter_attribute(attr: &str) -> bool {
        attr == "text-case"
    }
}

impl<T> AttrChecker for Option<T>
where
    T: AttrChecker,
{
    fn filter_attribute(attr: &str) -> bool {
        T::filter_attribute(attr)
    }
    fn filter_attribute_full(attr: &roxmltree::Attribute) -> bool {
        T::filter_attribute_full(attr)
    }
}

impl FromNode for TextCase {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(attribute_optional(node, "text-case", info)?)
    }
}

fn disallow_default<T: Default + FromNode + AttrChecker>(
    node: &Node,
    disallow: bool,
    info: &ParseInfo,
) -> Result<T, CslError> {
    if disallow {
        if T::is_on_node(node) {
            Err(InvalidCsl::new(
                node,
                &format!(
                    "Disallowed attribute on node: {:?}",
                    T::relevant_attrs(node)
                ),
            )
            .into())
        } else {
            Ok(T::default())
        }
    } else {
        T::from_node(node, info)
    }
}

impl DatePart {
    fn from_node_dp(node: &Node, full: bool, info: &ParseInfo) -> FromNodeResult<Self> {
        let name: DatePartName = attribute_required(node, "name", info)?;
        let form = match name {
            DatePartName::Year => DatePartForm::Year(attribute_optional(node, "form", info)?),
            DatePartName::Month => DatePartForm::Month(
                attribute_optional(node, "form", info)?,
                bool::attribute_default_val(node, "strip-periods", info, false)?,
            ),
            DatePartName::Day => DatePartForm::Day(attribute_optional(node, "form", info)?),
        };
        Ok(DatePart {
            form,
            // affixes not allowed in a locale date
            affixes: disallow_default(node, !full, info)?,
            formatting: Option::from_node(node, info)?,
            text_case: Option::from_node(node, info)?,
            range_delimiter: Option::from_node(node, info)?,
        })
    }
}

impl FromNode for IndependentDate {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let elements = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("date-part"))
            .map(|el| DatePart::from_node_dp(&el, true, info))
            .partition_results()?;
        Ok(IndependentDate {
            variable: attribute_var_type(node, "variable", NeedVarType::Date, info)?,
            date_parts: elements,
            text_case: TextCase::from_node(node, info)?,
            affixes: Option::from_node(node, info)?,
            formatting: Option::from_node(node, info)?,
            display: attribute_option(node, "display", info)?,
            delimiter: attribute_option(node, "delimiter", info)?,
        })
    }
}

impl FromNode for LocalizedDate {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let elements = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("date-part"))
            // no affixes if you're calling a locale date
            .map(|el| DatePart::from_node_dp(&el, false, info))
            .partition_results()?;
        Ok(LocalizedDate {
            variable: attribute_var_type(node, "variable", NeedVarType::Date, info)?,
            parts_selector: attribute_optional(node, "date-parts", info)?,
            date_parts: elements,
            form: attribute_required(node, "form", info)?,
            affixes: Option::from_node(node, info)?,
            formatting: Option::from_node(node, info)?,
            display: attribute_option(node, "display", info)?,
            text_case: TextCase::from_node(node, info)?,
        })
    }
}

impl FromNode for BodyDate {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        if node.has_attribute("form") {
            Ok(BodyDate::Local(LocalizedDate::from_node(node, info)?))
        } else {
            Ok(BodyDate::Indep(IndependentDate::from_node(node, info)?))
        }
    }
}

impl FromNode for Element {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        match node.tag_name().name() {
            "text" => Ok(Element::Text(TextElement::from_node(node, info)?)),
            "label" => Ok(Element::Label(LabelElement::from_node(node, info)?)),
            "number" => Ok(Element::Number(NumberElement::from_node(node, info)?)),
            "group" => Ok(Element::Group(Group::from_node(node, info)?)),
            "names" => Ok(Element::Names(Arc::new(Names::from_node(node, info)?))),
            "choose" => Ok(choose_el(node, info)?),
            "date" => Ok(Element::Date(Arc::new(BodyDate::from_node(node, info)?))),
            _ => Err(InvalidCsl::new(node, "Unrecognised node.").into()),
        }
    }
}

impl FromNode for MacroMap {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let elements: Result<Vec<_>, _> = node
            .children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el, info))
            .collect();
        let name = match node.attribute("name") {
            Some(n) => n,
            None => {
                return Err(InvalidCsl::new(node, "<macro> must have a `name` attribute.").into());
            }
        };
        Ok(MacroMap {
            name: name.into(),
            elements: elements?,
        })
    }
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("macro")
    }
    const CHILD_DESC: &'static str = "macro";
}

struct MacroHeader {
    name: SmartString,
}

impl FromNode for MacroHeader {
    fn from_node(node: &Node, _info: &ParseInfo) -> FromNodeResult<Self> {
        let name = match node.attribute("name") {
            Some(n) => n,
            None => {
                return Err(InvalidCsl::new(node, "<macro> must have a `name` attribute.").into());
            }
        };
        Ok(MacroHeader { name: name.into() })
    }
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("macro")
    }
    const CHILD_DESC: &'static str = "macro";
}

fn write_slot_once<T: FromNode>(
    el: &Node,
    info: &ParseInfo,
    slot: &mut Option<T>,
) -> FromNodeResult<()> {
    if slot.is_some() {
        return Err(InvalidCsl::new(
            &el,
            &format!(
                "There can only be one <{}> in a <names> block.",
                el.tag_name().name(),
            ),
        )
        .into());
    }
    let t = T::from_node(el, info)?;
    *slot = Some(t);
    Ok(())
}

impl FromNode for Names {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let mut name = None;
        let mut label: Option<NameLabelInput> = None;
        let mut et_al = None;
        let mut with = None;
        let mut institution = None;
        let mut substitute = None;
        for child in node.children().filter(|child| child.is_element()) {
            let tag_name = child.tag_name().name();
            match tag_name {
                "name" => write_slot_once(&child, info, &mut name)?,
                "institution" => write_slot_once(&child, info, &mut institution)?,
                "et-al" => write_slot_once(&child, info, &mut et_al)?,
                "label" => {
                    write_slot_once(&child, info, &mut label)?;
                    if let Some(ref mut label) = label {
                        label.after_name = name.is_some();
                    }
                }
                "with" => write_slot_once(&child, info, &mut with)?,
                "substitute" => write_slot_once(&child, info, &mut substitute)?,
                _ => {
                    return Err(InvalidCsl::unknown_element(node, &child).into());
                }
            }
        }

        Ok(Names {
            variables: attribute_array_var(node, "variable", NeedVarType::Name, info)?,
            name,
            institution,
            with,
            et_al,
            label,
            substitute,
            affixes: Option::from_node(node, info)?,
            formatting: Option::from_node(node, info)?,
            display: attribute_option(node, "display", info)?,
            delimiter: attribute_option(node, "delimiter", info)?,
        })
    }
}

impl FromNode for Institution {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        use crate::style::InstitutionUseFirst::*;
        let uf = node.attribute("use-first");
        let suf = node.attribute("substitute-use-first");
        let invalid = "<institution> may only use one of `use-first` or `substitute-use-first`";
        let use_first = match (uf, suf) {
            (Some(_), None) => Some(Normal(attribute_int(node, "use-first", 1)?)),
            (None, Some(_)) => Some(Substitute(attribute_int(node, "substitute-use-first", 1)?)),
            (None, None) => None,
            _ => return Err(InvalidCsl::new(node, invalid).into()),
        };

        let institution_parts = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("institution-part"))
            .map(|el| InstitutionPart::from_node(&el, info))
            .partition_results()?;

        Ok(Institution {
            and: attribute_option(node, "and", info)?,
            delimiter: attribute_option(node, "delimiter", info)?,
            use_first,
            use_last: attribute_option_int(node, "use-last")?,
            reverse_order: bool::attribute_default_val(node, "reverse-order", info, false)?,
            parts_selector: attribute_optional(node, "institution-parts", info)?,
            institution_parts,
        })
    }
}

impl FromNode for InstitutionPart {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(InstitutionPart {
            name: InstitutionPartName::from_node(node, info)?,
            formatting: Option::from_node(node, info)?,
            affixes: Option::from_node(node, info)?,
            strip_periods: bool::attribute_default_val(node, "strip-periods", info, false)?,
        })
    }
}

impl FromNode for InstitutionPartName {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        match node.attribute("name") {
            Some("long") => Ok(InstitutionPartName::Long(bool::attribute_default_val(
                node, "if-short", info, false,
            )?)),
            Some("short") => Ok(InstitutionPartName::Short),
            Some(ref val) => Err(InvalidCsl::attr_val(node, "name", val).into()),
            None => Err(InvalidCsl::missing(node, "name").into()),
        }
    }
}

impl FromNode for Name {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        // for inheriting from cs:style/cs:citation/cs:bibliography
        let mut delim_attr = "delimiter";
        let mut form_attr = "form";
        let mut name_part_given = None;
        let mut name_part_family = None;
        if node.tag_name().name() != "name" {
            delim_attr = "name-delimiter";
            form_attr = "name-form";
        } else {
            let parts = |val| {
                node.children()
                    .filter(move |el| {
                        el.is_element()
                            && el.has_tag_name("name-part")
                            && el.attribute("name") == Some(val)
                    })
                    .map(|el| NamePart::from_node(&el, info))
                    .filter_map(|np| np.ok())
            };
            name_part_given = parts("given").nth(0);
            name_part_family = parts("family").nth(0);
        }
        Ok(Name {
            and: attribute_option(node, "and", info)?,
            delimiter: attribute_option(node, delim_attr, info)?,
            delimiter_precedes_et_al: attribute_option(node, "delimiter-precedes-et-al", info)?,
            delimiter_precedes_last: attribute_option(node, "delimiter-precedes-last", info)?,
            et_al_min: attribute_option_int(node, "et-al-min")?,
            et_al_use_last: bool::attribute_option(node, "et-al-use-last", info)?,
            et_al_use_first: attribute_option_int(node, "et-al-use-first")?,
            et_al_subsequent_min: attribute_option_int(node, "et-al-subsequent-min")?,
            et_al_subsequent_use_first: attribute_option_int(node, "et-al-subsequent-use-first")?,
            form: attribute_option(node, form_attr, info)?,
            initialize: bool::attribute_option(node, "initialize", info)?,
            initialize_with: attribute_option(node, "initialize-with", info)?,
            name_as_sort_order: attribute_option(node, "name-as-sort-order", info)?,
            sort_separator: attribute_option(node, "sort-separator", info)?,
            formatting: Option::from_node(node, info)?,
            affixes: Option::from_node(node, info)?,
            name_part_given,
            name_part_family,
        })
    }
}

impl FromNode for NameEtAl {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(NameEtAl {
            term: attribute_string(node, "term"),
            formatting: Option::from_node(node, info)?,
        })
    }
}

impl FromNode for NameWith {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(NameWith {
            formatting: Option::from_node(node, info)?,
            affixes: Option::from_node(node, info)?,
        })
    }
}

impl FromNode for NamePart {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(NamePart {
            name: attribute_required(node, "name", info)?,
            text_case: TextCase::from_node(node, info)?,
            formatting: Option::from_node(node, info)?,
            affixes: Option::from_node(node, info)?,
        })
    }
}

impl FromNode for NameLabelInput {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(NameLabelInput {
            form: attribute_option(node, "form", info)?,
            plural: attribute_option(node, "plural", info)?,
            strip_periods: bool::attribute_option(node, "strip-periods", info)?,
            formatting: Option::from_node(node, info)?,
            affixes: Option::from_node(node, info)?,
            text_case: Option::from_node(node, info)?,
            // Context-dependent; we set this in Names::from_node()
            after_name: false,
        })
    }
}

impl FromNode for Substitute {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let els = node
            .children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el, info))
            .partition_results()?;
        Ok(Substitute(els))
    }
}

struct TextContent(Option<String>);

impl FromNode for TextContent {
    fn from_node(node: &Node, _info: &ParseInfo) -> FromNodeResult<Self> {
        let opt_s = node.text().and_then(|s| {
            if s.trim().is_empty() {
                None
            } else {
                let t = s.trim_matches('\n');
                if t.is_empty() {
                    None
                } else {
                    Some(String::from(t))
                }
            }
        });
        Ok(TextContent(opt_s))
    }
}

impl FromNode for TermPlurality {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let always: Option<String> = TextContent::from_node(node, info)?.0.map(|s| s.into());
        let single: Option<TextContent> = max1_child("term", "single", node.children(), info)?;
        let multiple: Option<TextContent> = max1_child("term", "multiple", node.children(), info)?;
        let msg = "<term> must contain either only text content or both <single> and <multiple>";
        match (always, single, multiple) {
            // empty term is valid
            (None, None, None) => Ok(TermPlurality::Invariant("".into())),
            // <term>plain text content</term>
            (Some(a), None, None) => Ok(TermPlurality::Invariant(a)),
            // <term> ANYTHING <single> s </single> <multiple> m </multiple></term>
            (_, Some(s), Some(m)) => Ok(TermPlurality::Pluralized {
                single: s.0.unwrap_or_else(|| "".into()),
                multiple: m.0.unwrap_or_else(|| "".into()),
            }),
            // had one of <single> or <multiple>, but not the other
            _ => Err(InvalidCsl::new(node, msg).into()),
        }
    }
}

impl OrdinalMatch {
    pub fn default_for(n: u32) -> Self {
        if n < 10 {
            OrdinalMatch::LastDigit
        } else {
            OrdinalMatch::LastTwoDigits
        }
    }
}

impl FromNode for TermFormExtended {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(attribute_optional(node, "form", info)?)
    }
}

impl FromNode for TermForm {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(attribute_optional(node, "form", info)?)
    }
}

impl FromNode for CslVersionReq {
    fn from_node(node: &Node, _info: &ParseInfo) -> FromNodeResult<Self> {
        let version = attribute_string(node, "version");
        let req = VersionReq::parse(&version).map_err(|_| {
            InvalidCsl::new(
                node,
                &format!("could not parse version string \"{}\"", &version),
            )
        })?;
        let supported = COMPILED_VERSION;
        if !req.matches(&supported) {
            return Err(InvalidCsl::new(
                node,
                &format!(
                    "Unsupported CSL version: \"{}\". This engine supports {}.",
                    req, supported
                ),
            )
            .into());
        }
        Ok(CslVersionReq(req))
    }
}

impl FromNode for CslCslMVersionReq {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let version = attribute_string(node, "version");
        let variant: CslVariant;
        let req = if version.ends_with("mlz1") {
            variant = CslVariant::CslM;
            VersionReq::parse(version.trim_end_matches("mlz1")).map_err(|_| {
                InvalidCsl::new(
                    node,
                    &"unsupported \"1.1mlz1\"-style version string (use variant=\"csl-m\" version=\"1.x\", for example)".to_string(),
                    )
            })?
        } else {
            // TODO: bootstrap attribute_optional with a dummy CslVariant::Csl
            variant = attribute_optional(node, "variant", info)?;
            VersionReq::parse(&version).map_err(|_| {
                InvalidCsl::new(
                    node,
                    &format!("could not parse version string \"{}\"", &version),
                )
            })?
        };
        let supported = match variant {
            CslVariant::Csl => COMPILED_VERSION,
            CslVariant::CslM => COMPILED_VERSION_M,
        };
        if !req.matches(&supported) {
            return Err(InvalidCsl::new(
                node,
                &format!(
                    "Unsupported version for variant {:?}: \"{}\". This engine supports {}.",
                    variant, req, supported
                ),
            )
            .into());
        }
        Ok(CslCslMVersionReq(variant, req))
    }
}

impl FromNode for Features {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let mut features = info
            .options
            .features
            .clone()
            .unwrap_or_else(Default::default);
        let input = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("feature"))
            .filter_map(|el| el.attribute("name"));
        read_features_into(input, &mut features)
            .map_err(|s| InvalidCsl::new(node, &format!("Unrecognised feature flag `{}`", s)))?;
        Ok(features)
    }
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("features")
    }
    const CHILD_DESC: &'static str = "features";
}

// type MacroDict = HashMap<SmartString, Vec<Element>>;
// fn check_recursion(macros: &MacroDict, layout: &Layout, cite_or_bib_node: &Node) -> FromNodeResult<()> {
//     let mut stack: Vec<SmartString> = Vec::new();
//     let mut check_element = |macros: &MacroDict, element: &Element| -> bool {
//         if let Element::Text(TextElement { source: TextSource::Macro(m), .. }) = &element {
//             if stack.contains(m) {
//                 Err(InvalidCsl::new(cite_or_bib_node, format!("Macros cannot be mutually recursive {:?}", stack)))
//             } else {
//                 stack.push(m.clone());
//                 Ok(())
//             }
//         }
//     };
// }

fn whitelist_child_nodes(node: &Node, whitelist: &[&str], errors: &mut Vec<InvalidCsl>) {
    node.children()
        .filter(|x| x.is_element() && !whitelist.contains(&x.tag_name().name()))
        .for_each(|unspec_node| errors.push(InvalidCsl::unknown_element(node, &unspec_node)))
}

impl Style {
    fn from_node_custom(
        node: &Node,
        default_info: &ParseInfo,
        info_block: Info,
    ) -> FromNodeResult<Self> {
        let version_req = CslVersionReq::from_node(node, default_info)?;
        let mut errors: Vec<InvalidCsl> = Vec::new();

        let whitelist_intext: &[&str] = &[
            "macro",        // 1
            "info",         // 2
            "features",     // 3
            "citation",     // 4
            "bibliography", // 5
            "locale",       // 6
            "intext",       // 7
        ];
        let whitelist: &[&str] = &whitelist_intext[..6];

        // Parse features first so we know how to interpret the rest.
        let features = max_one_child::<Features>(node, default_info, &mut errors)
            .ok()
            .flatten()
            .unwrap_or_else(|| {
                default_info
                    .options
                    .features
                    .clone()
                    .unwrap_or_else(Default::default)
            });

        whitelist_child_nodes(
            node,
            if features.custom_intext {
                whitelist_intext
            } else {
                whitelist
            },
            &mut errors,
        );

        // We will check again later (for MacroMap) if there are macros without names.
        let mut throwaway = Vec::new();
        let macro_headers = many_children::<MacroHeader>(node, default_info, &mut throwaway)
            .unwrap_or_else(|_| Vec::new());
        let macro_names = macro_headers
            .into_iter()
            .map(|header| header.name)
            .collect();

        // Create our own info struct, ignoring the one passed in.
        let parse_info = ParseInfo {
            options: default_info.options.clone(),
            features: features.clone(),
            macros: Some(macro_names),
        };

        let citation = exactly_one_child::<Citation>(node, &parse_info, &mut errors);
        let bibliography = max_one_child::<Bibliography>(node, &parse_info, &mut errors);
        let intext = if parse_info.features.custom_intext {
            max_one_child::<InText>(node, &parse_info, &mut errors)
        } else {
            Ok(None)
        };

        let mut macros = HashMap::default();
        let mut locale_overrides = FnvHashMap::default();

        let locales_res = many_children::<Locale>(node, &parse_info, &mut errors);
        if let Ok(locales) = locales_res {
            for loc in locales {
                locale_overrides.insert(loc.lang.clone(), loc);
            }
        }

        let macro_res = many_children::<MacroMap>(node, &parse_info, &mut errors);
        if let Ok(macro_maps) = macro_res {
            for mac in macro_maps {
                macros.insert(mac.name, mac.elements);
            }
        }

        if !errors.is_empty() {
            return Err(CslError(errors));
        }

        Ok(Style {
            macros,
            version_req,
            locale_overrides,
            features,
            info: info_block,
            citation: citation?,
            bibliography: bibliography?,
            intext: intext?,
            class: attribute_required(node, "class", &parse_info)?,
            default_locale: attribute_option(node, "default-locale", &parse_info)?,
            name_inheritance: Name::from_node(&node, &parse_info)?,
            page_range_format: attribute_option(node, "page-range-format", &parse_info)?,
            demote_non_dropping_particle: attribute_optional(
                node,
                "demote-non-dropping-particle",
                &parse_info,
            )?,
            initialize_with_hyphen: bool::attribute_default_val(
                node,
                "initialize-with-hyphen",
                &parse_info,
                true,
            )?,
            names_delimiter: attribute_option(node, "names-delimiter", &parse_info)?,
        })
    }
}
