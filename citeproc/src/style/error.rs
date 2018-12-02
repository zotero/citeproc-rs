use codespan::{CodeMap, FileMap, Span};
use codespan_reporting::termcolor::{ColorChoice, StandardStream};
use codespan_reporting::{emit, Diagnostic, Label, Severity};
use roxmltree::{Error, Node, TextPos};
use std::num::ParseIntError;
use failure::Fail;

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
    #[fail(display="TODO")]
    Invalid(CslError),
    #[fail(display="TODO")]
    ParseError(#[fail(cause)] Error),
}

#[derive(Debug)]
pub struct CslError(pub Vec<InvalidCsl>);

#[derive(Fail, Debug, PartialEq, Clone)]
#[fail(display="{}", message)]
pub struct InvalidCsl {
    pub severity: Severity,
    pub text_pos: TextPos,
    pub len: usize,
    pub message: String,
}

impl InvalidCsl {
    pub fn new(node: &Node, message: String) -> Self {
        let mut pos = node.node_pos();
        pos.col += 1;
        InvalidCsl {
            text_pos: pos,
            len: node.tag_name().name().len(),
            severity: Severity::Error,
            message,
        }
    }

    pub fn bad_int(node: &Node, attr: &str, uav: &ParseIntError) -> Self {
        InvalidCsl {
            text_pos: node
                .attribute_value_pos(attr)
                .unwrap_or_else(|| node.node_pos()),
            len: node.attribute("attr").map(|a| a.len()).unwrap_or_else(|| 1),
            message: format!("Invalid integer value for {}: {:?}", attr, uav),
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
            severity: Severity::Error,
        }
    }

    pub fn to_diagnostic(&self, file_map: &FileMap) -> Option<Diagnostic> {
        let str_start = file_map
            .byte_index(
                (self.text_pos.row - 1 as u32).into(),
                (self.text_pos.col - 1 as u32).into(),
            )
            .ok()?;

        Some(
            Diagnostic::new(self.severity, self.message.clone())
            .with_label(
                Label::new_primary(Span::from_offset(str_start, (self.len as i64).into()))
                .with_message("")
                // .with_message(self.message.clone()),
            ),
        )
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
        _ => TextPos::new(1, 1)
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
            text_pos: TextPos::new(0, 0),
            len: 0,
            message: String::from(""),
        }]))
    }
}

impl StyleError {
    pub fn diagnostics(&self, file_map: &FileMap) -> Vec<Result<Diagnostic, String>> {
        match *self {
            StyleError::Invalid(ref invs) => invs.0.iter()
                .map(|e| e.to_diagnostic(file_map)
                          .ok_or(e.message.clone()))
                .collect(),
            StyleError::ParseError(ref e) => {
                let pos = get_pos(&e);

                let str_start = file_map
                    .byte_index((pos.row - 1 as u32).into(), (pos.col - 1 as u32).into());

                if let Ok(start) = str_start {
                    vec![
                        Ok(Diagnostic::new(Severity::Error, format!("{}", e)).with_label(
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
