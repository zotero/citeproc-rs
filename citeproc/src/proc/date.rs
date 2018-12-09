use super::cite_context::*;
use super::ir::*;
use super::Proc;
use crate::input::Date;
use crate::output::OutputFormat;
use crate::style::element::{
    DatePart, DatePartForm, DayForm, IndependentDate, MonthForm, YearForm,
};

const MONTHS_SHORT: &'static [&'static str] = &[
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

const MONTHS_LONG: &'static [&'static str] = &[
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

impl DatePart {
    fn render<'c, 'r, 'ci, O: OutputFormat>(
        &self,
        ctx: &CiteContext<'c, 'r, 'ci, O>,
        date: &Date,
    ) -> O::Build {
        let string = match self.form {
            DatePartForm::Year(ref form) => match form {
                YearForm::Long => format!("{}", date.year),
                YearForm::Short => format!("{:02}", date.year % 100),
            },
            DatePartForm::Month(ref form) => match form {
                // TODO: locale getter for months
                MonthForm::Long => format!("{}", MONTHS_LONG[date.month as usize]),
                MonthForm::Short => format!("{}", MONTHS_SHORT[date.month as usize]),
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

impl<'c, 'r: 'c, 'ci: 'c, O> Proc<'c, 'r, 'ci, O> for IndependentDate
where
    O: OutputFormat,
{
    fn intermediate<'s: 'c>(&'s self, ctx: &CiteContext<'c, 'r, 'ci, O>) -> IR<'c, O>
    where
        O: OutputFormat,
    {
        let fmt = ctx.format;
        // TODO: support locale-defined dates with Date,
        // and use an IndependentDate for unlocalized.
        let content = ctx
            .reference
            .date
            .get(&self.variable)
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
