//! Describes the `<style>` element and all its children, and parses it from an XML tree.

use std::sync::Arc;

pub(crate) mod attr;
pub mod element;
pub mod error;
pub mod terms;
pub mod variables;
pub mod version;

use self::attr::*;
use self::element::*;
use self::error::*;
use self::terms::*;
use self::version::*;
use crate::locale::*;
use crate::utils::PartitionArenaErrors;
use crate::Atom;
use fnv::FnvHashMap;
use roxmltree::{Children, Node};
use semver::VersionReq;

pub type FromNodeResult<T> = Result<T, CslError>;

pub trait FromNode
where
    Self: Sized,
{
    fn from_node(node: &Node) -> FromNodeResult<Self>;
}

pub trait AttrChecker
where
    Self: Sized,
{
    fn filter_attribute(attr: &str) -> bool;
    fn is_on_node<'a>(node: &'a Node) -> bool {
        node.attributes()
            .iter()
            .filter(|a| Self::filter_attribute(a.name()))
            .next()
            != None
    }
    fn relevant_attrs<'a>(node: &'a Node) -> Vec<String> {
        node.attributes()
            .iter()
            .filter(|a| Self::filter_attribute(a.name()))
            .map(|a| String::from(a.name()))
            .collect()
    }
}

impl AttrChecker for Formatting {
    fn filter_attribute(attr: &str) -> bool {
        attr == "font-style"
            || attr == "font-variant"
            || attr == "font-weight"
            || attr == "text-decoration"
            || attr == "vertical-alignment"
            || attr == "strip-periods"
    }
}

impl<T> FromNode for Option<T>
where
    T: AttrChecker + FromNode,
{
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        if T::is_on_node(node) {
            Ok(Some(T::from_node(node)?))
        } else {
            Ok(None)
        }
    }
}

impl FromNode for Affixes {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(Affixes {
            prefix: attribute_atom(node, "prefix"),
            suffix: attribute_atom(node, "suffix"),
        })
    }
}

impl FromNode for RangeDelimiter {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(RangeDelimiter(attribute_atom_default(
            node,
            "range-delimiter",
            "\u{2013}".into(),
        )))
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
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(Formatting {
            font_style: attribute_optional(node, "font-style")?,
            font_variant: attribute_optional(node, "font-variant")?,
            font_weight: attribute_optional(node, "font-weight")?,
            text_decoration: attribute_optional(node, "text-decoration")?,
            vertical_alignment: attribute_optional(node, "vertical-alignment")?,
            // TODO: carry options from root
            // hyperlink: String::from(""),
        })
    }
}

impl FromNode for Citation {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        // TODO: remove collect() using Peekable
        let layouts: Vec<_> = node
            .children()
            .filter(|n| n.has_tag_name("layout"))
            .collect();
        if layouts.len() != 1 {
            return Ok(Err(InvalidCsl::new(
                node,
                "<citation> must contain exactly one <layout>",
            ))?);
        }
        let layout_node = layouts[0];
        Ok(Citation {
            disambiguate_add_names: attribute_bool(node, "disambiguate-add-names", false)?,
            disambiguate_add_givenname: attribute_bool(node, "disambiguate-add-givenname", false)?,
            givenname_disambiguation_rule: attribute_optional(
                node,
                "givenname-disambiguation-rule",
            )?,
            disambiguate_add_year_suffix: attribute_bool(
                node,
                "disambiguate-add-year-suffix",
                false,
            )?,
            layout: Layout::from_node(&layout_node)?,
            name_inheritance: Name::from_node(&node)?,
            names_delimiter: node
                .attribute("names-delimiter")
                .map(Atom::from)
                .map(Delimiter),
        })
    }
}

impl FromNode for Sort {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(Sort {
            keys: node
                .children()
                .filter(|n| n.has_tag_name("key"))
                .map(|node| SortKey::from_node(&node))
                .partition_results()?,
        })
    }
}

impl FromNode for SortKey {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(SortKey {
            sort_source: SortSource::from_node(node)?,
            names_min: attribute_option_int(node, "names-min")?,
            names_use_first: attribute_option_int(node, "names-min")?,
            names_use_last: attribute_option_int(node, "names-min")?,
            sort: attribute_option(node, "sort")?,
        })
    }
}

