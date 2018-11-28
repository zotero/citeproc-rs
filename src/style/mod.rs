pub mod locale;
pub mod element;
pub mod error;
mod take_while;
mod get_attribute;
use self::take_while::*;
use self::element::*;
use self::error::*;
use self::get_attribute::*;
use roxmltree::{ Node, Document };

fn attribute_bool(node: &Node, attr: &str, default: bool) -> Result<bool, CslValidationError> {
    match node.attribute(attr) {
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        None => Ok(default),
        Some(s) => Err(CslValidationError::unknown_attribute_value(node, attr,
                                                                   UnknownAttributeValue::new(s)))?
    }
}

fn attribute_only_true(node: &Node, attr: &str) -> Result<bool, CslValidationError> {
    match node.attribute(attr) {
        Some("true") => Ok(true),
        None => Ok(false),
        Some(s) => Err(CslValidationError::unknown_attribute_value(node, attr,
                                                                   UnknownAttributeValue::new(s)))
    }
}

fn attribute_int(node: &Node, attr: &str, default: u32) -> Result<u32, CslValidationError> {
    match node.attribute(attr) {
        Some(s) => {
            let parsed = u32::from_str_radix(s, 10);
            parsed.map_err(|e| CslValidationError::bad_int(node, attr, e))
        },
        None => Ok(default),
    }
}

fn attribute_string(node: &Node, attr: &str) -> String {
    node.attribute(attr).map(String::from).unwrap_or_else(|| String::from(""))
}

fn attribute_required<T: GetAttribute>(node: &Node, attr: &str) -> Result<T, CslValidationError> {
    match node.attribute(attr) {
        Some(a) => match T::get_attr(a) {
            Ok(val) => Ok(val),
            Err(e)  => Err(CslValidationError::unknown_attribute_value(node, attr, e))
        },
        None => Err(CslValidationError::new(node, format!("Must have '{}' attribute", attr)))
    }
}

fn attribute_optional<T: Default + GetAttribute>(node: &Node, attr: &str) -> Result<T, CslValidationError> {
    match node.attribute(attr) {
        Some(a) => match T::get_attr(a) {
            Ok(val) => Ok(val),
            Err(e)  => Err(CslValidationError::unknown_attribute_value(node, attr, e))
        },
        None => Ok(T::default())
    }
}

fn attribute_array<T: GetAttribute>(node: &Node, attr: &str) -> Result<Vec<T>, CslValidationError> {
    match node.attribute(attr) {
        Some(a) => {
            let split: Result<Vec<_>, _> = a.split(" ").map(T::get_attr).collect();
            match split {
                Ok(val) => Ok(val),
                Err(e)  => Err(CslValidationError::unknown_attribute_value(node, attr, e))
            }
        },
        None => Ok(vec![])
    }
}

fn attribute_optional2<T : Default, F : FnOnce(&str) -> Result<T, UnknownAttributeValue>>(node: &Node, attr: &str, result: F) -> Result<T, CslValidationError>
{
    match node.attribute(attr) {
        Some(a) => match result(a) {
            Ok(val) => Ok(val),
            Err(e)  => Err(CslValidationError::unknown_attribute_value(node, attr, e))
        },
        None => Ok(T::default())
    }
}

pub trait IsOnNode where Self : Sized {
    fn is_on_node(node: &Node) -> Vec<String>;
}

pub trait FromNode where Self : Sized {
    fn from_node(node: &Node) -> Result<Self, CslValidationError>;
}

impl FromNode for Affixes {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        Ok(Affixes {
            prefix: attribute_string(node, "prefix"),
            suffix: attribute_string(node, "suffix"),
        })
    }
}

impl FromNode for RangeDelimiter {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        Ok(attribute_optional(node, "range-delimiter")?)
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
            .filter(|a| a.name() == "prefix"
                     || a.name() == "suffix")
            .map(|a| a.name().to_owned())
            .collect()
    }
}

impl FromNode for Formatting {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
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
        .filter(|a| a.name() == "font-style"
                || a.name() == "font-variant"
                || a.name() == "font-weight"
                || a.name() == "text-decoration"
                || a.name() == "vertical-alignment"
                || a.name() == "display"
                || a.name() == "strip-periods"
        )
        .map(|a| a.name().to_owned())
        .collect()
    }
}

