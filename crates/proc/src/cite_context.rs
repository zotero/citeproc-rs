// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::prelude::*;

use super::DisambPass;
use crate::choose::CondChecker;
use crate::SmartString;
use citeproc_io::output::markup::Markup;
use citeproc_io::{Cite, DateOrRange, Locator, Name, NumericValue, Reference};
use csl::Features;
use csl::Locale;
use csl::*;
use csl::{CslType, LocatorType, Name as NameEl, Position, SortKey, Style, VariableForm};
use std::borrow::Cow;
use std::sync::Arc;

#[derive(Clone)]
pub struct CiteContext<
    'c,
    Output: OutputFormat + Sized = Markup,
    Input: OutputFormat + Sized = Output,
> {
    pub reference: &'c Reference,
    pub format: Output,
    pub cite_id: Option<CiteId>,
    pub style: &'c Style,
    pub locale: &'c Locale,
    pub name_citation: Arc<NameEl>,
    pub names_delimiter: Option<SmartString>,

    pub position: (Position, Option<u32>),

    pub disamb_pass: Option<DisambPass>,

    // These fields are synchronised with fields on EdgeData and IR.
    pub cite: &'c Cite<Input>,
    pub bib_number: Option<u32>,

    pub in_bibliography: bool,
    pub sort_key: Option<SortKey>,

    /// It isn't easy to sort by year-suffix. Year-suffix disambiguation requires a representation
    /// of the style's output (Called ir_gen2 in citeproc-rs). This requires knowing a cite's
    /// position, not only for position="X" testing, but for the et-al-subsequent-min, etc. Cite
    /// positions depend on cite sorting. If the sort depends on the year-suffix, the computation
    /// is cyclical. Technically, if none of the style used any position testing (i.e. the style is
    /// independent of the position variable), then it could be possible, by recognising this and
    /// using Position::First for everything, removing the part of the cycle where positions depend
    /// on sorting.
    ///
    /// (Would need FreeWalker's conds for Names blocks to mix in Position only when
    /// et-al-subsequent-* are used. Also, should not be reference-specific, so none of the
    /// normally-dependent variables can be used.)
    pub year_suffix: Option<u32>,
}

use std::fmt;
impl<'c, O: OutputFormat, I: OutputFormat> fmt::Debug for CiteContext<'c, O, I> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CiteContext")
            .field("ref_id", &AsRef::<str>::as_ref(&self.reference.id))
            .field("position", &self.position)
            .field("bib_number", &self.bib_number)
            .field("in_bibliography", &self.in_bibliography)
            .field("sort_key", &self.sort_key)
            .field("disamb_pass", &self.disamb_pass)
            .field("sort_key", &self.sort_key)
            .field("year_suffix", &self.year_suffix)
            .finish()
    }
}

// helper methods to access both cite and reference properties via Variables

impl<'c, O: OutputFormat, I: OutputFormat> CiteContext<'c, O, I> {
    pub fn change_format<O2: OutputFormat>(&self, new_fmt: O2) -> CiteContext<'c, O2, I> {
        CiteContext {
            format: new_fmt,
            reference: self.reference,
            cite_id: self.cite_id,
            cite: self.cite,
            style: self.style,
            locale: self.locale,
            name_citation: self.name_citation.clone(),
            names_delimiter: self.names_delimiter.clone(),
            position: self.position,
            disamb_pass: self.disamb_pass,
            bib_number: self.bib_number,
            in_bibliography: self.in_bibliography,
            sort_key: self.sort_key.clone(),
            year_suffix: self.year_suffix,
        }
    }
}

impl<'a, O: OutputFormat, I: OutputFormat> RenderContext for CiteContext<'a, O, I> {
    fn style(&self) -> &Style {
        self.style
    }
    fn reference(&self) -> &Reference {
        self.reference
    }
    fn locale(&self) -> &Locale {
        self.locale
    }

