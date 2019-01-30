use csl::error::{InvalidCsl, StyleError};

use csl::error::Severity as CslSeverity;

use codespan::{CodeMap, FileMap, Span};
use codespan_reporting::termcolor::{ColorChoice, StandardStream};
use codespan_reporting::{emit, Diagnostic, Label, Severity};

fn convert_sev(csl: CslSeverity) -> Severity {
    match csl {
        CslSeverity::Warning => Severity::Warning,
        CslSeverity::Error => Severity::Error,
    }
}

pub fn file_diagnostics<'a>(err: &StyleError, filename: &'a str, document: &'a str) {
    let mut code_map = CodeMap::new();
    let file_map = code_map.add_filemap(filename.to_owned().into(), document.to_string());
    let writer = StandardStream::stderr(ColorChoice::Auto);
    for diag in diagnostics(err, &file_map) {
        if let Ok(d) = diag {
            emit(&mut writer.lock(), &code_map, &d).unwrap();
            eprintln!();
        } else if let Err(emsg) = diag {
            eprintln!("{}", emsg);
        }
    }
}

pub(crate) fn diagnostics(err: &StyleError, file_map: &FileMap) -> Vec<Result<Diagnostic, String>> {
    match *err {
        StyleError::Invalid(ref invs) => invs
            .0
            .iter()
            .map(|e| to_diagnostic(e, file_map).ok_or(e.message.clone()))
            .collect(),
        StyleError::ParseError(ref e) => {
            let pos = e.pos();

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

pub fn to_diagnostic(inv: &InvalidCsl, file_map: &FileMap) -> Option<Diagnostic> {
    let str_start = file_map
        .byte_index(
            (inv.text_pos.row - 1 as u32).into(),
            (inv.text_pos.col - 1 as u32).into(),
        )
        .ok()?;
    let label = Label::new_primary(Span::from_offset(str_start, (inv.len as i64).into()))
        .with_message(inv.hint.to_string());
    let diag = Diagnostic::new(convert_sev(inv.severity), inv.message.clone()).with_label(label);
    Some(diag)
}

