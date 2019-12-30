// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::Disambiguation;
use super::EdgeData;
use crate::prelude::*;
use citeproc_io::output::markup::Markup;
use csl::*;

impl Disambiguation<Markup> for Style {
    fn ref_ir(
        &self,
        db: &impl IrDatabase,
        ctx: &RefContext<Markup>,
        state: &mut IrState,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        let els = &self.citation.layout.elements;
        ref_sequence(
            db,
            state,
            ctx,
            &els,
            "".into(),
            Some(stack),
            None,
            None,
            None,
            TextCase::None,
        )
    }
}

impl Disambiguation<Markup> for Group {
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
            state,
            ctx,
            &els,
            self.delimiter.0.clone(),
            stack,
            self.affixes.as_ref(),
            self.display,
            None,
            TextCase::None,
        );
        group_vars.implicit_conditional(seq)
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
        match self {
            // TODO: keep track of which empty variables caused GroupVars to not render, if
            // they are indeed free variables.
            Element::Group(g) => g.ref_ir(db, ctx, state, stack),
            Element::Names(n) => n.ref_ir(db, ctx, state, stack),
            Element::Choose(c) => c.ref_ir(db, ctx, state, stack),
            Element::Date(dt) => {
                let var = dt.variable();
                state.maybe_suppress_date(var, |state| {
                    dt.ref_ir(db, ctx, state, stack)
                })
            }
            Element::Number(number) => {
                let var = number.variable;
                let content = if state.is_suppressed_num(var) {
                    None
                } else {
                    state.maybe_suppress_num(var);
                    match var {
                        NumberVariable::Locator => {
                            let e = ctx.locator_type.map(|_| db.edge(EdgeData::Locator));
                            return (RefIR::Edge(e), GroupVars::Important);
                        }
                        v => ctx
                            .reference
                            .number
                            .get(&v)
                            .map(|val| renderer.number(number, &val.clone())),
                    }
                };
                let content = content
                    .map(|x| fmt.output_in_context(x, stack, None))
                    .map(EdgeData::<Markup>::Output)
                    .map(|label| db.edge(label));
                let gv = GroupVars::rendered_if(content.is_some());
                (RefIR::Edge(content), gv)
            }
            Element::Text(text) => match text.source {
                TextSource::Variable(var, form) => {
                    if var == StandardVariable::Number(NumberVariable::Locator) {
                        if let Some(_loctype) = ctx.locator_type {
                            let edge = db.edge(EdgeData::Locator);
                            return (RefIR::Edge(Some(edge)), GroupVars::Important);
                        }
                    }
                    if var == StandardVariable::Ordinary(Variable::YearSuffix) && ctx.year_suffix {
                        let edge = db.edge(EdgeData::YearSuffixExplicit);
                        return (RefIR::Edge(Some(edge)), GroupVars::Important);
                    }
                    if var == StandardVariable::Number(NumberVariable::FirstReferenceNoteNumber)
                        && ctx.position == Position::Subsequent
                    {
                        let edge = db.edge(EdgeData::Frnn);
                        return (RefIR::Edge(Some(edge)), GroupVars::Important);
                    }
                    if var == StandardVariable::Number(NumberVariable::CitationNumber)
                        && ctx.style.bibliography.is_some()
                    {
                        let edge = db.edge(EdgeData::CitationNumber);
                        return (RefIR::Edge(Some(edge)), GroupVars::Important);
                    }
                    let content = match var {
                        StandardVariable::Ordinary(v) => {
                            if state.is_suppressed_ordinary(v) {
                                None
                            } else {
                                state.maybe_suppress_ordinary(v);
                                ctx.get_ordinary(v, form)
                                    .map(|val| renderer.text_variable(text, var, val))
                            }
                        }
                        StandardVariable::Number(v) => {
                            if state.is_suppressed_num(v) {
                                None
                            } else {
                                state.maybe_suppress_num(v);
                                ctx.get_number(v)
                                    .map(|val| renderer.text_number_variable(text, v, &val))
                            }
                        }
                    };
                    let content = content
                        .map(|x| fmt.output_in_context(x, stack, None))
                        .map(EdgeData::<Markup>::Output)
                        .map(|label| db.edge(label));
                    let gv = GroupVars::rendered_if(content.is_some());
                    (RefIR::Edge(content), gv)
                }
                TextSource::Value(ref val) => {
                    let content = renderer
                        .text_value(text, &val)
                        .map(|x| fmt.output_in_context(x, stack, None))
                        .map(EdgeData::<Markup>::Output)
                        .map(|label| db.edge(label));
                    (RefIR::Edge(content), GroupVars::new())
                }
                TextSource::Term(term_selector, plural) => {
                    let content = renderer
                        .text_term(text, term_selector, plural)
                        .map(|x| fmt.output_in_context(x, stack, None))
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
                    state.push_macro(name);
                    let (seq, group_vars) = ref_sequence(
                        db,
                        state,
                        ctx,
                        &macro_unsafe,
                        "".into(),
                        text.formatting,
                        text.affixes.as_ref(),
                        text.display,
                        renderer.quotes_if(text.quotes),
                        text.text_case,
                    );
                    state.pop_macro(name);
                    group_vars.implicit_conditional(seq)
                }
            },
            Element::Label(label) => {
                let var = label.variable;
                let custom = match var {
                    NumberVariable::Locator if ctx.locator_type.is_some() => {
                        Some(EdgeData::LocatorLabel)
                    }
                    NumberVariable::FirstReferenceNoteNumber
                        if ctx.position == Position::Subsequent =>
                    {
                        Some(EdgeData::FrnnLabel)
                    }
                    NumberVariable::CitationNumber if ctx.style.bibliography.is_some() => {
                        Some(EdgeData::CitationNumberLabel)
                    }
                    NumberVariable::Locator
                    | NumberVariable::FirstReferenceNoteNumber
                    | NumberVariable::CitationNumber
                    | _ if state.is_suppressed_num(var) => {
                        return (RefIR::Edge(None), GroupVars::new());
                    }
                    _ => None,
                };
                if let Some(edge_data) = custom {
                    let edge = db.edge(edge_data);
                    return (RefIR::Edge(Some(edge)), GroupVars::Important);
                }
                let content = ctx
                    .get_number(var)
                    .and_then(|val| renderer.numeric_label(label, val))
                    .map(|x| fmt.output_in_context(x, stack, None))
                    .map(EdgeData::<Markup>::Output)
                    .map(|label| db.edge(label));
                let gv = GroupVars::rendered_if(content.is_some());
                (RefIR::Edge(content), gv)
            }
        }
    }
}
