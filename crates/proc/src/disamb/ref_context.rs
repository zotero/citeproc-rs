use crate::choose::CondChecker;
use crate::cite_context::RenderContext;
use crate::prelude::*;
use citeproc_io::output::markup::Markup;
use citeproc_io::{DateOrRange, NumericValue, Reference};
use csl::{style::*, terms::*, variables::*, Features, Locale, Name as NameEl};
use std::sync::Arc;

use crate::disamb::FreeCond;

#[derive(Clone)]
pub struct RefContext<'a, O: OutputFormat = Markup> {
    pub format: &'a O,
    pub style: &'a Style,
    pub locale: &'a Locale,
    pub reference: &'a Reference,
    pub locator_type: Option<LocatorType>,
    pub position: Position,
    pub year_suffix: bool,
    pub names_delimiter: Option<SmartString>,
    pub name_el: Arc<NameEl>,
    pub disamb_count: u32,
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
        let mut ctx = RefContext {
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
            name_el: ctx.name_citation.clone(),
            disamb_count: 0,
        };
        ctx.count_disambiguate_branches(CiteOrBib::Citation);
        ctx
    }

    pub fn from_free_cond(
        fc: FreeCond,
        format: &'c O,
        style: &'c Style,
        locale: &'c Locale,
        reference: &'c Reference,
        location: CiteOrBib,
    ) -> Self {
        let name_info = match location {
            CiteOrBib::Citation => style.name_info_citation(),
            CiteOrBib::Bibliography => style.name_info_bibliography(),
        };
        let mut ctx = RefContext {
            format,
            style,
            locale,
            reference,
            locator_type: fc.to_loc_type(),
            position: Position::from(fc),
            year_suffix: fc.contains(FreeCond::YEAR_SUFFIX),
            names_delimiter: name_info.0,
            name_el: name_info.1,
            disamb_count: 0,
        };
        ctx.count_disambiguate_branches(location);
        ctx
    }

    pub fn count_disambiguate_branches(&mut self, location: CiteOrBib) {
        let count = {
            let mut counter = DisambCounter::new(&self);
            match location {
                CiteOrBib::Citation => counter.walk_citation(self.style),
                CiteOrBib::Bibliography => counter.walk_bibliography(self.style).unwrap_or(0),
            }
        };
        self.disamb_count = count;
    }
}

impl<'c, O: OutputFormat> RenderContext for RefContext<'c, O> {
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
        let and_term = self.locale.and_term(None).unwrap_or("and");
        let get = |v: NumberVariable| {
            self.reference()
                .number
                .get(&v)
                .map(NumericValue::from_localized(and_term))
        };
        match var {
            NumberVariable::PageFirst => get(NumberVariable::Page).and_then(|pp| pp.page_first()),

            // Should never be accessed, handled without using the actual NumericValue
            NumberVariable::FirstReferenceNoteNumber
            | NumberVariable::CitationNumber
            | NumberVariable::Locator => None,

            _ => get(var),
        }
    }
}

impl<'c, O> CondChecker for RefContext<'c, O>
where
    O: OutputFormat,
{
    fn has_variable(&self, var: AnyVariable) -> bool {
        match var {
            AnyVariable::Number(v) => match v {
                NumberVariable::Locator => self.locator_type.is_some(),
                NumberVariable::PageFirst => {
                    self.is_numeric(AnyVariable::Number(NumberVariable::Page))
                }
                NumberVariable::FirstReferenceNoteNumber => {
                    self.position.matches(Position::Subsequent)
                }
                NumberVariable::CitationNumber => self.style.bibliography.is_some(),
                _ => self.get_number(v).is_some(),
            },
            AnyVariable::Ordinary(v) => match v {
                // Generated on demand
                Variable::CitationLabel => true,
                // TODO: make Hereinafter a FreeCond
                Variable::Hereinafter => unimplemented!("Hereinafter as a FreeCond"),
                Variable::YearSuffix => self.year_suffix,
                _ => self.get_ordinary(v, VariableForm::Long).is_some(),
            },
            AnyVariable::Date(v) => self.reference.date.contains_key(&v),
            AnyVariable::Name(NameVariable::Dummy) => false,
            AnyVariable::Name(v) => self.reference.name.contains_key(&v),
        }
    }

    fn is_numeric(&self, var: AnyVariable) -> bool {
        match &var {
            AnyVariable::Number(num) => self.get_number(*num).map_or(false, |r| r.is_numeric()),
            _ => false,
            // TODO: not very useful; implement for non-number variables (see CiteContext)
        }
    }
    fn csl_type(&self) -> CslType {
        self.reference.csl_type
    }
    fn locator_type(&self) -> Option<LocatorType> {
        self.locator_type
    }
    fn get_date(&self, dvar: DateVariable) -> Option<&DateOrRange> {
        self.reference.date.get(&dvar)
    }
    fn position(&self) -> Option<Position> {
        Some(self.position)
    }
    fn is_disambiguate(&self, current_count: u32) -> bool {
        // See docs on is_disambiguate
        // current_count is mutated as IR is rolled out;
        // so for
        //    RefContext { disamb_count: 0 } => is_disambiguate is always false
        //    RefContext { disamb_count: 1 } => is_disambiguate is true for the first disambiguate="X" check only
        //    RefContext { disamb_count: 2 } => etc...
        //
        // Then, when you create one RefContext for each count up to the total number of checks in
        // the style, then your cites will match the DFA as they incrementally re-calculate with
        // disambiguate se to true.
        current_count < self.disamb_count
    }
    fn features(&self) -> &Features {
        &self.style.features
    }
}

