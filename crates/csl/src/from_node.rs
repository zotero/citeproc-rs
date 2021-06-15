// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2020 Corporation for Digital Scholarship

use crate::error::{ChildGetterError, ChildGetterResult, CslError, InvalidCsl};
use crate::version::Features;
use crate::SmartString;
use fnv::FnvHashSet;
use roxmltree::{Attribute, Node};

#[allow(dead_code)]
#[cfg(test)]
pub(crate) fn parse_as<T>(s: &str) -> FromNodeResult<T>
where
    T: FromNode,
{
    let doc = roxmltree::Document::parse(s).unwrap();
    T::from_node(&doc.root_element(), &ParseInfo::default())
}

#[cfg(test)]
pub(crate) fn parse_as_with<T>(s: &str, options: Option<ParseOptions>) -> FromNodeResult<T>
where
    T: FromNode,
{
    let doc = roxmltree::Document::parse(s).unwrap();
    let o = options.unwrap_or_else(Default::default);
    let info = ParseInfo {
        features: o.features.clone().unwrap_or_else(Default::default),
        macros: None,
        options: o,
    };
    T::from_node(&doc.root_element(), &info)
}

#[derive(Default, Debug, Clone)]
pub struct ParseOptions {
    /// Allow style to omit the `<info>` block (good for tests).
    pub allow_no_info: bool,
    /// Feature overrides. Allows you to enable features programmatically. Features declared in the
    /// style will be added to this.
    pub features: Option<Features>,
    #[doc(hidden)]
    pub use_default_default: private::CannotConstruct,
}

mod private {
    #[derive(Clone, Default, Debug)]
    #[non_exhaustive]
    pub struct CannotConstruct;
}

#[derive(Debug, Default)]
pub(crate) struct ParseInfo {
    pub(crate) features: Features,
    pub(crate) options: ParseOptions,
    pub(crate) macros: Option<FnvHashSet<SmartString>>,
}

pub(crate) type FromNodeResult<T> = Result<T, CslError>;

pub(crate) trait FromNode
where
    Self: Sized,
{
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self>;

    /// Used to filter a node's children and extract the relevant ones
    fn select_child(_child_node: &Node) -> bool {
        false
    }
    const CHILD_DESC: &'static str = "unimplemented";
}

pub(crate) fn exactly_one_child<T: FromNode>(
    node: &Node,
    info: &ParseInfo,
    errors: &mut Vec<InvalidCsl>,
) -> ChildGetterResult<T> {
    let mut iter = node.children().filter(T::select_child);
    if let Some(child) = iter.next() {
        if iter.next().is_some() {
            errors.push(InvalidCsl::new(
                node,
                format!("Cannot have more than one <{}>", T::CHILD_DESC),
            ));
            return Err(ChildGetterError);
        }
        append_err!(T::from_node(&child, info), errors)
    } else {
        errors.push(InvalidCsl::new(
            node,
            format!("Must have exactly one <{}>", T::CHILD_DESC),
        ));
        Err(ChildGetterError)
    }
}

pub(crate) fn max_one_child<T: FromNode>(
    node: &Node,
    info: &ParseInfo,
    errors: &mut Vec<InvalidCsl>,
) -> ChildGetterResult<Option<T>> {
    let mut iter = node.children().filter(T::select_child);
    if let Some(child) = iter.next() {
        if iter.next().is_some() {
            errors.push(InvalidCsl::new(
                node,
                format!("Cannot have more than one <{}>", T::CHILD_DESC),
            ));
            return Err(ChildGetterError);
        }
        append_err!(T::from_node(&child, info), errors).map(Some)
    } else {
        Ok(None)
    }
}

pub(crate) fn many_children<T: FromNode>(
    node: &Node,
    info: &ParseInfo,
    errors: &mut Vec<InvalidCsl>,
) -> ChildGetterResult<Vec<T>> {
    let mut iter = node.children().filter(T::select_child);
    let mut results = Vec::new();
    while let Some(child) = iter.next() {
        if let Ok(ch) = append_err!(T::from_node(&child, info), errors) {
            results.push(ch);
        }
    }
    Ok(results)
}

pub(crate) trait AttrChecker
where
    Self: Sized,
{
    fn filter_attribute(attr: &str) -> bool;
    fn filter_attribute_full(a: &Attribute) -> bool {
        Self::filter_attribute(a.name())
    }
    fn is_on_node<'a>(node: &'a Node) -> bool {
        node.attributes()
            .iter()
            .find(|a| Self::filter_attribute_full(a))
            != None
    }
    fn relevant_attrs<'a>(node: &'a Node) -> Vec<String> {
        node.attributes()
            .iter()
            .filter(|a| Self::filter_attribute_full(a))
            .map(|a| String::from(a.name()))
            .collect()
    }
}

impl<T> FromNode for Option<T>
where
    T: AttrChecker + FromNode,
{
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        if T::is_on_node(node) {
            Ok(Some(T::from_node(node, info)?))
        } else {
            Ok(None)
        }
    }
}
