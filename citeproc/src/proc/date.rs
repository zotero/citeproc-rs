use super::cite_context::*;
use super::disamb::AddDisambTokens;
use super::group::GroupVars;
use super::ir::*;
use super::{IrState, Proc};
use crate::db::ReferenceDatabase;
use crate::input::Date;
use crate::output::OutputFormat;
use crate::style::element::{
    BodyDate, DatePart, DatePartForm, DateParts, DayForm, IndependentDate, LocalizedDate,
    MonthForm, YearForm,
};

const MONTHS_SHORT: &[&str] = &[
    "undefined",
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "May",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
];

const MONTHS_LONG: &[&str] = &[
    "undefined",
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

impl<'c, O> Proc<'c, O> for BodyDate
where
    O: OutputFormat,
{
    fn intermediate<'s: 'c>(
        &'s self,
        db: &impl ReferenceDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O>,
    ) -> IrSum<'c, O>
    where
        O: OutputFormat,
    {
        // TODO: wrap BodyDate in a YearSuffixHook::Date() under certain conditions
        match *self {
            BodyDate::Indep(ref idate) => idate.intermediate(db, state, ctx),
            BodyDate::Local(ref ldate) => ldate.intermediate(db, state, ctx),
        }
    }
}

impl<'c, O> Proc<'c, O> for LocalizedDate
where
    O: OutputFormat,
{
    fn intermediate<'s: 'c>(
        &'s self,
        db: &impl ReferenceDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O>,
    ) -> IrSum<'c, O>
    where
        O: OutputFormat,
    {
        let fmt = ctx.format;
        let locale = db.merged_locale(ctx.style.default_locale.clone());
        // TODO: handle missing
        let locale_date = locale.dates.get(&self.form).unwrap();
        // TODO: render date ranges
        // TODO: TextCase
        let date = ctx.reference.date.get(&self.variable).and_then(|d| {
            d.add_disamb_tokens(&mut state.tokens);
            d.single()
        });
        let content = date.map(|val| {
            let each: Vec<_> = locale_date
                .date_parts
                .iter()
                .filter(|d| d.matches(self.parts_selector.clone()))
                .map(|dp| dp.render(db, state, ctx, &val))
                .collect();
            let delim = &locale_date.delimiter.0;
            fmt.affixed(
                fmt.group(each, delim, self.formatting.as_ref()),
                &self.affixes,
            )
        });
        let gv = GroupVars::rendered_if(content.is_some());
        (IR::Rendered(content), gv)
    }
}

impl<'c, O> Proc<'c, O> for IndependentDate
where
    O: OutputFormat,
{
    fn intermediate<'s: 'c>(
        &'s self,
        db: &impl ReferenceDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O>,
    ) -> IrSum<'c, O>
    where
        O: OutputFormat,
    {
        let fmt = ctx.format;
        let content = ctx
            .reference
            .date
            .get(&self.variable)
            // TODO: render date ranges
            .and_then(|d| {
                d.add_disamb_tokens(&mut state.tokens);
                d.single()
            })
            .map(|val| {
                let each: Vec<_> = self
                    .date_parts
                    .iter()
                    .map(|dp| dp.render(db, state, ctx, &val))
                    .collect();
                let delim = &self.delimiter.0;
                fmt.affixed(
                    fmt.group(each, delim, self.formatting.as_ref()),
                    &self.affixes,
                )
            });
        let gv = GroupVars::rendered_if(content.is_some());
        (IR::Rendered(content), gv)
    }
}

impl DatePart {
    fn matches(&self, selector: DateParts) -> bool {
        match self.form {
            DatePartForm::Day(_) => selector == DateParts::YearMonthDay,
            DatePartForm::Month(..) => selector != DateParts::Year,
            DatePartForm::Year(_) => true,
        }
    }
    fn render<'c, O: OutputFormat>(
        &self,
        _db: &impl ReferenceDatabase,
        _state: &mut IrState,
        ctx: &CiteContext<'c, O>,
        date: &Date,
    ) -> O::Build {
        let string = match self.form {
            DatePartForm::Year(form) => match form {
                YearForm::Long => format!("{}", date.year),
                YearForm::Short => format!("{:02}", date.year % 100),
            },
            DatePartForm::Month(form, _strip_periods) => match form {
                // TODO: locale getter for months
                MonthForm::Long => MONTHS_LONG[date.month as usize].to_string(),
                MonthForm::Short => MONTHS_SHORT[date.month as usize].to_string(),
                MonthForm::Numeric => format!("{}", date.month),
                MonthForm::NumericLeadingZeros => format!("{:02}", date.month),
            },
            DatePartForm::Day(form) => match form {
                DayForm::Numeric => format!("{}", date.day),
                DayForm::NumericLeadingZeros => format!("{:02}", date.day),
                // TODO: implement ordinals
                DayForm::Ordinal => format!("{:02}", date.day),
            },
        };
        ctx.format
            .affixed_text(string, self.formatting.as_ref(), &self.affixes)
    }
}
