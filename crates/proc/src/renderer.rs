use crate::cite_context::RenderContext;
use crate::number::{arabic_number, render_ordinal, roman_lower, roman_representable};
use crate::prelude::*;
use citeproc_io::output::LocalizedQuotes;
use citeproc_io::{Name, NumericToken, NumericValue, Reference};
use csl::{
    Features, GenderedTermSelector, LabelElement, Lang, Locale, LocatorType, NameLabel,
    NameVariable, NumberElement, NumberVariable, NumericForm, PageRangeFormat, Plural,
    RoleTermSelector, SortKey, StandardVariable, Style, TextElement, TextTermSelector, Variable,
    VariableForm,
};

use crate::choose::CondChecker;
use citeproc_io::DateOrRange;
use csl::{AnyVariable, DateVariable};
use csl::{CslType, Position};
use std::borrow::Cow;

#[derive(Clone)]
pub enum GenericContext<'a, O: OutputFormat, I: OutputFormat = O> {
    Ref(&'a RefContext<'a, O>),
    Cit(&'a CiteContext<'a, O, I>),
}

macro_rules! forward_inner {
    {$($vis:vis fn $name:ident(&self $(, $arg:ident : $ty:ty)* $(,)?) -> $ret:ty; )+ } => {
        $(
            fn $name(&self$(, $arg : $ty)*) -> $ret {
                match self {
                    GenericContext::Ref(r) => r.$name($($arg),*),
                    GenericContext::Cit(c) => c.$name($($arg),*),
                }
            }
        )+
    };
}

impl<'a, O: OutputFormat, I: OutputFormat> RenderContext for GenericContext<'a, O, I> {
    forward_inner! {
        fn style(&self) -> &Style;
        fn reference(&self) -> &Reference;
        fn locale(&self) -> &Locale;
        fn cite_lang(&self) -> Option<&Lang>;
        fn get_number(&self, var: NumberVariable) -> Option<NumericValue<'_>>;
        fn get_ordinary(&self, var: Variable, form: VariableForm) -> Option<Cow<'_, str>>;
        fn get_name(&self, var: NameVariable) -> Option<&[Name]>;
    }
}

impl<'a, O: OutputFormat, I: OutputFormat> CondChecker for GenericContext<'a, O, I> {
    forward_inner! {
        fn has_variable(&self, var: AnyVariable) -> bool;
        fn is_numeric(&self, var: AnyVariable) -> bool;
        fn is_disambiguate(&self, current_count: u32) -> bool;
        fn csl_type(&self) -> CslType;
        fn locator_type(&self) -> Option<LocatorType>;
        fn get_date(&self, dvar: DateVariable) -> Option<&DateOrRange>;
        fn position(&self) -> Option<Position>;
        fn features(&self) -> &Features;
        fn has_year_only(&self, dvar: DateVariable) -> bool;
        fn has_month_or_season(&self, dvar: DateVariable) -> bool;
        fn has_day(&self, dvar: DateVariable) -> bool;
    }
}

impl<'a, O: OutputFormat, I: OutputFormat> GenericContext<'a, O, I> {
    pub fn sort_key(&self) -> Option<&SortKey> {
        match self {
            GenericContext::Cit(ctx) => ctx.sort_key.as_ref(),
            GenericContext::Ref(_ctx) => None,
        }
    }

    /// https://docs.citationstyles.org/en/stable/specification.html#non-english-items
    pub fn is_english(&self) -> bool {
        let sty = self.style();
        let cite = self.cite_lang();
        // Bit messy but matches the spec wording
        // If a style doesn't have a default, it's en-US, which is English.
        let default_is_english = sty.default_locale.as_ref().map_or(true, |x| x.is_english());
        cite.map_or(default_is_english, |l| l.is_english())
    }

