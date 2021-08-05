// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::prelude::*;

use crate::number::render_ordinal;
use citeproc_io::{lazy, Date, DateOrRange};
use csl::terms::*;
use csl::LocaleDate;
#[cfg(test)]
use csl::RangeDelimiter;
use csl::{
    BodyDate, DatePart, DatePartForm, DateParts, DateVariable, DayForm, IndependentDate, Locale,
    LocalizedDate, MonthForm, NumberVariable, SortKey, YearForm,
};
#[cfg(test)]
use pretty_assertions::assert_eq;
use std::fmt::Write;
use std::mem;

#[derive(Debug)]
enum Either<O: OutputFormat> {
    Build(Option<O::Build>),
    /// We will convert this to RefIR as necessary. It will only contain Outputs and
    /// YearSuffixHooks It will only contain Outputs and YearSuffixHooks.
    /// It will not be Rendered(None).
    Ir(NodeId),
}

impl<O: OutputFormat> Either<O> {
    fn into_cite_ir(self, var: DateVariable, arena: &mut IrArena<O>) -> NodeId {
        match self {
            Either::Build(opt) => {
                // Get CiteEdgeData::Accessed if it's DateVariable::Accessed
                // We guarantee below in dp_render_either that Accessed will not produce Either::Ir
                let mapper = CiteEdgeData::from_date_variable(var);
                let content = opt.map(mapper);
                let gv = GroupVars::rendered_if(content.is_some());
                arena.new_node((IR::Rendered(content), gv))
            }
            Either::Ir(node) => node,
        }
    }
}

