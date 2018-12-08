use crate::input::*;
use crate::output::*;
use crate::style::element::{Position, Style};
use crate::style::variables::*;

/// ## Lifetimes
///
/// * `'c`: CiteContext umbrella to live longer than `'r` and `'ci`
/// * `'r`: [Reference][]
/// * `'ci`: [Cite][]
///
/// [Reference]: ../input/struct.Reference.html
/// [Cite]: ../input/struct.Cite.html

#[derive(Clone)]
pub struct CiteContext<'c, 'r: 'c, 'ci: 'c, O: OutputFormat> {
    pub style: &'c Style,
    pub reference: &'c Reference<'r>,
    pub cite: &'c Cite<'ci, O>,
    pub format: &'c O,
    pub position: Position,
    pub citation_number: u32,
}

pub struct Cluster<'c, 'r: 'c, 'ci: 'c, O: OutputFormat> {
    pub cites: Vec<CiteContext<'c, 'r, 'ci, O>>,
}

// helper methods to access both cite and reference properties via Variables

impl<'c, 'r: 'c, 'ci: 'c, O: OutputFormat> CiteContext<'c, 'r, 'ci, O> {
    pub fn has_variable(&self, var: &AnyVariable) -> bool {
        use crate::style::variables::AnyVariable::*;
        match *var {
            // TODO: finish this list
            Number(NumberVariable::Locator) => self.cite.locator.is_some(),
            // we need Page to exist and be numeric
            Number(NumberVariable::PageFirst) => self.is_numeric(&NumberVariable::PageFirst),
            _ => self.reference.has_variable(var),
        }
    }

    /// Tests whether a variable is numeric.
    ///
    /// There are a few deviations in other implementations, notably:
    ///
    /// * `citeproc-js` always returns `false` for "page-first", even if "page" is numeric
    pub fn is_numeric(&self, var: &NumberVariable) -> bool {
        self.get_number(var)
            .map(|r| r.is_numeric())
            .unwrap_or(false)
    }
    pub fn get_number<'a>(&'a self, var: &NumberVariable) -> Option<NumericValue<'c>> {
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
}

impl<'r> Reference<'r> {

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
