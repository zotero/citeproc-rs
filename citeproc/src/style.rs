pub mod element;
pub mod error;
mod get_attribute;
pub mod locale;
pub mod terms;
pub mod variables;
pub mod version;

// mod take_while;
// use self::take_while::*;
use self::element::*;
use self::error::*;
use self::get_attribute::*;
use self::locale::*;
use self::terms::*;
use crate::utils::PartitionArenaErrors;
use fnv::FnvHashMap;
use roxmltree::{Children, Node};

pub trait FromNode
where
    Self: Sized,
{
    fn from_node(node: &Node) -> Result<Self, CslError>;
}

pub trait IsOnNode
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

impl IsOnNode for Formatting {
    fn filter_attribute(attr: &str) -> bool {
        attr == "font-style"
            || attr == "font-variant"
            || attr == "font-weight"
            || attr == "text-decoration"
            || attr == "vertical-alignment"
            || attr == "display"
            || attr == "strip-periods"
    }
}

impl<T> FromNode for Option<T>
where
    T: IsOnNode + FromNode,
{
    fn from_node(node: &Node) -> Result<Self, CslError> {
        if T::is_on_node(node) {
            Ok(Some(T::from_node(node)?))
        } else {
            Ok(None)
        }
    }
}

impl FromNode for Affixes {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        Ok(Affixes {
            prefix: attribute_string(node, "prefix"),
            suffix: attribute_string(node, "suffix"),
        })
    }
}

impl FromNode for RangeDelimiter {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        Ok(RangeDelimiter(attribute_string(node, "range-delimiter")))
    }
}

impl IsOnNode for RangeDelimiter {
    fn filter_attribute(attr: &str) -> bool {
        attr == "range-delimiter"
    }
}

impl IsOnNode for Affixes {
    fn filter_attribute(attr: &str) -> bool {
        attr == "prefix" || attr == "suffix"
    }
}

impl FromNode for Formatting {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        Ok(Formatting {
            font_style: attribute_optional(node, "font-style")?,
            font_variant: attribute_optional(node, "font-variant")?,
            font_weight: attribute_optional(node, "font-weight")?,
            text_decoration: attribute_optional(node, "text-decoration")?,
            vertical_alignment: attribute_optional(node, "vertical-alignment")?,
            display: attribute_optional(node, "display")?,
            strip_periods: attribute_bool(node, "strip-periods", false)?,

            // TODO: carry options from root
            hyperlink: String::from(""),
        })
    }
}

impl FromNode for Citation {
    fn from_node(node: &Node) -> Result<Self, CslError> {
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
                .map(String::from)
                .map(Delimiter),
        })
    }
}

impl FromNode for Delimiter {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        Ok(Delimiter(attribute_string(node, "delimiter")))
    }
}

impl FromNode for Layout {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        let elements = node
            .children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el))
            .partition_results()?;
        Ok(Layout {
            formatting: Option::from_node(node)?,
            affixes: Affixes::from_node(node)?,
            delimiter: Delimiter::from_node(node)?,
            elements,
        })
    }
}

impl FromNode for TextTermSelector {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        use self::terms::AnyTermName::*;
        // we already know term is on there
        let t = attribute_required(node, "term")?;
        match t {
            Edition => Ok(TextTermSelector::Gendered(GenderedTermSelector::Edition(
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
                TermForm::from_node(node)?,
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
                RoleTermForm::from_node(node)?,
            ))),
            Ordinal(_) => {
                Err(InvalidCsl::new(node, "you cannot render an ordinal term directly").into())
            }
        }
    }
}

fn text_el(node: &Node) -> Result<Element, CslError> {
    use self::element::Element::*;
    let formatting = Option::from_node(node)?;
    let affixes = Affixes::from_node(node)?;
    if let Some(m) = node.attribute("macro") {
        return Ok(Macro(
            m.to_owned(),
            formatting,
            affixes,
            attribute_bool(node, "quotes", false)?,
        ));
    }
    if let Some(_m) = node.attribute("variable") {
        return Ok(Variable(
            attribute_var_type(node, "variable", NeedVarType::TextVariable)?,
            formatting,
            affixes,
            attribute_optional(node, "form")?,
            attribute_bool(node, "quotes", false)?,
        ));
    }
    if let Some(v) = node.attribute("value") {
        return Ok(Const(
            v.to_owned(),
            formatting,
            affixes,
            attribute_bool(node, "quotes", false)?,
        ));
    }
    if let Some(_) = node.attribute("term") {
        return Ok(Term(
            TextTermSelector::from_node(node)?,
            formatting,
            affixes,
            attribute_bool(node, "plural", false)?,
        ));
    };
    Err(InvalidCsl::new(
        node,
        "<text> without a `variable`, `macro`, `term` or `value` is invalid",
    ))?
}

