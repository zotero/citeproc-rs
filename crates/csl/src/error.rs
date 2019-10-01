// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::variables::*;
use roxmltree::Node;
use std::num::ParseIntError;
use std::ops::Range;

#[derive(Debug, PartialEq)]
pub struct UnknownAttributeValue {
    pub value: String,
}

impl UnknownAttributeValue {
    pub fn new(s: &str) -> Self {
        UnknownAttributeValue {
            value: s.to_owned(),
        }
    }
}

use serde::Serializer;

fn rox_error_serialize<S>(x: &roxmltree::Error, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&ToString::to_string(x))
}

#[derive(Debug, Serialize)]
pub enum StyleError {
    Invalid(CslError),
    ParseError(#[serde(serialize_with = "rox_error_serialize")] roxmltree::Error),
}

#[derive(Debug, Serialize)]
pub struct CslError(pub Vec<InvalidCsl>);

#[derive(Debug, PartialEq, Copy, Clone, Serialize)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct InvalidCsl {
    pub severity: Severity,
    // TODO: serialize_with or otherwise get this into the output
    pub range: Range<usize>,
    pub message: String,
    pub hint: String,
}

#[derive(Debug, AsRefStr)]
pub enum NeedVarType {
    Any,
    TextVariable,
    NumberVariable,
    Date,
    // in condition matchers
    CondDate,
    CondType,
    CondPosition,
    CondLocator,
    CondIsPlural,
    // in <name variable="">
    Name,
}

// TODO: create a trait for producing hints
impl NeedVarType {
    pub fn hint(
        &self,
        attr: &str,
        var: &str,
        maybe_got: Option<AnyVariable>,
    ) -> (String, String, Severity) {
        use self::NeedVarType::*;
        let wrong_type_var = format!("Wrong variable type for `{}`: \"{}\"", attr, var);
        let empty = "".to_string();
        let unknown = (
            format!("Unknown variable \"{}\"", var),
            empty.clone(),
            Severity::Error,
        );
        match *self {

            Any => unknown,

            TextVariable => maybe_got.map(|got| {
                let wrong_type = "Wrong variable type for <text>".to_string();
                use crate::variables::AnyVariable::*;
                match got {
                    Name(_) => (wrong_type, "Hint: use <names> instead".to_string(), Severity::Error),
                    Date(_) => (wrong_type, "Hint: use <date> instead".to_string(), Severity::Error),
                    // this would be trying to print an error when the input was correct
                    _ => (empty, "???".to_string(), Severity::Warning),
                }
            }).unwrap_or(unknown),

            NumberVariable => maybe_got.map(|got| {
                let wrong_type = "Wrong variable type for <number>".to_string();
                use crate::variables::AnyVariable::*;
                match got {
                    Ordinary(_) => (wrong_type,
                                    format!("Hint: use <text variable=\"{}\" /> instead", var),
                                    Severity::Error),
                    Name(_) => (wrong_type, "Hint: use <names> instead".to_string(), Severity::Error),
                    Date(_) => (wrong_type, "Hint: use <date> instead".to_string(), Severity::Error),
                    // this would be trying to print an error when the input was correct
                    _ => (empty, "???".to_string(), Severity::Warning),
                }
            }).unwrap_or(unknown),

            CondDate => (wrong_type_var,
                                    format!("Hint: `{}` can only match date variables", attr),
                                    Severity::Error),

            CondType => (wrong_type_var,
                         "Hint: `type` can only match known types".to_string(),
                         Severity::Error),

            CondPosition => (wrong_type_var,
                             "Hint: `position` matches {{ first | ibid | ibid-with-locator | subsequent | near-note | far-note }}*".to_string(),
                             Severity::Error),

            CondLocator => (wrong_type_var, "Hint: `locator` only matches locator types".to_string(), Severity::Error),
            CondIsPlural => (wrong_type_var, "Hint: `is-plural` only matches name variables".to_string(), Severity::Error),
            Date => (wrong_type_var, "<date variable=\"...\"> can only render dates".to_string(), Severity::Error),
            Name => (wrong_type_var, "Hint: <names> can only render name variables".to_string(), Severity::Error),
        }
    }
}

impl InvalidCsl {
    pub fn new(node: &Node, message: &str) -> Self {
        let range = node.range();
        InvalidCsl {
            range,
            severity: Severity::Error,
            hint: "".to_string(),
            message: message.to_owned(),
        }
    }

    pub fn bad_int(node: &Node, attr: &str, uav: &ParseIntError) -> Self {
        let at = node.attribute_node(attr).unwrap();
        let range = at.range();
        InvalidCsl {
            range,
            message: format!("Invalid integer value for {}: {:?}", attr, uav),
            hint: "".to_string(),
            severity: Severity::Error,
        }
    }

    pub fn missing(node: &Node, attr: &str) -> Self {
        InvalidCsl::new(node, &format!("Must have `{}` attribute", attr))
    }

    pub fn attr_val(node: &Node, attr: &str, uav: &str) -> Self {
        let at = node.attribute_node(attr).unwrap();
        let range = at.range();
        InvalidCsl {
            range,
            message: format!("Unknown attribute value for `{}`: \"{}\"", attr, uav),
            hint: "".to_string(),
            severity: Severity::Error,
        }
    }

    pub fn wrong_var_type(
        node: &Node,
        attr: &str,
        uav: &str,
        needed: NeedVarType,
        got: Option<AnyVariable>,
    ) -> Self {
        let at = node.attribute_node(attr).unwrap();
        let range = at.range();
        let (message, hint, severity) = needed.hint(attr, uav, got);
        InvalidCsl {
            range,
            message,
            hint,
            severity,
        }
    }
}

impl From<roxmltree::Error> for StyleError {
    fn from(err: roxmltree::Error) -> StyleError {
        StyleError::ParseError(err)
    }
}

impl From<CslError> for StyleError {
    fn from(err: CslError) -> StyleError {
        StyleError::Invalid(err)
    }
}

impl From<Vec<CslError>> for CslError {
    fn from(errs: Vec<CslError>) -> CslError {
        // concat all of the sub-vecs into one
        let mut collect = Vec::with_capacity(errs.len());
        for err in errs {
            collect.extend_from_slice(&err.0);
        }
        CslError(collect)
    }
}

impl From<InvalidCsl> for CslError {
    fn from(err: InvalidCsl) -> CslError {
        CslError(vec![err])
    }
}

impl From<InvalidCsl> for StyleError {
    fn from(err: InvalidCsl) -> StyleError {
        StyleError::Invalid(CslError(vec![err]))
    }
}

impl Default for StyleError {
    fn default() -> Self {
        StyleError::Invalid(CslError(vec![InvalidCsl {
            severity: Severity::Error,
            range: 1usize..2usize,
            hint: "".to_string(),
            message: "".to_string(),
        }]))
    }
}

pub(crate) trait PartitionResults<O, E>: Iterator<Item = Result<O, E>>
where
    O: Sized,
    Self: Sized,
{
    fn partition_results<'a>(self) -> Result<Vec<O>, Vec<E>> {
        let mut errors = Vec::new();
        let oks = self
            .filter_map(|res| match res {
                Ok(ok) => Some(ok),
                Err(e) => {
                    errors.push(e);
                    None
                }
            })
            .collect();
        if !errors.is_empty() {
            Err(errors)
        } else {
            Ok(oks)
        }
    }
}

impl<O, E, I: Iterator<Item = Result<O, E>>> PartitionResults<O, E> for I {}
