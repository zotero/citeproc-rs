use crate::prelude::*;
use citeproc_io::output::LocalizedQuotes;
use citeproc_io::Name;
use citeproc_io::{Locator, NumericValue, Reference};
use csl::locale::Locale;
use csl::style::{NameLabel, NumericForm, Plural, Style};
use csl::terms::{
    GenderedTermSelector, LocatorType, RoleTerm, RoleTermSelector, TermForm, TermFormExtended,
    TextTermSelector,
};
use csl::variables::{NameVariable, NumberVariable, StandardVariable};
use csl::Atom;

#[derive(Clone)]
pub enum GenericContext<'a, O: OutputFormat> {
    Ref(&'a RefContext<'a, O>),
    Cit(&'a CiteContext<'a, O>),
}

#[allow(dead_code)]
impl<O: OutputFormat> GenericContext<'_, O> {
    pub fn locale(&self) -> &Locale {
        match self {
            GenericContext::Cit(ctx) => ctx.locale,
            GenericContext::Ref(ctx) => ctx.locale,
        }
    }
    pub fn style(&self) -> &Style {
        match self {
            GenericContext::Cit(ctx) => ctx.style,
            GenericContext::Ref(ctx) => ctx.style,
        }
    }
    pub fn reference(&self) -> &Reference {
        match self {
            GenericContext::Cit(ctx) => ctx.reference,
            GenericContext::Ref(ctx) => ctx.reference,
        }
    }
    pub fn format(&self) -> &O {
        match self {
            GenericContext::Cit(ctx) => &ctx.format,
            GenericContext::Ref(ctx) => ctx.format,
        }
    }
    pub fn should_add_year_suffix_hook(&self) -> bool {
        match self {
            GenericContext::Cit(ctx) => true,
            GenericContext::Ref(ctx) => ctx.year_suffix,
        }
    }
    pub fn locator_type(&self) -> Option<LocatorType> {
        match self {
            Cit(ctx) => ctx
                .cite
                .locators
                .as_ref()
                .and_then(|ls| ls.single())
                .map(Locator::type_of),
            Ref(ctx) => ctx.locator_type,
        }
    }
    pub fn get_name(&self, var: NameVariable) -> Option<&[Name]> {
        match self {
            Cit(ctx) => ctx.get_name(var),
            Ref(ctx) => ctx.reference.name.get(&var),
        }
        .map(|vec| vec.as_slice())
    }
    // fn get_number(&self, var: NumberVariable) -> Option<NumericValue> {
    //     match self {
    //         Cit(ctx) => ctx.get_number(var),
    //         Ref(ctx) => ctx.reference.number.get(var),
    //     }
    // }
}

use GenericContext::*;

pub struct Renderer<'a, O: OutputFormat> {
    ctx: GenericContext<'a, O>,
}

