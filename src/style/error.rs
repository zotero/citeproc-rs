use roxmltree::{ TextPos, Node };
use codespan::{CodeMap, Span, FileMap};
use codespan_reporting::termcolor::{ StandardStream, ColorChoice };
use codespan_reporting::{emit, Diagnostic, Label, Severity};
use std::num::ParseIntError;

pub struct UnknownAttributeValue {
    pub value: String,
}

impl UnknownAttributeValue {
    pub fn new(s: &str) -> Self {
        UnknownAttributeValue { value: s.to_owned() }
    }
}

#[derive(Debug, PartialEq)]
pub struct CslValidationError {
    pub severity: Severity,
    pub text_pos: TextPos,
    pub len: usize,
    pub message: String,
}

impl CslValidationError {
    pub fn new(node: &Node, message: String) -> Self {
        let mut pos = node.node_pos();
        pos.col = pos.col + 1;
        CslValidationError {
            text_pos: pos,
            len: node.tag_name().name().len(),
            severity: Severity::Error,
            message,
        }
    }

    pub fn bad_int(node: &Node, attr: &str, uav: ParseIntError) -> Self {
        CslValidationError {
            text_pos: node.attribute_value_pos(attr).unwrap_or(node.node_pos()),
            len: node.attribute("attr").map(|a| a.len()).unwrap_or(1),
            message: format!("Invalid integer value for {}: {:?}", attr, uav),
            severity: Severity::Error,
        }
    }

    pub fn unknown_attribute_value(node: &Node, attr: &str, uav: UnknownAttributeValue) -> Self {
        CslValidationError {
            text_pos: node.attribute_value_pos(attr).unwrap_or(node.node_pos()),
            len: uav.value.len(),
            message: format!("Unknown attribute value for {}: \"{}\"", attr, uav.value),
            severity: Severity::Error,
        }
    }

    pub fn to_diagnostic(&self, file_map: &FileMap) -> Diagnostic {
        let str_start = file_map
            .byte_index(
                (self.text_pos.row - 1 as u32).into(),
                (self.text_pos.col - 1 as u32).into()
            ).unwrap();

        Diagnostic::new(self.severity, self.message.clone())
            .with_label(
                Label::new_primary(Span::from_offset(str_start, (self.len as i64).into()))
                .with_message("")
                // .with_message(self.message.clone()),
            )
    }
}

impl Default for CslValidationError {
    fn default() -> Self {
        CslValidationError {
            severity: Severity::Error,
            text_pos: TextPos::new(0, 0),
            len: 0,
            message: String::from(""),
        }
    }
}

pub fn file_diagnostics(diagnostics: &Vec<CslValidationError>, filename: String, document: &String) {
    let mut code_map = CodeMap::new();
    let file_map = code_map.add_filemap(filename.into(), document.to_string());
    let writer = StandardStream::stderr(ColorChoice::Auto);
    for diag in diagnostics.iter().map(|d| d.to_diagnostic(&file_map)) {
        emit(&mut writer.lock(), &code_map, &diag).unwrap();
        println!();
    }
}
