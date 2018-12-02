pub mod element;
pub mod error;
mod get_attribute;
pub mod locale;
pub mod terms;
pub mod variables;
pub mod version;

use typed_arena::Arena;

// mod take_while;
// use self::take_while::*;
use self::element::*;
use self::error::*;
use self::get_attribute::*;
use roxmltree::{Children, Node};

pub trait IsOnNode
where
    Self: Sized,
{
    fn is_on_node(node: &Node) -> Vec<String>;
}

pub trait FromNode<'s>
where
    Self: Sized,
{
    fn from_node(node: &Node, arena: &'s Arena<Element>) -> Result<Self, InvalidCsl>;
}

impl<'s> FromNode<'s> for Affixes {
    fn from_node(node: &Node, _arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        Ok(Affixes {
            prefix: attribute_string(node, "prefix"),
            suffix: attribute_string(node, "suffix"),
        })
    }
}

impl<'s> FromNode<'s> for RangeDelimiter {
    fn from_node(node: &Node, _arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        Ok(RangeDelimiter(attribute_string(node, "range-delimiter")))
    }
}

impl IsOnNode for RangeDelimiter {
    fn is_on_node(node: &Node) -> Vec<String> {
        node.attributes()
            .iter()
            .filter(|a| a.name() == "range-delimiter")
            .map(|a| a.name().to_owned())
            .collect()
    }
}

impl IsOnNode for Affixes {
    fn is_on_node(node: &Node) -> Vec<String> {
        node.attributes()
            .iter()
            .filter(|a| a.name() == "prefix" || a.name() == "suffix")
            .map(|a| a.name().to_owned())
            .collect()
    }
}

impl<'s> FromNode<'s> for Formatting {
    fn from_node(node: &Node, _arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
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

impl IsOnNode for Formatting {
    fn is_on_node(node: &Node) -> Vec<String> {
        node.attributes()
            .iter()
            .filter(|a| {
                a.name() == "font-style"
                    || a.name() == "font-variant"
                    || a.name() == "font-weight"
                    || a.name() == "text-decoration"
                    || a.name() == "vertical-alignment"
                    || a.name() == "display"
                    || a.name() == "strip-periods"
            })
            .map(|a| a.name().to_owned())
            .collect()
    }
}

impl<'s> FromNode<'s> for Citation<'s> {
    fn from_node(node: &Node, arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        let layouts: Vec<_> = node
            .children()
            .filter(|n| n.has_tag_name("layout"))
            .collect();
        if layouts.len() != 1 {
            return Err(InvalidCsl::new(
                node,
                "<citation> must contain exactly one <layout>".into(),
            ))?;
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
            layout: Layout::from_node(&layout_node, arena)?,
        })
    }
}

impl<'s> FromNode<'s> for Delimiter {
    fn from_node(node: &Node, _arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        Ok(Delimiter(attribute_string(node, "delimiter")))
    }
}

impl<'s> FromNode<'s> for Layout<'s> {
    fn from_node(node: &Node, arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        let els = node
            .children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el, arena))
            .filter_map(Result::ok);
        let elements = arena.alloc_extend(els);
        Ok(Layout {
            formatting: Formatting::from_node(node, arena)?,
            affixes: Affixes::from_node(node, arena)?,
            delimiter: Delimiter::from_node(node, arena)?,
            elements,
        })
    }
}

fn text_el<'s>(node: &Node, arena: &'s Arena<Element>) -> Result<Element, InvalidCsl> {
    use self::element::Element::*;
    let formatting = Formatting::from_node(node, arena)?;
    let affixes = Affixes::from_node(node, arena)?;
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
            attribute_required(node, "variable")?,
            formatting,
            affixes,
            attribute_optional(node, "form")?,
            Delimiter::from_node(node, arena)?,
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
    if let Some(t) = node.attribute("term") {
        return Ok(Term(
            t.to_owned(),
            attribute_optional(node, "form")?,
            formatting,
            affixes,
            attribute_bool(node, "plural", false)?,
        ));
    };
    Err(InvalidCsl::new(node, "yeah".to_owned()))?
}

fn label_el<'s>(node: &Node, arena: &'s Arena<Element>) -> Result<Element, InvalidCsl> {
    Ok(Element::Label(
        attribute_required(node, "variable")?,
        attribute_optional(node, "form")?,
        Formatting::from_node(node, arena)?,
        Affixes::from_node(node, arena)?,
        attribute_optional(node, "plural")?,
    ))
}