impl FromNode for Citation {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        let children: Vec<_> = node.children().filter(|n| n.is_element()).collect();
        if children.len() != 1 {
            println!("{:?}", children);
            return Err(CslValidationError::new(node, "<citation> must contain exactly one <layout>".into()))?
        }
        let layout_node = children[0];
        Ok(Citation{
            disambiguate_add_names: attribute_bool(node, "disambiguate-add-names", false)?,
            disambiguate_add_givenname: attribute_bool(node, "disambiguate-add-givenname", false)?,
            givenname_disambiguation_rule: attribute_optional(node, "givenname-disambiguation-rule")?,
            disambiguate_add_year_suffix: attribute_bool(node, "disambiguate-add-year-suffix", false)?,
            layout: Layout::from_node(&layout_node)?
        })
    }
}

impl FromNode for Delimiter {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        Ok(Delimiter(attribute_string(node, "delimiter")))
    }
}

impl FromNode for Layout {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        let elements: Result<Vec<_>, _> = node.children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el)).collect();
        Ok(Layout {
            formatting: Formatting::from_node(node)?,
            affixes: Affixes::from_node(node)?,
            delimiter: Delimiter::from_node(node)?,
            elements: elements?
        })
    }
}

fn text_el(node: &Node) -> Result<Element, CslValidationError> {
    use self::element::Element::*;
    let formatting = Formatting::from_node(node)?;
    let affixes = Affixes::from_node(node)?;
    match node.attribute("term") {
        Some(t) => return Ok(Term(
                t.to_owned(),
                attribute_optional2(node, "form", Form::from_str)?,
                formatting, affixes,
                attribute_bool(node, "plural", false)?)),
        None => {}
    };
    match node.attribute("macro") {
        Some(m) => return Ok(Macro(m.to_owned(), formatting, affixes)),
        None => {}
    };
    match node.attribute("variable") {
        Some(m) => return Ok(Variable(
                m.to_owned(),
                attribute_optional2(node, "form", Form::from_str)?,
                formatting, affixes,
                Delimiter::from_node(node)?)),
        None => {}
    }
    match node.attribute("value") {
        Some(v) => return Ok(Const(v.to_owned(), formatting, affixes)),
        None => {}
    };
    Err(CslValidationError::new(node, "yeah".to_owned()))?
}

fn label_el(node: &Node) -> Result<Element, CslValidationError> {
    Ok(Element::Label(
            attribute_required(node, "variable")?,
            attribute_optional2(node, "form", Form::from_str)?,
            Formatting::from_node(node)?,
            Affixes::from_node(node)?,
            attribute_optional(node, "plural")?))
}

fn number_el(node: &Node) -> Result<Element, CslValidationError> {
    Ok(Element::Number(
            attribute_required(node, "variable")?,
            attribute_optional(node, "form")?,
            Formatting::from_node(node)?,
            Affixes::from_node(node)?,
            attribute_optional(node, "plural")?))
}

fn group_el(node: &Node) -> Result<Element, CslValidationError> {
    let elements: Result<Vec<_>, _> = node.children()
        .filter(|n| n.is_element())
        .map(|el| Element::from_node(&el)).collect();
    Ok(Element::Group(
            Formatting::from_node(node)?,
            Delimiter::from_node(node)?,
            elements?))
}

impl FromNode for Else {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        let elements: Result<Vec<_>, _> = node.children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el)).collect();
        Ok(Else(elements?))
    }
}

impl FromNode for Match {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        Ok(attribute_optional(node, "match")?)
    }
}

impl FromNode for Condition {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        Ok(Condition {
            match_type: Match::from_node(node)?,
            disambiguate: attribute_only_true(node, "disambiguate")?,
            is_numeric: attribute_array(node, "is-numeric")?,
            variable: attribute_array(node, "variable")?,
            position: attribute_array(node, "position")?,
            is_uncertain_date: attribute_array(node, "is-uncertain-date")?,
            csl_type: attribute_array(node, "type")?,
        })
    }
}

impl FromNode for IfThen {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        let elements: Result<Vec<_>, _> = node.children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el)).collect();
        Ok(IfThen(
                Condition::from_node(node)?,
                elements?
        ))
    }
}