impl FromNode for SortSource {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let macro_ = node.attribute("macro");
        let variable = node.attribute("variable");
        let err = "<key> must have either a `macro` or `variable` attribute";
        match (macro_, variable) {
            (Some(mac), None) => Ok(SortSource::Macro(mac.into())),
            (None, Some(_)) => Ok(SortSource::Variable(attribute_var_type(
                node,
                "variable",
                NeedVarType::Any,
            )?)),
            _ => Err(InvalidCsl::new(node, err).into()),
        }
    }
}

impl FromNode for Bibliography {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        // TODO: layouts matching locales in CSL-M mode
        // TODO: make sure that all elements are under the control of a display attribute
        //       if any of them are
        let layouts: Vec<_> = node
            .children()
            .filter(|n| n.has_tag_name("layout"))
            .collect();
        if layouts.len() != 1 {
            return Ok(Err(InvalidCsl::new(
                node,
                "<citation> must contain exactly one <layout>",
            ))?);
        }
        let layout_node = layouts[0];
        let line_spaces = attribute_int(node, "line-spaces", 1)?;
        if line_spaces < 1 {
            return Err(InvalidCsl::new(node, "line-spaces must be >= 1").into());
        }
        let entry_spacing = attribute_int(node, "entry-spacing", 1)?;
        let sorts: Vec<_> = node.children().filter(|n| n.has_tag_name("sort")).collect();
        if sorts.len() > 1 {
            return Ok(Err(InvalidCsl::new(
                node,
                "<bibliography> can only contain one <sort>",
            ))?);
        }
        let sort = if sorts.len() == 0 {
            None
        } else {
            Some(Sort::from_node(&sorts[0])?)
        };
        Ok(Bibliography {
            sort,
            layout: Layout::from_node(&layout_node)?,
            hanging_indent: attribute_bool(node, "hanging-indent", false)?,
            second_field_align: attribute_option(node, "second-field-align")?,
            line_spaces,
            entry_spacing,
            name_inheritance: Name::from_node(&node)?,
            subsequent_author_substitute: attribute_option_atom(
                node,
                "subsequent-author-substitute",
            ),
            subsequent_author_substitute_rule: attribute_optional(
                node,
                "subsequent-author-substitute-rule",
            )?,
            names_delimiter: node
                .attribute("names-delimiter")
                .map(Atom::from)
                .map(Delimiter),
        })
    }
}

impl FromNode for Delimiter {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(Delimiter(attribute_atom(node, "delimiter")))
    }
}

impl FromNode for Layout {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let elements = node
            .children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el))
            .partition_results()?;
        Ok(Layout {
            formatting: Option::from_node(node)?,
            affixes: Affixes::from_node(node)?,
            delimiter: Delimiter::from_node(node)?,
            locale: attribute_array(node, "locale")?,
            elements,
        })
    }
}

impl FromNode for TextTermSelector {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        use self::terms::AnyTermName::*;
        // we already know term is on there
        let t = attribute_required(node, "term")?;
        match t {
            Number(v) => Ok(TextTermSelector::Gendered(GenderedTermSelector::Number(
                v,
                TermForm::from_node(node)?,
            ))),
            Month(t) => Ok(TextTermSelector::Gendered(GenderedTermSelector::Month(
                t,
                TermForm::from_node(node)?,
            ))),
            Loc(t) => Ok(TextTermSelector::Gendered(GenderedTermSelector::Locator(
                t,
                TermForm::from_node(node)?,
            ))),
            Misc(t) => Ok(TextTermSelector::Simple(SimpleTermSelector::Misc(
                t,
                TermFormExtended::from_node(node)?,
            ))),
            Season(t) => Ok(TextTermSelector::Simple(SimpleTermSelector::Season(
                t,
                TermForm::from_node(node)?,
            ))),
            Quote(t) => Ok(TextTermSelector::Simple(SimpleTermSelector::Quote(
                t,
                TermForm::from_node(node)?,
            ))),
            Role(t) => Ok(TextTermSelector::Role(RoleTermSelector(
                t,
                TermFormExtended::from_node(node)?,
            ))),
            Ordinal(_) => {
                Err(InvalidCsl::new(node, "you cannot render an ordinal term directly").into())
            }
        }
    }
}