fn number_el<'s>(node: &Node, arena: &'s Arena<Element>) -> Result<Element, InvalidCsl> {
    Ok(Element::Number(
        attribute_required(node, "variable")?,
        attribute_optional(node, "form")?,
        Formatting::from_node(node, arena)?,
        Affixes::from_node(node, arena)?,
        attribute_optional(node, "plural")?,
    ))
}

fn group_el<'s>(node: &Node, arena: &'s Arena<Element>) -> Result<Element, InvalidCsl> {
    let elements: Result<Vec<_>, _> = node
        .children()
        .filter(|n| n.is_element())
        .map(|el| Element::from_node(&el, arena))
        .collect();
    Ok(Element::Group(
        Formatting::from_node(node, arena)?,
        Delimiter::from_node(node, arena)?,
        elements?,
    ))
}

impl<'s> FromNode<'s> for Else {
    fn from_node(node: &Node, arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        let elements: Result<Vec<_>, _> = node
            .children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el, arena))
            .collect();
        Ok(Else(elements?))
    }
}

impl<'s> FromNode<'s> for Match {
    fn from_node(node: &Node, _arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        Ok(attribute_optional(node, "match")?)
    }
}

impl<'s> FromNode<'s> for Condition {
    fn from_node(node: &Node, arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        Ok(Condition {
            match_type: Match::from_node(node, arena)?,
            disambiguate: attribute_only_true(node, "disambiguate")?,
            is_numeric: attribute_array(node, "is-numeric")?,
            variable: attribute_array(node, "variable")?,
            position: attribute_array(node, "position")?,
            is_uncertain_date: attribute_array(node, "is-uncertain-date")?,
            csl_type: attribute_array(node, "type")?,
            locator: attribute_array(node, "locator")?,
        })
    }
}

impl<'s> FromNode<'s> for IfThen {
    fn from_node(node: &Node, arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        let elements: Result<Vec<_>, _> = node
            .children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el, arena))
            .collect();
        Ok(IfThen(Condition::from_node(node, arena)?, elements?))
    }
}

fn choose_el<'s>(node: &Node, arena: &'s Arena<Element>) -> Result<Element, InvalidCsl> {
    let els: Vec<Node> = node.children().filter(|n| n.is_element()).collect();

    let mut if_block: Option<IfThen> = None;
    let mut elseifs = vec![];
    let mut else_block = Else(vec![]);
    let mut seen_if = false;
    let mut seen_else = false;

    let unrecognised = |el, tag| {
        if tag == "if" || tag == "else-if" || tag == "else" {
            return Err(InvalidCsl::new(
                el,
                format!(
                    "<choose> elements out of order; found <{}> in wrong position",
                    tag
                ),
            ));
        }
        Err(InvalidCsl::new(
            el,
            format!("Unrecognised element {} in <choose>", tag),
        ))
    };

    for el in els.into_iter() {
        let tn = el.tag_name();
        let tag = tn.name().to_owned();
        if !seen_if {
            if tag == "if" {
                seen_if = true;
                if_block = Some(IfThen::from_node(&el, arena)?);
            } else {
                return Err(InvalidCsl::new(
                    &el,
                    "<choose> blocks must begin with an <if>".into(),
                ))?;
            }
        } else if !seen_else {
            if tag == "else-if" {
                elseifs.push(IfThen::from_node(&el, arena)?);
            } else if tag == "else" {
                seen_else = true;
                else_block = Else::from_node(&el, arena)?;
            } else {
                return unrecognised(&el, tag);
            }
        } else {
            return unrecognised(&el, tag);
        }
    }

    let _if = if_block
        .ok_or_else(|| InvalidCsl::new(node, "<choose> blocks must have an <if>".into()))?;

    Ok(Element::Choose(Choose(_if, elseifs, else_block)))
}

impl<'s> FromNode<'s> for NameLabel {
    fn from_node(node: &Node, arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        Ok(NameLabel {
            form: attribute_optional(node, "form")?,
            formatting: Formatting::from_node(node, arena)?,
            delimiter: Delimiter::from_node(node, arena)?,
            plural: attribute_optional(node, "plural")?,
        })
    }
}

