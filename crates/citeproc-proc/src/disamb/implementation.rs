// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::Disambiguation;
use super::EdgeData;
use super::{cross_product, mult_identity, FreeCondSets};
use crate::prelude::*;
use citeproc_io::output::markup::Markup;
use csl::style::{Affixes, Formatting, Position};
use csl::variables::*;

use csl::{
    style::{Cond, Element, Group, Style, TextSource},
    variables::AnyVariable,
    IsIndependent,
};

impl Disambiguation<Markup> for Style {
    fn get_free_conds(&self, db: &impl IrDatabase) -> FreeCondSets {
        let els = &self.citation.layout.elements;
        cross_product(db, els)
    }

    fn ref_ir(
        &self,
        db: &impl IrDatabase,
        ctx: &RefContext<Markup>,
        state: &mut IrState,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        let els = &self.citation.layout.elements;
        ref_sequence(db, ctx, state, &els, "".into(), Some(stack), Affixes::default())
    }
}

impl Disambiguation<Markup> for Group {
    fn get_free_conds(&self, db: &impl IrDatabase) -> FreeCondSets {
        // TODO: keep track of which empty variables caused GroupVars to not render, if
        // they are indeed free variables.
        cross_product(db, &self.elements)
    }

    fn ref_ir(
        &self,
        db: &impl IrDatabase,
        ctx: &RefContext<Markup>,
        state: &mut IrState,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        // TODO: handle GroupVars
        let stack = self.formatting.map(|mine| stack.override_with(mine));
        let els = &self.elements;
        let (seq, group_vars) = ref_sequence(
            db,
            ctx,
            state,
            &els,
            self.delimiter.0.clone(),
            stack,
            self.affixes.clone(),
        );
        if group_vars.should_render_tree() {
            // "reset" the group vars so that G(NoneSeen, G(OnlyEmpty)) will
            // render the NoneSeen part. Groups shouldn't look inside inner
            // groups.
            (seq, group_vars)
        } else {
            // Don't render the group!
            (RefIR::Edge(None), GroupVars::NoneSeen)
        }
    }
}

impl Disambiguation<Markup> for Element {
    fn ref_ir(
        &self,
        db: &impl IrDatabase,
        ctx: &RefContext<Markup>,
        state: &mut IrState,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        let renderer = Renderer::refr(ctx);
        let fmt = ctx.format;
        match *self {
            // TODO: keep track of which empty variables caused GroupVars to not render, if
            // they are indeed free variables.
            Element::Group(ref g) => g.ref_ir(db, ctx, state, stack),
            Element::Names(ref n) => n.ref_ir(db, ctx, state, stack),
            Element::Choose(ref c) => c.ref_ir(db, ctx, state, stack),
            Element::Date(ref d) => d.ref_ir(db, ctx, state, stack),
            Element::Number(var, form, f, ref af, _tc, _dm) => {
                let content = match var {
                    NumberVariable::Locator => {
                        let e = ctx.locator_type.map(|_| db.edge(EdgeData::Locator));
                        return (RefIR::Edge(e), GroupVars::DidRender);
                    }
                    v => ctx
                        .reference
                        .number
                        .get(&v)
                        .map(|val| renderer.number(var, form, &val.clone(), f, af)),
                };
                let content = content
                    .map(|x| fmt.output_in_context(x, stack))
                    .map(EdgeData::<Markup>::Output)
                    .map(|label| db.edge(label));
                let gv = GroupVars::rendered_if(content.is_some());
                (RefIR::Edge(content), gv)
            }
            Element::Text(ref src, f, ref af, quo, _sp, _tc, _disp) => match *src {
                TextSource::Variable(var, _form) => {
                    // let fmt_plain_edge = |e: Edge| {
                    //     if f.is_some() || af != &Affixes::default() {
                    //         RefIR::Seq(RefIrSeq {
                    //             contents: vec![RefIR::Edge(Some(e))],
                    //             delimiter: csl::Atom::from(""),
                    //             affixes: af.clone(),
                    //             formatting: f,
                    //         })
                    //     } else {
                    //         RefIR::Edge(Some(e))
                    //     }
                    // };
                    if var == StandardVariable::Number(NumberVariable::Locator) {
                        if let Some(_loctype) = ctx.locator_type {
                            let edge = db.edge(EdgeData::Locator);
                            return (RefIR::Edge(Some(edge)), GroupVars::DidRender);
                        }
                    }
                    if var == StandardVariable::Ordinary(Variable::YearSuffix) {
                        if ctx.year_suffix {
                            let edge = db.edge(EdgeData::YearSuffixExplicit);
                            return (RefIR::Edge(Some(edge)), GroupVars::DidRender);
                        }
                    }
                    if var == StandardVariable::Number(NumberVariable::FirstReferenceNoteNumber) {
                        if ctx.position == Position::Subsequent {
                            let edge = db.edge(EdgeData::Frnn);
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
                        .map(EdgeData::<Markup>::Output)
                        .map(|label| db.edge(label));
                    let gv = GroupVars::rendered_if(content.is_some());
                    (RefIR::Edge(content), gv)
                }
                TextSource::Value(ref val) => {
                    let content = renderer
                        .text_value(&val, f, af, quo)
                        .map(|x| fmt.output_in_context(x, stack))
                        .map(EdgeData::<Markup>::Output)
                        .map(|label| db.edge(label));
                    (RefIR::Edge(content), GroupVars::new())
                }
                TextSource::Term(term_selector, plural) => {
                    let content = renderer
                        .text_term(term_selector, plural, f, &af, quo)
                        .map(|x| fmt.output_in_context(x, stack))
                        .map(EdgeData::<Markup>::Output)
                        .map(|label| db.edge(label));
                    (RefIR::Edge(content), GroupVars::new())
                }
                TextSource::Macro(ref name) => {
                    let macro_unsafe = ctx
                        .style
                        .macros
                        .get(name)
                        .expect("macro errors not implemented!");
                    // state.macro_stack.insert(name.clone());
                    let out = ref_sequence(db, ctx, state, &macro_unsafe, "".into(), f, af.clone());
                    // state.macro_stack.remove(&name);
                    out
                }
            },
            Element::Label(var, form, f, ref af, _tc, _sp, pl) => {
                if var == NumberVariable::Locator {
                    if let Some(_loctype) = ctx.locator_type {
                        let edge = db.edge(EdgeData::Locator);
                        return (RefIR::Edge(Some(edge)), GroupVars::DidRender);
                    }
                }
                if var == NumberVariable::FirstReferenceNoteNumber {
                    if ctx.position == Position::Subsequent {
                        let edge = db.edge(EdgeData::Frnn);
                        return (RefIR::Edge(Some(edge)), GroupVars::DidRender);
                    }
                }
                let content = ctx
                    .reference
                    .number
                    .get(&var)
                    .and_then(|val| renderer.numeric_label(var, form, val.clone(), pl, f, af))
                    .map(|x| fmt.output_in_context(x, stack))
                    .map(EdgeData::<Markup>::Output)
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
        }
    }
}