fn text_el(node: &Node) -> Result<Element, CslError> {
    let macro_ = node.attribute("macro");
    let value = node.attribute("value");
    let variable = node.attribute("variable");
    let term = node.attribute("term");
    let invalid = "<text> without a `variable`, `macro`, `term` or `value` is invalid";

    let source = match (macro_, value, variable, term) {
        (Some(mac), None, None, None) => TextSource::Macro(mac.into()),
        (None, Some(val), None, None) => TextSource::Value(val.into()),
        (None, None, Some(___), None) => TextSource::Variable(
            attribute_var_type(node, "variable", NeedVarType::TextVariable)?,
            attribute_optional(node, "form")?,
        ),
        (None, None, None, Some(___)) => TextSource::Term(
            TextTermSelector::from_node(node)?,
            attribute_bool(node, "plural", false)?,
        ),
        _ => return Err(InvalidCsl::new(node, invalid).into()),
    };

    let formatting = Option::from_node(node)?;
    let affixes = Affixes::from_node(node)?;
    let quotes = attribute_bool(node, "quotes", false)?;
    let strip_periods = attribute_bool(node, "strip-periods", false)?;
    let text_case = TextCase::from_node(node)?;
    let display = attribute_option(node, "display")?;

    Ok(Element::Text(
        source,
        formatting,
        affixes,
        quotes,
        strip_periods,
        text_case,
        display,
    ))
}

fn label_el(node: &Node) -> Result<Element, CslError> {
    Ok(Element::Label(
        attribute_var_type(node, "variable", NeedVarType::NumberVariable)?,
        attribute_optional(node, "form")?,
        Option::from_node(node)?,
        Affixes::from_node(node)?,
        attribute_bool(node, "strip-periods", false)?,
        TextCase::from_node(node)?,
        attribute_optional(node, "plural")?,
    ))
}

fn number_el(node: &Node) -> Result<Element, CslError> {
    Ok(Element::Number(
        attribute_var_type(node, "variable", NeedVarType::NumberVariable)?,
        attribute_optional(node, "form")?,
        Option::from_node(node)?,
        Affixes::from_node(node)?,
        attribute_optional(node, "plural")?,
        attribute_option(node, "display")?,
    ))
}

impl FromNode for Group {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let elements = node
            .children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el))
            .partition_results()?;
        Ok(Group {
            elements,
            formatting: Option::from_node(node)?,
            delimiter: Delimiter::from_node(node)?,
            affixes: Affixes::from_node(node)?,
            display: attribute_option(node, "display")?,
            // TODO: CSL-M only
            is_parallel: attribute_bool(node, "is-parallel", false)?,
        })
    }
}

impl FromNode for Else {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let elements = node
            .children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el))
            .partition_results()?;
        Ok(Else(elements))
    }
}

impl FromNode for Match {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(attribute_optional(node, "match")?)
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

impl Condition {
    fn from_node_custom(node: &Node) -> Result<Self, ConditionError> {
        let cond = Condition {
            match_type: Match::from_node(node)?,
            jurisdiction: attribute_option_atom(node, "jurisdiction"),
            subjurisdictions: attribute_option_int(node, "subjurisdictions")?,
            context: attribute_option(node, "context")?,
            disambiguate: attribute_only_true(node, "disambiguate")?,
            variable: attribute_array_var(node, "variable", NeedVarType::Any)?,
            position: attribute_array_var(node, "position", NeedVarType::CondPosition)?,
            is_plural: attribute_array_var(node, "is-plural", NeedVarType::CondIsPlural)?,
            csl_type: attribute_array_var(node, "type", NeedVarType::CondType)?,
            locator: attribute_array_var(node, "locator", NeedVarType::CondLocator)?,
            has_year_only: attribute_array_var(node, "has-year-only", NeedVarType::CondDate)?,
            has_day: attribute_array_var(node, "has-day", NeedVarType::CondDate)?,
            is_uncertain_date: attribute_array_var(
                node,
                "is-uncertain-date",
                NeedVarType::CondDate,
            )?,
            is_numeric: attribute_array_var(node, "is-numeric", NeedVarType::Any)?,
            has_to_month_or_season: attribute_array_var(
                node,
                "has-to-month-or-season",
                NeedVarType::CondDate,
            )?,
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
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let match_type = attribute_required(node, "match")?;
        let conds = node
            .children()
            .filter(|n| n.has_tag_name("condition"))
            .map(|el| Condition::from_node_custom(&el).map_err(|e| e.into_inner()))
            .partition_results()?;
        if conds.is_empty() {
            Err(InvalidCsl::new(node, "Unconditional <choose> branch").into())
        } else {
            Ok(Conditions(match_type, conds))
        }
    }
}

// TODO: need context to determine if the CSL-M syntax can be used
impl FromNode for IfThen {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let tag = "if or else-if";