impl<'s> FromNode<'s> for Substitute {
    fn from_node(node: &Node, arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        let els: Result<Vec<_>, _> = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("name"))
            .map(|el| Element::from_node(&el, arena))
            .collect();
        Ok(Substitute(els?))
    }
}

fn max1_child<'s, T: FromNode<'s>>(
    parent_tag: &str,
    child_tag: &str,
    els: Children,
    arena: &'s Arena<Element>,
) -> Result<Option<T>, InvalidCsl> {
    let subst_els: Vec<_> = els.filter(|n| n.has_tag_name(child_tag)).collect();
    if subst_els.len() > 1 {
        return Err(InvalidCsl::new(
            &subst_els[1],
            format!(
                "There can only be one <{}> in a <{}> block.",
                child_tag, parent_tag
            ),
        ))?;
    }
    let substs: Result<Vec<_>, _> = subst_els
        .iter()
        .map(|el| T::from_node(&el, arena))
        .collect();
    let substitute = substs?.into_iter().nth(0);
    Ok(substitute)
}

fn names_el<'s>(node: &Node, arena: &'s Arena<Element>) -> Result<Element, InvalidCsl> {
    // let variable: Vec<String> = attribute_string(node, "variable")
    //     .split(" ")
    //     .filter(|s| s.len() > 0)
    //     .map(|s| s.to_owned())
    //     .collect();

    let children = node.children();
    let name_els: Result<Vec<_>, _> = children
        .filter(|n| n.has_tag_name("name"))
        .map(|el| Name::from_node(&el, arena))
        .collect();
    let names = name_els?;

    let label = max1_child("names", "label", node.children(), arena)?;
    let substitute = max1_child("names", "substitute", node.children(), arena)?;

    Ok(Element::Names(Names(
        attribute_array(node, "variable")?,
        names,
        label,
        Formatting::from_node(node, arena)?,
        Delimiter::from_node(node, arena)?,
        substitute,
    )))
}

impl IsOnNode for TextCase {
    fn is_on_node(node: &Node) -> Vec<String> {
        node.attributes()
            .iter()
            .filter(|a| a.name() == "text-case")
            .map(|a| a.name().to_owned())
            .collect()
    }
}

impl<'s> FromNode<'s> for TextCase {
    fn from_node(node: &Node, _arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        Ok(attribute_optional(node, "text-case")?)
    }
}

fn disallow_default<'s, T: Default + FromNode<'s> + IsOnNode>(
    node: &Node,
    arena: &'s Arena<Element>,
    disallow: bool,
) -> Result<T, InvalidCsl> {
    if disallow {
        let attrs = T::is_on_node(node);
        if !attrs.is_empty() {
            Err(InvalidCsl::new(
                node,
                format!("Disallowed attribute on node: {:?}", attrs),
            ))?
        } else {
            Ok(T::default())
        }
    } else {
        T::from_node(node, arena)
    }
}

impl DatePart {
    fn from_node_dp<'s>(
        node: &Node,
        arena: &'s Arena<Element>,
        full: bool,
    ) -> Result<Self, InvalidCsl> {
        let name: DatePartName = attribute_required(node, "name")?;
        let form = match name {
            DatePartName::Year => DatePartForm::Year(attribute_optional(node, "form")?),
            DatePartName::Month => DatePartForm::Month(attribute_optional(node, "form")?),
            DatePartName::Day => DatePartForm::Day(attribute_optional(node, "form")?),
        };
        Ok(DatePart {
            name,
            form,
            affixes: disallow_default(node, arena, !full)?,
            formatting: disallow_default(node, arena, !full)?,
            text_case: disallow_default(node, arena, !full)?,
            range_delimiter: disallow_default(node, arena, !full)?,
        })
    }
}

impl Date {
    fn from_node_date<'s>(
        node: &Node,
        arena: &'s Arena<Element>,
        is_in_locale: bool,
    ) -> Result<Self, InvalidCsl> {
        let form: DateForm = attribute_optional(node, "form")?;
        let not_set = form == DateForm::NotSet;
        let full = if is_in_locale { true } else { not_set };
        let elements: Result<Vec<_>, _> = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("date-part"))
            .map(|el| DatePart::from_node_dp(&el, arena, full))
            .collect();
        Ok(Date {
            variable: attribute_required(node, "variable")?,
            form,
            date_parts: elements?,
            date_parts_attr: attribute_optional(node, "date-parts")?,
            affixes: Affixes::from_node(node, arena)?,
            formatting: Formatting::from_node(node, arena)?,
            delimiter: Delimiter::from_node(node, arena)?,
        })
    }
}

