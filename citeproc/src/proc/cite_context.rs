use crate::input::*;
use crate::output::*;
use crate::style::element::{Position, Style};
use crate::style::variables::*;

// in Proc, 's has to live at least as long as 'c ('s: 'c)
pub struct CiteContext<'c, 'r: 'c, O: OutputFormat> {
    pub style: &'c Style,
    pub reference: &'c Reference<'r>,
    pub cite: &'c Cite<'r, O::Output>,
    pub format: &'c O,
    pub position: Position,
    pub citation_number: u32,
}

pub struct Cluster<'a, 'c, 'r: 'a + 'c, O: OutputFormat> {
    pub cites: &'a [CiteContext<'c, 'r, O>],
}

// helper methods to access both cite and reference properties via Variables

impl<'c, 'r: 'c, O: OutputFormat> CiteContext<'c, 'r, O> {
    pub fn has_variable(&self, var: &AnyVariable) -> bool {
        use crate::style::variables::AnyVariable::*;
        match *var {
            // TODO: finish this list
            Ordinary(Variable::Locator) => self.cite.locator.is_some(),
            Number(NumberVariable::Locator) => self.cite.locator.is_some(),
            _ => self.reference.has_variable(var)
        }
    }
    pub fn is_numeric(&self, var: &NumberVariable) -> bool {
        match var {
            // TODO: finish this list
            NumberVariable::Locator => self.cite.locator.as_ref().map(|r| r.is_numeric()).unwrap_or(false),
            _ => self.reference.number.get(var).map(|v| v.is_numeric()).unwrap_or(false),
        }
    }
}