fn to_ref_ir<F>(
    root: NodeId,
    arena: &IrArena<Markup>,
    stack: Formatting,
    ys_edge: EdgeData,
    to_edge: &F,
) -> (RefIR, GroupVars)
where
    F: Fn(Option<&CiteEdgeData<Markup>>, Formatting) -> Option<EdgeData>,
{
    struct Scope<'a, F>
    where
        F: Fn(Option<&CiteEdgeData<Markup>>, Formatting) -> Option<EdgeData>,
    {
        stack: Formatting,
        ys_edge: EdgeData,
        to_edge: &'a F,
        arena: &'a IrArena<Markup>,
    }
    let scope = Scope {
        stack,
        ys_edge,
        to_edge,
        arena,
    };
    fn walk<F>(node: NodeId, arena: &IrArena<Markup>, scope: &Scope<'_, F>) -> (RefIR, GroupVars)
    where
        F: Fn(Option<&CiteEdgeData<Markup>>, Formatting) -> Option<EdgeData>,
    {
        arena
            .get(node)
            .map(|n| n.get())
            .map(|n| match &n.0 {
                IR::Rendered(opt_build) => (
                    RefIR::Edge((scope.to_edge)(opt_build.as_ref(), scope.stack)),
                    GroupVars::Important,
                ),
                IR::YearSuffix(_ys) => (
                    RefIR::Edge(Some(scope.ys_edge.clone())),
                    GroupVars::Important,
                ),
                IR::Seq(ir_seq) => {
                    let contents: Vec<RefIR> = node
                        .children(scope.arena)
                        .map(|child_id| walk(child_id, arena, scope).0)
                        .collect();
                    let gv = GroupVars::rendered_if(!contents.is_empty());
                    let ref_seq = RefIrSeq {
                        contents,
                        formatting: ir_seq.formatting,
                        affixes: ir_seq.affixes.clone(),
                        delimiter: ir_seq.delimiter.clone(),
                        text_case: ir_seq.text_case,
                        ..Default::default()
                    };
                    (RefIR::Seq(ref_seq), gv)
                }
                _ => unreachable!(
                    "The date processing code only creates Rendered, YearSuffix and Seq."
                ),
            })
            .unwrap_or((RefIR::Edge(None), GroupVars::Missing))
    }
    walk(root, arena, &scope)
}

impl Either<Markup> {
    fn into_ref_ir(
        self,
        ctx: &RefContext<Markup>,
        arena: &mut IrArena<Markup>,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        let fmt = ctx.format;
        let to_edge =
            |opt_cite_edge: Option<&CiteEdgeData<Markup>>, stack: Formatting| -> Option<EdgeData> {
                opt_cite_edge.map(|cite_edge| cite_edge.to_edge_data(fmt, stack))
            };
        let ys_edge = EdgeData::YearSuffixPlain;
        match self {
            Either::Build(opt) => {
                let content = opt.map(CiteEdgeData::Output);
                let edge = to_edge(content.as_ref(), stack);
                let gv = GroupVars::rendered_if(edge.is_some());
                (RefIR::Edge(edge), gv)
            }
            Either::Ir(id) => {
                // If it's Ir we'll assume there is a year suffix hook in there -- so not
                // Rendered(None), at least.
                to_ref_ir(id, arena, stack, ys_edge, &to_edge)
            }
        }
    }
}

impl<'c, O, I> Proc<'c, O, I> for BodyDate
where
    O: OutputFormat,
    I: OutputFormat,
{
    fn intermediate(
        &self,
        _db: &dyn IrDatabase,
        _state: &mut IrState,
        ctx: &CiteContext<'c, O, I>,
        arena: &mut IrArena<O>,
    ) -> NodeId {
        let (either, var) = match self {
            BodyDate::Indep(idate) => (
                intermediate_generic_indep(idate, GenericContext::Cit(ctx), arena),
                idate.variable,
            ),
            BodyDate::Local(ldate) => (
                intermediate_generic_local(ldate, GenericContext::Cit(ctx), arena),
                ldate.variable,
            ),
        };
        either
            .map(|e| e.into_cite_ir(var, arena))
            .unwrap_or_else(|| arena.new_node((IR::Rendered(None), GroupVars::rendered_if(false))))
    }
}

impl Disambiguation<Markup> for BodyDate {
    fn ref_ir(
        &self,
        _db: &dyn IrDatabase,
        ctx: &RefContext<Markup>,
        _state: &mut IrState,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        let _fmt = ctx.format;
        let mut arena = IrArena::new();
        let (either, var) = match self {
            BodyDate::Indep(idate) => (
                intermediate_generic_indep::<Markup, Markup>(
                    idate,
                    GenericContext::Ref(ctx),
                    &mut arena,
                ),
                idate.variable,
            ),
            BodyDate::Local(ldate) => (
                intermediate_generic_local::<Markup, Markup>(
                    ldate,
                    GenericContext::Ref(ctx),
                    &mut arena,
                ),
                ldate.variable,
            ),
        };
        if var == DateVariable::Accessed {
            either.map(|_| (RefIR::Edge(Some(EdgeData::Accessed)), GroupVars::Important))
        } else {
            either.map(|e| e.into_ref_ir(ctx, &mut arena, stack))
        }
        .unwrap_or((RefIR::Edge(None), GroupVars::Missing))
    }
}

#[derive(Clone)]
struct GenericDateBits<'a> {
    overall_formatting: Option<Formatting>,
    overall_affixes: Option<Affixes>,
    overall_delimiter: SmartString,
    overall_text_case: TextCase,
    display: Option<DisplayMode>,
    sorting: bool,
    locale: &'a Locale,
}

struct PartBuilder<'a, O: OutputFormat> {
    bits: GenericDateBits<'a>,
    acc: PartAccumulator<O>,
}

enum PartAccumulator<O: OutputFormat> {
    Builds(Vec<O::Build>),
    Seq(NodeId),
}

impl<'a, O: OutputFormat> PartBuilder<'a, O> {
    fn new(bits: GenericDateBits<'a>, len_hint: usize) -> Self {
        PartBuilder {
            bits,
            acc: PartAccumulator::Builds(Vec::with_capacity(len_hint)),
        }
    }

    fn upgrade(&mut self, arena: &mut IrArena<O>) {
        let PartBuilder {
            ref mut acc,
            ref mut bits,
        } = self;
        *acc = match acc {
            PartAccumulator::Builds(ref mut vec) => {
                let vec = mem::replace(vec, Vec::new());
                let seq_node = arena.new_node((
                    IR::Seq(IrSeq {
                        formatting: bits.overall_formatting,
                        affixes: bits.overall_affixes.clone(),
                        text_case: bits.overall_text_case,
                        display: bits.display,
                        ..Default::default()
                    }),
                    GroupVars::Important,
                ));
                for built in vec {
                    let node = arena.new_node((
                        IR::Rendered(Some(CiteEdgeData::Output(built))),
                        GroupVars::Important,
                    ));
                    seq_node.append(node, arena);
                }
                PartAccumulator::Seq(seq_node)
            }
            _ => return,
        }
    }

