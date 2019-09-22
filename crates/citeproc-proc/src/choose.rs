// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::prelude::*;

use crate::helpers::sequence;
use citeproc_io::DateOrRange;
use csl::style::{
    Affixes, Choose, Cond, CondSet, Conditions, CslType, Element, Else, IfThen, Match, Position,
    Style,
};
use csl::variables::{AnyVariable, DateVariable};

use std::sync::Arc;

impl<'c, O> Proc<'c, O> for Arc<Choose>
where
    O: OutputFormat,
{
    fn intermediate(&self, state: &mut IrState, ctx: &CiteContext<'c, O>) -> IrSum<O>
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
            } = eval_ifthen(head, state, ctx);
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
                } = eval_ifthen(branch, state, ctx);
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
            let (content, gv) = sequence(state, ctx, &els, "".into(), None, Affixes::default());
            if disamb {
                (IR::ConditionalDisamb(self.clone(), Box::new(content)), gv)
            } else {
                (content, gv)
            }
        }
    }
}

impl Disambiguation<Markup> for Choose {
    fn ref_ir(
        &self,
        db: &impl IrDatabase,
        ctx: &RefContext<Markup>,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        let Choose(head, rest, last) = self;
        if let Some(els) = eval_ifthen_ref(head, ctx).0 {
            return ref_sequence(db, ctx, els, "".into(), Some(stack), Affixes::default());
        }
        for branch in rest {
            if let Some(els) = eval_ifthen_ref(branch, ctx).0 {
                return ref_sequence(db, ctx, els, "".into(), Some(stack), Affixes::default());
            }
        }
        return ref_sequence(db, ctx, &last.0, "".into(), Some(stack), Affixes::default());
    }

    fn get_free_conds(&self, db: &impl IrDatabase) -> FreeCondSets {
        use std::iter;
        let Choose(ifthen, elseifs, else_) = self;
        let IfThen(if_conditions, if_els) = ifthen;
        assert!(if_conditions.0 == Match::All);
        assert!(if_conditions.1.len() == 1);
        let if_els = cross_product(db, if_els);
        let ifthen = (&if_conditions.1[0], if_els);
        let first: Vec<_> = iter::once(ifthen)
            .chain(elseifs.iter().map(|fi: &IfThen| {
                let IfThen(if_conditions, if_els) = fi;
                assert!(if_conditions.0 == Match::All);
                assert!(if_conditions.1.len() == 1);
                let if_els = cross_product(db, if_els);
                (&if_conditions.1[0], if_els)
            }))
            .collect();
        FreeCondSets::all_branches(
            first.into_iter(),
            if else_.0.len() > 0 {
                Some(cross_product(db, &else_.0))
            } else {
                None
            },
        )
    }
}

struct BranchEval<O: OutputFormat> {
    // the bools indicate if disambiguate was set
    disambiguate: bool,
    content: Option<IrSum<O>>,
}

fn eval_ifthen<'c, O>(
    branch: &'c IfThen,
    state: &mut IrState,
    ctx: &CiteContext<'c, O>,
) -> BranchEval<O>
where
    O: OutputFormat,
{
    let IfThen(ref conditions, ref elements) = *branch;
    let (matched, disambiguate) = eval_conditions(conditions, ctx);
    let content = if matched {
        Some(sequence(
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

fn eval_ifthen_ref<'c, Ck>(branch: &'c IfThen, checker: &Ck) -> (Option<&'c [Element]>, bool)
where
    Ck: CondChecker,
{
    let IfThen(ref conditions, ref elements) = *branch;
    let (matched, disambiguate) = eval_conditions(conditions, checker);
    let content = if matched {
        Some(elements.as_slice())
    } else {
        None
    };
    (content, disambiguate)
}

fn run_matcher<I: Iterator<Item = bool>>(bools: &mut I, match_type: &Match) -> bool {
    match *match_type {
        Match::Any => bools.any(|b| b),
        Match::Nand => bools.any(|b| !b),
        Match::All => bools.all(|b| b),
        Match::None => bools.all(|b| !b),
    }
}

// first bool is the match result
// second bool is disambiguate=true
fn eval_conditions<'c, Ck>(conditions: &'c Conditions, checker: &Ck) -> (bool, bool)
where
    Ck: CondChecker,
{
    let Conditions(ref match_type, ref conditions) = *conditions;
    let mut tests = conditions.iter().map(|c| eval_condset(c, checker));
    let disambiguate = conditions.iter().any(|c| {
        c.conds.contains(&Cond::Disambiguate(true)) || c.conds.contains(&Cond::Disambiguate(false))
    }) && !checker.is_disambiguate();

    (run_matcher(&mut tests, match_type), disambiguate)
}

fn eval_condset<'c, Ck>(cond_set: &'c CondSet, checker: &Ck) -> bool
where
    Ck: CondChecker,
{
    let style = checker.style();

    let mut iter_all = cond_set.conds.iter().filter_map(|cond| {
        Some(match cond {
            Cond::Variable(var) => checker.has_variable(*var),
            Cond::IsNumeric(var) => checker.is_numeric(*var),
            Cond::Disambiguate(d) => *d == checker.is_disambiguate(),
            Cond::Type(typ) => checker.csl_type() == typ,
            Cond::Position(pos) => checker.position().matches(*pos),

            Cond::HasYearOnly(_) | Cond::HasMonthOrSeason(_) | Cond::HasDay(_)
                if !style.features.condition_date_parts =>
            {
                return None;
            }

            Cond::HasYearOnly(dvar) => checker.has_year_only(*dvar),
            Cond::HasMonthOrSeason(dvar) => checker.has_month_or_season(*dvar),
            Cond::HasDay(dvar) => checker.has_day(*dvar),
            _ => return None,
        })
    });

    run_matcher(&mut iter_all, &cond_set.match_type)
}

pub trait CondChecker {
    fn has_variable(&self, var: AnyVariable) -> bool;
    fn is_numeric(&self, var: AnyVariable) -> bool;
    fn is_disambiguate(&self) -> bool;
    fn csl_type(&self) -> &CslType;
    fn get_date(&self, dvar: DateVariable) -> Option<&DateOrRange>;
    fn position(&self) -> Position;
    fn style(&self) -> &Style;
    fn has_year_only(&self, dvar: DateVariable) -> bool {
        self.get_date(dvar)
            .map(|dor| match dor {
                DateOrRange::Single(d) => d.month == 0 && d.day == 0,
                DateOrRange::Range(d1, d2) => {
                    d1.month == 0 && d1.day == 0 && d2.month == 0 && d2.day == 0
                }
                _ => false,
            })
            .unwrap_or(false)
    }
    fn has_month_or_season(&self, dvar: DateVariable) -> bool {
        self.get_date(dvar)
            .map(|dor| match dor {
                DateOrRange::Single(d) => d.month != 0,
                DateOrRange::Range(d1, d2) => {
                    // XXX: is OR the right operator here?
                    d1.month != 0 || d2.month != 0
                }
                _ => false,
            })
            .unwrap_or(false)
    }
    fn has_day(&self, dvar: DateVariable) -> bool {
        self.get_date(dvar)
            .map(|dor| match dor {
                DateOrRange::Single(d) => d.day != 0,
                DateOrRange::Range(d1, d2) => {
                    // XXX: is OR the right operator here?
                    d1.day != 0 || d2.day != 0
                }
                _ => false,
            })
            .unwrap_or(false)
    }
    // TODO: is_uncertain_date ("ca. 2003"). CSL and CSL-JSON do not specify how this is meant to
    // work.
    // Actually, is_uncertain_date (+ circa) is is a CSL-JSON thing.
}
