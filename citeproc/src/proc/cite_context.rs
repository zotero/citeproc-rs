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
            Ordinary(Variable::Locator) => self.cite.locator.is_some(),
            Number(NumberVariable::Locator) => self.cite.locator.is_some(),
            _ => self.reference.has_variable(var),
        }
    }
    pub fn is_numeric(&self, var: &NumberVariable) -> bool {
        match var {
            // TODO: finish this list
            NumberVariable::Locator => self
                .cite
                .locator
                .as_ref()
                .map(|r| r.is_numeric())
                .unwrap_or(false),
            _ => self
                .reference
                .number
                .get(var)
                .map(|v| v.is_numeric())
                .unwrap_or(false),
        }
    }
}