    fn push_either(&mut self, arena: &mut IrArena<O>, either: Either<O>) {
        match either {
            Either::Ir(ir) => {
                self.upgrade(arena);
                match &mut self.acc {
                    PartAccumulator::Seq(ref mut seq) => {
                        seq.append(ir, arena);
                    }
                    _ => unreachable!(),
                }
            }
            Either::Build(Some(built)) => match &mut self.acc {
                PartAccumulator::Builds(ref mut vec) => {
                    vec.push(built);
                }
                PartAccumulator::Seq(seq_node) => seq_node.append(
                    arena.new_node((
                        IR::Rendered(Some(CiteEdgeData::Output(built))),
                        GroupVars::Important,
                    )),
                    arena,
                ),
            },
            Either::Build(None) => {}
        }
    }

    pub fn into_either(self, fmt: &O) -> Either<O> {
        let PartBuilder { bits, acc } = self;
        match acc {
            PartAccumulator::Builds(each) => {
                if each.is_empty() {
                    return Either::Build(None);
                }
                let mut built = fmt.affixed(
                    fmt.group(each, "", bits.overall_formatting),
                    bits.overall_affixes.as_ref(),
                );
                let options = IngestOptions {
                    text_case: bits.overall_text_case,
                    ..Default::default()
                };
                if bits.overall_text_case != TextCase::None {
                    fmt.apply_text_case(&mut built, &options);
                }
                Either::Build(Some(built))
            }
            PartAccumulator::Seq(seq) => Either::Ir(seq),
        }
    }
}

impl<'a> GenericDateBits<'a> {
    fn sorting(locale: &'a Locale) -> Self {
        GenericDateBits {
            overall_delimiter: "".into(),
            overall_formatting: None,
            overall_affixes: Some(crate::sort::natural_sort::date_affixes()),
            overall_text_case: TextCase::None,
            display: None,
            sorting: true,
            locale,
        }
    }
}

fn intermediate_generic_local<'c, O, I>(
    local: &LocalizedDate,
    ctx: GenericContext<'c, O, I>,
    arena: &mut IrArena<O>,
) -> Option<Either<O>>
where
    O: OutputFormat,
    I: OutputFormat,
{
    let locale = ctx.locale();
    // TODO: handle missing
    let locale_date: &LocaleDate = locale.dates.get(&local.form).unwrap();
    let gen_date = if ctx.sort_key().is_some() {
        GenericDateBits::sorting(locale)
    } else {
        GenericDateBits {
            overall_delimiter: locale_date
                .delimiter
                .clone()
                .unwrap_or_else(Default::default),
            overall_formatting: local.formatting,
            overall_affixes: local.affixes.clone(),
            overall_text_case: local.text_case,
            display: if ctx.in_bibliography() {
                local.display
            } else {
                None
            },
            sorting: false,
            locale,
        }
    };
    let mut parts = Vec::with_capacity(locale_date.date_parts.len());
    for part in &locale_date.date_parts {
        let form = WhichDelim::from_form(&part.form);
        if let Some(localized) = local.date_parts.iter().find(|p| form.matches_form(&p.form)) {
            let merged = DatePart {
                form: localized.form,
                // Attributes for affixes are allowed, unless cs:date calls a localized date format.
                // So localized.affixes should be ignored.
                affixes: part.affixes.clone(),
                formatting: localized.formatting.or(part.formatting),
                text_case: localized.text_case.or(part.text_case),
                range_delimiter: localized.range_delimiter.clone(),
            };
            parts.push(merged);
        } else {
            parts.push(part.clone());
        }
    }
    if gen_date.sorting {
        parts.sort_by_key(|part| part.form)
    }
    build_parts(
        &ctx,
        arena,
        local.variable,
        gen_date,
        &parts,
        Some(local.parts_selector),
    )
}