        // CSL 1.0.1 <if match="MMM" vvv ></if> equivalent to
        // CSL-M <if><conditions match="all"><condition match="MMM" vvv /></conditions></if>
        let own_conditions: Result<Conditions, ConditionError> =
            Condition::from_node_custom(node).map(|c| Conditions(Match::All, vec![c]));

        // TODO: only accept <conditions> in head position
        let sub_conditions: Result<Option<Conditions>, CslError> =
            max1_child(tag, "conditions", node.children());

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
            | (Ok(_), Err(_)) => Ok(Err(InvalidCsl::new(
                node,
                &format!(
                    "{} can only have its own conditions OR a <conditions> block",
                    tag
                ),
            ))?),
        })?;
        let elements = node
            .children()
            .filter(|n| n.is_element() && !n.has_tag_name("conditions"))
            .map(|el| Element::from_node(&el))
            .partition_results()?;
        Ok(IfThen(conditions, elements))
    }
}

fn choose_el(node: &Node) -> Result<Element, CslError> {
    let mut if_block: Option<IfThen> = None;
    let mut elseifs = vec![];
    let mut else_block = Else(vec![]);
    let mut seen_if = false;
    let mut seen_else = false;

    let unrecognised = |el, tag| {
        if tag == "if" || tag == "else-if" || tag == "else" {
            return Ok(Err(InvalidCsl::new(
                el,
                &format!(
                    "<choose> elements out of order; found <{}> in wrong position",
                    tag
                ),
            ))?);
        }
        Ok(Err(InvalidCsl::new(
            el,
            &format!("Unrecognised element {} in <choose>", tag),
        ))?)
    };

    for el in node.children().filter(|n| n.is_element()) {
        // TODO: figure out why doing this without a clone causes 'borrowed value does not
        // live long enough' problems.
        let tn = el.tag_name();
        let tag = tn.name().to_owned();
        if !seen_if {
            if tag == "if" {
                seen_if = true;
                if_block = Some(IfThen::from_node(&el)?);
            } else {
                return Err(InvalidCsl::new(
                    &el,
                    "<choose> blocks must begin with an <if>",
                ))?;
            }
        } else if !seen_else {
            if tag == "else-if" {
                elseifs.push(IfThen::from_node(&el)?);
            } else if tag == "else" {
                seen_else = true;
                else_block = Else::from_node(&el)?;
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

fn max1_child<T: FromNode>(
    parent_tag: &str,
    child_tag: &str,
    els: Children,
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
        ))?;
    }
    let substs = subst_els
        .iter()
        .map(|el| T::from_node(&el))
        .partition_results()?;
    let substitute = substs.into_iter().nth(0);
    Ok(substitute)
}

impl AttrChecker for TextCase {
    fn filter_attribute(attr: &str) -> bool {
        attr == "text-case"
    }
}

impl FromNode for TextCase {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(attribute_optional(node, "text-case")?)
    }
}

fn disallow_default<T: Default + FromNode + AttrChecker>(
    node: &Node,
    disallow: bool,
) -> Result<T, CslError> {
    if disallow {
        if T::is_on_node(node) {
            Err(InvalidCsl::new(
                node,
                &format!(
                    "Disallowed attribute on node: {:?}",
                    T::relevant_attrs(node)
                ),
            ))?
        } else {
            Ok(T::default())
        }
    } else {
        T::from_node(node)
    }
}

impl DatePart {
    fn from_node_dp(node: &Node, full: bool) -> FromNodeResult<Self> {
        let name: DatePartName = attribute_required(node, "name")?;
        let form = match name {
            DatePartName::Year => DatePartForm::Year(attribute_optional(node, "form")?),
            DatePartName::Month => DatePartForm::Month(
                attribute_optional(node, "form")?,
                attribute_bool(node, "strip-periods", false)?,
            ),
            DatePartName::Day => DatePartForm::Day(attribute_optional(node, "form")?),
        };
        Ok(DatePart {
            form,
            // affixes not allowed in a locale date
            affixes: disallow_default(node, !full)?,
            formatting: Option::from_node(node)?,
            text_case: TextCase::from_node(node)?,
            range_delimiter: RangeDelimiter::from_node(node)?,
        })
    }
}

