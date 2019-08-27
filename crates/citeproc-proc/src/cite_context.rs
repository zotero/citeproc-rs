// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::prelude::*;

use super::DisambPass;
use citeproc_io::{Cite, Locator, Name, NumericValue, Reference};
use csl::style::{Position, VariableForm};
use csl::variables::*;

#[derive(Clone)]
pub struct CiteContext<'c, O: OutputFormat + Sized> {
    // can technically get this from db
    pub reference: &'c Reference,
    pub format: O,
    //
    pub cite: &'c Cite<O>,
    // could store in the DB
    pub position: Position,
    //
    pub citation_number: u32,
    // TODO: keep track of which variables have so far been substituted
    pub disamb_pass: Option<DisambPass>,
}

// helper methods to access both cite and reference properties via Variables

impl<'c, O: OutputFormat> CiteContext<'c, O> {
    pub fn get_ordinary(&self, var: Variable, form: VariableForm) -> Option<&str> {
        (match (var, form) {
            (Variable::Title, VariableForm::Short) => {
                self.reference.ordinary.get(&Variable::TitleShort)
            }
            _ => self.reference.ordinary.get(&var),
        })
        .map(|s| s.as_str())
    }

    pub fn has_variable(&self, var: AnyVariable, db: &impl IrDatabase) -> bool {
        use csl::variables::AnyVariable::*;
        match var {
            Name(NameVariable::Dummy) => false,
            // TODO: finish this list
            Number(NumberVariable::Locator) => !self.cite.locators.is_empty(),
            // we need Page to exist and be numeric
            Number(NumberVariable::PageFirst) => {
                self.is_numeric(AnyVariable::Number(NumberVariable::Page), db)
            }
            Number(NumberVariable::FirstReferenceNoteNumber) => {
                db.cite_position(self.cite.id).1.is_some()
            }
            Number(NumberVariable::CitationNumber) => db.bib_number(self.cite.id).is_some(),
            _ => ref_has_variable(self.reference, var),
        }
    }

    /// Tests whether a variable is numeric.
    ///
    /// There are a few deviations in other implementations, notably:
    ///
    /// * `citeproc-js` always returns `false` for "page-first", even if "page" is numeric
    /// * `citeproc-js` represents version numbers as numerics, which differs from the spec. I'm
    ///   not aware of any version numbers that actually are numbers. Semver hyphens, for example,
    ///   are literal hyphens, not number ranges.
    ///   By not representing them as numbers, `is-numeric="version"` won't work.
    pub fn is_numeric(&self, var: AnyVariable, db: &impl IrDatabase) -> bool {
        match var {
            AnyVariable::Number(num) => self
                .get_number(num, db)
                .map(|r| r.is_numeric())
                .unwrap_or(false),

            // TODO: this isn't very useful
            _ => false,
        }
    }

    pub fn get_number(&self, var: NumberVariable, db: &impl IrDatabase) -> Option<NumericValue> {
        match var {
            // TODO: get all the locators?
            NumberVariable::Locator => self
                .cite
                .locators
                .get(0)
                .map(Locator::value)
                .map(Clone::clone),
            NumberVariable::FirstReferenceNoteNumber => {
                db.cite_position(self.cite.id).1.map(NumericValue::num)
            }
            NumberVariable::CitationNumber => db.bib_number(self.cite.id).map(NumericValue::num),
            NumberVariable::PageFirst => self
                .reference
                .number
                .get(&NumberVariable::Page)
                .and_then(|pp| pp.page_first())
                .clone(),
            _ => self.reference.number.get(&var).cloned(),
            // TODO: finish this list
        }
    }

    pub fn get_name(&self, var: NameVariable) -> Option<&Vec<Name>> {
        match var {
            NameVariable::Dummy => None,
            _ => self.reference.name.get(&var),
        }
    }
}

// Implemented here privately so we don't use it by mistake.
// It's meant to be used only by CiteContext::has_variable, which wraps it and prevents
// testing variables that only exist on the Cite.
fn ref_has_variable(refr: &Reference, var: AnyVariable) -> bool {
    match var {
        AnyVariable::Ordinary(v) => refr.ordinary.contains_key(&v),
        AnyVariable::Number(v) => refr.number.contains_key(&v),
        AnyVariable::Name(v) => refr.name.contains_key(&v),
        AnyVariable::Date(v) => refr.date.contains_key(&v),
    }
}