fn intermediate_generic_indep<'c, O, I>(
    indep: &IndependentDate,
    ctx: GenericContext<'c, O, I>,
    arena: &mut IrArena<O>,
) -> Option<Either<O>>
where
    O: OutputFormat,
    I: OutputFormat,
{
    let locale = ctx.locale();
    let gen_date = if ctx.sort_key().is_some() {
        GenericDateBits::sorting(locale)
    } else {
        GenericDateBits {
            overall_delimiter: indep.delimiter.clone().unwrap_or_else(Default::default),
            overall_formatting: indep.formatting,
            overall_affixes: indep.affixes.clone(),
            overall_text_case: indep.text_case,
            display: if ctx.in_bibliography() {
                indep.display
            } else {
                None
            },
            sorting: false,
            locale,
        }
    };
    let mut parts_slice = indep.date_parts.as_slice();
    let mut parts;
    if gen_date.sorting {
        parts = indep.date_parts.clone();
        // The parts are filtered, but we're not going to be able to parse out the year if they are
        // not in order YYYY-MM-DD.
        parts.sort_by_key(|part| part.form);
        parts_slice = parts.as_slice();
    }
    build_parts(&ctx, arena, indep.variable, gen_date, parts_slice, None)
}

fn build_parts<'c, O: OutputFormat, I: OutputFormat>(
    ctx: &GenericContext<'c, O, I>,
    arena: &mut IrArena<O>,
    var: DateVariable,
    gen_date: GenericDateBits,
    parts: &[DatePart],
    selector: Option<DateParts>,
) -> Option<Either<O>> {
    // TODO: text-case
    let fmt = ctx.format();
    let len_hint = parts.len();
    let mut val = ctx.reference().date.get(&var)?.clone();
    let sorting = gen_date.sorting;
    if gen_date.sorting {
        // force range with zeroes on the end date if single
        val = match val {
            DateOrRange::Single(single) => DateOrRange::Range(single, Date::new(0, 0, 0)),
            _ => val,
        };
    }
    let do_single =
        |builder: &mut PartBuilder<O>, single: &Date, delim: &str, arena: &mut IrArena<O>| {
            let mut seen_one = false;
            for dp in parts.iter() {
                if let Some((_form, either)) = {
                    let matches = selector.map_or(true, |sel| dp_matches(dp, sel));
                    if sorting || matches {
                        let is_filtered =
                            !matches && ctx.sort_key().map_or(false, |k| k.is_macro());
                        dp_render_either(var, dp, ctx.clone(), arena, single, false, is_filtered)
                    } else {
                        None
                    }
                } {
                    if seen_one && !delim.is_empty() {
                        builder.push_either(arena, Either::Build(Some(fmt.plain(delim))))
                    }
                    seen_one = true;
                    builder.push_either(arena, either);
                }
            }
        };
    match &val {
        DateOrRange::Single(single) => {
            let delim = gen_date.overall_delimiter.clone();
            let mut builder = PartBuilder::new(gen_date, len_hint);
            do_single(&mut builder, single, &delim, arena);
            Some(builder.into_either(fmt))
        }
        DateOrRange::Range(first, second) => {
            let sorting = gen_date.sorting;
            let delim = gen_date.overall_delimiter.clone();
            if sorting {
                let mut builder = PartBuilder::new(gen_date, len_hint);
                do_single(&mut builder, first, &delim, arena);
                builder.push_either(arena, Either::Build(Some(fmt.plain("/"))));
                do_single(&mut builder, second, &delim, arena);
                return Some(builder.into_either(fmt));
            }
            let tokens = DateRangePartsIter::new(gen_date.sorting, parts, selector, first, second);
            let mut builder = PartBuilder::new(gen_date, len_hint);
            let mut seen_one = false;
            let mut last_rdel = false;
            for token in tokens {
                match token {
                    DateToken::RangeDelim(mut range_delim) => {
                        if sorting {
                            range_delim = "/";
                        }
                        builder.push_either(arena, Either::Build(Some(fmt.plain(range_delim))));
                        last_rdel = true;
                    }
                    DateToken::Part(date, part, is_max_diff) => {
                        if !last_rdel && seen_one && !delim.is_empty() {
                            builder.push_either(arena, Either::Build(Some(fmt.plain(&delim))))
                        }
                        last_rdel = false;
                        if let Some((_form, either)) = dp_render_either(
                            var,
                            part,
                            ctx.clone(),
                            arena,
                            date,
                            is_max_diff,
                            false,
                        ) {
                            builder.push_either(arena, either);
                        }
                    }
                }
                seen_one = true;
            }
            Some(builder.into_either(fmt))
        }
        DateOrRange::Literal { literal, circa: _ } => {
            let options = IngestOptions {
                text_case: gen_date.overall_text_case,
                ..Default::default()
            };
            let b = fmt.ingest(&literal, &options);
            let b = fmt.with_format(b, gen_date.overall_formatting);
            let b = fmt.affixed(b, gen_date.overall_affixes.as_ref());
            Some(Either::Build(Some(b)))
        }
    }
}

