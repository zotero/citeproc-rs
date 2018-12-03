use super::variables::*;
use codespan::{CodeMap, FileMap, Span};
use codespan_reporting::termcolor::{ColorChoice, StandardStream};
use codespan_reporting::{emit, Diagnostic, Label, Severity};
use failure::Fail;
use roxmltree::{Error, Node, TextPos};
use std::num::ParseIntError;

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

#[derive(Debug, Fail)]
pub enum StyleError {
    #[fail(display = "TODO")]
    Invalid(CslError),
    #[fail(display = "TODO")]
    ParseError(#[fail(cause)] Error),
}

#[derive(Debug)]
pub struct CslError(pub Vec<InvalidCsl>);

#[derive(Fail, Debug, PartialEq, Clone)]
#[fail(display = "{}", message)]
pub struct InvalidCsl {
    pub severity: Severity,
    pub text_pos: TextPos,
    pub len: usize,
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
    CondIsUncertainDate,
    CondIsNumeric,
    CondType,
    CondPosition,
    CondLocator,
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
        let wrong_type = format!("Wrong variable type for `{}`", attr);
        let wrong_type_var = format!("Wrong variable type for `{}`: \"{}\"", attr, var);
        let empty = format!("");
        let unknown = (
            format!("Unknown variable \"{}\"", var),
            empty.clone(),
            Severity::Error,
        );
        match *self {

            Any => unknown,

            TextVariable => maybe_got.map(|got| {
                use crate::style::variables::AnyVariable::*;
                match got {
                    Number(_) => (wrong_type,
                                  format!("Hint: use <number variable=\"{}\" /> instead", var),
                                  Severity::Error),
                    Name(_) => (wrong_type, format!("Hint: use <names> instead"), Severity::Error),
                    Date(_) => (wrong_type, format!("Hint: use <date> instead"), Severity::Error),
                    // this would be trying to print an error when the input was correct
                    _ => (empty, format!("???"), Severity::Warning),
                }
            }).unwrap_or(unknown),

            NumberVariable => maybe_got.map(|got| {
                use crate::style::variables::AnyVariable::*;
                match got {
                    Standard(_) => (wrong_type,
                                    format!("Hint: use <text variable=\"{}\" /> instead", var),
                                    Severity::Error),
                    Name(_) => (wrong_type, format!("Hint: use <names> instead"), Severity::Error),
                    Date(_) => (wrong_type, format!("Hint: use <date> instead"), Severity::Error),
                    // this would be trying to print an error when the input was correct
                    _ => (empty, format!("???"), Severity::Warning),
                }
            }).unwrap_or(unknown),

            CondIsNumeric => {
                maybe_got
                    .map(|_| (wrong_type_var,
                              format!("Hint: `is-numeric` can only match numeric variables"),
                              Severity::Error))
                    .unwrap_or(unknown)
            }

            CondIsUncertainDate => (wrong_type_var,
                                    format!("Hint: `is-uncertain-date` can only match date variables"),
                                    Severity::Error),

            CondType => (wrong_type_var,
                         format!("Hint: `type` can only match known types"),
                         Severity::Error),

            CondPosition => (wrong_type_var,
                             format!("Hint: `position` matches {{ first | ibid | ibid-with-locator | subsequent | near-note }}*"),
                             Severity::Error),

            CondLocator => (wrong_type_var, format!("Hint: `locator` only matches locator types"), Severity::Error),
            Date => (wrong_type_var, format!("<date variable=\"...\"> can only render dates"), Severity::Error),
            Name => (wrong_type_var, format!("Hint: <names> can only render name variables"), Severity::Error),
        }
    }
}

impl InvalidCsl {
    pub fn new(node: &Node, message: &str) -> Self {
        let mut pos = node.node_pos();
        pos.col += 1;
        InvalidCsl {
            text_pos: pos,
            len: node.tag_name().name().len(),
            severity: Severity::Error,
            hint: format!(""),
            message: message.to_owned(),
        }
    }

    pub fn bad_int(node: &Node, attr: &str, uav: &ParseIntError) -> Self {
        InvalidCsl {
            text_pos: node
                .attribute_value_pos(attr)
                .unwrap_or_else(|| node.node_pos()),
            len: node.attribute("attr").map(|a| a.len()).unwrap_or_else(|| 1),
            message: format!("Invalid integer value for {}: {:?}", attr, uav),
            hint: format!(""),
            severity: Severity::Error,
        }
    }

