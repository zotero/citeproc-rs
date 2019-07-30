// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::helpers::sequence;
use super::ir::*;
use super::ProcDatabase;
use super::{CiteContext, IrState, Proc};
use citeproc_io::DateOrRange;
use citeproc_io::output::OutputFormat;
use csl::style::{Affixes, Choose, Condition, Conditions, Else, IfThen, Match};
use std::sync::Arc;

impl<'c, O> Proc<'c, O> for Arc<Choose>
where
    O: OutputFormat,
{
    fn intermediate(
        &self,
        db: &impl ProcDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O>,
    ) -> IrSum<O>
    where
        O: OutputFormat,
    {
        // XXX: should you treat conditional evaluations as a "variable test"?
        let Choose(ref head, ref rest, ref last) = **self;
        let mut disamb = false;
        let mut found;
        {
            let BranchEval {
                disambiguate,
                content,
            } = eval_ifthen(head, db, state, ctx);
            found = content;
            disamb = disamb || disambiguate;
        }
        // check the <if> element
        if let Some((content, gv)) = found {
            return if disamb {
                (IR::ConditionalDisamb(self.clone(), Box::new(content)), gv)
            } else {
                (content, gv)
            };
        } else {
            // check the <else-if> elements
            for branch in rest.iter() {
                if found.is_some() {
                    break;
                }
                let BranchEval {
                    disambiguate,
                    content,
                } = eval_ifthen(branch, db, state, ctx);
                found = content;
                disamb = disamb || disambiguate;
            }
        }
        // did any of the <else-if> elements match?
        if let Some((content, gv)) = found {
            if disamb {
                (IR::ConditionalDisamb(self.clone(), Box::new(content)), gv)
            } else {
                (content, gv)
            }
        } else {
            // if not, <else>
            let Else(ref els) = last;
            let (content, gv) = sequence(db, state, ctx, &els, "".into(), None, Affixes::default());
            if disamb {
                (IR::ConditionalDisamb(self.clone(), Box::new(content)), gv)
            } else {
                (content, gv)
            }
        }
    }
}

struct BranchEval<O: OutputFormat> {
    // the bools indicate if disambiguate was set
    disambiguate: bool,
    content: Option<IrSum<O>>,
}

fn eval_ifthen<'c, O>(
    branch: &'c IfThen,
    db: &impl ProcDatabase,
    state: &mut IrState,
    ctx: &CiteContext<'c, O>,
) -> BranchEval<O>
where
    O: OutputFormat,
{
    let IfThen(ref conditions, ref elements) = *branch;
    let (matched, disambiguate) = eval_conditions(conditions, ctx, db);
    let content = if matched {
        Some(sequence(
            db,
            state,
            ctx,
            &elements,
            "".into(),
            None,
            Affixes::default(),
        ))
    } else {
        None
    };
    BranchEval {
        disambiguate,
        content,
    }
}

// first bool is the match result
// second bool is disambiguate=true
fn eval_conditions<'c, O>(
    conditions: &'c Conditions,
    ctx: &CiteContext<'c, O>,
    db: &impl ProcDatabase,
) -> (bool, bool)
where
    O: OutputFormat,
{
    let Conditions(ref match_type, ref conds) = *conditions;
    let mut tests = conds.iter().map(|c| eval_cond(c, ctx, db));
    let disambiguate = conds.iter().any(|c| c.disambiguate.is_some())
        && ctx.disamb_pass != Some(DisambPass::Conditionals);

    (run_matcher(&mut tests, match_type), disambiguate)
}

fn eval_cond<'c, O>(cond: &'c Condition, ctx: &CiteContext<'c, O>, db: &impl ProcDatabase) -> bool
where
    O: OutputFormat,
{
    let vars = cond.variable.iter().map(|&var| ctx.has_variable(var, db));

    let nums = cond.is_numeric.iter().map(|&var| ctx.is_numeric(var, db));

    let disambiguate = cond
        .disambiguate
        .iter()
        .map(|&d| d == (ctx.disamb_pass == Some(DisambPass::Conditionals)));

    let types = cond
        .csl_type
        .iter()
        .map(|typ| ctx.reference.csl_type == *typ);

    let positions = cond
        .position
        .iter()
        .map(|&pos| db.cite_pos(ctx.cite.id).matches(pos));

    // TODO: is_uncertain_date ("ca. 2003"). CSL and CSL-JSON do not specify how this is meant to
    // work.
    // Actually, is_uncertain_date (+ circa) is is a CSL-JSON thing.

    let has_year_only = cond.has_year_only.iter().map(|dvar| {
        ctx.reference
            .date
            .get(&dvar)
            .map(|dor| match dor {
                DateOrRange::Single(d) => d.month == 0 && d.day == 0,
                DateOrRange::Range(d1, d2) => {
                    d1.month == 0 && d1.day == 0 && d2.month == 0 && d2.day == 0
                }
                _ => false,
            })
            .unwrap_or(false)
    });

    let has_month_or_season = cond.has_month_or_season.iter().map(|dvar| {
        ctx.reference
            .date
            .get(&dvar)
            .map(|dor| match dor {
                DateOrRange::Single(d) => d.month != 0,
                DateOrRange::Range(d1, d2) => {
                    // XXX: is OR the right operator here?
                    d1.month != 0 || d2.month != 0
                }
                _ => false,
            })
            .unwrap_or(false)
    });

    let has_day = cond.has_day.iter().map(|dvar| {
        ctx.reference
            .date
            .get(&dvar)
            .map(|dor| match dor {
                DateOrRange::Single(d) => d.day != 0,
                DateOrRange::Range(d1, d2) => {
                    // XXX: is OR the right operator here?
                    d1.day != 0 || d2.day != 0
                }
                _ => false,
            })
            .unwrap_or(false)
    });

    let basic = vars
        .chain(nums)
        .chain(types)
        .chain(positions)
        .chain(disambiguate);

    let mut date_parts = None;

    let style = db.style_el();
    if style.features.condition_date_parts {
        date_parts = Some(has_year_only.chain(has_month_or_season).chain(has_day));
    }

    // If a condition matcher is enabled, flattening the option::Iter pulls out the internal iterator if any
    let mut bools = basic.chain(date_parts.into_iter().flatten());

    run_matcher(&mut bools, &cond.match_type)
}

fn run_matcher<I: Iterator<Item = bool>>(bools: &mut I, match_type: &Match) -> bool {
    match *match_type {
        Match::Any => bools.any(|b| b),
        Match::Nand => bools.any(|b| !b),
        Match::All => bools.all(|b| b),
        Match::None => bools.all(|b| !b),
    }
}