type IsMaxDiff = bool;

#[derive(Debug, Clone, Copy, PartialEq)]
enum DateToken<'a> {
    Part(&'a Date, &'a DatePart, IsMaxDiff),
    RangeDelim(&'a str),
}

struct DateRangePartsIter<'a> {
    tokens: std::vec::IntoIter<DateToken<'a>>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[repr(u8)]
enum WhichDelim {
    None = 0,
    Day = 1,
    Month = 2,
    Year = 3,
}
impl WhichDelim {
    fn matches_form(&self, form: &DatePartForm) -> bool {
        *self == WhichDelim::from_form(form)
    }
    fn from_form(form: &DatePartForm) -> Self {
        match form {
            DatePartForm::Day(_) => WhichDelim::Day,
            DatePartForm::Month(..) => WhichDelim::Month,
            DatePartForm::Year(_) => WhichDelim::Year,
        }
    }
    fn diff(parts: &[DatePart], first: &Date, second: &Date) -> Self {
        // Find the biggest differing date part
        let mut max_diff = WhichDelim::None;
        for part in parts {
            use std::cmp::max;
            match part.form {
                DatePartForm::Day(_) if first.day != second.day => {
                    max_diff = max(max_diff, WhichDelim::Day)
                }
                DatePartForm::Month(..) if first.month != second.month => {
                    max_diff = max(max_diff, WhichDelim::Month)
                }
                DatePartForm::Year(_) if first.year != second.year => {
                    max_diff = max(max_diff, WhichDelim::Year)
                }
                _ => {}
            }
        }
        max_diff
    }
}

impl<'a> DateRangePartsIter<'a> {
    fn new(
        sorting: bool,
        parts: &'a [DatePart],
        selector: Option<DateParts>,
        first: &'a Date,
        second: &'a Date,
    ) -> Self {
        let mut vec = Vec::with_capacity(parts.len() + 2);

        let max_diff = WhichDelim::diff(parts, first, second);
        let matches = |part: &DatePart| {
            if let Some(selector) = selector {
                // Don't filter out if we're sorting -- just render zeroes later
                sorting || dp_matches(part, selector)
            } else {
                true
            }
        };
        for part in parts {
            let is_max_diff = max_diff.matches_form(&part.form);
            if matches(part) {
                vec.push(DateToken::Part(first, part, is_max_diff));
            }
            if is_max_diff {
                let delim = part
                    .range_delimiter
                    .as_ref()
                    .map(|rd| rd.0.as_ref())
                    .unwrap_or("\u{2013}");
                vec.push(DateToken::RangeDelim(delim));
                for p in parts {
                    if matches(p) && WhichDelim::from_form(&p.form) <= max_diff {
                        vec.push(DateToken::Part(second, p, false));
                    }
                }
            }
        }

        DateRangePartsIter {
            tokens: vec.into_iter(),
        }
    }
}

impl<'a> Iterator for DateRangePartsIter<'a> {
    type Item = DateToken<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        self.tokens.next()
    }
}