fn choose_el(node: &Node) -> Result<Element, CslValidationError> {
    let els: Vec<Node> = node.children().filter(|n| n.is_element()).collect();

    let mut if_block: Option<IfThen> = None;
    let mut elseifs = vec![];
    let mut else_block = Else(vec![]);
    let mut seen_if = false;
    let mut seen_else = false;

    let unrecognised = |el, tag| {
        if tag == "if" || tag == "else-if" || tag == "else" {
            return Err(CslValidationError::new(el, format!("<choose> elements out of order; found <{}> in wrong position", tag)))
        }
        return Err(CslValidationError::new(el, format!("Unrecognised element {} in <choose>", tag)))
    };

    for el in els.into_iter() {
        let tn = el.tag_name();
        let tag = tn.name().to_owned();
        if !seen_if {
            if tag == "if" {
                seen_if = true;
                if_block = Some(IfThen::from_node(&el)?);
            } else {
                return Err(CslValidationError::new(&el, "<choose> blocks must begin with an <if>".into()))?;
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

    let _if = if_block.ok_or(CslValidationError::new(node, "<choose> blocks must have an <if>".into()))?;

    Ok(Element::Choose(
            _if,
            elseifs,
            else_block))
}

impl FromNode for NameLabel {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        Ok(NameLabel {
            form: attribute_optional2(node, "form", Form::from_str_names)?,
            formatting: Formatting::from_node(node)?,
            delimiter: Delimiter::from_node(node)?,
            plural: attribute_optional(node, "plural")?,
        })
    }
}

impl FromNode for Substitute {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        let els: Result<Vec<_>, _> = node.children()
            .filter(|n| n.is_element() && n.has_tag_name("name"))
            .map(|el| Element::from_node(&el)).collect();
        Ok(Substitute(els?))
    }
}

fn names_el(node: &Node) -> Result<Element, CslValidationError> {
    let variable: Vec<String> = attribute_string(node, "variable")
        .split(" ")
        .filter(|s| s.len() > 0)
        .map(|s| s.to_owned())
        .collect();

    let els: Vec<Node> = node.children().filter(|n| n.is_element()).collect();

    let mut names = vec![];
    let mut label = None;
    let mut substitute = None;
    let mut seen_label = false;
    let mut seen_subst = false;

    let unrecognised = |el, tag| {
        if tag == "name" || tag == "label" || tag == "substitute" {
            return Err(CslValidationError::new(el, format!("<names> elements out of order; found <{}> in wrong position", tag)))
        }
        return Err(CslValidationError::new(el, format!("Unrecognised element {} in <names>", tag)))
    };

    for el in els.into_iter() {
        let tag = el.tag_name().name().to_owned();
        println!("{}", tag);
        if !seen_label {
            if tag == "name" {
                names.push(Name::from_node(&el)?);
            } else if tag == "label" {
                seen_label = true;
                label = Some(NameLabel::from_node(&el)?);
            } else if tag == "substitute" {
                seen_subst = true;
                substitute = Some(Substitute::from_node(&el)?);
            }
        } else if !seen_subst {
            if tag == "label" {
                label = Some(NameLabel::from_node(&el)?);
            } else if tag == "substitute" {
                seen_subst = true;
                substitute = Some(Substitute::from_node(&el)?);
            } else {
                return unrecognised(&el, tag);
            }
        } else {
            return unrecognised(&el, tag);
        }
    }

    Ok(Element::Names(
            variable,
            names,
            label,
            Formatting::from_node(node)?,
            Delimiter::from_node(node)?,
            substitute))
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

impl FromNode for TextCase {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        Ok(attribute_optional(node, "text-case")?)
    }
}

fn disallow_default<T : Default + FromNode + IsOnNode>(node: &Node, disallow: bool) -> Result<T, CslValidationError> {
    if disallow {
        let attrs = T::is_on_node(node);
        if attrs.len() > 0 {
            Err(CslValidationError::new(node, format!("Disallowed attribute on node: {:?}", attrs)))?
        } else {
            Ok(T::default())
        }
    } else {
        T::from_node(node)
    }
}

impl DatePart {
    fn from_node(node: &Node, full: bool) -> Result<Self, CslValidationError> {
        let name: DatePartName = attribute_required(node, "name")?;
        let form = match name {
            DatePartName::Year => DatePartForm::Year(attribute_optional(node, "form")?),
            DatePartName::Month => DatePartForm::Month(attribute_optional(node, "form")?),
            DatePartName::Day => DatePartForm::Day(attribute_optional(node, "form")?),
        };
        Ok(DatePart {
            name,
            form,
            affixes: disallow_default(node, !full)?,
            formatting: disallow_default(node, !full)?,
            text_case: disallow_default(node, !full)?,
            range_delimiter: disallow_default(node, !full)?,
        })
    }
}

impl Date {
    fn from_node(node: &Node, is_in_locale: bool) -> Result<Self, CslValidationError> {
        let form: DateForm = attribute_optional(node, "form")?;
        let not_set = form == DateForm::NotSet;
        let full = if is_in_locale { true } else { not_set };
        let elements: Result<Vec<_>, _> = node.children()
            .filter(|n| n.is_element() && n.has_tag_name("date-part"))
            .map(|el| DatePart::from_node(&el, full)).collect();
        Ok(Date{
            form,
            date_parts: elements?,
            date_parts_attr: attribute_optional(node, "date-parts")?,
            affixes: Affixes::from_node(node)?,
            formatting: Formatting::from_node(node)?,
            delimiter: Delimiter::from_node(node)?,
        })
    }
}

impl FromNode for Element {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        match node.tag_name().name() {
            "text" => Ok(text_el(node)?),
            "label" => Ok(label_el(node)?),
            "group" => Ok(group_el(node)?),
            "number" => Ok(number_el(node)?),
            "names" => Ok(names_el(node)?),
            "choose" => Ok(choose_el(node)?),
            "date" => Ok(Element::Date(Date::from_node(node, false)?)),
            _ => Err(CslValidationError::new(node, "Dunno how to work that one".into()))?
        }
    }
}