struct DisambCounter<'a, O: OutputFormat> {
    ctx: &'a RefContext<'a, O>,
}

impl<'a, O: OutputFormat> DisambCounter<'a, O> {
    fn new(ctx: &'a RefContext<'a, O>) -> Self {
        DisambCounter { ctx }
    }
}

use csl::Choose;

impl<'a, O: OutputFormat> StyleWalker for DisambCounter<'a, O> {
    type Output = u32;
    type Checker = RefContext<'a, O>;
    fn default(&mut self) -> Self::Output {
        0
    }
    fn fold(&mut self, elements: &[Element], _fold_type: WalkerFoldType) -> Self::Output {
        elements.iter().fold(0, |_acc, el| self.element(el))
    }
    fn choose(&mut self, choose: &Choose) -> Self::Output {
        let Choose(head, rest, last) = choose;
        let iter = std::iter::once(head).chain(rest.iter());
        let mut sum = 0u32;
        for branch in iter {
            // Run with a count of MAX so that eval_true means "true even without disambiguate set to
            // true", hence still works as a circuit-breaking "stop counting" mechanism.
            let (eval_true, is_disambiguate) =
                crate::choose::eval_conditions(&branch.0, self.ctx, std::u32::MAX);
            if is_disambiguate {
                sum += 1;
            }
            if eval_true {
                let count = self.fold(&branch.1, WalkerFoldType::IfThen);
                return sum + count;
            }
        }
        sum += self.fold(&last.0, WalkerFoldType::Else);
        sum
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::disamb::FreeCond;

    use crate::test::with_test_citation;
    use citeproc_db::LocaleFetcher;
    use csl::Atom;
    use csl::Lang;

    #[test]
    fn test_counter() {
        fn count(fc: FreeCond, f: impl Fn(&mut Reference) + Copy) -> impl Fn(Style) -> u32 + Copy {
            move |style: Style| {
                let format = Markup::default();
                let locale = citeproc_db::PredefinedLocales::bundled_en_us()
                    .fetch_locale(&Lang::en_us())
                    .unwrap();
                use citeproc_io::Reference;
                let mut reference = Reference::empty(Atom::from("id"), CslType::Book);
                f(&mut reference);
                let ctx = RefContext::from_free_cond(
                    fc,
                    &format,
                    &style,
                    &locale,
                    &reference,
                    CiteOrBib::Citation,
                );
                let mut counter = DisambCounter::new(&ctx);
                counter.walk_citation(&style)
            }
        }
        let count_plain = count(FreeCond::empty(), |_| {});

        assert_eq!(
            with_test_citation(
                count_plain,
                r#"<choose><if disambiguate="true" /></choose>"#
            ),
            1
        );
        assert_eq!(
            with_test_citation(
                count_plain,
                r#"<choose>
                    <if position="subsequent" />
                    <else-if disambiguate="true" />
                </choose>"#
            ),
            1,
        );
        assert_eq!(
            with_test_citation(
                count_plain,
                r#"<choose>
                    <if position="subsequent" />
                    <else-if disambiguate="true" />
                    <else>
                        <choose>
                            <if disambiguate="true" />
                        </choose>
                    </else>
                </choose>"#,
            ),
            2,
        );
        assert_eq!(
            with_test_citation(
                count(FreeCond::SUBSEQUENT, |refr| {
                    refr.ordinary.insert(Variable::Title, "Something".into());
                }),
                r#"<choose>
                    <if position="subsequent" />
                    <else-if variable="title" />
                    <else-if disambiguate="true" />
                    <else>
                        <choose>
                            <if disambiguate="true" />
                        </choose>
                    </else>
                </choose>"#,
            ),
            0,
        );
    }
}