impl FromNode for IndependentDate {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let elements = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("date-part"))
            .map(|el| DatePart::from_node_dp(&el, true))
            .partition_results()?;
        Ok(IndependentDate {
            variable: attribute_var_type(node, "variable", NeedVarType::Date)?,
            date_parts: elements,
            text_case: TextCase::from_node(node)?,
            affixes: Affixes::from_node(node)?,
            formatting: Option::from_node(node)?,
            display: attribute_option(node, "display")?,
            delimiter: Delimiter::from_node(node)?,
        })
    }
}

impl FromNode for LocaleDate {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let elements = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("date-part"))
            .map(|el| DatePart::from_node_dp(&el, true))
            .partition_results()?;
        Ok(LocaleDate {
            form: attribute_required(node, "form")?,
            date_parts: elements,
            formatting: Option::from_node(node)?,
            delimiter: Delimiter::from_node(node)?,
            text_case: TextCase::from_node(node)?,
        })
    }
}

impl FromNode for LocalizedDate {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let elements = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("date-part"))
            // no affixes if you're calling a locale date
            .map(|el| DatePart::from_node_dp(&el, false))
            .partition_results()?;
        Ok(LocalizedDate {
            variable: attribute_var_type(node, "variable", NeedVarType::Date)?,
            parts_selector: attribute_optional(node, "date-parts")?,
            date_parts: elements,
            form: attribute_required(node, "form")?,
            affixes: Affixes::from_node(node)?,
            formatting: Option::from_node(node)?,
            display: attribute_option(node, "display")?,
            text_case: TextCase::from_node(node)?,
        })
    }
}

impl FromNode for BodyDate {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        if node.has_attribute("form") {
            Ok(BodyDate::Local(LocalizedDate::from_node(node)?))
        } else {
            Ok(BodyDate::Indep(IndependentDate::from_node(node)?))
        }
    }
}

impl FromNode for Element {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        match node.tag_name().name() {
            "text" => Ok(text_el(node)?),
            "label" => Ok(label_el(node)?),
            "group" => Ok(Element::Group(Group::from_node(node)?)),
            "number" => Ok(number_el(node)?),
            "names" => Ok(Element::Names(Arc::new(Names::from_node(node)?))),
            "choose" => Ok(choose_el(node)?),
            "date" => Ok(Element::Date(Arc::new(BodyDate::from_node(node)?))),
            _ => Err(InvalidCsl::new(node, "Unrecognised node."))?,
        }
    }
}

fn get_toplevel<'a, 'd: 'a>(
    root: &Node<'a, 'd>,
    nodename: &'static str,
) -> Result<Node<'a, 'd>, CslError> {
    // TODO: remove collect()
    let matches: Vec<_> = root
        .children()
        .filter(|n| n.has_tag_name(nodename))
        .collect();
    if matches.len() > 1 {
        Ok(Err(InvalidCsl::new(
            &root,
            &format!("Cannot have more than one <{}>", nodename),
        ))?)
    } else {
        // move matches into its first item
        Ok(matches
            .into_iter()
            .nth(0)
            .ok_or_else(|| InvalidCsl::new(&root, "Must have one <...>"))?)
    }
}

impl FromNode for MacroMap {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        // TODO: remove collect()
        let elements: Result<Vec<_>, _> = node
            .children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el))
            .collect();
        let name = match node.attribute("name") {
            Some(n) => n,
            None => {
                return Ok(Err(InvalidCsl::new(
                    node,
                    "Macro must have a 'name' attribute.",
                ))?);
            }
        };
        Ok(MacroMap {
            name: name.into(),
            elements: elements?,
        })
    }
}

impl FromNode for Names {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let name = max1_child("names", "name", node.children())?;
        let institution = max1_child("names", "institution", node.children())?;
        let et_al = max1_child("names", "et-al", node.children())?;
        let label = max1_child("names", "label", node.children())?;
        let with = max1_child("names", "with", node.children())?;
        let substitute = max1_child("names", "substitute", node.children())?;
        Ok(Names {
            variables: attribute_array_var(node, "variable", NeedVarType::Name)?,
            name,
            institution,
            with,
            et_al,
            label,
            substitute,
            affixes: Affixes::from_node(node)?,
            formatting: Option::from_node(node)?,
            display: attribute_option(node, "display")?,
            delimiter: node.attribute("delimiter").map(Atom::from).map(Delimiter),
        })
    }
}

impl FromNode for Institution {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        use crate::style::element::InstitutionUseFirst::*;
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
            .map(|el| InstitutionPart::from_node(&el))
            .partition_results()?;