pub fn get_toplevel<'a, 'd: 'a>(doc: &'d Document, nodename: &'static str) -> Result<Node<'a, 'd>, CslValidationError> {
    let root = doc.root_element();
    let matches = root.children().filter(|n| n.has_tag_name(nodename))
        .collect::<Vec<Node<'a, 'd>>>();
    if matches.len() > 1 {
        Err(CslValidationError::new(&root, "yeah".to_owned()))
    } else {
        // move matches into its first item
        matches.into_iter().nth(0).ok_or(CslValidationError::new(&root, "yeah".to_owned()))
    }
}

impl FromNode for MacroMap {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        let elements: Result<Vec<_>, _> = node.children()
            .filter(|n| n.is_element())
            .map(|el| Element::from_node(&el)).collect();
        let name = match node.attribute("name") {
            Some(n) => n,
            None => return Err(
                CslValidationError::new(
                    node,
                    "Macro must have a 'name' attribute.".into()))
        };
        Ok(MacroMap {
            name: name.to_owned(),
            elements: elements?
        })
    }
}

impl FromNode for NamePart {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        Ok(NamePart{
            name: attribute_required(node, "name")?,
            text_case: TextCase::from_node(node)?,
            formatting: Formatting::from_node(node)?,
        })
    }
}

impl FromNode for Name {
    fn from_node(node: &Node) -> Result<Self, CslValidationError> {
        Ok(Name{
            and: attribute_string(node, "and"),
            delimiter: Delimiter::from_node(node)?,
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
            formatting: Formatting::from_node(node)?,
            affixes: Affixes::from_node(node)?,
        })
    }
}

fn build_style_inner(doc: Document) ->Result<Style, CslValidationError> {
    let info_node = get_toplevel(&doc, "info")?;
    let locale_node = get_toplevel(&doc, "locale")?;
    let macros: Result<Vec<_>, _> = doc.root_element().children()
            .filter(|n| n.is_element() && n.has_tag_name("macro"))
            .map(|el| MacroMap::from_node(&el)).collect();
    let citation = Citation::from_node(&get_toplevel(&doc, "citation")?);

    Ok(Style{
        macros: macros?,
        citation: citation?,
        info: Info{},
        class: StyleClass::Note
    })
}

pub fn build_style(text: &String) -> Result<Style, StyleError> {
    let doc = Document::parse(text)?;
    let style = build_style_inner(doc)?;
    Ok(style)
}

pub fn drive_style(path: &str, text: &String) {
    match build_style(text) {
        Ok(style) => println!("{:?}", style),
        Err(e) => file_diagnostics(&vec![e], path.into(), text)
    }
}
