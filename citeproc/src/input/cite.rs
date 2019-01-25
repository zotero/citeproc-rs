use super::{DateOrRange, Name, NumericValue, Reference};
use crate::output::OutputFormat;
use crate::style::element::{Position, Style};
use crate::style::terms::LocatorType;
use crate::style::variables::*;

use crate::Atom;

/// Represents one cite in someone's document, to exactly one reference.
///
/// Prefixes and suffixes
#[derive(Deserialize)]
pub struct Cite<O: OutputFormat> {
    pub id: Atom,
    pub prefix: O::Build,
    pub suffix: O::Build,
    pub locator_type: Option<LocatorType>,

    // TODO: gotta be careful not to look up locators in the hashmap, even though
    // they are 'variables'.
    // in CSL-M they are number variables. also review the rest of the
    // vars that are like this
    pub locator: Option<NumericValue>,
    // TODO: parse these out of the locator
    // CSL-M only
    pub locator_extra: Option<String>,
    // CSL-M only
    pub locator_date: Option<DateOrRange>,

    // pub note_number: u32,
    pub near_note: bool,

    // TODO: allow suppression of any variables
    pub author_in_text: bool,
    pub suppress_author: bool,
    // Is this necessary?
    // citeHash       :: Int
}

impl<O: OutputFormat> Cite<O> {
    pub fn basic(id: Atom, prefix: &O::Build) -> Self {
        Cite {
            id,
            prefix: prefix.clone(),
            suffix: prefix.clone(),
            locator: None,
            locator_type: None,
            locator_extra: None,
            locator_date: None,
            near_note: false,
            author_in_text: false,
            suppress_author: false,
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
    // *could* get this from db if every element was an Arc for IR to hold it
    // OR you reused the lifetime from &impl Databse
    pub style: &'c Style,
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
}

pub struct Cluster<'c, O: OutputFormat> {
    pub cites: Vec<CiteContext<'c, O>>,
}

// helper methods to access both cite and reference properties via Variables

impl<'c, O: OutputFormat> CiteContext<'c, O> {
    pub fn has_variable(&self, var: &AnyVariable) -> bool {
        use crate::style::variables::AnyVariable::*;
        match *var {
            Name(NameVariable::Dummy) => false,
            // TODO: finish this list
            Number(NumberVariable::Locator) => self.cite.locator.is_some(),
            // we need Page to exist and be numeric
            Number(NumberVariable::PageFirst) => self.is_numeric(var),
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
    pub fn is_numeric(&self, var: &AnyVariable) -> bool {
        match var {
            AnyVariable::Number(num) => self
                .get_number(num)
                .map(|r| r.is_numeric())
                .unwrap_or(false),

            // TODO: this isn't very useful
            _ => false,
        }
    }

    pub fn get_number<'a>(&'a self, var: &NumberVariable) -> Option<NumericValue> {
        match var {
            // TODO: finish this list
            NumberVariable::Locator => self.cite.locator.clone(),
            NumberVariable::PageFirst => self
                .reference
                .number
                .get(&NumberVariable::Page)
                .and_then(|pp| pp.page_first())
                .clone(),
            _ => self.reference.number.get(var).cloned(),
        }
    }

    pub fn get_name(&self, var: &NameVariable) -> Option<&Vec<Name>> {
        match var {
            NameVariable::Dummy => None,
            _ => self.reference.name.get(var),
        }
    }
}

impl Reference {
    // Implemented here privately so we don't use it by mistake.
    // It's meant to be used only by CiteContext::has_variable, which wraps it and prevents
    // testing variables that only exist on the Cite.
    fn has_variable(&self, var: &AnyVariable) -> bool {
        match *var {
            AnyVariable::Ordinary(ref v) => self.ordinary.contains_key(v),
            AnyVariable::Number(ref v) => self.number.contains_key(v),
            AnyVariable::Name(ref v) => self.name.contains_key(v),
            AnyVariable::Date(ref v) => self.date.contains_key(v),
        }
    }
}