        Ok(Institution {
            and: attribute_option(node, "and")?,
            delimiter: node.attribute("delimiter").map(Atom::from).map(Delimiter),
            use_first,
            use_last: attribute_option_int(node, "use-last")?,
            reverse_order: attribute_bool(node, "reverse-order", false)?,
            parts_selector: attribute_optional(node, "institution-parts")?,
            institution_parts,
        })
    }
}

impl FromNode for InstitutionPart {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(InstitutionPart {
            name: InstitutionPartName::from_node(node)?,
            formatting: Option::from_node(node)?,
            affixes: Affixes::from_node(node)?,
            strip_periods: attribute_bool(node, "strip-periods", false)?,
        })
    }
}

impl FromNode for InstitutionPartName {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        match node.attribute("name") {
            Some("long") => Ok(InstitutionPartName::Long(attribute_bool(
                node, "if-short", false,
            )?)),
            Some("short") => Ok(InstitutionPartName::Short),
            Some(ref val) => Err(InvalidCsl::attr_val(node, "name", val).into()),
            None => Err(InvalidCsl::missing(node, "name").into()),
        }
    }
}

impl FromNode for Name {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        // for inheriting from cs:style/cs:citation/cs:bibliography
        let mut delim_attr = "delimiter";
        let mut form_attr = "form";
        let mut name_part_given = None;
        let mut name_part_family = None;
        if node.tag_name().name() != "name" {
            delim_attr = "name-delimiter";
            form_attr = "name-form";
        } else {
            let parts = move |val| {
                node.children()
                    .filter(move |el| {
                        el.is_element()
                            && el.has_tag_name("name-part")
                            && el.attribute("name") == Some(val)
                    })
                    .map(|el| NamePart::from_node(&el))
                    .filter_map(|np| np.ok())
            };
            name_part_given = parts("given").nth(0);
            name_part_family = parts("family").nth(0);
        }
        Ok(Name {
            and: attribute_option(node, "and")?,
            delimiter: node.attribute(delim_attr).map(Atom::from).map(Delimiter),
            delimiter_precedes_et_al: attribute_option(node, "delimiter-precedes-et-al")?,
            delimiter_precedes_last: attribute_option(node, "delimiter-precedes-last")?,
            et_al_min: attribute_option_int(node, "et-al-min")?,
            et_al_use_last: attribute_option_bool(node, "et-al-use-last")?,
            et_al_use_first: attribute_option_int(node, "et-al-use-first")?,
            et_al_subsequent_min: attribute_option_int(node, "et-al-subsequent-min")?,
            et_al_subsequent_use_first: attribute_option_int(node, "et-al-subsequent-use-first")?,
            form: attribute_option(node, form_attr)?,
            initialize: attribute_option_bool(node, "initialize")?,
            initialize_with: attribute_option_atom(node, "initialize-with"),
            name_as_sort_order: attribute_option(node, "name-as-sort-order")?,
            sort_separator: attribute_option_atom(node, "sort-separator"),
            formatting: Option::from_node(node)?,
            affixes: Affixes::from_node(node)?,
            name_part_given,
            name_part_family,
        })
    }
}

impl FromNode for NameEtAl {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(NameEtAl {
            term: attribute_string(node, "term"),
            formatting: Option::from_node(node)?,
        })
    }
}

impl FromNode for NameWith {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(NameWith {
            formatting: Option::from_node(node)?,
            affixes: Affixes::from_node(node)?,
        })
    }
}

impl FromNode for NamePart {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(NamePart {
            name: attribute_required(node, "name")?,
            text_case: TextCase::from_node(node)?,
            formatting: Option::from_node(node)?,
            affixes: Affixes::from_node(node)?,
        })
    }
}

impl FromNode for NameLabel {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(NameLabel {
            form: attribute_optional(node, "form")?,
            formatting: Option::from_node(node)?,
            delimiter: Delimiter::from_node(node)?,
            plural: attribute_optional(node, "plural")?,
            strip_periods: attribute_bool(node, "strip-periods", false)?,
        })
    }
}

impl FromNode for Substitute {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let els = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("name"))
            .map(|el| Element::from_node(&el))
            .partition_results()?;
        Ok(Substitute(els))
    }
}

struct TextContent(Option<String>);

impl FromNode for TextContent {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let opt_s = node.text().map(String::from);
        Ok(TextContent(opt_s))
    }
}

