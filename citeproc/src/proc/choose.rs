use super::cite_context::*;
use super::helpers::sequence;
use super::ir::*;
use super::Proc;
use crate::output::OutputFormat;
use crate::style::element::{Choose, Condition, Conditions, Else, Formatting, IfThen, Match};

impl<'c, 'r: 'c, 'ci: 'c, O> Proc<'c, 'r, 'ci, O> for Choose
where
    O: OutputFormat,
{
    fn intermediate<'s: 'c>(&'s self, ctx: &CiteContext<'c, 'r, 'ci, O>) -> IR<'c, O>
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

struct BranchEval<'a, O: OutputFormat> {
    // the bools indicate if disambiguate was set
    disambiguate: bool,
    content: Option<IR<'a, O>>,
}

fn eval_ifthen<'c, 'r, 'ci, O>(
    branch: &'c IfThen,
    ctx: &CiteContext<'c, 'r, 'ci, O>,
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
fn eval_conditions<'c, 'r: 'c, 'ci, O>(
    conditions: &'c Conditions,
    ctx: &CiteContext<'c, 'r, 'ci, O>,
) -> (bool, bool)
where
    O: OutputFormat,
{
    let Conditions(ref match_type, ref conds) = *conditions;
    let mut tests = conds.iter().map(|c| eval_cond(c, ctx));
    let disambiguate = conds.iter().any(|c| c.disambiguate);

    (run_matcher(&mut tests, match_type), disambiguate)
}

fn eval_cond<'c, 'r: 'c, 'ci, O>(cond: &'c Condition, ctx: &CiteContext<'c, 'r, 'ci, O>) -> bool
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