fn label_el(node: &Node) -> Result<Element, CslError> {
    Ok(Element::Label(
        attribute_required(node, "variable")?,
        attribute_optional(node, "form")?,
        Option::from_node(node)?,
        Affixes::from_node(node)?,
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
    ))
}

fn group_el(node: &Node) -> Result<Element, CslError> {
    let elements = node
        .children()
        .filter(|n| n.is_element())
        .map(|el| Element::from_node(&el))
        .partition_results()?;
    Ok(Element::Group(
        Option::from_node(node)?,
        Delimiter::from_node(node)?,
        Affixes::from_node(node)?,
        elements,
    ))
}

impl FromNode for Else {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        let elements = node
            .children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el))
            .partition_results()?;
        Ok(Else(elements))
    }
}

impl FromNode for Match {
    fn from_node(node: &Node) -> Result<Self, CslError> {
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
            disambiguate: attribute_only_true(node, "disambiguate")?,
            is_numeric: attribute_array_var(node, "is-numeric", NeedVarType::CondIsNumeric)?,
            variable: attribute_array_var(node, "variable", NeedVarType::Any)?,
            position: attribute_array_var(node, "position", NeedVarType::CondPosition)?,
            is_uncertain_date: attribute_array_var(
                node,
                "is-uncertain-date",
                NeedVarType::CondIsUncertainDate,
            )?,
            csl_type: attribute_array_var(node, "type", NeedVarType::CondType)?,
            locator: attribute_array_var(node, "locator", NeedVarType::CondLocator)?,
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
    fn from_node(node: &Node) -> Result<Self, CslError> {
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
    fn from_node(node: &Node) -> Result<Self, CslError> {
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
    let els: Vec<Node> = node.children().filter(|n| n.is_element()).collect();

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

    for el in els.into_iter() {
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

    Ok(Element::Choose(Choose(_if, elseifs, else_block)))
}

fn max1_child<T: FromNode>(
    parent_tag: &str,
    child_tag: &str,
    els: Children,
) -> Result<Option<T>, CslError> {
    // TODO: remove the allocation here, with a cloned iterator
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

impl IsOnNode for TextCase {
    fn filter_attribute(attr: &str) -> bool {
        attr == "text-case"
    }
}

impl FromNode for TextCase {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        Ok(attribute_optional(node, "text-case")?)
    }
}

fn disallow_default<T: Default + FromNode + IsOnNode>(
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
    fn from_node_dp(node: &Node, full: bool) -> Result<Self, CslError> {
        let name: DatePartName = attribute_required(node, "name")?;
        let form = match name {
            DatePartName::Year => DatePartForm::Year(attribute_optional(node, "form")?),
            DatePartName::Month => DatePartForm::Month(attribute_optional(node, "form")?),
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
    fn from_node(node: &Node) -> Result<Self, CslError> {
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
            delimiter: Delimiter::from_node(node)?,
        })
    }
}

impl FromNode for LocaleDate {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        let elements = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("date-part"))
            .map(|el| DatePart::from_node_dp(&el, false))
            .partition_results()?;
        Ok(LocaleDate {
            form: attribute_required(node, "form")?,
            date_parts: elements,
            delimiter: Delimiter::from_node(node)?,
            text_case: TextCase::from_node(node)?,
        })
    }
}

impl FromNode for LocalizedDate {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        Ok(LocalizedDate {
            variable: attribute_var_type(node, "variable", NeedVarType::Date)?,
            parts_selector: attribute_optional(node, "date-parts")?,
            form: attribute_required(node, "form")?,
            affixes: Affixes::from_node(node)?,
            formatting: Option::from_node(node)?,
            text_case: TextCase::from_node(node)?,
        })
    }
}

impl FromNode for BodyDate {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        if node.has_attribute("form") {
            Ok(BodyDate::Local(LocalizedDate::from_node(node)?))
        } else {
            Ok(BodyDate::Indep(IndependentDate::from_node(node)?))
        }
    }
}

