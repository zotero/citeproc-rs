// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use csl::{InvalidCsl, StyleError};
use std::ops::Range;

use csl::Severity as CslSeverity;

use codespan::{ByteIndex, ByteSpan, CodeMap, FileMap, Span};
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
            .map(|e| to_diagnostic(e).ok_or(e.message.clone()))
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

fn span_from_range(range: &Range<usize>) -> ByteSpan {
    Span::new(
        ByteIndex(range.start as u32 + 1),
        ByteIndex(range.end as u32 + 1),
    )
}

pub fn to_diagnostic(inv: &InvalidCsl) -> Option<Diagnostic> {
    let label = Label::new_primary(span_from_range(&inv.range)).with_message(inv.hint.to_string());
    let diag = Diagnostic::new(convert_sev(inv.severity), inv.message.clone()).with_label(label);
    Some(diag)
}