    fn get_number(&self, var: NumberVariable) -> Option<NumericValue<'_>> {
        // TODO: always use the default locale
        let and_term = self.locale.and_term(None).unwrap_or("and");
        let get = |v: NumberVariable| {
            self.reference()
                .number
                .get(&v)
                .map(NumericValue::from_localized(and_term))
        };
        match var {
            NumberVariable::Locator => self
                .cite
                .locators
                .as_ref()
                // You'd need new CSL syntax to render more than one locator properly.
                // For now we'll just ignore any more than the one.
                .and_then(|ls| ls.single())
                .map(Locator::value)
                .map(NumericValue::from_localized(and_term)),
            NumberVariable::FirstReferenceNoteNumber => self.position.1.map(NumericValue::num),
            NumberVariable::CitationNumber => self.bib_number.map(NumericValue::num),
            NumberVariable::PageFirst => get(NumberVariable::Page).and_then(|pp| pp.page_first()),
            _ => get(var),
        }
    }
}

pub trait RenderContext {
    fn style(&self) -> &Style;
    fn reference(&self) -> &Reference;
    fn locale(&self) -> &Locale;
    fn get_number(&self, var: NumberVariable) -> Option<NumericValue<'_>>;

    fn cite_lang(&self) -> Option<&Lang> {
        let refr = self.reference();
        refr.language.as_ref()
    }

    /// Common functionality between CiteContext and RefContext.
    fn get_ordinary(&self, var: Variable, form: VariableForm) -> Option<Cow<'_, str>> {
        let refr = self.reference();
        let get = |v: Variable| refr.ordinary.get(&v).map(|s| s.as_str()).map(Cow::Borrowed);
        match (var, form) {
            (Variable::Title, VariableForm::Short) => {
                get(Variable::TitleShort).or_else(|| get(Variable::Title))
            }
            (Variable::ContainerTitleShort, _) => {
                get(Variable::ContainerTitleShort).or_else(|| get(Variable::JournalAbbreviation))
            }
            (Variable::ContainerTitle, VariableForm::Short) => get(Variable::ContainerTitleShort)
                .or_else(|| get(Variable::JournalAbbreviation))
                .or_else(|| get(Variable::ContainerTitle)),
            (Variable::CitationLabel, _) if refr.ordinary.get(&var).is_none() => {
                let tri = crate::citation_label::Trigraph::default();
                Some(Cow::Owned(tri.make_label(self.reference())))
            }
            _ => get(var),
        }
    }

    /// Just makes sure NameVariable::Dummy can't produce anything
    fn get_name(&self, var: NameVariable) -> Option<&[Name]> {
        match var {
            NameVariable::Dummy => None,
            _ => self.reference().name.get(&var).map(|x| x.as_slice()),
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

impl<'a, O, I> CondChecker for CiteContext<'a, O, I>
where
    O: OutputFormat,
    I: OutputFormat,
{
    fn has_variable(&self, var: AnyVariable) -> bool {
        match var {
            AnyVariable::Name(NameVariable::Dummy) => false,
            // TODO: finish this list
            AnyVariable::Number(v) => match v {
                NumberVariable::Locator => self.cite.locators.is_some(),
                NumberVariable::FirstReferenceNoteNumber => self.position.1.is_some(),
                NumberVariable::CitationNumber => self.bib_number.is_some(),
                // PageFirst is covered in get_number
                _ => self.get_number(v).is_some(),
            },
            // Generated on demand
            AnyVariable::Ordinary(Variable::CitationLabel) => true,
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
    fn is_numeric(&self, var: AnyVariable) -> bool {
        match var {
            AnyVariable::Number(num) => self
                .get_number(num)
                .map(|r| r.is_numeric())
                .unwrap_or(false),

            // TODO: this isn't very useful
            _ => false,
        }
    }

    fn csl_type(&self) -> CslType {
        self.reference.csl_type
    }
    fn locator_type(&self) -> Option<LocatorType> {
        self.cite
            .locators
            .as_ref()
            .and_then(|l| l.single().map(|l| l.type_of()))
    }
    fn get_date(&self, dvar: DateVariable) -> Option<&DateOrRange> {
        self.reference.date.get(&dvar)
    }
    fn position(&self) -> Option<Position> {
        if self.in_bibliography {
            return None;
        }
        Some(self.position.0)
    }
    fn is_disambiguate(&self, _current_count: u32) -> bool {
        // ignore count as that's for references
        self.disamb_pass == Some(DisambPass::Conditionals)
    }
    fn features(&self) -> &Features {
        &self.style.features
    }
}
