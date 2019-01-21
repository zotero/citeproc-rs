use super::cite_context::*;
use super::ir::*;
use super::Proc;
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
    fn intermediate<'s: 'c>(&'s self, ctx: &CiteContext<'c, O>) -> IR<'c, O>
    where
        O: OutputFormat,
    {
        // TODO: wrap BodyDate in a YearSuffixHook::Date() under certain conditions
        match *self {
            BodyDate::Indep(ref idate) => idate.intermediate(ctx),
            BodyDate::Local(ref ldate) => ldate.intermediate(ctx),
        }
    }
}

impl<'c, O> Proc<'c, O> for LocalizedDate
where
    O: OutputFormat,
{
    fn intermediate<'s: 'c>(&'s self, ctx: &CiteContext<'c, O>) -> IR<'c, O>
    where
        O: OutputFormat,
    {
        let fmt = ctx.format;
        let locale = ctx.style.locale_overrides.get(&None).unwrap();
        let locale_date = locale.dates.get(&self.form).unwrap();
        // TODO: render date ranges
        // TODO: TextCase
        let date = ctx
            .reference
            .date
            .get(&self.variable)
            .and_then(|d| d.single());
        let content = date.map(|val| {
            let each: Vec<_> = locale_date
                .date_parts
                .iter()
                .filter(|d| d.matches(self.parts_selector.clone()))
                .map(|dp| dp.render(ctx, &val))
                .collect();
            let delim = &locale_date.delimiter.0;
            fmt.affixed(
                fmt.group(each, delim, self.formatting.as_ref()),
                &self.affixes,
            )
        });
        IR::Rendered(content)
    }
}

impl<'c, O> Proc<'c, O> for IndependentDate
where
    O: OutputFormat,
{
    fn intermediate<'s: 'c>(&'s self, ctx: &CiteContext<'c, O>) -> IR<'c, O>
    where
        O: OutputFormat,
    {
        let fmt = ctx.format;
        let content = ctx
            .reference
            .date
            .get(&self.variable)
            // TODO: render date ranges
            .and_then(|d| d.single())
            .map(|val| {
                let each: Vec<_> = self
                    .date_parts
                    .iter()
                    .map(|dp| dp.render(ctx, &val))
                    .collect();
                let delim = &self.delimiter.0;
                fmt.affixed(
                    fmt.group(each, delim, self.formatting.as_ref()),
                    &self.affixes,
                )
            });
        IR::Rendered(content)
    }
}

impl DatePart {
    fn matches(&self, selector: DateParts) -> bool {
        match self.form {
            DatePartForm::Day(_) => selector == DateParts::YearMonthDay,
            DatePartForm::Month(_) => selector != DateParts::Year,
            DatePartForm::Year(_) => true,
        }
    }
    fn render<'c, O: OutputFormat>(&self, ctx: &CiteContext<'c, O>, date: &Date) -> O::Build {
        let string = match self.form {
            DatePartForm::Year(ref form) => match form {
                YearForm::Long => format!("{}", date.year),
                YearForm::Short => format!("{:02}", date.year % 100),
            },
            DatePartForm::Month(ref form) => match form {
                // TODO: locale getter for months
                MonthForm::Long => MONTHS_LONG[date.month as usize].to_string(),
                MonthForm::Short => MONTHS_SHORT[date.month as usize].to_string(),
                MonthForm::Numeric => format!("{}", date.month),
                MonthForm::NumericLeadingZeros => format!("{:02}", date.month),
            },
            DatePartForm::Day(ref form) => match form {
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
