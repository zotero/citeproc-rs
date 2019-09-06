// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::free::{FreeCond, FreeCondSets};
use super::Disambiguation;
use super::EdgeData;
use crate::prelude::*;
use citeproc_io::output::html::Html;
use csl::style::{Affixes, Formatting, Position};
use csl::variables::*;

use csl::{
    style::{BodyDate, Choose, Cond, Element, Group, IfThen, Match, Names, Style, TextSource},
    variables::AnyVariable,
    IsIndependent,
};

fn cross_product(db: &impl IrDatabase, els: &[Element]) -> FreeCondSets {
    // XXX: include layout parts?
    let mut all = fnv_set_with_cap(els.len());
    all.insert(FreeCond::empty());
    let mut f = FreeCondSets(all);
    for el in els {
        f.cross_product(el.get_free_conds(db));
    }
    f
}

fn mult_identity() -> FreeCondSets {
    let mut f = FreeCondSets::default();
    f.0.insert(FreeCond::empty());
    f
}

impl Disambiguation<Html> for Style {
    fn get_free_conds(&self, db: &impl IrDatabase) -> FreeCondSets {
        let els = &self.citation.layout.elements;
        cross_product(db, els)
    }

    fn ref_ir(
        &self,
        db: &impl IrDatabase,
        ctx: &RefContext<Html>,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        let els = &self.citation.layout.elements;
        ref_sequence(db, ctx, &els, "".into(), None, Affixes::default())
    }
}

impl Disambiguation<Html> for Group {
    fn get_free_conds(&self, db: &impl IrDatabase) -> FreeCondSets {
        // TODO: keep track of which empty variables caused GroupVars to not render, if
        // they are indeed free variables.
        cross_product(db, &self.elements)
    }

    fn ref_ir(
        &self,
        db: &impl IrDatabase,
        ctx: &RefContext<Html>,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        let stack = self.formatting.map(|mine| stack.override_with(mine));
        let els = &self.elements;
        ref_sequence(
            db,
            ctx,
            &els,
            self.delimiter.0.clone(),
            stack,
            self.affixes.clone(),
        )
    }
}

impl Disambiguation<Html> for BodyDate {
    fn get_free_conds(&self, db: &impl IrDatabase) -> FreeCondSets {
        mult_identity()
    }

    fn ref_ir(
        &self,
        db: &impl IrDatabase,
        ctx: &RefContext<Html>,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        unimplemented!()
    }
}

impl Disambiguation<Html> for Names {
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
        db: &impl IrDatabase,
        ctx: &RefContext<Html>,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        unimplemented!()
    }
}

impl Disambiguation<Html> for Element {
    fn ref_ir(
        &self,
        db: &impl IrDatabase,
        ctx: &RefContext<Html>,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        let renderer = Renderer::refr(ctx);
        let fmt = ctx.format;
        match *self {
            // TODO: keep track of which empty variables caused GroupVars to not render, if
            // they are indeed free variables.
            Element::Group(ref g) => g.ref_ir(db, ctx, stack),
            Element::Names(ref n) => n.ref_ir(db, ctx, stack),
            Element::Choose(ref c) => c.ref_ir(db, ctx, stack),
            Element::Date(ref d) => d.ref_ir(db, ctx, stack),
            Element::Number(ref var, ..) => unimplemented!(),
            Element::Text(ref src, f, ref af, quo, _sp, _tc, _disp) => match *src {
                TextSource::Variable(var, form) => {
                    if var == StandardVariable::Number(NumberVariable::Locator) {
                        if let Some(loctype) = ctx.locator_type {
                            let edge = db.edge(EdgeData::Locator);
                            return (RefIR::Edge(Some(edge)), GroupVars::DidRender);
                        }
                    }
                    let content = match var {
                        StandardVariable::Ordinary(v) => ctx
                            .reference
                            .ordinary
                            .get(&v)
                            .map(|val| renderer.text_variable(var, val, f, af, quo)),
                        StandardVariable::Number(v) => ctx
                            .reference
                            .number
                            .get(&v)
                            .map(|val| renderer.text_variable(var, val.verbatim(), f, af, quo)),
                    };
                    let content = content
                        .map(|x| fmt.output_in_context(x, stack))
                        .map(EdgeData::<Html>::Output)
                        .map(|label| db.edge(label));
                    let gv = GroupVars::rendered_if(content.is_some());
                    (RefIR::Edge(content), gv)
                }
                _ => unimplemented!(),
            },
            Element::Label(var, form, f, ref af, _tc, _sp, pl) => {
                if var == NumberVariable::Locator {
                    if let Some(loctype) = ctx.locator_type {
                        let edge = db.edge(EdgeData::LocatorLabel);
                        return (RefIR::Edge(Some(edge)), GroupVars::DidRender);
                    }
                }
                if var == NumberVariable::FirstReferenceNoteNumber {
                    if ctx.position == Position::Subsequent {
                        let edge = db.edge(EdgeData::LocatorLabel);
                        return (RefIR::Edge(Some(edge)), GroupVars::DidRender);
                    }
                }
                let content = ctx
                    .reference
                    .number
                    .get(&var)
                    .and_then(|val| renderer.label(var, form, val.clone(), pl, f, af))
                    .map(|x| fmt.output_in_context(x, stack))
                    .map(EdgeData::<Html>::Output)
                    .map(|label| db.edge(label));
                (RefIR::Edge(content), GroupVars::new())
            }
        }
    }

    fn get_free_conds(&self, db: &impl IrDatabase) -> FreeCondSets {
        match self {
            Element::Group(g) => g.get_free_conds(db),
            Element::Names(n) => n.get_free_conds(db),
            Element::Date(d) => d.get_free_conds(db),
            Element::Choose(c) => c.get_free_conds(db),
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
                    cross_product(db, macro_unsafe)
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

// pub struct Choose(pub IfThen, pub Vec<IfThen>, pub Else);
impl Disambiguation<Html> for Choose {
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

    fn ref_ir(
        &self,
        db: &impl IrDatabase,
        ctx: &RefContext<Html>,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        unimplemented!()
    }
}
