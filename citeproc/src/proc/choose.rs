use super::cite_context::*;
use super::helpers::sequence;
use super::ir::*;
use super::Proc;
use crate::output::OutputFormat;
use crate::style::element::{Choose, Condition, Conditions, Else, Formatting, IfThen, Match};

impl<'c, 's: 'c> Proc<'c, 's> for Choose {
    #[cfg_attr(feature = "flame_it", flame("Choose"))]
    fn intermediate<'r, O>(&'s self, ctx: &CiteContext<'c, 'r, O>) -> IR<'c, O>
    where
        O: OutputFormat,
    {
        // TODO: work out if disambiguate appears on the conditions
        let Choose(ref head, ref rest, ref last) = *self;
        let mut disamb = false;
        let mut found;
        {
            let BranchEval {
                disambiguate,
                content,
            } = eval_ifthen(head, ctx);
            found = content;
            disamb = disamb || disambiguate;
        }
        if let Some(content) = found {
            return if disamb {
                IR::ConditionalDisamb(self, Box::new(content))
            } else {
                content
            };
        } else {
            let mut iter = rest.iter();
            while let Some(branch) = iter.next() {
                if found.is_some() {
                    break;
                }
                let BranchEval {
                    disambiguate,
                    content,
                } = eval_ifthen(branch, ctx);
                found = content;
                disamb = disamb || disambiguate;
            }
        }
        if let Some(content) = found {
            return if disamb {
                IR::ConditionalDisamb(self, Box::new(content))
            } else {
                content
            };
        } else {
            let Else(ref els) = last;
            sequence(ctx, &Formatting::default(), "", &els)
        }
    }
}

struct BranchEval<'s, O: OutputFormat> {
    // the bools indicate if disambiguate was set
    disambiguate: bool,
    content: Option<IR<'s, O>>,
}

#[cfg_attr(feature = "flame_it", flame)]
fn eval_ifthen<'c, 's: 'c, 'r, O>(
    branch: &'s IfThen,
    ctx: &CiteContext<'c, 'r, O>,
) -> BranchEval<'c, O>
where
    O: OutputFormat,
{
    let IfThen(ref conditions, ref elements) = *branch;
    let (matched, disambiguate) = eval_conditions(conditions, ctx);
    let content = match matched {
        false => None,
        true => Some(sequence(ctx, &Formatting::default(), "", &elements)),
    };
    BranchEval {
        disambiguate,
        content,
    }
}

// first bool is the match result
// second bool is disambiguate=true
#[cfg_attr(feature = "flame_it", flame)]
fn eval_conditions<'c, 's: 'c, 'r: 'c, O>(
    conditions: &'s Conditions,
    ctx: &CiteContext<'c, 'r, O>,
) -> (bool, bool)
where
    O: OutputFormat,
{
    let Conditions(ref match_type, ref conds) = *conditions;
    let tests: Vec<_> = conds.iter().map(|c| eval_cond(c, ctx)).collect();
    let disambiguate = conds.iter().any(|c| c.disambiguate);
    (run_matcher(&tests, match_type), disambiguate)
}

#[cfg_attr(feature = "flame_it", flame)]
fn eval_cond<'c, 's: 'c, 'r: 'c, O>(cond: &'s Condition, ctx: &CiteContext<'c, 'r, O>) -> bool
where
    O: OutputFormat,
{
    let mut tests = Vec::with_capacity(cond.variable.len() + cond.is_numeric.len() + cond.csl_type.len() + cond.position.len());
    for var in cond.variable.iter() {
        tests.push(ctx.has_variable(var));
    }
    for var in cond.is_numeric.iter() {
        tests.push(ctx.is_numeric(var));
    }
    for typ in cond.csl_type.iter() {
        tests.push(ctx.reference.csl_type == *typ);
    }
    for pos in cond.position.iter() {
        tests.push(ctx.position == *pos);
    }
    // TODO: is_uncertain_date ("ca. 2003"). CSL and CSL-JSON do not specify how this is meant to
    // work.

    run_matcher(&tests, &cond.match_type)
}

#[cfg_attr(feature = "flame_it", flame)]
fn run_matcher(bools: &[bool], match_type: &Match) -> bool {
    match *match_type {
        Match::Any => bools.iter().any(|b| *b),
        Match::Nand => bools.iter().any(|b| !*b),
        Match::All => bools.iter().all(|b| *b),
        Match::None => bools.iter().all(|b| !*b),
    }
}