    pub fn attr_val(node: &Node, attr: &str, uav: &str) -> Self {
        let full_val = node.attribute(attr);
        InvalidCsl {
            text_pos: node
                .attribute_value_pos(attr)
                .unwrap_or_else(|| node.node_pos()),
            len: full_val.map(|v| v.len()).unwrap_or(1),
            message: format!("Unknown attribute value for {}: \"{}\"", attr, uav),
            hint: format!(""),
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
        let full_val = node.attribute(attr);
        let (message, hint, severity) = needed.hint(attr, uav, got);
        InvalidCsl {
            text_pos: node
                .attribute_value_pos(attr)
                .unwrap_or_else(|| node.node_pos()),
            len: full_val.map(|v| v.len()).unwrap_or(1),
            message,
            hint,
            severity,
        }
    }

    pub fn to_diagnostic(&self, file_map: &FileMap) -> Option<Diagnostic> {
        let str_start = file_map
            .byte_index(
                (self.text_pos.row - 1 as u32).into(),
                (self.text_pos.col - 1 as u32).into(),
            )
            .ok()?;

        let label = Label::new_primary(Span::from_offset(str_start, (self.len as i64).into()))
            .with_message(format!("{}", self.hint));
        let diag = Diagnostic::new(self.severity, self.message.clone()).with_label(label);
        Some(diag)
    }
}

impl From<Error> for StyleError {
    fn from(err: Error) -> StyleError {
        StyleError::ParseError(err)
    }
}

impl From<Vec<CslError>> for CslError {
    fn from(errs: Vec<CslError>) -> CslError {
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

fn get_pos(e: &Error) -> TextPos {
    use xmlparser::Error as XP;
    let pos = match *e {
        Error::InvalidXmlPrefixUri(pos) => pos,
        Error::UnexpectedXmlUri(pos) => pos,
        Error::UnexpectedXmlnsUri(pos) => pos,
        Error::InvalidElementNamePrefix(pos) => pos,
        Error::DuplicatedNamespace(ref _name, pos) => pos,
        Error::UnexpectedCloseTag { pos, .. } => pos,
        Error::UnexpectedEntityCloseTag(pos) => pos,
        Error::UnknownEntityReference(ref _name, pos) => pos,
        Error::EntityReferenceLoop(pos) => pos,
        Error::DuplicatedAttribute(ref _name, pos) => pos,
        Error::ParserError(ref err) => match *err {
            XP::InvalidToken(_, pos, _) => pos,
            XP::UnexpectedToken(_, pos) => pos,
            XP::UnknownToken(pos) => pos,
        },
        _ => TextPos::new(1, 1),
    };
    // make sure, because 0 = panic further down
    TextPos {
        row: if pos.row < 1 { 1 } else { pos.row },
        col: if pos.col < 1 { 1 } else { pos.col },
    }
}

impl Default for StyleError {
    fn default() -> Self {
        StyleError::Invalid(CslError(vec![InvalidCsl {
            severity: Severity::Error,
            text_pos: TextPos::new(1, 1),
            len: 0,
            hint: format!(""),
            message: format!(""),
        }]))
    }
}

impl StyleError {
    pub fn diagnostics(&self, file_map: &FileMap) -> Vec<Result<Diagnostic, String>> {
        match *self {
            StyleError::Invalid(ref invs) => invs
                .0
                .iter()
                .map(|e| e.to_diagnostic(file_map).ok_or(e.message.clone()))
                .collect(),
            StyleError::ParseError(ref e) => {
                let pos = get_pos(&e);

                let str_start =
                    file_map.byte_index((pos.row - 1 as u32).into(), (pos.col - 1 as u32).into());

                if let Ok(start) = str_start {
                    vec![Ok(Diagnostic::new(Severity::Error, format!("{}", e))
                        .with_label(
                            Label::new_primary(Span::from_offset(start, (1 as i64).into()))
                                .with_message(""),
                        ))]
                } else {
                    vec![]
                }
            }
        }
    }
}

pub fn file_diagnostics<'a>(err: &StyleError, filename: &'a str, document: &'a str) {
    let mut code_map = CodeMap::new();
    let file_map = code_map.add_filemap(filename.to_owned().into(), document.to_string());
    let writer = StandardStream::stderr(ColorChoice::Auto);
    for diag in err.diagnostics(&file_map) {
        if let Ok(d) = diag {
            emit(&mut writer.lock(), &code_map, &d).unwrap();
            eprintln!();
        } else if let Err(emsg) = diag {
            eprintln!("{}", emsg);
        }
    }
}