impl<O: OutputFormat> Renderer<'_, O> {
    pub fn refr<'c>(c: &'c RefContext<'c, O>) -> Renderer<'c, O> {
        Renderer {
            ctx: GenericContext::Ref(c),
        }
    }

    pub fn cite<'c>(c: &'c CiteContext<'c, O>) -> Renderer<'c, O> {
        Renderer {
            ctx: GenericContext::Cit(c),
        }
    }

    #[inline]
    fn fmt(&self) -> &O {
        match self.ctx {
            GenericContext::Cit(c) => &c.format,
            GenericContext::Ref(c) => c.format,
        }
    }

    pub fn number(
        &self,
        var: NumberVariable,
        form: NumericForm,
        val: &NumericValue,
        f: Option<Formatting>,
        af: &Affixes,
    ) -> O::Build {
        use crate::number::{roman_lower, roman_representable};
        let fmt = self.fmt();
        match (val, form) {
            (NumericValue::Tokens(_, ts), NumericForm::Roman) if roman_representable(&val) => {
                let options = IngestOptions {
                    replace_hyphens: var.should_replace_hyphens(),
                };
                let string = roman_lower(&ts);
                let b = fmt.ingest(&string, options);
                let b = fmt.with_format(b, f);
                fmt.affixed(b, af)
            }
            _ => fmt.affixed_text(val.as_number(var.should_replace_hyphens()), f, af),
        }
    }

    fn quotes(quo: bool) -> Option<LocalizedQuotes> {
        let q = LocalizedQuotes::Single(Atom::from("'"), Atom::from("'"));
        let quotes = if quo { Some(q) } else { None };
        quotes
    }

    pub fn text_variable(
        &self,
        var: StandardVariable,
        value: &str,
        f: Option<Formatting>,
        af: &Affixes,
        quo: bool,
        // sp, tc, disp
    ) -> O::Build {
        let fmt = self.fmt();
        let quotes = Renderer::<O>::quotes(quo);
        let options = IngestOptions {
            replace_hyphens: match var {
                StandardVariable::Ordinary(v) => v.should_replace_hyphens(),
                StandardVariable::Number(v) => v.should_replace_hyphens(),
            },
        };
        let b = fmt.ingest(value, options);
        let txt = fmt.with_format(b, f);

        let txt = match var {
            StandardVariable::Ordinary(v) => {
                let maybe_link = v.hyperlink(value);
                fmt.hyperlinked(txt, maybe_link)
            }
            StandardVariable::Number(_) => txt,
        };
        fmt.affixed_quoted(txt, &af, quotes.as_ref())
    }

    pub fn text_value(
        &self,
        value: &str,
        f: Option<Formatting>,
        af: &Affixes,
        quo: bool,
        // sp, tc, disp
    ) -> Option<O::Build> {
        if value.len() == 0 {
            return None;
        }
        let fmt = self.fmt();
        let quotes = Renderer::<O>::quotes(quo);
        let b = fmt.ingest(value, Default::default());
        let txt = fmt.with_format(b, f);
        Some(fmt.affixed_quoted(txt, af, quotes.as_ref()))
    }

    pub fn text_term(
        &self,
        term_selector: TextTermSelector,
        plural: bool,
        f: Option<Formatting>,
        af: &Affixes,
        quo: bool,
        // sp, tc, disp
    ) -> Option<O::Build> {
        let fmt = self.fmt();
        let locale = self.ctx.locale();
        let quotes = Renderer::<O>::quotes(quo);
        locale
            .get_text_term(term_selector, plural)
            .map(|val| fmt.affixed_text_quoted(val.to_owned(), f, af, quotes.as_ref()))
    }

    pub fn name_label(&self, label: &NameLabel, var: NameVariable) -> Option<O::Build> {
        let NameLabel {
            form,
            formatting,
            plural,
            affixes,
            // TODO: strip-periods
            strip_periods: _,
            // TODO: text-case
            text_case: _,
        } = label;
        let fmt = self.fmt();
        let selector = RoleTermSelector::from_name_variable(var, *form);
        let val = self.ctx.get_name(var);
        let len = val.map(|v| v.len()).unwrap_or(0);
        let plural = match (len, plural) {
            (0, Plural::Contextual) => return None,
            (1, Plural::Contextual) => false,
            (_, Plural::Contextual) => true,
            (_, Plural::Always) => true,
            (_, Plural::Never) => false,
        };
        selector.and_then(|sel| {
            self.ctx
                .locale()
                .get_text_term(TextTermSelector::Role(sel), plural)
                .map(|term_text| fmt.affixed_text(term_text.to_owned(), *formatting, affixes))
        })
    }

    pub fn numeric_label(
        &self,
        var: NumberVariable,
        form: TermForm,
        num_val: NumericValue,
        plural: Plural,
        f: Option<Formatting>,
        af: &Affixes,
    ) -> Option<O::Build> {
        let fmt = self.fmt();
        let selector =
            GenderedTermSelector::from_number_variable(&self.ctx.locator_type(), var, form);
        let plural = match (num_val, plural) {
            (ref val, Plural::Contextual) => val.is_multiple(),
            (_, Plural::Always) => true,
            (_, Plural::Never) => false,
        };
        selector.and_then(|sel| {
            self.ctx
                .locale()
                .get_text_term(TextTermSelector::Gendered(sel), plural)
                .map(|val| fmt.affixed_text(val.to_owned(), f, &af))
        })
    }
}
