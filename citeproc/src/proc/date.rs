use super::ir::*;
use super::Proc;
use crate::input::DateOrRange;
use crate::input::{ Reference, Date };
use crate::output::OutputFormat;
use crate::style::element::{IndependentDate, DatePartForm, DatePartName, YearForm, MonthForm, DayForm, DatePart, Formatting};
use super::cite_context::*;

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
    fn render<'c, 'r, O: OutputFormat>(&self, ctx: &CiteContext<'c, 'r, O>, date: &Date) -> O::Build {
        let string = match self.form {
            DatePartForm::Year(ref form) => match form {
                YearForm::Long => format!("{}", date.year),
                YearForm::Short => format!("{:02}", date.year % 100),
            }
            DatePartForm::Month(ref form) => match form {
                // TODO: locale getter for months
                MonthForm::Long => format!("{}", MONTHS_LONG[date.month as usize]),
                MonthForm::Short => format!("{}", MONTHS_SHORT[date.month as usize]),
                MonthForm::Numeric => format!("{}", date.month),
                MonthForm::NumericLeadingZeros => format!("{:02}", date.month),
            }
            DatePartForm::Day(ref form) => match form {
                DayForm::Numeric => format!("{}", date.day),
                DayForm::NumericLeadingZeros => format!("{:02}", date.day),
                // TODO: implement ordinals
                DayForm::Ordinal => format!("{:02}", date.day),
            }
        };
        ctx.format.affixed(&string, &self.formatting, &self.affixes)
    }
}

impl<'c, 's: 'c> Proc<'c, 's> for IndependentDate {
    #[cfg_attr(feature = "flame_it", flame)]
    fn intermediate<'r, O>(&'s self, ctx: &CiteContext<'c, 'r, O>) -> IR<'c, O>
    where
        O: OutputFormat,
    {
        let fmt = ctx.format;
        // TODO: support locale-defined dates with Date,
        // and use an IndependentDate for unlocalized.
        let content = ctx.reference.date
            .get(&self.variable)
            .and_then(|d| d.single())
            .map(|val| {
                let mut each: Vec<_> = self.date_parts
                    .iter()
                    .map(|dp| dp.render(ctx, &val))
                    .collect();
                let delim = &self.delimiter.0;
                fmt.group(
                    &[
                        fmt.plain(&self.affixes.prefix),
                        fmt.group(&each, delim, &self.formatting),
                        fmt.plain(&self.affixes.suffix),
                    ],
                    "",
                    &Formatting::default(),
                )

                // let string = format!("{}-{}-{}", val.year, val.month, val.day);
                // fmt.affixed(&string, &self.formatting, &self.affixes)
            });
        IR::Rendered(content)
    }
}