impl FromNode for TermPlurality {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let always: Option<String> = TextContent::from_node(node)?.0.map(|s| s.trim().into());
        let single: Option<TextContent> = max1_child("term", "single", node.children())?;
        let multiple: Option<TextContent> = max1_child("term", "multiple", node.children())?;
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
            _ => Ok(Err(InvalidCsl::new(node, msg))?),
        }
    }
}

impl FromNode for OrdinalMatch {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(attribute_optional(node, "match")?)
    }
}

impl FromNode for TermFormExtended {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(attribute_optional(node, "form")?)
    }
}

impl FromNode for TermForm {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(attribute_optional(node, "form")?)
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
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        use self::terms::AnyTermName::*;
        let name: AnyTermName = attribute_required(node, "name")?;
        let content = TermPlurality::from_node(node)?;
        match name {
            Number(v) => Ok(TermEl::Gendered(
                GenderedTermSelector::Number(v, TermForm::from_node(node)?),
                GenderedTerm(content, attribute_optional(node, "gender")?),
            )),
            Month(mt) => Ok(TermEl::Gendered(
                GenderedTermSelector::Month(mt, TermForm::from_node(node)?),
                GenderedTerm(content, attribute_optional(node, "gender")?),
            )),
            Loc(lt) => Ok(TermEl::Gendered(
                GenderedTermSelector::Locator(lt, TermForm::from_node(node)?),
                GenderedTerm(content, attribute_optional(node, "gender")?),
            )),
            Misc(t) => Ok(TermEl::Simple(
                SimpleTermSelector::Misc(t, TermFormExtended::from_node(node)?),
                content,
            )),
            Season(t) => Ok(TermEl::Simple(
                SimpleTermSelector::Season(t, TermForm::from_node(node)?),
                content,
            )),
            Quote(t) => Ok(TermEl::Simple(
                SimpleTermSelector::Quote(t, TermForm::from_node(node)?),
                content,
            )),
            Role(t) => Ok(TermEl::Role(
                RoleTermSelector(t, TermFormExtended::from_node(node)?),
                content,
            )),
            Ordinal(t) => match content {
                TermPlurality::Invariant(a) => Ok(TermEl::Ordinal(
                    OrdinalTermSelector(
                        t,
                        attribute_optional(node, "gender-form")?,
                        OrdinalMatch::from_node(node)?,
                    ),
                    a,
                )),
                _ => Err(InvalidCsl::new(node, "ordinal terms cannot be pluralized").into()),
            },
        }
    }
}

impl FromNode for LocaleOptionsNode {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        Ok(LocaleOptionsNode {
            limit_ordinals_to_day_1: attribute_option_bool(node, "limit-ordinals-to-day-1")?,
            punctuation_in_quote: attribute_option_bool(node, "punctuation-in-quote")?,
        })
    }
}

impl FromNode for Locale {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let lang = attribute_option(node, ("xml", "lang"))?;

        // TODO: one slot for each date form, avoid allocations?
        let dates_vec = node
            .children()
            .filter(|el| el.has_tag_name("date"))
            .map(|el| LocaleDate::from_node(&el))
            .partition_results()?;

        let mut dates = FnvHashMap::default();
        for date in dates_vec.into_iter() {
            dates.insert(date.form, date);
        }

        let mut simple_terms = SimpleMapping::default();
        let mut gendered_terms = GenderedMapping::default();
        let mut ordinal_terms = OrdinalMapping::default();
        let mut role_terms = RoleMapping::default();

        let options_node = node
            .children()
            .filter(|el| el.has_tag_name("style-options"))
            .nth(0)
            .map(|o_node| LocaleOptionsNode::from_node(&o_node))
            .unwrap_or_else(|| Ok(LocaleOptionsNode::default()))?;