impl FromNode for Element {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        match node.tag_name().name() {
            "text" => Ok(text_el(node)?),
            "label" => Ok(label_el(node)?),
            "group" => Ok(group_el(node)?),
            "number" => Ok(number_el(node)?),
            "names" => Ok(Element::Names(Names::from_node(node)?)),
            "choose" => Ok(choose_el(node)?),
            "date" => Ok(Element::Date(BodyDate::from_node(node)?)),
            _ => Err(InvalidCsl::new(node, "Unrecognised node."))?,
        }
    }
}

fn get_toplevel<'a, 'd: 'a>(
    root: &Node<'a, 'd>,
    nodename: &'static str,
) -> Result<Node<'a, 'd>, CslError> {
    let matches = root
        .children()
        .filter(|n| n.has_tag_name(nodename))
        .collect::<Vec<Node<'a, 'd>>>();
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
    fn from_node(node: &Node) -> Result<Self, CslError> {
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
                ))?)
            }
        };
        Ok(MacroMap {
            name: name.to_owned(),
            elements: elements?,
        })
    }
}

impl FromNode for Names {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        // TODO: did I have Vec<Name> originally because some styles have more than one?
        let name = max1_child("names", "name", node.children())?;
        let et_al = max1_child("names", "et-al", node.children())?;
        let label = max1_child("names", "label", node.children())?;
        let substitute = max1_child("names", "substitute", node.children())?;
        Ok(Names {
            variables: attribute_array_var(node, "variable", NeedVarType::Name)?,
            name,
            et_al,
            label,
            substitute,
            formatting: Option::from_node(node)?,
            delimiter: node.attribute("delimiter").map(String::from).map(Delimiter),
        })
    }
}

impl FromNode for Name {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        // for inheriting from cs:style/cs:citation/cs:bibliography
        let mut delim_attr = "delimiter";
        let mut form_attr = "form";
        let mut name_part_given = None;
        let mut name_part_family = None;
        if node.tag_name().name().clone() != "name" {
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
            and: attribute_option_string(node, "and"),
            delimiter: node.attribute(delim_attr).map(String::from).map(Delimiter),
            delimiter_precedes_et_al: attribute_option(node, "delimiter-precedes-et-al")?,
            delimiter_precedes_last: attribute_option(node, "delimiter-precedes-last")?,
            et_al_min: attribute_option_int(node, "et-al-min")?,
            et_al_use_last: attribute_option_bool(node, "et-al-use-last")?,
            et_al_use_first: attribute_option_int(node, "et-al-use-first")?,
            et_al_subsequent_min: attribute_option_int(node, "et-al-subsequent-min")?,
            et_al_subsequent_use_first: attribute_option_int(node, "et-al-subsequent-use-first")?,
            form: attribute_option(node, form_attr)?,
            initialize: attribute_option_bool(node, "initialize")?,
            initialize_with: attribute_option_string(node, "initialize-with"),
            name_as_sort_order: attribute_option(node, "name-as-sort-order")?,
            sort_separator: attribute_option_string(node, "sort-separator"),
            formatting: Option::from_node(node)?,
            affixes: Affixes::from_node(node)?,
            name_part_given,
            name_part_family,
        })
    }
}

impl FromNode for EtAl {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        Ok(EtAl {
            term: attribute_string(node, "term"),
            formatting: Option::from_node(node)?,
        })
    }
}

impl FromNode for NamePart {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        Ok(NamePart {
            name: attribute_required(node, "name")?,
            text_case: TextCase::from_node(node)?,
            formatting: Option::from_node(node)?,
            affixes: Affixes::from_node(node)?,
        })
    }
}

impl FromNode for NameLabel {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        Ok(NameLabel {
            form: attribute_optional(node, "form")?,
            formatting: Option::from_node(node)?,
            delimiter: Delimiter::from_node(node)?,
            plural: attribute_optional(node, "plural")?,
        })
    }
}

