use super::helpers::sequence;
use super::ir::*;
use super::Proc;
use crate::input::Reference;
use crate::output::OutputFormat;
use crate::style::element::{Delimiter, Choose, Formatting, IfThen, Else, Conditions, Condition, Match};

impl<'s> Proc<'s> for Choose {
    #[cfg_attr(feature = "flame_it", flame)]
    fn intermediate<'r, O>(&'s self, fmt: &O, refr: &Reference<'r>) -> IR<'s, O>
    where
        O: OutputFormat,
    {
        // TODO: work out if disambiguate appears on the conditions
        let Choose(ref head, ref rest, ref last) = *self;
        let mut disamb = false;
        let mut found;
        {
            let BranchEval { disambiguate, content } = eval_ifthen(head, fmt, refr);
            found = content;
            disamb = disamb || disambiguate;
        }
        if let Some(content) = found {
            return if disamb {
                IR::ConditionalDisamb(self, Box::new(content))
            } else {
                content
            }
        } else {
            let mut iter = rest.iter();
            while let Some(branch) = iter.next() {
                if found.is_some() { break; }
                let BranchEval { disambiguate, content } = eval_ifthen(branch, fmt, refr);
                found = content;
                disamb = disamb || disambiguate;
            }
        }
        if let Some(content) = found {
            return if disamb {
                IR::ConditionalDisamb(self, Box::new(content))
            } else {
                content
            }
        } else {
            let Else(ref els) = last;
            sequence(fmt, refr, &Formatting::default(), &Delimiter("".into()), &els)
        }
    }
}

struct BranchEval<'s, O: OutputFormat> {
    // the bools indicate if disambiguate was set
    disambiguate: bool,
    content: Option<IR<'s, O>>,
}

fn eval_ifthen<'s, 'r, O>(
    branch: &'s IfThen,
    fmt: &O,
    refr: &Reference<'r>,
) -> BranchEval<'s, O>
where
    O: OutputFormat,
{
    let IfThen(ref conditions, ref elements) = *branch;
    let (matched, disambiguate) = eval_conditions(conditions, refr);
    let content = match matched {
        false => None,
        true  => Some(sequence(fmt, refr, &Formatting::default(), &Delimiter("".into()), &elements))
    };
    BranchEval {
        disambiguate,
        content,
    }
}

// first bool is the match result
// second bool is disambiguate=true
fn eval_conditions<'s, 'r>(
    conditions: &'s Conditions,
    refr: &Reference<'r>,
) -> (bool, bool)
{
    let Conditions(ref match_type, ref conds) = *conditions;
    let tests: Vec<_> = conds.iter().map(|c| eval_cond(c, refr)).collect();
    let disambiguate = conds.iter().any(|c| c.disambiguate);
    (run_matcher(&tests, match_type), disambiguate)
}

fn eval_cond<'s, 'r>(cond: &'s Condition, refr: &Reference<'r>) -> bool {
    let mut tests = Vec::new();
    for var in cond.variable.iter() {
        tests.push(refr.has_variable(var));
    }
    for var in cond.is_numeric.iter() {
        tests.push(refr.number.get(var).map(|v| v.is_ok()).unwrap_or(false));
    }
    for typ in cond.csl_type.iter() {
        tests.push(refr.csl_type == *typ);
    }
    // TODO: pass down the current Cite to this point here so we can test positions and locators
    // TODO: is_uncertain_date ("ca. 2003"). CSL and CSL-JSON do not specify how this is meant to
    // work.

    run_matcher(&tests, &cond.match_type)
}

fn run_matcher(bools: &[bool], match_type: &Match) -> bool {
    match *match_type {
        Match::Any  => bools.iter().any(|b| *b),
        Match::Nand => bools.iter().any(|b| !*b),
        Match::All  => bools.iter().all(|b| *b),
        Match::None => bools.iter().all(|b| !*b),
    }
}

