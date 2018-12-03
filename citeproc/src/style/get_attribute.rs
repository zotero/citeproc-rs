use super::version::CslVersion;
use crate::style::error::*;
use roxmltree::Node;
use std::str::FromStr;
use strum::EnumProperty;

// Temporary
pub const CSL_VERSION: CslVersion = CslVersion::Csl101;

pub trait GetAttribute
where
    Self: Sized,
{
    fn get_attr(s: &str, csl_version: CslVersion) -> Result<Self, UnknownAttributeValue>;
}

impl<T: FromStr + EnumProperty> GetAttribute for T {
    fn get_attr(s: &str, csl_version: CslVersion) -> Result<Self, UnknownAttributeValue> {
        match T::from_str(s) {
            Ok(a) => csl_version
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

pub fn attribute_only_true(node: &Node, attr: &str) -> Result<bool, InvalidCsl> {
    match node.attribute(attr) {
        Some("true") => Ok(true),
        None => Ok(false),
        Some(s) => Err(InvalidCsl::attr_val(node, attr, s)),
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

pub fn attribute_string(node: &Node, attr: &str) -> String {
    node.attribute(attr)
        .map(String::from)
        .unwrap_or_else(|| String::from(""))
}

pub fn attribute_required<T: GetAttribute>(node: &Node, attr: &str) -> Result<T, InvalidCsl> {
    match node.attribute(attr) {
        Some(a) => match T::get_attr(a, CSL_VERSION) {
            Ok(val) => Ok(val),
            Err(e) => Err(InvalidCsl::attr_val(node, attr, &e.value)),
        },
        None => Err(InvalidCsl::new(
            node,
            &format!("Must have '{}' attribute", attr),
        )),
    }
}

use super::variables::*;

pub fn attribute_var_type<T: GetAttribute>(
    node: &Node,
    attr: &str,
    need: NeedVarType,
) -> Result<T, InvalidCsl> {
    match node.attribute(attr) {
        Some(a) => match T::get_attr(a, CSL_VERSION) {
            Ok(val) => Ok(val),
            Err(e) => Err(InvalidCsl::wrong_var_type(
                node,
                attr,
                &e.value,
                need,
                AnyVariable::get_attr(a, CSL_VERSION).ok(),
            )),
        },
        None => Err(InvalidCsl::new(
            node,
            &format!("Must have '{}' attribute", attr),
        )),
    }
}

pub fn attribute_optional<T: Default + GetAttribute>(
    node: &Node,
    attr: &str,
) -> Result<T, InvalidCsl> {
    match node.attribute(attr) {
        Some(a) => match T::get_attr(a, CSL_VERSION) {
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
                .map(|a| T::get_attr(a, CSL_VERSION))
                .collect();
            match split {
                Ok(val) => Ok(val),
                Err(e) => Err(InvalidCsl::wrong_var_type(
                    node,
                    attr,
                    &e.value,
                    need,
                    AnyVariable::get_attr(&e.value, CSL_VERSION).ok(),
                )),
            }
        }
        None => Ok(vec![]),
    }
}
