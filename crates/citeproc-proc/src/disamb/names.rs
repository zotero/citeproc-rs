use super::mult_identity;
use crate::prelude::*;
use csl::style::{Citation, GivenNameDisambiguationRule, Name as NameEl, NameForm, Names};

impl Disambiguation<Markup> for Names {
    fn get_free_conds(&self, db: &impl IrDatabase) -> FreeCondSets {
        // TODO: drill down into the substitute logic here
        if let Some(subst) = &self.substitute {
            cross_product(db, &subst.0)
        } else {
            mult_identity()
        }
    }
    fn ref_ir(
        &self,
        _db: &impl IrDatabase,
        _ctx: &RefContext<Markup>,
        _stack: Formatting,
    ) -> (RefIR, GroupVars) {
        warn!("ref_ir not implemented for Names");
        (RefIR::Edge(None), GroupVars::new())
    }
}

use csl::Atom;

/// The GivenNameDisambiguationRule variants are poorly worded. "-with-initials" doesn't *add*
/// steps, it removes steps / limits the expansion. This is a bit clearer to work with, and mixes
/// in the information about whether a name is primary or not.
#[derive(Clone, Copy, Debug)]
enum SingleNameDisambMethod {
    None,
    AddInitials,
    AddInitialsThenGivenName,
}

impl SingleNameDisambMethod {
    /// `is_primary` refers to whether this is the first name to be rendered in a Names element.
    fn from_rule(rule: GivenNameDisambiguationRule, is_primary: bool) -> Self {
        match (rule, is_primary) {
            (GivenNameDisambiguationRule::ByCite, _)
            | (GivenNameDisambiguationRule::AllNames, _) => {
                SingleNameDisambMethod::AddInitialsThenGivenName
            }
            (GivenNameDisambiguationRule::AllNamesWithInitials, _) => {
                SingleNameDisambMethod::AddInitials
            }
            (GivenNameDisambiguationRule::PrimaryName, true) => {
                SingleNameDisambMethod::AddInitialsThenGivenName
            }
            (GivenNameDisambiguationRule::PrimaryNameWithInitials, true) => {
                SingleNameDisambMethod::AddInitials
            }
            _ => SingleNameDisambMethod::None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SingleNameDisambIter {
    /// If this is None, the iterator won't produce anything. Essentially the null object
    /// pattern.
    method: SingleNameDisambMethod,
    /// Whether to use part 1 or part 2 of the name expansion steps (confusing, because you are
    /// never running both in sequence, it's a choice)
    initialize_with: bool,
    name_form: NameForm,
    state: NameDisambState,
}

#[derive(Clone, Copy, Debug)]
enum NameDisambState {
    Original,
    AddedInitials,
    AddedGivenName,
}

impl SingleNameDisambIter {
    fn new(method: SingleNameDisambMethod, name_options: &NameEl) -> Self {
        SingleNameDisambIter {
            method,
            initialize_with: name_options.initialize_with.is_some()
                && name_options.initialize == Some(true),
            name_form: name_options.form.unwrap_or(NameForm::Long),
            state: NameDisambState::Original,
        }
    }
}

impl Iterator for SingleNameDisambIter {
    type Item = NameDisambPass;
    fn next(&mut self) -> Option<Self::Item> {
        match self.method {
            SingleNameDisambMethod::None => None,
            SingleNameDisambMethod::AddInitials => {
                if self.initialize_with {
                    match self.state {
                        NameDisambState::Original => {
                            if self.name_form == NameForm::Short {
                                self.state = NameDisambState::AddedInitials;
                                Some(NameDisambPass::WithFormLong)
                            } else {
                                None
                            }
                        }
                        NameDisambState::AddedInitials => None,
                        NameDisambState::AddedGivenName => unreachable!(),
                    }
                } else {
                    None
                }
            }
            SingleNameDisambMethod::AddInitialsThenGivenName => {
                if self.initialize_with {
                    match (self.state, self.name_form) {
                        (NameDisambState::Original, NameForm::Short) => {
                            self.state = NameDisambState::AddedInitials;
                            Some(NameDisambPass::WithFormLong)
                        }
                        (NameDisambState::Original, _) | (NameDisambState::AddedInitials, _) => {
                            self.state = NameDisambState::AddedGivenName;
                            Some(NameDisambPass::WithInitializeFalse)
                        }
                        (NameDisambState::AddedGivenName, _) => None,
                    }
                } else {
                    match self.state {
                        NameDisambState::Original => {
                            self.state = NameDisambState::AddedGivenName;
                            if self.name_form == NameForm::Short {
                                Some(NameDisambPass::WithFormLong)
                            } else {
                                None
                            }
                        }
                        NameDisambState::AddedInitials => unreachable!(),
                        NameDisambState::AddedGivenName => None,
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameDisambPass {
    WithFormLong,
    WithInitializeFalse,
}

#[cfg(test)]
fn test(name: &NameEl, rule: GivenNameDisambiguationRule, primary: bool) -> Vec<NameDisambPass> {
    let method = SingleNameDisambMethod::from_rule(rule, primary);
    let mut iter = SingleNameDisambIter::new(method, name);
    let passes: Vec<_> = iter.collect();
    passes
}

#[test]
fn test_name_disamb_iter() {
    let mut name = NameEl::default();
    name.form = Some(NameForm::Long); // default
    name.initialize = Some(true); // default
    assert_eq!(
        test(&name, GivenNameDisambiguationRule::AllNames, true),
        vec![]
    );

    name.form = Some(NameForm::Short);
    assert_eq!(
        test(&name, GivenNameDisambiguationRule::AllNames, true),
        vec![NameDisambPass::WithFormLong]
    );

    name.form = Some(NameForm::Short);
    assert_eq!(
        test(&name, GivenNameDisambiguationRule::PrimaryName, true),
        vec![NameDisambPass::WithFormLong]
    );
    assert_eq!(
        test(&name, GivenNameDisambiguationRule::PrimaryName, false),
        vec![]
    );
    name.initialize_with = Some(Atom::from("."));
    assert_eq!(
        test(&name, GivenNameDisambiguationRule::AllNames, true),
        vec![
            NameDisambPass::WithFormLong,
            NameDisambPass::WithInitializeFalse
        ]
    );
    assert_eq!(
        test(
            &name,
            GivenNameDisambiguationRule::AllNamesWithInitials,
            true
        ),
        vec![NameDisambPass::WithFormLong]
    );
}
