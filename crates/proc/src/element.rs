use crate::helpers::sequence;
use crate::prelude::*;
use csl::variables::*;
use csl::Atom;
use csl::{
    Affixes, Bibliography, Element, LabelElement, NumberElement, Style, TextElement, TextSource,
};

impl<'c, O, I> Proc<'c, O, I> for Style
where
    O: OutputFormat,
    I: OutputFormat,
{
    fn intermediate(
        &self,
        db: &impl IrDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O, I>,
    ) -> IrSum<O> {
        let layout = &self.citation.layout;
        // Layout's delimiter and affixes are going to be applied later, when we join a cluster.
        sequence(
            db,
            state,
            ctx,
            &layout.elements,
            "".into(),
            None,
            Affixes::default(),
        )
    }
}

impl<'c, O, I> Proc<'c, O, I> for Bibliography
where
    O: OutputFormat,
    I: OutputFormat,
{
    fn intermediate(
        &self,
        db: &impl IrDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O, I>,
    ) -> IrSum<O> {
        let layout = &self.layout;
        // Layout's delimiter and affixes are going to be applied later, when we join a cluster.
        sequence(
            db,
            state,
            ctx,
            &layout.elements,
            "".into(),
            None,
            Affixes::default(),
        )
    }
}

impl<'c, O, I> Proc<'c, O, I> for Element
where
    O: OutputFormat,
    I: OutputFormat,
{
    fn intermediate(
        &self,
        db: &impl IrDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O, I>,
    ) -> IrSum<O> {
        let _fmt = &ctx.format;
        let renderer = Renderer::cite(ctx);
        match *self {
            Element::Choose(ref ch) => ch.intermediate(db, state, ctx),

            Element::Text(ref text) => {
                use citeproc_io::output::LocalizedQuotes;
                use csl::TextSource;
                let q = LocalizedQuotes::Single(Atom::from("'"), Atom::from("'"));
                let _quotes = if text.quotes { Some(&q) } else { None };
                match text.source {
                    TextSource::Macro(ref name) => {
                        // TODO: be able to return errors
                        let macro_unsafe = ctx
                            .style
                            .macros
                            .get(name)
                            .expect("macro errors not implemented!");
                        // Technically, if re-running a style with a fresh IrState, you might
                        // get an extra level of recursion before it panics. BUT, then it will
                        // already have panicked when it was run the first time! So we're OK.
                        if state.macro_stack.contains(&name) {
                            panic!(
                                "foiled macro recursion: {} called from within itself; exiting",
                                &name
                            );
                        }
                        state.macro_stack.insert(name.clone());
                        let out = sequence(
                            db,
                            state,
                            ctx,
                            &macro_unsafe,
                            "".into(),
                            text.formatting,
                            text.affixes.clone(),
                        );
                        state.macro_stack.remove(&name);
                        out
                    }
                    TextSource::Value(ref value) => {
                        let content = renderer.text_value(text, value).map(CiteEdgeData::Output);
                        (IR::Rendered(content), GroupVars::NoneSeen)
                    }
                    TextSource::Variable(var, form) => {
                        if var == StandardVariable::Ordinary(Variable::YearSuffix) {
                            if let Some(DisambPass::AddYearSuffix(i)) = ctx.disamb_pass {
                                let base26 = citeproc_io::utils::to_bijective_base_26(i);
                                return (
                                    IR::Rendered(Some(CiteEdgeData::YearSuffix(
                                        renderer
                                            .text_value(text, &base26)
                                            .expect("we made base26 ourselves, it is not empty"),
                                    ))),
                                    GroupVars::DidRender,
                                );
                            }
                            let ysh = YearSuffixHook::Explicit(self.clone());
                            return (IR::YearSuffix(ysh, None), GroupVars::OnlyEmpty);
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
                                    ctx.get_number(v).map(|val| {
                                        renderer.text_variable(text, var, val.verbatim())
                                    })
                                }
                            }
                        };
                        let content = content.map(CiteEdgeData::from_standard_variable(var, false));
                        let gv = GroupVars::rendered_if(content.is_some());
                        (IR::Rendered(content), gv)
                    }
                    TextSource::Term(term_selector, plural) => {
                        let content = renderer
                            .text_term(text, term_selector, plural)
                            .map(CiteEdgeData::Output);
                        (IR::Rendered(content), GroupVars::new())
                    }
                }
            }

            Element::Label(ref label) => {
                let var = label.variable;
                let content = if state.is_suppressed_num(var) {
                    None
                } else {
                    ctx.get_number(var)
                        .and_then(|val| renderer.numeric_label(label, val))
                        .map(CiteEdgeData::from_number_variable(var, true))
                };
                (IR::Rendered(content), GroupVars::new())
            }

            Element::Number(ref number) => {
                let var = number.variable;
                let content = if state.is_suppressed_num(var) {
                    None
                } else {
                    state.maybe_suppress_num(var);
                    ctx.get_number(var)
                        .map(|val| renderer.number(number, &val))
                        .map(CiteEdgeData::Output)
                };
                let gv = GroupVars::rendered_if(content.is_some());
                (IR::Rendered(content), gv)
            }

            Element::Names(ref ns) => ns.intermediate(db, state, ctx),

            //
            // You're going to have to replace sequence() with something more complicated.
            // And pass up information about .any(|v| used variables).
            Element::Group(ref g) => {
                let (seq, group_vars) = sequence(
                    db,
                    state,
                    ctx,
                    g.elements.as_ref(),
                    g.delimiter.0.clone(),
                    g.formatting,
                    g.affixes.clone(),
                );
                if group_vars.should_render_tree() {
                    // "reset" the group vars so that G(NoneSeen, G(OnlyEmpty)) will
                    // render the NoneSeen part. Groups shouldn't look inside inner
                    // groups.
                    (seq, group_vars)
                } else {
                    // Don't render the group!
                    (IR::Rendered(None), GroupVars::NoneSeen)
                }
            }
            Element::Date(ref dt) => {
                let var = dt.variable();
                if state.is_suppressed_date(var) {
                    (IR::Rendered(None), GroupVars::OnlyEmpty)
                } else {
                    state.maybe_suppress_date(var);
                    dt.intermediate(db, state, ctx)
                }
            }
        }
    }
}