impl<'s> FromNode<'s> for Element {
    fn from_node(node: &Node, arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        match node.tag_name().name() {
            "text" => Ok(text_el(node, arena)?),
            "label" => Ok(label_el(node, arena)?),
            "group" => Ok(group_el(node, arena)?),
            "number" => Ok(number_el(node, arena)?),
            "names" => Ok(names_el(node, arena)?),
            "choose" => Ok(choose_el(node, arena)?),
            "date" => Ok(Element::Date(Date::from_node_date(node, arena, false)?)),
            _ => Err(InvalidCsl::new(node, "Unrecognised node.".into()))?,
        }
    }
}

pub fn get_toplevel<'a, 'd: 'a>(
    root: &Node<'a, 'd>,
    nodename: &'static str,
) -> Result<Node<'a, 'd>, InvalidCsl> {
    let matches = root
        .children()
        .filter(|n| n.has_tag_name(nodename))
        .collect::<Vec<Node<'a, 'd>>>();
    if matches.len() > 1 {
        Err(InvalidCsl::new(
            &root,
            format!("Cannot have more than one <{}>", nodename),
        ))
    } else {
        // move matches into its first item
        matches
            .into_iter()
            .nth(0)
            .ok_or_else(|| InvalidCsl::new(&root, "Must have one <...>".to_owned()))
    }
}

impl<'s> FromNode<'s> for MacroMap {
    fn from_node(node: &Node, arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        let elements: Result<Vec<_>, _> = node
            .children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el, arena))
            .collect();
        let name = match node.attribute("name") {
            Some(n) => n,
            None => {
                return Err(InvalidCsl::new(
                    node,
                    "Macro must have a 'name' attribute.".into(),
                ))
            }
        };
        Ok(MacroMap {
            name: name.to_owned(),
            elements: elements?,
        })
    }
}

impl<'s> FromNode<'s> for NamePart {
    fn from_node(node: &Node, arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        Ok(NamePart {
            name: attribute_required(node, "name")?,
            text_case: TextCase::from_node(node, arena)?,
            formatting: Formatting::from_node(node, arena)?,
        })
    }
}

impl<'s> FromNode<'s> for Name {
    fn from_node(node: &Node, arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        Ok(Name {
            and: attribute_string(node, "and"),
            delimiter: Delimiter::from_node(node, arena)?,
            delimiter_precedes_et_al: attribute_optional(node, "delimiter-precedes-et-al")?,
            delimiter_precedes_last: attribute_optional(node, "delimiter-precedes-last")?,
            et_al_min: attribute_int(node, "et-al-min", 0)?,
            et_al_use_first: attribute_int(node, "et-al-use-first", 0)?,
            et_al_subsequent_min: attribute_int(node, "et-al-subsequent-min", 0)?,
            et_al_subsequent_use_first: attribute_int(node, "et-al-subsequent-use-first", 0)?,
            et_al_use_last: attribute_bool(node, "et-al-use-last", false)?,
            form: attribute_optional(node, "form")?,
            initialize: attribute_bool(node, "initialize", true)?,
            initialize_with: attribute_string(node, "initialize-with"),
            name_as_sort_order: attribute_optional(node, "name-as-sort-order")?,
            sort_separator: attribute_string(node, "sort-separator"),
            formatting: Formatting::from_node(node, arena)?,
            affixes: Affixes::from_node(node, arena)?,
        })
    }
}

impl<'s> FromNode<'s> for Style<'s> {
    fn from_node(node: &Node, arena: &'s Arena<Element>) -> Result<Self, InvalidCsl> {
        let macros: Result<Vec<_>, _> = node
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("macro"))
            .map(|el| MacroMap::from_node(&el, arena))
            .collect();
        let citation = Citation::from_node(&get_toplevel(&node, "citation")?, arena);
        // let info_node = get_toplevel(&doc, "info")?;
        // let locale_node = get_toplevel(&doc, "locale")?;
        Ok(Style {
            macros: macros?,
            citation: citation?,
            info: Info {},
            class: StyleClass::Note,
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