#[test]
fn test_range_dp_sequence() {
    let parts = vec![
        DatePart {
            form: DatePartForm::Day(DayForm::Numeric),
            range_delimiter: Some(RangeDelimiter("..".into())),
            ..Default::default()
        },
        DatePart {
            form: DatePartForm::Month(MonthForm::Numeric, false),
            range_delimiter: Some(RangeDelimiter("-".into())),
            ..Default::default()
        },
        DatePart {
            form: DatePartForm::Year(YearForm::Long),
            range_delimiter: Some(RangeDelimiter(" to ".into())),
            ..Default::default()
        },
    ];

    let day = &parts[0];
    let month = &parts[1];
    let year = &parts[2];

    let first = Date::new(1998, 3, 27);
    let second = Date::new(1998, 3, 29);
    let iter = DateRangePartsIter::new(false, &parts, None, &first, &second);
    assert_eq!(
        iter.collect::<Vec<_>>(),
        vec![
            DateToken::Part(&first, day, true),
            DateToken::RangeDelim(".."),
            DateToken::Part(&second, day, false),
            DateToken::Part(&first, month, false),
            DateToken::Part(&first, year, false),
        ]
    );

    let first = Date::new(1998, 3, 27);
    let second = Date::new(1998, 4, 29);
    let iter = DateRangePartsIter::new(false, &parts, None, &first, &second);
    assert_eq!(
        iter.collect::<Vec<_>>(),
        vec![
            DateToken::Part(&first, day, false),
            DateToken::Part(&first, month, true),
            DateToken::RangeDelim("-"),
            DateToken::Part(&second, day, false),
            DateToken::Part(&second, month, false),
            DateToken::Part(&first, &parts[2], false),
        ]
    );
}

fn dp_matches(part: &DatePart, selector: DateParts) -> bool {
    match part.form {
        DatePartForm::Day(_) => selector == DateParts::YearMonthDay,
        DatePartForm::Month(..) => selector != DateParts::Year,
        DatePartForm::Year(_) => true,
    }
}

fn dp_render_either<'c, O: OutputFormat, I: OutputFormat>(
    var: DateVariable,
    part: &DatePart,
    ctx: GenericContext<'c, O, I>,
    arena: &mut IrArena<O>,
    date: &Date,
    is_max_diff: bool,
    is_filtered: bool,
) -> Option<(DatePartForm, Either<O>)> {
    let fmt = ctx.format();
    if let Some(key) = ctx.sort_key() {
        let string = dp_render_sort_string(part, date, key, is_filtered);
        return string.map(|s| (part.form, Either::Build(Some(fmt.text_node(s, None)))));
    }
    let string = dp_render_string(part, &ctx, date);
    string
        .map(|s| {
            let mut affixes = part.affixes.clone();
            if is_max_diff {
                if let Some(ref mut aff) = affixes {
                    aff.suffix = "".into();
                }
            }
            if let DatePartForm::Year(_) = part.form {
                if var == DateVariable::Accessed {
                    let b = fmt.affixed_text(s, part.formatting, affixes.as_ref());
                    Either::Build(Some(b))
                } else {
                    let b = fmt.plain(&s);
                    let seq = arena.new_node((
                        IR::Seq(IrSeq {
                            affixes,
                            formatting: part.formatting,
                            ..Default::default()
                        }),
                        GroupVars::Important,
                    ));
                    let year_part = IR::Rendered(Some(CiteEdgeData::Year(b)));
                    // Important because we got it from a date variable.
                    let year_node = arena.new_node((year_part, GroupVars::Important));
                    seq.append(year_node, arena);
                    // Why not move this if branch up and emit Either::Build?
                    //
                    // We don't emit Either::Build for normal date vars with
                    // ctx.should_add_year_suffix_hook, because otherwise there is a mismatch
                    // between the edges produced by {cite with year-suffix not filled} and RefIR,
                    // specifically when affixes are nonzero. Like: ["(", "1986", ")"] vs
                    // ["(1986)"]
                    if ctx.should_add_year_suffix_hook() {
                        let suffix = arena.new_node(IR::year_suffix(YearSuffixHook::Plain));
                        seq.append(suffix, arena);
                    }
                    Either::Ir(seq)
                }
            } else {
                let options = IngestOptions {
                    text_case: part.text_case.unwrap_or_default(),
                    ..Default::default()
                };
                let b = fmt.ingest(&s, &options);
                let b = fmt.with_format(b, part.formatting);
                let b = fmt.affixed(b, affixes.as_ref());
                Either::Build(Some(b))
            }
        })
        .map(|x| (part.form, x))
}

