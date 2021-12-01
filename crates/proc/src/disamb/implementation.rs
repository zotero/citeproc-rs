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
        db: &dyn IrDatabase,
        ctx: &RefContext<Markup>,
        state: &mut IrState,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        let els = &self.citation.layout.elements;
        ref_sequence(db, state, ctx, els, false, Some(stack), None)
    }
}

impl Disambiguation<Markup> for Group {
    fn ref_ir(
        &self,
        db: &dyn IrDatabase,
        ctx: &RefContext<Markup>,
        state: &mut IrState,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        // TODO: handle GroupVars
        let stack = self.formatting.map(|mine| stack.override_with(mine));
        let els = &self.elements;
        ref_sequence(
            db,
            state,
            ctx,
            els,
            // implicit_conditional
            true,
            stack,
            Some(&|| RefIrSeq {
                delimiter: self.delimiter.clone(),
                affixes: self.affixes.clone(),
                ..Default::default()
            }),
        )
    }
}

impl Disambiguation<Markup> for Element {
    fn ref_ir(
        &self,
        db: &dyn IrDatabase,
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
                state.maybe_suppress_date(var, |state| dt.ref_ir(db, ctx, state, stack))
            }
            Element::Number(number) => {
                let var = number.variable;
                if var == NumberVariable::Locator {
                    let edge = state
                        .maybe_suppress_num(var, |_| ctx.locator_type.map(|_| EdgeData::Locator));
                    let gv = GroupVars::rendered_if(edge.is_some());
                    return (RefIR::Edge(edge), gv);
                }
                let content = state.maybe_suppress_num(var, |_| {
                    ctx.get_number(var).map(|val| renderer.number(number, &val))
                });
                let content = content
                    .map(|x| fmt.output_in_context(x, stack, None))
                    .map(EdgeData::Output);
                let gv = GroupVars::rendered_if(content.is_some());
                (RefIR::Edge(content), gv)
            }
            Element::Text(text) => match text.source {
                TextSource::Variable(var, form) => {
                    match var {
                        StandardVariable::Number(v @ NumberVariable::Locator) => {
                            if let Some(_loctype) = ctx.locator_type {
                                let edge = state.maybe_suppress_num(v, |_| Some(EdgeData::Locator));
                                let gv = GroupVars::rendered_if(edge.is_some());
                                return (RefIR::Edge(edge), gv);
                            }
                        }
                        StandardVariable::Number(v @ NumberVariable::FirstReferenceNoteNumber) => {
                            if ctx.position.matches(Position::Subsequent) {
                                let edge = state.maybe_suppress_num(v, |_| Some(EdgeData::Frnn));
                                let gv = GroupVars::rendered_if(edge.is_some());
                                return (RefIR::Edge(edge), gv);
                            }
                        }
                        StandardVariable::Number(v @ NumberVariable::CitationNumber) => {
                            if ctx.style.bibliography.is_some() {
                                let edge =
                                    state.maybe_suppress_num(v, |_| Some(EdgeData::CitationNumber));
                                let gv = GroupVars::rendered_if(edge.is_some());
                                return (RefIR::Edge(edge), gv);
                            }
                        }
                        StandardVariable::Ordinary(v @ Variable::YearSuffix) => {
                            if ctx.year_suffix {
                                let edge = state
                                    .maybe_suppress(v, |_state| Some(EdgeData::YearSuffixExplicit));
                                let gv = GroupVars::rendered_if(edge.is_some());
                                return (RefIR::Edge(edge), gv);
                            } else {
                                return (RefIR::Edge(None), GroupVars::Missing);
                            }
                        }
                        StandardVariable::Ordinary(v @ Variable::CitationLabel) => {
                            let vario = state.maybe_suppress(v, |_state| {
                                ctx.get_ordinary(v, form).map(|val| {
                                    renderer.text_variable(
                                        &crate::helpers::plain_text_element(v),
                                        var,
                                        &val,
                                    )
                                })
                            });
                            return vario
                                .map(|x| fmt.output_in_context(x, stack, None))
                                .map(EdgeData::Output)
                                .map(|edge| {
                                    let label = RefIR::Edge(Some(edge));
                                    let suffix_edge = RefIR::Edge(Some(EdgeData::YearSuffixPlain));
                                    let mut contents = Vec::new();
                                    contents.push(label);
                                    if ctx.year_suffix {
                                        contents.push(suffix_edge);
                                    }
                                    let seq = RefIrSeq {
                                        contents,
                                        affixes: text.affixes.clone(),
                                        formatting: text.formatting,
                                        text_case: text.text_case,
                                        quotes: renderer.quotes_if(text.quotes),
                                        ..Default::default()
                                    };
                                    (RefIR::Seq(seq), GroupVars::Important)
                                })
                                .unwrap_or((RefIR::Edge(None), GroupVars::Missing));
                        }
                        _ => {}
                    }
                    let content = match var {
                        StandardVariable::Ordinary(v) => state.maybe_suppress(v, |_| {
                            ctx.get_ordinary(v, form)
                                .map(|val| renderer.text_variable(text, var, &val))
                        }),
                        StandardVariable::Number(v) => state.maybe_suppress_num(v, |_| {
                            ctx.get_number(v)
                                .map(|val| renderer.text_number_variable(text, v, &val))
                        }),
                    };
                    let content = content
                        .map(|x| fmt.output_in_context(x, stack, None))
                        .map(EdgeData::Output);
                    let gv = GroupVars::rendered_if(content.is_some());
                    (RefIR::Edge(content), gv)
                }
                TextSource::Value(ref val) => {
                    let content = renderer
                        .text_value(text, &val)
                        .map(|x| fmt.output_in_context(x, stack, None))
                        .map(EdgeData::Output);
                    (RefIR::Edge(content), GroupVars::new())
                }
                TextSource::Term(term_selector, plural) => {
                    let content = renderer
                        .text_term(text, term_selector, plural)
                        .map(|x| fmt.output_in_context(x, stack, None))
                        .map(EdgeData::Output)
                        .map(|label| label);
                    let gv = if term_selector == csl::MiscTerm::NoDate {
                        GroupVars::Important // Make this Important (same for element.rs) to have no-date act as a variable
                    } else {
                        GroupVars::Plain
                    };
                    (RefIR::Edge(content), gv)
                }
                TextSource::Macro(ref name) => {
                    let macro_elements = ctx
                        .style
                        .macros
                        .get(name)
                        .expect("undefined macro should not be valid CSL");
                    state.push_macro(name);
                    let (seq, group_vars) = ref_sequence(
                        db,
                        state,
                        ctx,
                        &macro_elements,
                        true,
                        text.formatting,
                        Some(&|| RefIrSeq {
                            affixes: text.affixes.clone(),
                            quotes: renderer.quotes_if(text.quotes),
                            text_case: text.text_case,
                            ..Default::default()
                        }),
                    );
                    state.pop_macro(name);
                    (seq, group_vars)
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
                    return (RefIR::Edge(Some(edge_data)), GroupVars::Important);
                }
                let content = ctx
                    .get_number(var)
                    .and_then(|val| renderer.numeric_label(label, &val))
                    .map(|x| fmt.output_in_context(x, stack, None))
                    .map(EdgeData::Output);
                let gv = GroupVars::rendered_if(content.is_some());
                (RefIR::Edge(content), gv)
            }
        }
    }
}
