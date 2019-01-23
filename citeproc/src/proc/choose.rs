use super::cite_context::*;
use super::helpers::sequence;
use super::ir::*;
use super::Proc;
use crate::output::OutputFormat;
use crate::style::element::{Affixes, Choose, Condition, Conditions, Else, IfThen, Match};

impl<'c, O> Proc<'c, O> for Choose
where
    O: OutputFormat,
{
    fn intermediate<'s: 'c>(&'s self, ctx: &CiteContext<'c, O>) -> IrSum<'c, O>
    where
        O: OutputFormat,
    {
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
        if let Some((content, gv)) = found {
            return if disamb {
                (IR::ConditionalDisamb(self, Box::new(content)), gv)
            } else {
                (content, gv)
            };
        } else {
            for branch in rest.iter() {
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
        if let Some((content, gv)) = found {
            return if disamb {
                (IR::ConditionalDisamb(self, Box::new(content)), gv)
            } else {
                (content, gv)
            };
        } else {
            let Else(ref els) = last;
            sequence(ctx, &els, "", None, Affixes::default())
        }
    }
}

struct BranchEval<'a, O: OutputFormat> {
    // the bools indicate if disambiguate was set
    disambiguate: bool,
    content: Option<IrSum<'a, O>>,
}

fn eval_ifthen<'c, O>(branch: &'c IfThen, ctx: &CiteContext<'c, O>) -> BranchEval<'c, O>
where
    O: OutputFormat,
{
    let IfThen(ref conditions, ref elements) = *branch;
    let (matched, disambiguate) = eval_conditions(conditions, ctx);
    let content = match matched {
        false => None,
        true => Some(sequence(ctx, &elements, "", None, Affixes::default())),
    };
    BranchEval {
        disambiguate,
        content,
    }
}

// first bool is the match result
// second bool is disambiguate=true
fn eval_conditions<'c, O>(conditions: &'c Conditions, ctx: &CiteContext<'c, O>) -> (bool, bool)
where
    O: OutputFormat,
{
    let Conditions(ref match_type, ref conds) = *conditions;
    let mut tests = conds.iter().map(|c| eval_cond(c, ctx));
    let disambiguate = conds.iter().any(|c| c.disambiguate);

    (run_matcher(&mut tests, match_type), disambiguate)
}

fn eval_cond<'c, O>(cond: &'c Condition, ctx: &CiteContext<'c, O>) -> bool
where
    O: OutputFormat,
{
    let vars = cond.variable.iter().map(|var| ctx.has_variable(var));

    let nums = cond.is_numeric.iter().map(|var| ctx.is_numeric(var));

    let types = cond
        .csl_type
        .iter()
        .map(|typ| ctx.reference.csl_type == *typ);

    let positions = cond.position.iter().map(|pos| ctx.position == *pos);

    // TODO: is_uncertain_date ("ca. 2003"). CSL and CSL-JSON do not specify how this is meant to
    // work.
    // Actually, is_uncertain_date (+ circa) is is a CSL-JSON thing.

    let mut chain = vars.chain(nums).chain(types).chain(positions);

    run_matcher(&mut chain, &cond.match_type)
}

fn run_matcher<I: Iterator<Item = bool>>(bools: &mut I, match_type: &Match) -> bool {
    match *match_type {
        Match::Any => bools.any(|b| b),
        Match::Nand => bools.any(|b| !b),
        Match::All => bools.all(|b| b),
        Match::None => bools.all(|b| !b),
    }
}
