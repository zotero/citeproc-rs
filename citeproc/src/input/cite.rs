use super::{DateOrRange, Name, NumericValue, Reference};
use crate::output::OutputFormat;
use crate::proc::ReEvaluation;
use crate::style::element::Position;
use crate::style::terms::LocatorType;
use crate::style::variables::*;

use crate::Atom;

pub type CiteId = u64;
pub type ClusterId = u64;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum Suppression {
    // For author-in-text, or whatever the style author wants to put inline.
    //
    // E.g. the author, or party names for a case.
    InText,
    // For the rest.
    //
    // E.g. the cite with the author suppressed, or a case without party names.
    Rest,
}

use pandoc_types::definition::CitationMode;
impl Suppression {
    pub fn from_pandoc_mode(mode: CitationMode) -> Option<Self> {
        match mode {
            CitationMode::AuthorInText => Some(Suppression::InText),
            CitationMode::SuppressAuthor => Some(Suppression::Rest),
            CitationMode::NormalCitation => None,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Locator(LocatorType, NumericValue);

impl Locator {
    pub fn type_of(&self) -> LocatorType {
        self.0
    }
    pub fn value(&self) -> &NumericValue {
        &self.1
    }
}

/// Represents one cite in someone's document, to exactly one reference.
///
/// Prefixes and suffixes
#[derive(Debug, Clone)]
pub struct Cite<O: OutputFormat> {
    pub id: CiteId,
    pub ref_id: Atom,
    pub prefix: O::Build,
    pub suffix: O::Build,
    pub suppression: Option<Suppression>,
    // TODO: parse these out of the locator string
    // Enforce len() == 1 in CSL mode
    pub locators: Vec<Locator>,
    // CSL-M only
    pub locator_extra: Option<String>,
    // CSL-M only
    pub locator_date: Option<DateOrRange>,
}

impl<O: OutputFormat> Cite<O> {
    pub fn basic(id: CiteId, ref_id: Atom) -> Self {
        Cite {
            id,
            ref_id,
            prefix: O::Build::default(),
            suffix: O::Build::default(),
            suppression: None,
            locators: Vec::new(),
            locator_extra: None,
            locator_date: None,
        }
    }
}

/// ## Lifetimes
///
/// * `'c`: CiteContext umbrella
///
/// [Reference]: ../input/struct.Reference.html
/// [Cite]: ../input/struct.Cite.html

#[derive(Clone)]
pub struct CiteContext<'c, O: OutputFormat> {
    // can get this from db
    pub reference: &'c Reference,
    // could pull one out of thin air! all the useful formatters are ZSTs.
    pub format: &'c O,
    //
    pub cite: &'c Cite<O>,
    // could store in the DB
    pub position: Position,
    //
    pub citation_number: u32,
    // TODO: keep track of which variables have so far been substituted
    pub re_evaluation: Option<ReEvaluation>,
    // TODO
    // pub note_number: u32,
}

#[derive(Debug)]
pub struct Cluster<O: OutputFormat> {
    pub id: ClusterId,
    pub cites: Vec<Cite<O>>,
}

// helper methods to access both cite and reference properties via Variables

impl<'c, O: OutputFormat> CiteContext<'c, O> {
    pub fn has_variable(&self, var: AnyVariable) -> bool {
        use crate::style::variables::AnyVariable::*;
        match var {
            Name(NameVariable::Dummy) => false,
            // TODO: finish this list
            Number(NumberVariable::Locator) => !self.cite.locators.is_empty(),
            // we need Page to exist and be numeric
            Number(NumberVariable::PageFirst) => self.is_numeric(
                AnyVariable::Number(NumberVariable::Page)
            ),
            _ => self.reference.has_variable(var),
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

    pub fn get_number<'a>(&'a self, var: NumberVariable) -> Option<NumericValue> {
        match var {
            // TODO: get all the locators?
            NumberVariable::Locator => self
                .cite
                .locators
                .get(0)
                .map(Locator::value)
                .map(Clone::clone),
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

    pub fn get_name(&self, var: &NameVariable) -> Option<&Vec<Name>> {
        match var {
            NameVariable::Dummy => None,
            _ => self.reference.name.get(&var),
        }
    }
}

impl Reference {
    // Implemented here privately so we don't use it by mistake.
    // It's meant to be used only by CiteContext::has_variable, which wraps it and prevents
    // testing variables that only exist on the Cite.
    fn has_variable(&self, var: AnyVariable) -> bool {
        match var {
            AnyVariable::Ordinary(v) => self.ordinary.contains_key(&v),
            AnyVariable::Number(v) => self.number.contains_key(&v),
            AnyVariable::Name(v) => self.name.contains_key(&v),
            AnyVariable::Date(v) => self.date.contains_key(&v),
        }
    }
}
