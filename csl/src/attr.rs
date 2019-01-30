use super::version::CslVariant;
use crate::error::{InvalidCsl, UnknownAttributeValue, NeedVarType};
use crate::Atom;
use roxmltree::{ExpandedName, Node};
use std::str::FromStr;
use strum::EnumProperty;

// Temporary
pub const CSL_VARIANT: CslVariant = CslVariant::CslM;

pub trait GetAttribute
where
    Self: Sized,
{
    fn get_attr(s: &str, csl_variant: CslVariant) -> Result<Self, UnknownAttributeValue>;
}

impl<T: FromStr + EnumProperty> GetAttribute for T {
    fn get_attr(s: &str, csl_variant: CslVariant) -> Result<Self, UnknownAttributeValue> {
        match T::from_str(s) {
            Ok(a) => csl_variant
                .filter_arg(a)
                .ok_or_else(|| UnknownAttributeValue::new(s)),
            Err(_) => Err(UnknownAttributeValue::new(s)),
        }
    }
}

pub fn attribute_bool(node: &Node, attr: &str, default: bool) -> Result<bool, InvalidCsl> {
    match node.attribute(attr) {
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        None => Ok(default),
        Some(s) => Err(InvalidCsl::attr_val(node, attr, s))?,
    }
}

pub fn attribute_option_bool(node: &Node, attr: &str) -> Result<Option<bool>, InvalidCsl> {
    match node.attribute(attr) {
        Some("true") => Ok(Some(true)),
        Some("false") => Ok(Some(false)),
        None => Ok(None),
        Some(s) => Err(InvalidCsl::attr_val(node, attr, s))?,
    }
}

pub fn attribute_int(node: &Node, attr: &str, default: u32) -> Result<u32, InvalidCsl> {
    match node.attribute(attr) {
        Some(s) => {
            let parsed = u32::from_str_radix(s, 10);
            parsed.map_err(|e| InvalidCsl::bad_int(node, attr, &e))
        }
        None => Ok(default),
    }
}

pub fn attribute_option_int(node: &Node, attr: &str) -> Result<Option<u32>, InvalidCsl> {
    match node.attribute(attr) {
        Some(s) => {
            let parsed = u32::from_str_radix(s, 10);
            parsed
                .map(Some)
                .map_err(|e| InvalidCsl::bad_int(node, attr, &e))
        }
        None => Ok(None),
    }
}

pub fn attribute_string(node: &Node, attr: &str) -> String {
    node.attribute(attr)
        .map(String::from)
        .unwrap_or_else(|| String::from(""))
}

pub fn attribute_atom(node: &Node, attr: &str) -> Atom {
    node.attribute(attr)
        .map(Atom::from)
        .unwrap_or_else(|| Atom::from(""))
}

pub fn attribute_atom_default(node: &Node, attr: &str, default: Atom) -> Atom {
    node.attribute(attr).map(Atom::from).unwrap_or(default)
}

pub fn attribute_option_atom(node: &Node, attr: &str) -> Option<Atom> {
    node.attribute(attr).map(Atom::from)
}

pub fn attribute_required<T: GetAttribute>(node: &Node, attr: &str) -> Result<T, InvalidCsl> {
    match node.attribute(attr) {
        Some(a) => match T::get_attr(a, CSL_VARIANT) {
            Ok(val) => Ok(val),
            Err(e) => Err(InvalidCsl::attr_val(node, attr, &e.value)),
        },
        None => Err(InvalidCsl::missing(node, attr)),
    }
}

use super::variables::*;

pub fn attribute_var_type<T: GetAttribute>(
    node: &Node,
    attr: &str,
    need: NeedVarType,
) -> Result<T, InvalidCsl> {
    match node.attribute(attr) {
        Some(a) => match T::get_attr(a, CSL_VARIANT) {
            Ok(val) => Ok(val),
            Err(e) => Err(InvalidCsl::wrong_var_type(
                node,
                attr,
                &e.value,
                need,
                AnyVariable::get_attr(a, CSL_VARIANT).ok(),
            )),
        },
        None => Err(InvalidCsl::new(
            node,
            &format!("Must have '{}' attribute", attr),
        )),
    }
}

pub fn attribute_option<'a, 'd: 'a, T: GetAttribute>(
    node: &Node<'a, 'd>,
    attr: impl Into<ExpandedName<'a>> + Clone,
) -> Result<Option<T>, InvalidCsl> {
    match node.attribute(attr.clone()) {
        Some(a) => match T::get_attr(a, CSL_VARIANT) {
            Ok(val) => Ok(Some(val)),
            Err(e) => Err(InvalidCsl::attr_val(
                node,
                &format!("{:?}", attr.into()),
                &e.value,
            )),
        },
        None => Ok(None),
    }
}

pub fn attribute_optional<T: Default + GetAttribute>(
    node: &Node,
    attr: &str,
) -> Result<T, InvalidCsl> {
    match node.attribute(attr) {
        Some(a) => match T::get_attr(a, CSL_VARIANT) {
            Ok(val) => Ok(val),
            Err(e) => Err(InvalidCsl::attr_val(node, attr, &e.value)),
        },
        None => Ok(T::default()),
    }
}

pub fn attribute_array_var<T: GetAttribute>(
    node: &Node,
    attr: &str,
    need: NeedVarType,
) -> Result<Vec<T>, InvalidCsl> {
    match node.attribute(attr) {
        Some(array) => {
            let split: Result<Vec<_>, _> = array
                .split(' ')
                .filter(|a| a.len() > 0)
                .map(|a| T::get_attr(a, CSL_VARIANT))
                .collect();
            match split {
                Ok(val) => Ok(val),
                Err(e) => Err(InvalidCsl::wrong_var_type(
                    node,
                    attr,
                    &e.value,
                    need,
                    AnyVariable::get_attr(&e.value, CSL_VARIANT).ok(),
                )),
            }
        }
        None => Ok(vec![]),
    }
}

pub fn attribute_array<T: GetAttribute>(node: &Node, attr: &str) -> Result<Vec<T>, InvalidCsl> {
    match node.attribute(attr) {
        Some(array) => {
            let split: Result<Vec<_>, _> = array
                .split(' ')
                .filter(|a| a.len() > 0)
                .map(|a| T::get_attr(a, CSL_VARIANT))
                .collect();
            match split {
                Ok(val) => Ok(val),
                Err(e) => Err(InvalidCsl::attr_val(node, attr, &e.value)),
            }
        }
        None => Ok(vec![]),
    }
}