        let terms_node = node.children().filter(|el| el.has_tag_name("terms")).nth(0);
        if let Some(tn) = terms_node {
            for n in tn.children().filter(|el| el.has_tag_name("term")) {
                match TermEl::from_node(&n)? {
                    TermEl::Simple(sel, con) => {
                        simple_terms.insert(sel, con);
                    }
                    TermEl::Gendered(sel, con) => {
                        gendered_terms.insert(sel, con);
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

impl FromNode for CslVersionReq {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let version = attribute_string(node, "version");
        let variant: CslVariant;
        let req = if version.ends_with("mlz1") {
            variant = CslVariant::CslM;
            VersionReq::parse(version.trim_end_matches("mlz1")).map_err(|_| {
                InvalidCsl::new(
                    node,
                    &format!(
r#"unsupported "1.1mlz1"-style version string (use variant="csl-m" version="1.x", for example)"#),
                )
            })?
        } else {
            // TODO: bootstrap attribute_optional with a dummy CslVariant::Csl
            variant = attribute_optional(node, "variant")?;
            VersionReq::parse(&version).map_err(|_| {
                InvalidCsl::new(
                    node,
                    &format!("could not parse version string \"{}\"", &version),
                )
            })?
        };
        let supported = match &variant {
            CslVariant::Csl => &*COMPILED_VERSION,
            CslVariant::CslM => &*COMPILED_VERSION_M,
        };
        if !req.matches(supported) {
            return Err(InvalidCsl::new(
                    node,
                    &format!(
                        "Unsupported version for variant {:?}: \"{}\". This engine supports {} and later.",
                            variant,
                            req,
                            supported)).into());
        }
        Ok(CslVersionReq(variant, req))
    }
}

impl FromNode for Style {
    fn from_node(node: &Node) -> FromNodeResult<Self> {
        let version_req = CslVersionReq::from_node(node)?;
        // let info_node = get_toplevel(&doc, "info")?;
        let mut macros = FnvHashMap::default();
        let mut locale_overrides = FnvHashMap::default();
        let mut errors: Vec<CslError> = Vec::new();

        let locales_res = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("locale"))
            .map(|el| Locale::from_node(&el))
            .partition_results();
        match locales_res {
            Ok(locales) => {
                for loc in locales {
                    locale_overrides.insert(loc.lang.clone(), loc);
                }
            }
            Err(mut errs) => {
                errors.append(&mut errs);
            }
        }
        // TODO: output errors from macros, locales as well as citation and bibliography, if there are errors in
        // all
        let macro_res = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("macro"))
            .map(|el| MacroMap::from_node(&el))
            .partition_results();
        match macro_res {
            Ok(macro_maps) => {
                for mac in macro_maps {
                    macros.insert(mac.name, mac.elements);
                }
            }
            Err(mut errs) => {
                errors.append(&mut errs);
            }
        }
        let citation = match Citation::from_node(&get_toplevel(&node, "citation")?) {
            Ok(cit) => Ok(cit),
            Err(err) => {
                errors.push(err);
                Err(CslError(Vec::new()))
            }
        };

        let matches: Vec<_> = node
            .children()
            .filter(|n| n.has_tag_name("bibliography"))
            .collect();
        let bib_node = if matches.len() > 1 {
            Ok(Err(InvalidCsl::new(
                &node,
                "Cannot have more than one <bibliography>",
            ))?)
        } else {
            // move matches into its first item
            Ok(matches.into_iter().nth(0))
        };

        // TODO: push instead of bubble?
        let bibliography = match bib_node {
            Ok(Some(node)) => match Bibliography::from_node(&node) {
                Ok(bib) => Some(bib),
                Err(err) => {
                    errors.push(err);
                    None
                }
            },
            Ok(None) => None,
            Err(e) => {
                errors.push(e);
                None
            }
        };

        if errors.len() > 0 {
            return Err(errors.into());
        }

        Ok(Style {
            macros,
            version_req,
            locale_overrides,
            default_locale: attribute_optional(node, "default-locale")?,
            citation: citation?,
            bibliography,
            info: Info {},
            class: attribute_required(node, "class")?,
            name_inheritance: Name::from_node(&node)?,
            page_range_format: attribute_option(node, "page-range-format")?,
            demote_non_dropping_particle: attribute_optional(node, "demote-non-dropping-particle")?,
            initialize_with_hyphen: attribute_bool(node, "initialize-with-hyphen", true)?,
            names_delimiter: node
                .attribute("names-delimiter")
                .map(Atom::from)
                .map(Delimiter),
        })
    }
}

pub(crate) mod db {
    use std::sync::Arc;

    use super::{Name, Style};

    /// Salsa interface to a CSL style.
    #[salsa::query_group]
    pub trait StyleDatabase: salsa::Database {
        #[salsa::input]
        fn style(&self, key: ()) -> Arc<Style>;
        fn name_citation(&self, key: ()) -> Arc<Name>;
    }

    fn name_citation(db: &impl StyleDatabase, _: ()) -> Arc<Name> {
        let style = db.style(());
        let default = Name::root_default();
        let root = &style.name_inheritance;
        let citation = &style.citation.name_inheritance;
        Arc::new(default.merge(root).merge(citation))
    }
}
