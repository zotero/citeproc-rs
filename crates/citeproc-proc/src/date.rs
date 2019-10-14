// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::disamb::mult_identity;
use crate::prelude::*;

use citeproc_io::Date;
use csl::locale::LocaleDate;
use csl::style::{
    BodyDate, DatePart, DatePartForm, DateParts, DayForm, IndependentDate, LocalizedDate,
    MonthForm, YearForm,
};
use csl::Atom;
use std::mem;

enum Either<O: OutputFormat> {
    Build(Option<O::Build>),
    /// We will convert this to RefIR as necessary. It will only contain Outputs and
    /// YearSuffixHooks It will only contain Outputs and YearSuffixHooks.
    /// It will not be Rendered(None).
    Ir(IR<O>),
}

impl<O: OutputFormat> Either<O> {
    fn into_cite_ir(self) -> IrSum<O> {
        match self {
            Either::Build(opt) => {
                let content = opt.map(CiteEdgeData::Output);
                let gv = GroupVars::rendered_if(content.is_some());
                (IR::Rendered(content), gv)
            }
            Either::Ir(ir) => {
                let gv = if let IR::Rendered(None) = &ir {
                    GroupVars::OnlyEmpty
                } else {
                    GroupVars::DidRender
                };
                (ir, gv)
            }
        }
    }
}

fn to_ref_ir(
    ir: IR<Markup>,
    stack: Formatting,
    ys_edge: Edge,
    to_edge: &impl Fn(Option<CiteEdgeData<Markup>>, Formatting) -> Option<Edge>,
) -> RefIR {
    match ir {
        // Either Rendered(Some(CiteEdgeData::YearSuffix)) or explicit year suffixes can end up as
        // EdgeData::YearSuffix edges in RefIR. Because we don't care whether it's been rendered or
        // not -- in RefIR's comparison, it must always be an EdgeData::YearSuffix.
        IR::Rendered(opt_build) => RefIR::Edge(to_edge(opt_build, stack)),
        IR::YearSuffix(ysh, _opt_build) => RefIR::Edge(Some(ys_edge)),
        IR::Seq(ir_seq) => RefIR::Seq(RefIrSeq {
            contents: ir_seq
                .contents
                .into_iter()
                .map(|x| to_ref_ir(x, stack, ys_edge, to_edge))
                .collect(),
            formatting: ir_seq.formatting,
            affixes: ir_seq.affixes,
            delimiter: ir_seq.delimiter,
        }),
        IR::ConditionalDisamb(..) | IR::Name(_) => unreachable!(),
    }
}

impl Either<Markup> {
    fn into_ref_ir(
        self,
        db: &impl IrDatabase,
        ctx: &RefContext<Markup>,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        let fmt = ctx.format;
        let to_edge =
            |opt_cite_edge: Option<CiteEdgeData<Markup>>, stack: Formatting| -> Option<Edge> {
                opt_cite_edge.map(|cite_edge| db.edge(cite_edge.to_edge_data(fmt, stack)))
            };
        let ys_edge = db.edge(EdgeData::YearSuffix);
        match self {
            Either::Build(opt) => {
                let content = opt.map(CiteEdgeData::Output);
                let edge = to_edge(content, stack);
                let gv = GroupVars::rendered_if(edge.is_some());
                (RefIR::Edge(edge), gv)
            }
            Either::Ir(ir) => {
                // If it's Ir we'll assume there is a year suffix hook in there -- so not
                // Rendered(None), at least.
                (
                    to_ref_ir(ir, stack, ys_edge, &to_edge),
                    GroupVars::DidRender,
                )
            }
        }
    }
}

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
        match self {
            BodyDate::Indep(idate) => {
                intermediate_generic_indep(idate, db, GenericContext::Cit(ctx))
            }
            BodyDate::Local(ldate) => {
                intermediate_generic_local(ldate, db, GenericContext::Cit(ctx))
            }
        }
        .map(Either::into_cite_ir)
        .unwrap_or((IR::Rendered(None), GroupVars::rendered_if(false)))
    }
}

