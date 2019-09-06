use crate::helpers::sequence;
use crate::prelude::*;
use citeproc_io::Locator;
use csl::style::{Affixes, Element, Style};
use csl::terms::{GenderedTermSelector, TextTermSelector};
use csl::variables::*;
use csl::Atom;

impl<'c, O> Proc<'c, O> for Style
where
    O: OutputFormat,
{
    fn intermediate(&self, state: &mut IrState, ctx: &CiteContext<'c, O>) -> IrSum<O> {
        let layout = &self.citation.layout;
        // Layout's delimiter and affixes are going to be applied later, when we join a cluster.
        sequence(
            state,
            ctx,
            &layout.elements,
            "".into(),
            None,
            Affixes::default(),
        )
    }
}

impl<'c, O> Proc<'c, O> for Element
where
    O: OutputFormat,
{
    fn intermediate(&self, state: &mut IrState, ctx: &CiteContext<'c, O>) -> IrSum<O> {
        let fmt = &ctx.format;
        let renderer = Renderer::cite(ctx);
        match *self {
            Element::Choose(ref ch) => ch.intermediate(state, ctx),

            Element::Text(ref source, f, ref af, quo, _sp, _tc, _disp) => {
                use citeproc_io::output::LocalizedQuotes;
                use csl::style::TextSource;
                let q = LocalizedQuotes::Single(Atom::from("'"), Atom::from("'"));
                let quotes = if quo { Some(&q) } else { None };
                match *source {
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
                        let out = sequence(state, ctx, &macro_unsafe, "".into(), f, af.clone());
                        state.macro_stack.remove(&name);
                        out
                    }
                    TextSource::Value(ref value) => {
                        state.tokens.insert(DisambToken::Str(value.clone()));
                        let content = renderer
                            .text_value(value, f, af, quo)
                            .map(CiteEdgeData::Output);
                        (IR::Rendered(content), GroupVars::new())
                    }
                    TextSource::Variable(var, form) => {
                        if var == StandardVariable::Ordinary(Variable::YearSuffix) {
                            if let Some(DisambPass::AddYearSuffix(i)) = ctx.disamb_pass {
                                let base26 = citeproc_io::utils::to_bijective_base_26(i);
                                state
                                    .tokens
                                    .insert(DisambToken::Str(base26.as_str().into()));
                                return (
                                    IR::Rendered(Some(CiteEdgeData::YearSuffix(
                                        fmt.text_node(base26, None),
                                    ))),
                                    GroupVars::DidRender,
                                );
                            }
                            let ysh = YearSuffixHook::Explicit(self.clone());
                            return (
                                IR::YearSuffix(ysh, O::Build::default()),
                                GroupVars::OnlyEmpty,
                            );
                        }
                        let content = match var {
                            StandardVariable::Ordinary(v) => ctx.get_ordinary(v, form).map(|val| {
                                state.tokens.insert(DisambToken::Str(val.into()));
                                renderer.text_variable(var, val, f, af, quo)
                            }),
                            StandardVariable::Number(v) => ctx.get_number(v).map(|val| {
                                state.tokens.insert(DisambToken::Num(val.clone()));
                                renderer.text_variable(var, val.verbatim(), f, af, quo)
                            }),
                        };
                        let content = content.map(CiteEdgeData::from_standard_variable(var));
                        let gv = GroupVars::rendered_if(content.is_some());
                        (IR::Rendered(content), gv)
                    }
                    TextSource::Term(term_selector, plural) => {
                        let content = renderer
                            .text_term(term_selector, plural, f, &af, quo)
                            .map(CiteEdgeData::Output);
                        (IR::Rendered(content), GroupVars::new())
                    }
                }
            }

            Element::Label(var, form, f, ref af, _tc, _sp, pl) => {
                let content = ctx
                    .get_number(var)
                    .and_then(|val| renderer.label(var, form, val, pl, f, af))
                    .map(CiteEdgeData::from_number_variable(var));
                (IR::Rendered(content), GroupVars::new())
            }

            Element::Number(var, _form, f, ref af, ref _tc, _disp) => {
                let content = ctx
                    .get_number(var)
                    .map(|val| renderer.number(var, val, f, af))
                    .map(CiteEdgeData::Output);
                let gv = GroupVars::rendered_if(content.is_some());
                (IR::Rendered(content), gv)
            }

            Element::Names(ref ns) => ns.intermediate(state, ctx),

            //
            // You're going to have to replace sequence() with something more complicated.
            // And pass up information about .any(|v| used variables).
            Element::Group(ref g) => {
                let (seq, group_vars) = sequence(
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
                dt.intermediate(state, ctx)
                // IR::YearSuffix(YearSuffixHook::Date(dt.clone()), fmt.plain("date"))
            }
        }
    }
}
