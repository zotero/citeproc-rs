// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::prelude::*;

use citeproc_io::Date;
use csl::style::{
    BodyDate, DatePart, DatePartForm, DateParts, DayForm, IndependentDate, LocalizedDate,
    MonthForm, YearForm,
};

impl<'c, O> Proc<'c, O> for BodyDate
where
    O: OutputFormat,
{
    fn intermediate(
        &self,
        db: &impl IrDatabase,
        _state: &mut IrState,
        ctx: &CiteContext<'c, O>,
    ) -> IrSum<O>
    where
        O: OutputFormat,
    {
        // TODO: wrap BodyDate in a YearSuffixHook::Date() under certain conditions
        let content = match *self {
            BodyDate::Indep(ref idate) => {
                intermediate_generic_indep(idate, db, GenericContext::Cit(ctx))
            }
            BodyDate::Local(ref ldate) => {
                intermediate_generic_local(ldate, db, GenericContext::Cit(ctx))
            }
        }
        .map(|o| CiteEdgeData::Output(o));
        let gv = GroupVars::rendered_if(content.is_some());
        (IR::Rendered(content), gv)
    }
}

impl Disambiguation<Markup> for BodyDate {
    fn ref_ir(
        &self,
        db: &impl IrDatabase,
        ctx: &RefContext<Markup>,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        let fmt = ctx.format;
        let edge = match *self {
            BodyDate::Indep(ref idate) => {
                intermediate_generic_indep(idate, db, GenericContext::Ref(ctx))
            }
            BodyDate::Local(ref ldate) => {
                intermediate_generic_local(ldate, db, GenericContext::Ref(ctx))
            }
        }
        .map(|b| fmt.output_in_context(b, stack))
        .map(|o| db.edge(EdgeData::Output(o)));
        let gv = GroupVars::rendered_if(edge.is_some());
        (RefIR::Edge(edge), gv)
    }
}

fn intermediate_generic_local<'c, O>(
    local: &LocalizedDate,
    _db: &impl IrDatabase,
    ctx: GenericContext<'c, O>,
) -> Option<O::Build>
where
    O: OutputFormat,
{
    let fmt = ctx.format();
    let locale = ctx.locale();
    let refr = ctx.reference();
    // TODO: handle missing
    let locale_date = locale.dates.get(&local.form).unwrap();
    // TODO: render date ranges
    // TODO: TextCase
    let date = refr
        .date
        .get(&local.variable)
        .and_then(|r| r.single_or_first());
    date.map(|val| {
        let each: Vec<_> = locale_date
            .date_parts
            .iter()
            .filter(|dp| dp_matches(dp, local.parts_selector))
            .filter_map(|dp| dp_render(dp, ctx.clone(), &val))
            .collect();
        let delim = &locale_date.delimiter.0;
        fmt.affixed(fmt.group(each, delim, local.formatting), &local.affixes)
    })
}

fn intermediate_generic_indep<'c, O>(
    indep: &IndependentDate,
    _db: &impl IrDatabase,
    ctx: GenericContext<'c, O>,
) -> Option<O::Build>
where
    O: OutputFormat,
{
    let fmt = ctx.format();
    ctx.reference()
        .date
        .get(&indep.variable)
        // TODO: render date ranges
        .and_then(|r| r.single_or_first())
        .map(|val| {
            let each: Vec<_> = indep
                .date_parts
                .iter()
                .filter_map(|dp| dp_render(dp, ctx.clone(), &val))
                .collect();
            let delim = &indep.delimiter.0;
            fmt.affixed(fmt.group(each, delim, indep.formatting), &indep.affixes)
        })
}

fn dp_matches(part: &DatePart, selector: DateParts) -> bool {
    match part.form {
        DatePartForm::Day(_) => selector == DateParts::YearMonthDay,
        DatePartForm::Month(..) => selector != DateParts::Year,
        DatePartForm::Year(_) => true,
    }
}

fn dp_render<'c, O: OutputFormat>(
    part: &DatePart,
    ctx: GenericContext<'c, O>,
    date: &Date,
) -> Option<O::Build> {
    let locale = ctx.locale();
    let string = match part.form {
        DatePartForm::Year(form) => match form {
            YearForm::Long => Some(format!("{}", date.year)),
            YearForm::Short => Some(format!("{:02}", date.year % 100)),
        },
        DatePartForm::Month(form, _strip_periods) => match form {
            MonthForm::Numeric => {
                if date.month == 0 || date.month > 12 {
                    None
                } else {
                    Some(format!("{}", date.month))
                }
            }
            MonthForm::NumericLeadingZeros => {
                if date.month == 0 || date.month > 12 {
                    None
                } else {
                    Some(format!("{:02}", date.month))
                }
            }
            _ => {
                // TODO: support seasons
                if date.month == 0 || date.month > 12 {
                    return None;
                }
                use csl::terms::*;
                let term_form = match form {
                    MonthForm::Long => TermForm::Long,
                    MonthForm::Short => TermForm::Short,
                    _ => TermForm::Long,
                };
                let sel = GenderedTermSelector::Month(
                    MonthTerm::from_u32(date.month).expect("TODO: support seasons"),
                    term_form,
                );
                Some(
                    locale
                        .gendered_terms
                        .get(&sel)
                        .map(|gt| gt.0.singular().to_string())
                        .unwrap_or_else(|| {
                            let fallback = if term_form == TermForm::Short {
                                MONTHS_SHORT
                            } else {
                                MONTHS_LONG
                            };
                            fallback[date.month as usize].to_string()
                        }),
                )
            }
        },
        DatePartForm::Day(form) => match form {
            DayForm::Numeric => {
                if date.day == 0 {
                    None
                } else {
                    Some(format!("{}", date.day))
                }
            }
            DayForm::NumericLeadingZeros => {
                if date.day == 0 {
                    None
                } else {
                    Some(format!("{:02}", date.day))
                }
            }
            // TODO: implement ordinals
            DayForm::Ordinal => {
                if date.day == 0 {
                    None
                } else {
                    Some(format!("{}ORD", date.day))
                }
            }
        },
    };
    string.map(|s| ctx.format().affixed_text(s, part.formatting, &part.affixes))
}

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