impl FromNode for Substitute {
    fn from_node(node: &Node) -> Result<Self, CslError> {
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
    fn from_node(node: &Node) -> Result<Self, CslError> {
        let opt_s = node.text().map(String::from);
        Ok(TextContent(opt_s))
    }
}

impl FromNode for TermPlurality {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        let always: Option<String> = TextContent::from_node(node)?.0.map(|s| s.trim().into());
        let single: Option<TextContent> = max1_child("term", "single", node.children())?;
        let multiple: Option<TextContent> = max1_child("term", "multiple", node.children())?;
        let msg = "<term> must contain either only text content or both <single> and <multiple>";
        match (always, single, multiple) {
            // empty term is valid
            (None, None, None) => Ok(TermPlurality::Always("".into())),
            // <term>plain text content</term>
            (Some(a), None, None) => Ok(TermPlurality::Always(a)),
            // <term> ANYTHING <single> s </single> <multiple> m </multiple></term>
            (_, Some(s), Some(m)) => Ok(TermPlurality::Pluralized {
                single: s.0.unwrap_or("".into()),
                multiple: m.0.unwrap_or("".into()),
            }),
            // had one of <single> or <multiple>, but not the other
            _ => Ok(Err(InvalidCsl::new(node, msg))?),
        }
    }
}

impl FromNode for OrdinalMatch {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        Ok(attribute_optional(node, "match")?)
    }
}

impl FromNode for RoleTermForm {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        Ok(attribute_optional(node, "form")?)
    }
}

impl FromNode for TermForm {
    fn from_node(node: &Node) -> Result<Self, CslError> {
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
    fn from_node(node: &Node) -> Result<Self, CslError> {
        use self::terms::AnyTermName::*;
        let name: AnyTermName = attribute_required(node, "name")?;
        let content = TermPlurality::from_node(node)?;
        match name {
            Edition => Ok(TermEl::Gendered(
                GenderedTermSelector::Edition(TermForm::from_node(node)?),
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
                SimpleTermSelector::Misc(t, TermForm::from_node(node)?),
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
                RoleTermSelector(t, RoleTermForm::from_node(node)?),
                content,
            )),
            Ordinal(t) => match content {
                TermPlurality::Always(a) => Ok(TermEl::Ordinal(
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

impl FromNode for Locale {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        // TODO: make this an Option?
        let lang = node.attribute(("xml", "lang")).unwrap_or("en-GB");

        // TODO: one slot for each date form, avoid allocations?
        let dates = node
            .children()
            .filter(|el| el.has_tag_name("date"))
            .map(|el| LocaleDate::from_node(&el))
            .partition_results()?;

        let mut simple_terms = SimpleMapping::default();
        let mut gendered_terms = GenderedMapping::default();
        let mut ordinal_terms = OrdinalMapping::default();
        let mut role_terms = RoleMapping::default();

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
            lang: lang.into(),
            options: vec![],
            simple_terms,
            gendered_terms,
            ordinal_terms,
            role_terms,
            dates,
        })
    }
}

impl FromNode for Style {
    fn from_node(node: &Node) -> Result<Self, CslError> {
        // let info_node = get_toplevel(&doc, "info")?;
        let mut macros = FnvHashMap::default();
        let mut locale_overrides = FnvHashMap::default();

        let locales = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("locale"));
        for el in locales {
            let loc = Locale::from_node(&el)?;
            locale_overrides.insert(loc.lang.clone(), loc);
        }
        let macro_maps = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("macro"));
        for el in macro_maps {
            let mac = MacroMap::from_node(&el)?;
            macros.insert(mac.name, mac.elements);
        }
        let citation = Citation::from_node(&get_toplevel(&node, "citation")?);
        Ok(Style {
            macros,
            locale_overrides,
            citation: citation?,
            info: Info {},
            class: StyleClass::Note,
            name_inheritance: Name::from_node(&node)?,
            names_delimiter: node
                .attribute("names-delimiter")
                .map(String::from)
                .map(Delimiter),
        })
    }
}

// pub fn drive_style(path: &str, text: &str) -> String {
//     match build_style(text) {
//         Ok(_style) => "done!".to_string(),
//         Err(e) => {
//             file_diagnostics(&[e], path, text);
//             "failed".into()
//         }
//     }
// }
