// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::prelude::*;

use super::DisambPass;
use crate::choose::CondChecker;
use citeproc_io::output::markup::Markup;
use citeproc_io::{Cite, DateOrRange, Locator, Name, NumericValue, Reference};
use csl::locale::Locale;
use csl::style::{CslType, Name as NameEl, Position, Style, VariableForm};
use csl::variables::*;
use std::sync::Arc;

#[derive(Clone)]
pub struct CiteContext<'c, O: OutputFormat + Sized = Markup> {
    pub reference: &'c Reference,
    pub format: O,
    pub cite_id: Option<CiteId>,
    pub style: &'c Style,
    pub locale: &'c Locale,
    pub name_citation: Arc<NameEl>,

    pub position: (Position, Option<u32>),

    pub disamb_pass: Option<DisambPass>,

    // These fields are synchronised with fields on EdgeData and IR.
    pub cite: &'c Cite<O>,
    pub citation_number: u32,
    pub bib_number: Option<u32>,

    pub in_bibliography: bool,

    // TODO: keep track of which variables have so far been substituted
    
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

    pub fn has_variable(&self, var: AnyVariable) -> bool {
        use csl::variables::AnyVariable::*;
        match var {
            Name(NameVariable::Dummy) => false,
            // TODO: finish this list
            Number(NumberVariable::Locator) => self.cite.locators.is_some(),
            // we need Page to exist and be numeric
            Number(NumberVariable::PageFirst) => {
                self.is_numeric(AnyVariable::Number(NumberVariable::Page))
            }
            Number(NumberVariable::FirstReferenceNoteNumber) => self.position.1.is_some(),
            Number(NumberVariable::CitationNumber) => self.bib_number.is_some(),
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
    pub fn is_numeric(&self, var: AnyVariable) -> bool {
        match var {
            AnyVariable::Number(num) => self
                .get_number(num)
                .map(|r| r.is_numeric())
                .unwrap_or(false),

            // TODO: this isn't very useful
            _ => false,
        }
    }

    pub fn get_number(&self, var: NumberVariable) -> Option<NumericValue> {
        match var {
            NumberVariable::Locator => self
                .cite
                .locators
                .as_ref()
                // You'd need new CSL syntax to render more than one locator properly.
                // For now we'll just ignore any more than the one.
                .and_then(|ls| ls.single())
                .map(Locator::value)
                .map(Clone::clone),
            NumberVariable::FirstReferenceNoteNumber => self.position.1.map(NumericValue::num),
            NumberVariable::CitationNumber => self.bib_number.map(NumericValue::num),
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

impl<'c, O> CondChecker for CiteContext<'c, O>
where
    O: OutputFormat,
{
    fn has_variable(&self, var: AnyVariable) -> bool {
        CiteContext::has_variable(self, var)
    }
    fn is_numeric(&self, var: AnyVariable) -> bool {
        CiteContext::is_numeric(self, var)
    }
    fn csl_type(&self) -> &CslType {
        &self.reference.csl_type
    }
    fn get_date(&self, dvar: DateVariable) -> Option<&DateOrRange> {
        self.reference.date.get(&dvar)
    }
    fn position(&self) -> Position {
        self.position.0
    }
    fn is_disambiguate(&self) -> bool {
        self.disamb_pass == Some(DisambPass::Conditionals)
    }
    fn style(&self) -> &Style {
        self.style
    }
}