impl Disambiguation<Markup> for BodyDate {
    fn get_free_conds(&self, db: &impl IrDatabase) -> FreeCondSets {
        use csl::style::Cond;
        use csl::variables::{AnyVariable, Variable};
        // Position may be involved for NASO and primary disambiguation
        let mut base = mult_identity();
        let cond = Cond::Variable(AnyVariable::Ordinary(Variable::YearSuffix));
        base.scalar_multiply_cond(cond, true);
        base
    }

    fn ref_ir(
        &self,
        db: &impl IrDatabase,
        ctx: &RefContext<Markup>,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        let fmt = ctx.format;
        match self {
            BodyDate::Indep(idate) => {
                intermediate_generic_indep(idate, db, GenericContext::Ref(ctx))
            }
            BodyDate::Local(ldate) => {
                intermediate_generic_local(ldate, db, GenericContext::Ref(ctx))
            }
        }
        .map(|e| e.into_ref_ir(db, ctx, stack))
        .unwrap_or((RefIR::Edge(None), GroupVars::rendered_if(false)))
    }
}

struct GenericDateBits<'a> {
    overall_formatting: Option<Formatting>,
    overall_affixes: &'a Affixes,
    overall_delimiter: &'a Atom,
}

struct PartBuilder<'a, O: OutputFormat> {
    bits: GenericDateBits<'a>,
    acc: PartAccumulator<O>,
}

enum PartAccumulator<O: OutputFormat> {
    Builds(Vec<O::Build>),
    Seq(IrSeq<O>),
}

impl<'a, O: OutputFormat> PartBuilder<'a, O> {
    fn new(bits: GenericDateBits<'a>, len_hint: usize) -> Self {
        PartBuilder {
            bits,
            acc: PartAccumulator::Builds(Vec::with_capacity(len_hint)),
        }
    }

    fn upgrade(&mut self) {
        let PartBuilder {
            ref mut acc,
            ref mut bits,
        } = self;
        *acc = match acc {
            PartAccumulator::Builds(ref mut vec) => {
                let vec = mem::replace(vec, Vec::new());
                let mut seq = IrSeq {
                    contents: Vec::with_capacity(vec.capacity()),
                    formatting: bits.overall_formatting,
                    delimiter: bits.overall_delimiter.clone(),
                    affixes: bits.overall_affixes.clone(),
                };
                for built in vec {
                    seq.contents
                        .push(IR::Rendered(Some(CiteEdgeData::Output(built))))
                }
                PartAccumulator::Seq(seq)
            }
            _ => return,
        }
    }

    fn push_either(&mut self, either: Either<O>) {
        match either {
            Either::Ir(ir) => {
                self.upgrade();
                match &mut self.acc {
                    PartAccumulator::Seq(ref mut seq) => {
                        seq.contents.push(ir);
                    }
                    _ => unreachable!(),
                }
            }
            Either::Build(Some(built)) => match &mut self.acc {
                PartAccumulator::Builds(ref mut vec) => {
                    vec.push(built);
                }
                PartAccumulator::Seq(ref mut seq) => seq
                    .contents
                    .push(IR::Rendered(Some(CiteEdgeData::Output(built)))),
            },
            Either::Build(None) => return,
        }
    }

    pub fn into_either(self, fmt: &O) -> Either<O> {
        let PartBuilder { bits, acc } = self;
        match acc {
            PartAccumulator::Builds(each) => {
                if each.is_empty() {
                    return Either::Build(None);
                }
                let built = fmt.affixed(
                    fmt.group(each, &bits.overall_delimiter, bits.overall_formatting),
                    bits.overall_affixes,
                );
                Either::Build(Some(built))
            }
            PartAccumulator::Seq(seq) => Either::Ir(IR::Seq(seq)),
        }
    }
}

