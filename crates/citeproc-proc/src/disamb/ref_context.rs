use crate::choose::CondChecker;
use crate::prelude::*;
use citeproc_io::output::markup::Markup;
use citeproc_io::{DateOrRange, NumericValue, Reference};
use csl::locale::Locale;
use csl::style::{CslType, Position, Style, VariableForm, Delimiter};
use csl::terms::LocatorType;
use csl::variables::*;

use crate::disamb::FreeCond;

pub struct RefContext<'a, O: OutputFormat = Markup> {
    pub format: &'a O,
    pub style: &'a Style,
    pub locale: &'a Locale,
    pub reference: &'a Reference,
    pub locator_type: Option<LocatorType>,
    pub position: Position,
    pub year_suffix: bool,
    pub names_delimiter: Option<Delimiter>,
}

impl From<FreeCond> for Position {
    fn from(pos: FreeCond) -> Self {
        if pos.contains(FreeCond::IBID_WITH_LOCATOR) {
            Position::IbidWithLocator
        } else if pos.contains(FreeCond::IBID) {
            Position::Ibid
        } else if pos.contains(FreeCond::NEAR_NOTE) {
            Position::NearNote
        } else if pos.contains(FreeCond::FAR_NOTE) {
            Position::FarNote
        } else if pos.contains(FreeCond::SUBSEQUENT) || pos.contains(FreeCond::FIRST_FALSE) {
            Position::Subsequent
        } else {
            // TODO: check this
            Position::First
        }
        // if not mentioned, it doesn't matter!
    }
}

impl<'c, O> RefContext<'c, O>
where
    O: OutputFormat,
{
    pub fn from_cite_context(refr: &'c Reference, ctx: &'c CiteContext<'c, O>) -> Self {
        use citeproc_io::Locators;
        RefContext {
            format: &ctx.format,
            style: ctx.style,
            locale: ctx.locale,
            reference: refr,
            locator_type: ctx.cite.locators.as_ref().and_then(|locs| match locs {
                Locators::Single(l) => Some(l.loc_type),
                // XXX
                Locators::Multiple { .. } => None,
            }),
            position: ctx.position.0,
            // XXX: technically Cites need to know this during the Conditionals pass as well,
            // so it should be promoted beyond that single DisambPass::AddYearSuffix(ys) variant.
            year_suffix: false,
            names_delimiter: ctx.names_delimiter.clone(),
        }
    }
    pub fn from_free_cond(
        fc: FreeCond,
        format: &'c O,
        style: &'c Style,
        locale: &'c Locale,
        reference: &'c Reference,
    ) -> Self {
        let ni = style.names_delimiter.clone();
        let citation_ni = style.citation.names_delimiter.clone();
        RefContext {
            format,
            style,
            locale,
            reference,
            locator_type: fc.to_loc_type(),
            position: Position::from(fc),
            year_suffix: fc.contains(FreeCond::YEAR_SUFFIX),
            names_delimiter: citation_ni.or(ni),
        }
    }
    pub fn get_ordinary(&self, var: Variable, form: VariableForm) -> Option<&str> {
        (match (var, form) {
            (Variable::TitleShort, _) |
            (Variable::Title, VariableForm::Short) => {
                self.reference.ordinary.get(&Variable::TitleShort)
                    .or(self.reference.ordinary.get(&Variable::Title))
            }
            (Variable::ContainerTitleShort, _) |
            (Variable::ContainerTitle, VariableForm::Short) => {
                self.reference.ordinary.get(&Variable::ContainerTitleShort)
                    .or(self.reference.ordinary.get(&Variable::ContainerTitle))
            }
            _ => self.reference.ordinary.get(&var),
        })
        .map(|s| s.as_str())
    }
    pub fn get_number(&self, var: NumberVariable) -> Option<&NumericValue> {
        self.reference.number.get(&var)
    }
    pub fn has_variable(&self, var: AnyVariable) -> bool {
        match var {
            AnyVariable::Number(v) => match v {
                NumberVariable::Locator => self.locator_type.is_some(),
                NumberVariable::FirstReferenceNoteNumber => {
                    self.position.matches(Position::Subsequent)
                }
                NumberVariable::CitationNumber => self.style.bibliography.is_some(),
                _ => self.get_number(v).is_some(),
            },
            AnyVariable::Ordinary(v) => {
                match v {
                    // TODO: make Hereinafter a FreeCond
                    Variable::Hereinafter => unimplemented!("Hereinafter as a FreeCond"),
                    Variable::YearSuffix => self.year_suffix,
                    _ => self.reference.ordinary.contains_key(&v),
                }
            }
            AnyVariable::Date(v) => self.reference.date.contains_key(&v),
            AnyVariable::Name(v) => self.reference.name.contains_key(&v),
        }
    }
}

impl<'c, O> CondChecker for RefContext<'c, O>
where
    O: OutputFormat,
{
    fn has_variable(&self, var: AnyVariable) -> bool {
        RefContext::has_variable(self, var)
    }
    fn is_numeric(&self, var: AnyVariable) -> bool {
        match &var {
            AnyVariable::Number(num) => self
                .reference
                .number
                .get(num)
                .map(|r| r.is_numeric())
                .unwrap_or(false),
            _ => false,
            // TODO: not very useful; implement for non-number variables (see CiteContext)
        }
    }
    fn csl_type(&self) -> &CslType {
        &self.reference.csl_type
    }
    fn get_date(&self, dvar: DateVariable) -> Option<&DateOrRange> {
        self.reference.date.get(&dvar)
    }
    fn position(&self) -> Position {
        self.position
    }
    fn is_disambiguate(&self) -> bool {
        false
    }
    fn style(&self) -> &Style {
        self.style
    }
}