    /// For setting display="X" on elements, where this should only take effect in the
    /// bibliography.
    pub fn in_bibliography(&self) -> bool {
        match self {
            GenericContext::Cit(ctx) => ctx.in_bibliography,
            // Do not say "we're in a bibliography" if you're generating RefIR for cites to match
            // against.
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
}

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
    pub fn gen(ctx: GenericContext<'c, O, I>) -> Renderer<'c, O, I> {
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

    fn page_range_format(&self, var: NumberVariable) -> Option<PageRangeFormat> {
        let style = self.ctx.style();
        style.page_range_format.filter(|_| {
            var == NumberVariable::Page
                || (var == NumberVariable::Locator
                    && self
                        .ctx
                        .locator_type()
                        .map_or(false, |l| l == LocatorType::Page))
        })
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
    ) -> O::Build {
        let locale = self.ctx.locale();
        let fmt = self.fmt();
        let prf = self.page_range_format(var);
        use crate::sort::natural_sort;
        let affixes = Some(if var == NumberVariable::CitationNumber {
            natural_sort::citation_number_affixes()
        } else {
            natural_sort::num_affixes()
        });
        match (val, form) {
            (NumericValue::Tokens(_, ts, is_numeric), _) if *is_numeric => {
                let mut s = SmartString::new();
                for t in ts {
                    if !s.is_empty() {
                        s.push(',');
                    }
                    if let NumericToken::Num(n) = t {
                        s.push_str(&format!("{:08}", n));
                    }
                }
                fmt.affixed_text(s, None, affixes.as_ref())
            }
            _ => fmt.affixed_text(arabic_number(val, locale, var, prf), None, affixes.as_ref()),
        }
    }

    /// With variable="locator", this assumes ctx has a locator_type and will panic otherwise.
    pub fn number(&self, number: &NumberElement, val: &NumericValue<'_>) -> O::Build {
        let locale = self.ctx.locale();
        debug!("number {:?}", val);
        let prf = self.page_range_format(number.variable);
        let string = if let NumericValue::Tokens(_s, ts, true) = val {
            match number.form {
                NumericForm::Roman if roman_representable(&val) => {
                    roman_lower(&ts, locale, number.variable, prf)
                }
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
                    render_ordinal(&ts, locale, number.variable, prf, gender, long)
                }
                _ => arabic_number(val, locale, number.variable, prf),
            }
        } else {
            arabic_number(val, locale, number.variable, prf)
        };
        let fmt = self.fmt();
        let options = IngestOptions {
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
        val: &NumericValue<'_>,
    ) -> O::Build {
        let style = self.ctx.style();
        let _mod_page = style.page_range_format.is_some();
        if variable == NumberVariable::Locator || variable == NumberVariable::Page {
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
                StandardVariable::Number(v) => v.should_replace_hyphens(self.ctx.style()),
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
        locale
            .get_text_term(term_selector, plural)
            .filter(|x| !x.is_empty())
            .map(|val| {
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
        let mut affixes = text.affixes.as_ref();
        let (mut b, fixed_af) = self.try_link(string, options, hyper, affixes);
        affixes = fixed_af.as_ref().or(affixes);
        b = fmt.with_format(b, text.formatting);
        b = fmt.affixed_quoted(b, affixes, self.quotes_if(text.quotes));
        fmt.with_display(b, text.display, self.ctx.in_bibliography())
    }

    fn try_link(
        &self,
        string: &str,
        options: &IngestOptions,
        hyper: Option<Variable>,
        affixes: Option<&Affixes>,
    ) -> (O::Build, Option<Affixes>) {
        let fmt = self.fmt();
        if let Some(var) = hyper {
            if let Some((link, fixed_af)) =
                citeproc_io::output::links::try_link_affixed_opt(var, string, affixes)
            {
                let linked = fmt.link(link);
                return (linked, fixed_af);
            }
        }
        (fmt.ingest(string, options), None)
    }

    pub fn name_label(
        &self,
        label: &NameLabel,
        var: NameVariable,
        label_var: NameVariable,
    ) -> Option<O::Build> {
        let NameLabel {
            form,
            formatting,
            ref plural,
            ref affixes,
            strip_periods,
            text_case,
            after_name: _,
        } = *label;
        let fmt = self.fmt();
        let selector = RoleTermSelector::from_name_variable(label_var, form);
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
                .filter(|x| !x.is_empty())
                .map(|term_text| {
                    let options = IngestOptions {
                        text_case,
                        strip_periods,
                        quotes: self.quotes(),
                        is_english: self.ctx.is_english(),
                        ..Default::default()
                    };
                    let b = fmt.ingest(term_text, &options);
                    let b = fmt.with_format(b, formatting);
                    fmt.affixed(b, affixes.as_ref())
                })
        })
    }

    pub fn numeric_label(
        &self,
        label: &LabelElement,
        num_val: &NumericValue<'_>,
    ) -> Option<O::Build> {
        let fmt = self.fmt();
        let selector = GenderedTermSelector::from_number_variable(
            self.ctx.locator_type(),
            label.variable,
            label.form,
        );
        let plural = match label.plural {
            Plural::Contextual => num_val.is_multiple(label.variable),
            Plural::Always => true,
            Plural::Never => false,
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
                .filter(|x| !x.is_empty())
                .map(|val| {
                    let b = fmt.ingest(val, &options);
                    let b = fmt.with_format(b, label.formatting);
                    fmt.affixed(b, label.affixes.as_ref())
                })
        })
    }
}
