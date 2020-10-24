// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::error::ExpName;
use crate::error::{InvalidCsl, NeedVarType, UnknownAttributeValue};
use crate::version::Features;
use crate::Atom;
use crate::ParseInfo;
use roxmltree::Node;
use std::str::FromStr;
use strum::EnumProperty;

pub trait GetAttribute
where
    Self: Sized,
{
    fn get_attr(s: &str, features: &Features) -> Result<Self, UnknownAttributeValue>;
}

impl<T: FromStr + EnumProperty> GetAttribute for T {
    fn get_attr(s: &str, features: &Features) -> Result<Self, UnknownAttributeValue> {
        match T::from_str(s) {
            Ok(a) => features
                .filter_arg(a)
                .ok_or_else(|| UnknownAttributeValue::new(s)),
            Err(_) => Err(UnknownAttributeValue::new(s)),
        }
    }
}

pub(crate) fn attribute_bool(
    node: &Node,
    attr: &'static str,
    default: bool,
) -> Result<bool, InvalidCsl> {
    match node.attribute(attr) {
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        None => Ok(default),
        Some(s) => Err(InvalidCsl::attr_val(node, attr, s)),
    }
}

pub(crate) fn attribute_option_bool(
    node: &Node,
    attr: &'static str,
) -> Result<Option<bool>, InvalidCsl> {
    match node.attribute(attr) {
        Some("true") => Ok(Some(true)),
        Some("false") => Ok(Some(false)),
        None => Ok(None),
        Some(s) => Err(InvalidCsl::attr_val(node, attr, s)),
    }
}

pub(crate) fn attribute_int(
    node: &Node,
    attr: &'static str,
    default: u32,
) -> Result<u32, InvalidCsl> {
    match node.attribute(attr) {
        Some(s) => {
            let parsed = u32::from_str_radix(s.trim(), 10);
            parsed.map_err(|e| InvalidCsl::bad_int(node, attr, &e))
        }
        None => Ok(default),
    }
}

pub(crate) fn attribute_option_int(
    node: &Node,
    attr: &'static str,
) -> Result<Option<u32>, InvalidCsl> {
    match node.attribute(attr) {
        Some(s) => {
            let parsed = u32::from_str_radix(s.trim(), 10);
            parsed
                .map(Some)
                .map_err(|e| InvalidCsl::bad_int(node, attr, &e))
        }
        None => Ok(None),
    }
}

pub(crate) fn attribute_string(node: &Node, attr: &'static str) -> String {
    node.attribute(attr)
        .map(String::from)
        .unwrap_or_else(|| String::from(""))
}

pub(crate) fn attribute_atom(node: &Node, attr: &'static str) -> Atom {
    node.attribute(attr)
        .map(Atom::from)
        .unwrap_or_else(|| Atom::from(""))
}

pub(crate) fn attribute_atom_default(node: &Node, attr: &'static str, default: Atom) -> Atom {
    node.attribute(attr).map(Atom::from).unwrap_or(default)
}

pub(crate) fn attribute_option_atom(node: &Node, attr: &'static str) -> Option<Atom> {
    node.attribute(attr).map(Atom::from)
}

pub(crate) fn attribute_required<T: GetAttribute>(
    node: &Node,
    attr: impl Into<ExpName>,
    info: &ParseInfo,
) -> Result<T, InvalidCsl> {
    let attr = attr.into();
    match node.attribute(attr) {
        Some(a) => match T::get_attr(a, &info.features) {
            Ok(val) => Ok(val),
            Err(e) => Err(InvalidCsl::attr_val(node, attr, &e.value)),
        },
        None => Err(InvalidCsl::missing(node, attr)),
    }
}

use super::variables::*;

pub(crate) fn attribute_var_type<T: GetAttribute>(
    node: &Node,
    attr: &'static str,
    need: NeedVarType,
    info: &ParseInfo,
) -> Result<T, InvalidCsl> {
    match node.attribute(attr) {
        Some(a) => match T::get_attr(a, &info.features) {
            Ok(val) => Ok(val),
            Err(e) => Err(InvalidCsl::wrong_var_type(
                node,
                attr,
                &e.value,
                need,
                AnyVariable::get_attr(a, &info.features).ok(),
            )),
        },
        None => Err(InvalidCsl::new(
            node,
            &format!("Must have '{:?}' attribute", attr),
        )),
    }
}

pub(crate) fn attribute_option<T: GetAttribute>(
    node: &Node,
    attr: impl Into<ExpName>,
    info: &ParseInfo,
) -> Result<Option<T>, InvalidCsl> {
    let attr = attr.into();
    match node.attribute(attr.clone()) {
        Some(a) => match T::get_attr(a, &info.features) {
            Ok(val) => Ok(Some(val)),
            Err(e) => Err(InvalidCsl::attr_val(node, attr, &e.value)),
        },
        None => Ok(None),
    }
}

pub(crate) fn attribute_optional<T: Default + GetAttribute>(
    node: &Node,
    attr: impl Into<ExpName>,
    info: &ParseInfo,
) -> Result<T, InvalidCsl> {
    let attr = attr.into();
    match node.attribute(attr) {
        Some(a) => match T::get_attr(a, &info.features) {
            Ok(val) => Ok(val),
            Err(e) => Err(InvalidCsl::attr_val(node, attr, &e.value)),
        },
        None => Ok(T::default()),
    }
}

pub(crate) fn attribute_array_var<T: GetAttribute>(
    node: &Node,
    attr: &'static str,
    need: NeedVarType,
    info: &ParseInfo,
) -> Result<Vec<T>, InvalidCsl> {
    match node.attribute(attr) {
        Some(array) => {
            let split: Result<Vec<_>, _> = array
                .split(' ')
                .filter(|a| !a.is_empty())
                .map(|a| T::get_attr(a, &info.features))
                .collect();
            match split {
                Ok(val) => Ok(val),
                Err(e) => Err(InvalidCsl::wrong_var_type(
                    node,
                    attr,
                    &e.value,
                    need,
                    AnyVariable::get_attr(&e.value, &info.features).ok(),
                )),
            }
        }
        None => Ok(vec![]),
    }
}

pub(crate) fn attribute_array<T: GetAttribute>(
    node: &Node,
    attr: &'static str,
    info: &ParseInfo,
) -> Result<Vec<T>, InvalidCsl> {
    match node.attribute(attr) {
        Some(array) => {
            let split: Result<Vec<_>, _> = array
                .split(' ')
                .filter(|a| !a.is_empty())
                .map(|a| T::get_attr(a, &info.features))
                .collect();
            match split {
                Ok(val) => Ok(val),
                Err(e) => Err(InvalidCsl::attr_val(node, attr, &e.value)),
            }
        }
        None => Ok(vec![]),
    }
}