fn dp_render_sort_string(
    part: &DatePart,
    date: &Date,
    _key: &SortKey,
    is_filtered: bool,
) -> Option<SmartString> {
    match part.form {
        DatePartForm::Year(_) => Some(smart_format!("{:04}_", date.year)),
        DatePartForm::Month(..) => {
            if is_filtered {
                return None;
            }
            // Sort strings do not compare seasons
            if date.month > 0 && date.month <= 12 {
                Some(smart_format!("{:02}", date.month))
            } else {
                Some("00".into())
            }
        }
        DatePartForm::Day(_) => {
            if is_filtered {
                return None;
            }
            if date.day > 0 {
                Some(smart_format!("{:02}", date.day))
            } else {
                Some("00".into())
            }
        }
    }
}

fn render_year(year: i32, form: YearForm, locale: &Locale) -> SmartString {
    let mut s = SmartString::new();
    if year == 0 {
        // Open year range
        return s;
    }
    // Only do short form ('07) for four-digit years
    match (form, year > 1000) {
        (YearForm::Short, true) => write!(s, "{:02}", year.abs() % 100).unwrap(),
        (YearForm::Long, _) | (YearForm::Short, false) => write!(s, "{}", year.abs()).unwrap(),
    }
    if year < 0 {
        let sel = SimpleTermSelector::Misc(MiscTerm::Bc, TermFormExtended::Long);
        let sel = TextTermSelector::Simple(sel);
        if let Some(bc) = locale.get_text_term(sel, false) {
            s.push_str(bc);
        } else {
            s.push_str("BC");
        }
    } else if year < 1000 {
        let sel = SimpleTermSelector::Misc(MiscTerm::Ad, TermFormExtended::Long);
        let sel = TextTermSelector::Simple(sel);
        if let Some(ad) = locale.get_text_term(sel, false) {
            s.push_str(ad);
        } else {
            s.push_str("AD");
        }
    }
    s
}

fn dp_render_string<'c, O: OutputFormat, I: OutputFormat>(
    part: &DatePart,
    ctx: &GenericContext<'c, O, I>,
    date: &Date,
) -> Option<SmartString> {
    let locale = ctx.locale();
    match part.form {
        DatePartForm::Year(form) => Some(render_year(date.year, form, ctx.locale())),
        DatePartForm::Month(form, strip_periods) => match form {
            MonthForm::Numeric => {
                if date.month == 0 || date.month > 12 {
                    None
                } else {
                    Some(smart_format!("{}", date.month))
                }
            }
            MonthForm::NumericLeadingZeros => {
                if date.month == 0 || date.month > 12 {
                    None
                } else {
                    Some(smart_format!("{:02}", date.month))
                }
            }
            _ => {
                let sel = GenderedTermSelector::from_month_u32(date.month, form)?;
                let string: SmartString = locale
                    .gendered_terms
                    .get(&sel)
                    .map(|gt| gt.0.singular().into())
                    .unwrap_or_else(|| {
                        let fallback = if form == MonthForm::Short {
                            MONTHS_SHORT
                        } else {
                            MONTHS_LONG
                        };
                        fallback[date.month as usize].into()
                    });
                Some(if strip_periods {
                    lazy::lazy_replace_char_owned(string, '.', "")
                } else {
                    string
                })
            }
        },
        DatePartForm::Day(form) => match form {
            _ if date.day == 0 => None,
            DayForm::NumericLeadingZeros => Some(smart_format!("{:02}", date.day)),
            DayForm::Ordinal
                if !locale
                    .options_node
                    .limit_day_ordinals_to_day_1
                    .unwrap_or(false)
                    || date.day == 1 =>
            {
                use citeproc_io::NumericToken;
                // The 'target noun' is the month term.
                MonthTerm::from_u32(date.month)
                    .map(|month| locale.get_month_gender(month))
                    .map(|gender| {
                        // the specific number variable does not matter as the tokens do not
                        // contain any hyphens to pick \u{2013} for
                        render_ordinal(
                            &[NumericToken::Num(date.day)],
                            locale,
                            NumberVariable::Number,
                            None,
                            gender,
                            false,
                        )
                    })
            }
            // Numeric or ordinal with limit-day-ordinals-to-day-1
            _ => Some(smart_format!("{}", date.day)),
        },
    }
}

// Some fallbacks so we don't have to panic so much if en-US is absent.

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
    "Spring",
    "Summer",
    "Autumn",
    "Winter",
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
    "Spring",
    "Summer",
    "Autumn",
    "Winter",
];
