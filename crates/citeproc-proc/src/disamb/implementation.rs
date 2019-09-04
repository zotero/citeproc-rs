// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::free::{FreeCond, FreeCondSets};
use super::knowledge::Knowledge;
use super::Disambiguation;
use crate::prelude::*;
use citeproc_io::output::html::Html;
use citeproc_io::{Cite, Reference};
use csl::{
    style::{
        Choose, Cond, CondSet, Conditions, Element, Formatting, Group, IfThen, Match, Position,
        Style, TextSource,
    },
    variables::AnyVariable,
    IsIndependent,
};
use fnv::FnvHashSet;

fn cross_product(db: &impl IrDatabase, knowledge: &mut Knowledge, els: &[Element]) -> FreeCondSets {
    // XXX: include layout parts?
    let mut all = fnv_set_with_cap(els.len());
    all.insert(FreeCond::empty());
    let mut f = FreeCondSets(all);
    for el in els {
        f.cross_product(el.get_free_conds(db, knowledge));
    }
    f
}

fn mult_identity() -> FreeCondSets {
    let mut f = FreeCondSets::default();
    f.0.insert(FreeCond::empty());
    f
}

impl Disambiguation for Style {
    fn get_free_conds(&self, db: &impl IrDatabase, knowledge: &mut Knowledge) -> FreeCondSets {
        let els = &self.citation.layout.elements;
        cross_product(db, knowledge, els)
    }
}

impl Disambiguation for Element {
    fn get_free_conds(&self, db: &impl IrDatabase, knowledge: &mut Knowledge) -> FreeCondSets {
        match self {
            Element::Group(g) => {
                // TODO: keep track of which empty variables caused GroupVars to not render, if
                // they are indeed free variables.
                cross_product(db, knowledge, &g.elements)
            }
            Element::Names(n) => {
                // TODO: drill down into the substitute logic here
                if let Some(subst) = &n.substitute {
                    cross_product(db, knowledge, &subst.0)
                } else {
                    mult_identity()
                }
            }
            Element::Choose(c) => c.get_free_conds(db, knowledge),
            Element::Number(num_var, ..) | Element::Label(num_var, ..) => {
                if num_var.is_independent() {
                    let mut implicit_var_test = FreeCondSets::default();
                    let cond = Cond::Variable(AnyVariable::Number(*num_var));
                    implicit_var_test.scalar_multiply_cond(cond, true);
                    implicit_var_test
                } else {
                    mult_identity()
                }
            }
            Element::Text(src, ..) => match src {
                TextSource::Macro(m) => {
                    // TODO: same todos as in Proc
                    let style = db.style();
                    let macro_unsafe = style.macros.get(m).expect("macro errors not implemented!");
                    // TODO: reinstate macro recursion prevention with a new state arg
                    // if state.macro_stack.contains(&name) {
                    //     panic!(
                    //         "foiled macro recursion: {} called from within itself; exiting",
                    //         &name
                    //     );
                    // }
                    // state.macro_stack.insert(name.clone());
                    cross_product(db, knowledge, macro_unsafe)
                }
                TextSource::Variable(sv, ..) => {
                    if sv.is_independent() {
                        let mut implicit_var_test = FreeCondSets::default();
                        let cond = Cond::Variable(sv.into());
                        implicit_var_test.scalar_multiply_cond(cond, true);
                        implicit_var_test
                    } else {
                        mult_identity()
                    }
                }
                TextSource::Value(_) | TextSource::Term(..) => mult_identity(),
            },
            _ => mult_identity(),
        }
    }
}

impl Disambiguation for Choose {
    // pub struct Choose(pub IfThen, pub Vec<IfThen>, pub Else);
    fn get_free_conds(&self, db: &impl IrDatabase, knowledge: &mut Knowledge) -> FreeCondSets {
        use std::iter;
        let Choose(ifthen, elseifs, else_) = self;
        let IfThen(if_conditions, if_els) = ifthen;
        assert!(if_conditions.0 == Match::All);
        assert!(if_conditions.1.len() == 1);
        let if_els = cross_product(db, knowledge, if_els);
        let ifthen = (&if_conditions.1[0], if_els);
        let first: Vec<_> = iter::once(ifthen)
            .chain(elseifs.iter().map(|fi: &IfThen| {
                let IfThen(if_conditions, if_els) = fi;
                assert!(if_conditions.0 == Match::All);
                assert!(if_conditions.1.len() == 1);
                let if_els = cross_product(db, knowledge, if_els);
                (&if_conditions.1[0], if_els)
            }))
            .collect();
        FreeCondSets::all_branches(
            first.into_iter(),
            if else_.0.len() > 0 {
                Some(cross_product(db, knowledge, &else_.0))
            } else {
                None
            },
        )
    }
}