fn intermediate_generic_local<'c, O>(
    local: &LocalizedDate,
    _db: &impl IrDatabase,
    ctx: GenericContext<'c, O>,
) -> Option<Either<O>>
where
    O: OutputFormat,
{
    let fmt = ctx.format();
    let locale = ctx.locale();
    let refr = ctx.reference();
    // TODO: handle missing
    let locale_date: &LocaleDate = locale.dates.get(&local.form).unwrap();
    let gen_date = GenericDateBits {
        overall_delimiter: &locale_date.delimiter.0,
        overall_formatting: local.formatting,
        overall_affixes: &local.affixes,
    };
    // TODO: render date ranges
    // TODO: TextCase
    let date = refr
        .date
        .get(&local.variable)
        .and_then(|r| r.single_or_first());
    date.map(|val| {
        let len_hint = locale_date.date_parts.len();
        let rendered_parts = locale_date
            .date_parts
            .iter()
            .filter(|dp| dp_matches(dp, local.parts_selector))
            .filter_map(|dp| dp_render_either(dp, ctx.clone(), &val));
        let mut builder = PartBuilder::new(gen_date, len_hint);
        for (form, either) in rendered_parts {
            builder.push_either(either);
        }
        builder.into_either(fmt)
    })
}

fn intermediate_generic_indep<'c, O>(
    indep: &IndependentDate,
    _db: &impl IrDatabase,
    ctx: GenericContext<'c, O>,
) -> Option<Either<O>>
where
    O: OutputFormat,
{
    let fmt = ctx.format();
    let gen_date = GenericDateBits {
        overall_delimiter: &indep.delimiter.0,
        overall_formatting: indep.formatting,
        overall_affixes: &indep.affixes,
    };
    let date = ctx
        .reference()
        .date
        .get(&indep.variable)
        // TODO: render date ranges
        .and_then(|r| r.single_or_first());
    date.map(|val| {
        let len_hint = indep.date_parts.len();
        let each = indep
            .date_parts
            .iter()
            .filter_map(|dp| dp_render_either(dp, ctx.clone(), &val));
        let mut builder = PartBuilder::new(gen_date, len_hint);
        for (form, either) in each {
            builder.push_either(either);
        }
        builder.into_either(fmt)
    })
}

fn dp_matches(part: &DatePart, selector: DateParts) -> bool {
    match part.form {
        DatePartForm::Day(_) => selector == DateParts::YearMonthDay,
        DatePartForm::Month(..) => selector != DateParts::Year,
        DatePartForm::Year(_) => true,
    }
}

fn dp_render_either<'c, O: OutputFormat>(
    part: &DatePart,
    ctx: GenericContext<'c, O>,
    date: &Date,
) -> Option<(DatePartForm, Either<O>)> {
    let fmt = ctx.format();
    let string = dp_render_string(part, &ctx, date);
    string
        .map(|s| {
            if let DatePartForm::Year(_) = part.form {
                Either::Ir({
                    let year_part = IR::Rendered(Some(CiteEdgeData::Output(fmt.plain(&s))));
                    let mut contents = Vec::with_capacity(2);
                    contents.push(year_part);
                    if ctx.should_add_year_suffix_hook() {
                        let hook = IR::YearSuffix(YearSuffixHook::Plain, None);
                        contents.push(hook);
                    }
                    IR::Seq(IrSeq {
                        contents,
                        affixes: part.affixes.clone(),
                        formatting: part.formatting,
                        delimiter: Atom::from(""),
                    })
                })
            } else {
                Either::Build(Some(fmt.affixed_text(s, part.formatting, &part.affixes)))
            }
        })
        .map(|x| (part.form, x))
}

fn dp_render_string<'c, O: OutputFormat>(
    part: &DatePart,
    ctx: &GenericContext<'c, O>,
    date: &Date,
) -> Option<String> {
    let locale = ctx.locale();
    match part.form {
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
    }
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
