use crate::prelude::*;
use citeproc_io::output::LocalizedQuotes;
use citeproc_io::{Locator, Name, NumericToken, NumericValue, Reference};
use crate::number::{render_ordinal, roman_lower, roman_representable, arabic_number};
use csl::{
    GenderedTermSelector, LabelElement, Lang, Locale, LocatorType, NameLabel, NameVariable,
    NumberElement, NumberVariable, NumericForm, Plural, RoleTermSelector, SortKey,
    StandardVariable, Style, TextCase, TextElement, TextTermSelector, Variable,
};

#[derive(Clone)]
pub enum GenericContext<'a, O: OutputFormat, I: OutputFormat = O> {
    Ref(&'a RefContext<'a, O>),
    Cit(&'a CiteContext<'a, O, I>),
}

#[allow(dead_code)]
impl<O: OutputFormat, I: OutputFormat> GenericContext<'_, O, I> {
    pub fn sort_key(&self) -> Option<&SortKey> {
        match self {
            GenericContext::Cit(ctx) => ctx.sort_key.as_ref(),
            GenericContext::Ref(_ctx) => None,
        }
    }
    pub fn locale(&self) -> &Locale {
        match self {
            GenericContext::Cit(ctx) => ctx.locale,
            GenericContext::Ref(ctx) => ctx.locale,
        }
    }
    pub fn cite_lang(&self) -> Option<&Lang> {
        let refr = self.reference();
        refr.language.as_ref()
    }
    /// https://docs.citationstyles.org/en/stable/specification.html#non-english-items
    pub fn is_english(&self) -> bool {
        let sty = self.style();
        let cite = self.cite_lang();
        // Bit messy but matches the spec wording
        if sty.default_locale.is_english() {
            cite.map_or(true, |l| l.is_english())
        } else {
            cite.map_or(false, |l| l.is_english())
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
    pub fn in_bibliography(&self) -> bool {
        match self {
            GenericContext::Cit(ctx) => ctx.in_bibliography,
            GenericContext::Ref(_ctx) => false,
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
            GenericContext::Cit(ctx) => ctx.style.citation.disambiguate_add_year_suffix,
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
    fn get_number(&self, var: NumberVariable) -> Option<NumericValue> {
        match self {
            Cit(ctx) => ctx.get_number(var),
            Ref(ctx) => ctx.get_number(var),
        }
    }
}

use crate::choose::CondChecker;
use citeproc_io::DateOrRange;
use csl::{AnyVariable, DateVariable};
use csl::{CslType, Position};

impl<'a, O: OutputFormat> CondChecker for GenericContext<'a, O> {
    fn has_variable(&self, var: AnyVariable) -> bool {
        match self {
            Ref(ctx) => <RefContext<'a, O> as CondChecker>::has_variable(ctx, var),
            Cit(ctx) => <CiteContext<'a, O> as CondChecker>::has_variable(ctx, var),
        }
    }
    fn is_numeric(&self, var: AnyVariable) -> bool {
        match self {
            Ref(ctx) => <RefContext<'a, O> as CondChecker>::is_numeric(ctx, var),
            Cit(ctx) => <CiteContext<'a, O> as CondChecker>::is_numeric(ctx, var),
        }
    }
    fn is_disambiguate(&self, current_count: u32) -> bool {
        match self {
            Ref(ctx) => <RefContext<'a, O> as CondChecker>::is_disambiguate(ctx, current_count),
            Cit(ctx) => <CiteContext<'a, O> as CondChecker>::is_disambiguate(ctx, current_count),
        }
    }
    fn csl_type(&self) -> CslType {
        match self {
            Ref(ctx) => <RefContext<'a, O> as CondChecker>::csl_type(ctx),
            Cit(ctx) => <CiteContext<'a, O> as CondChecker>::csl_type(ctx),
        }
    }
    fn locator_type(&self) -> Option<LocatorType> {
        match self {
            Ref(ctx) => <RefContext<'a, O> as CondChecker>::locator_type(ctx),
            Cit(ctx) => <CiteContext<'a, O> as CondChecker>::locator_type(ctx),
        }
    }
    fn get_date(&self, dvar: DateVariable) -> Option<&DateOrRange> {
        match self {
            Ref(ctx) => <RefContext<'a, O> as CondChecker>::get_date(ctx, dvar),
            Cit(ctx) => <CiteContext<'a, O> as CondChecker>::get_date(ctx, dvar),
        }
    }
    fn position(&self) -> Position {
        match self {
            Ref(ctx) => <RefContext<'a, O> as CondChecker>::position(ctx),
            Cit(ctx) => <CiteContext<'a, O> as CondChecker>::position(ctx),
        }
    }
    fn features(&self) -> &csl::version::Features {
        match self {
            Ref(ctx) => <RefContext<'a, O> as CondChecker>::features(ctx),
            Cit(ctx) => <CiteContext<'a, O> as CondChecker>::features(ctx),
        }
    }
}

use GenericContext::*;

pub struct Renderer<'a, O: OutputFormat, Custom: OutputFormat = O> {
    ctx: GenericContext<'a, O, Custom>,
}

impl<'c, O: OutputFormat> Renderer<'c, O, O> {
    pub fn refr(c: &'c RefContext<'c, O>) -> Self {
        Renderer {
            ctx: GenericContext::Ref(c),
        }
    }
}

impl<'c, O: OutputFormat, I: OutputFormat> Renderer<'c, O, I> {
    pub fn sorting(ctx: GenericContext<'c, O, I>) -> Renderer<'c, O, I> {
        Renderer { ctx }
    }
    pub fn cite(c: &'c CiteContext<'c, O, I>) -> Self {
        Renderer {
            ctx: GenericContext::Cit(c),
        }
    }

    #[inline]
    fn fmt(&self) -> &O {
        self.ctx.format()
    }

    /// The spec is slightly impractical to implement:
    ///
    /// > Number variables rendered within the macro with cs:number and date variables are treated
    /// > the same as when they are called via variable.
    ///
    /// ... bu when it's a macro, you have to produce a string. So we just do an arbitrary amount
    /// of left-padding.
    pub fn number_sort_string(
        &self,
        var: NumberVariable,
        form: NumericForm,
        val: &NumericValue,
        _af: Option<&Affixes>,
        text_case: TextCase,
    ) -> O::Build {
        let locale = self.ctx.locale();
        let style = self.ctx.style();
        let fmt = self.fmt();
        let prf = if var == NumberVariable::Page && style.page_range_format.is_some() {
            style.page_range_format
        } else {
            None
        };
        match (val, form) {
            (NumericValue::Tokens(_, ts), _) => {
                let mut s = String::new();
                for t in ts {
                    if !s.is_empty() {
                        s.push(',');
                    }
                    if let NumericToken::Num(n) = t {
                        s.push_str(&format!("{:08}", n));
                    }
                }
                let _options = IngestOptions {
                    replace_hyphens: false,
                    text_case,
                    quotes: self.quotes(),
                    is_english: self.ctx.is_english(),
                    ..Default::default()
                };
                fmt.affixed_text(
                    s,
                    None,
                    Some(crate::sort::natural_sort::num_affixes()).as_ref(),
                )
            }
            // TODO: text-case
            _ => fmt.affixed_text(
                arabic_number(val, locale, prf),
                None,
                Some(crate::sort::natural_sort::num_affixes()).as_ref(),
            ),
        }
    }

    /// With variable="locator", this assumes ctx has a locator_type and will panic otherwise.
    pub fn number(&self, number: &NumberElement, val: &NumericValue) -> O::Build {
        let locale = self.ctx.locale();
        let style = self.ctx.style();
        debug!("number {:?}", val);
        let prf = if number.variable == NumberVariable::Page && style.page_range_format.is_some() {
            style.page_range_format
        } else {
            None
        };
        let string = if let NumericValue::Tokens(_, ts) = val {
            match number.form {
                NumericForm::Roman if roman_representable(&val) => roman_lower(&ts, locale, prf),
                NumericForm::Ordinal | NumericForm::LongOrdinal => {
                    let loc_type = if number.variable == NumberVariable::Locator {
                        self.ctx
                            .locator_type()
                            .expect("already known that locator exists and therefore has a type")
                    } else {
                        // Not used
                        LocatorType::default()
                    };
                    let gender = locale.get_num_gender(number.variable, loc_type);
                    let long = number.form == NumericForm::LongOrdinal;
                    render_ordinal(&ts, locale, prf, gender, long)
                }
                _ => arabic_number(val, locale, prf),
            }
        } else {
            arabic_number(val, locale, prf)
        };
        let fmt = self.fmt();
        let options = IngestOptions {
            replace_hyphens: number.variable.should_replace_hyphens(),
            text_case: number.text_case,
            quotes: self.quotes(),
            is_english: self.ctx.is_english(),
            ..Default::default()
        };
        let b = fmt.ingest(&string, &options);
        let b = fmt.with_format(b, number.formatting);
        let b = fmt.affixed(b, number.affixes.as_ref());
        fmt.with_display(b, number.display, self.ctx.in_bibliography())
    }
    pub fn quotes(&self) -> LocalizedQuotes {
        LocalizedQuotes::from_locale(self.ctx.locale())
    }
    pub fn quotes_if(&self, quo: bool) -> Option<LocalizedQuotes> {
        let q = self.quotes();
        if quo {
            Some(q)
        } else {
            None
        }
    }


    pub fn text_number_variable(
        &self,
        text: &TextElement,
        variable: NumberVariable,
        val: &NumericValue,
    ) -> O::Build {
        let style = self.ctx.style();
        let mod_page = style.page_range_format.is_some();
        if variable == NumberVariable::Locator || (variable == NumberVariable::Page && mod_page) {
            let number = csl::NumberElement {
                variable,
                form: csl::NumericForm::default(),
                formatting: text.formatting,
                affixes: text.affixes.clone(),
                text_case: text.text_case,
                display: text.display,
            };
            self.number(&number, val)
        } else {
            self.text_variable(text, StandardVariable::Number(variable), val.verbatim())
        }
    }

    pub fn text_variable(
        &self,
        text: &TextElement,
        var: StandardVariable,
        value: &str,
    ) -> O::Build {
        let options = IngestOptions {
            replace_hyphens: match var {
                StandardVariable::Ordinary(v) => v.should_replace_hyphens(),
                StandardVariable::Number(v) => v.should_replace_hyphens(),
            },
            text_case: text.text_case,
            quotes: self.quotes(),
            strip_periods: text.strip_periods,
            is_english: self.ctx.is_english(),
            ..Default::default()
        };
        let hyper = match var {
            StandardVariable::Ordinary(v) => Some(v),
            StandardVariable::Number(_) => None,
        };
        self.render_text_el(value, text, &options, hyper)
    }

    pub fn text_value(&self, text: &TextElement, value: &str) -> Option<O::Build> {
        if value.is_empty() {
            return None;
        }
        let options = IngestOptions {
            text_case: text.text_case,
            quotes: self.quotes(),
            strip_periods: text.strip_periods,
            is_english: self.ctx.is_english(),
            ..Default::default()
        };
        Some(self.render_text_el(value, text, &options, None))
    }

    pub fn text_term(
        &self,
        text: &TextElement,
        term_selector: TextTermSelector,
        plural: bool,
    ) -> Option<O::Build> {
        let locale = self.ctx.locale();
        locale.get_text_term(term_selector, plural).map(|val| {
            let options = IngestOptions {
                text_case: text.text_case,
                quotes: self.quotes(),
                strip_periods: text.strip_periods,
                is_english: self.ctx.is_english(),
                ..Default::default()
            };
            self.render_text_el(val, text, &options, None)
        })
    }

    fn render_text_el(
        &self,
        string: &str,
        text: &TextElement,
        options: &IngestOptions,
        hyper: Option<Variable>,
    ) -> O::Build {
        let fmt = self.fmt();
        let mut b = fmt.ingest(string, &options);
        b = fmt.with_format(b, text.formatting);
        if let Some(hyper) = hyper {
            let maybe_link = hyper.hyperlink(string);
            b = fmt.hyperlinked(b, maybe_link)
        }
        b = fmt.affixed_quoted(b, text.affixes.as_ref(), self.quotes_if(text.quotes));
        fmt.with_display(b, text.display, self.ctx.in_bibliography())
    }

    pub fn name_label(&self, label: &NameLabel, var: NameVariable) -> Option<O::Build> {
        let NameLabel {
            form,
            formatting,
            plural,
            affixes,
            ..
            // after_name: _,
            // TODO: strip-periods
            // strip_periods: _,
            // TODO: text-case
            // text_case: _,
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
                .map(|term_text| {
                    fmt.affixed_text(term_text.to_owned(), *formatting, affixes.as_ref())
                })
        })
    }

    pub fn numeric_label(&self, label: &LabelElement, num_val: NumericValue) -> Option<O::Build> {
        let fmt = self.fmt();
        let selector = GenderedTermSelector::from_number_variable(
            self.ctx.locator_type(),
            label.variable,
            label.form,
        );
        let plural = match (num_val, label.plural) {
            (ref val, Plural::Contextual) => val.is_multiple(),
            (_, Plural::Always) => true,
            (_, Plural::Never) => false,
        };
        selector.and_then(|sel| {
            let options = IngestOptions {
                text_case: label.text_case,
                quotes: self.quotes(),
                is_english: self.ctx.is_english(),
                ..Default::default()
            };
            self.ctx
                .locale()
                .get_text_term(TextTermSelector::Gendered(sel), plural)
                .map(|val| {
                    let b = fmt.ingest(val, &options);
                    let b = fmt.with_format(b, label.formatting);
                    fmt.affixed(b, label.affixes.as_ref())
                })
        })
    }
}
