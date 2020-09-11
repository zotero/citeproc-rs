use crate::helpers::plain_text_element;
use crate::prelude::*;
use csl::variables::*;
use csl::*;

impl<'c, O, I> Proc<'c, O, I> for Style
where
    O: OutputFormat,
    I: OutputFormat,
{
    fn intermediate(
        &self,
        db: &dyn IrDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O, I>,
        arena: &mut IrArena<O>,
    ) -> NodeId {
        let layout = &self.citation.layout;
        sequence_basic(db, state, ctx, arena, &layout.elements)
    }
}

impl<'c, O, I> Proc<'c, O, I> for Bibliography
where
    O: OutputFormat,
    I: OutputFormat,
{
    fn intermediate(
        &self,
        db: &dyn IrDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O, I>,
        arena: &mut IrArena<O>,
    ) -> NodeId {
        // Unlike cite, we will apply affixes and formatting in the seq, so that they go inside
        // any second-field-align content.
        let layout = &self.layout;
        sequence(
            db,
            state,
            ctx,
            arena,
            &layout.elements,
            // no such thing as layout delimiters in a bibliography
            "".into(),
            layout.formatting,
            layout.affixes.as_ref(),
            None,
            None,
            TextCase::None,
            true,
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
        db: &dyn IrDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O, I>,
        arena: &mut IrArena<O>,
    ) -> NodeId {
        let renderer = Renderer::cite(ctx);
        match *self {
            Element::Choose(ref ch) => ch.intermediate(db, state, ctx, arena),

            Element::Text(ref text) => {
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
                        // XXX: that's not quite true
                        state.push_macro(name);
                        let ir_sum = sequence(
                            db,
                            state,
                            ctx,
                            arena,
                            &macro_unsafe,
                            "".into(),
                            text.formatting,
                            text.affixes.as_ref(),
                            text.display,
                            renderer.quotes_if(text.quotes),
                            text.text_case,
                            true,
                        );
                        state.pop_macro(name);
                        ir_sum
                    }
                    TextSource::Value(ref value) => {
                        let content = renderer.text_value(text, value).map(CiteEdgeData::Output);
                        arena.new_node((IR::Rendered(content), GroupVars::Plain))
                    }
                    TextSource::Variable(var, form) => {
                        if var == StandardVariable::Ordinary(Variable::YearSuffix) {
                            let hook = YearSuffixHook::Explicit(text.clone());
                            // Only available when sorting, and ir_gen3 and later
                            if let Some(i) = ctx.year_suffix {
                                return arena.new_node(hook.render(ctx, i));
                            }
                            return arena.new_node(IR::year_suffix(hook));
                        }
                        if var == StandardVariable::Ordinary(Variable::CitationLabel) {
                            let hook = IR::year_suffix(YearSuffixHook::Plain);
                            let v = Variable::CitationLabel;
                            let vario = if state.is_suppressed_ordinary(v) {
                                None
                            } else {
                                state.maybe_suppress_ordinary(v);
                                ctx.get_ordinary(v, form).map(|val| {
                                    renderer.text_variable(&plain_text_element(v), var, &val)
                                })
                            };
                            return vario
                                .map(|label| {
                                    let label_node = arena.new_node((
                                        IR::Rendered(Some(CiteEdgeData::Output(label))),
                                        GroupVars::Important,
                                    ));
                                    let hook_node = arena.new_node(hook);
                                    let seq = IrSeq {
                                        formatting: text.formatting,
                                        affixes: text.affixes.clone(),
                                        text_case: text.text_case,
                                        delimiter: Atom::from(""),
                                        display: text.display,
                                        quotes: renderer.quotes_if(text.quotes),
                                        dropped_gv: None,
                                    };
                                    // the citation-label is important, so so is the seq
                                    arena.new_node((IR::Seq(seq), GroupVars::Important))
                                })
                                .unwrap_or_else(|| {
                                    arena.new_node((IR::Rendered(None), GroupVars::Missing))
                                });
                        }
                        let content = match var {
                            StandardVariable::Ordinary(v) => {
                                if state.is_suppressed_ordinary(v) {
                                    None
                                } else {
                                    state.maybe_suppress_ordinary(v);
                                    ctx.get_ordinary(v, form)
                                        .map(|val| renderer.text_variable(text, var, &val))
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
                        let content = content.map(CiteEdgeData::from_standard_variable(var, false));
                        let gv = GroupVars::rendered_if(content.is_some());
                        arena.new_node((IR::Rendered(content), gv))
                    }
                    TextSource::Term(term_selector, plural) => {
                        let content = renderer
                            .text_term(text, term_selector, plural)
                            .map(CiteEdgeData::Term);
                        arena.new_node((IR::Rendered(content), GroupVars::new()))
                    }
                }
            }

            Element::Label(ref label) => {
                let var = label.variable;
                let content = if state.is_suppressed_num(var) {
                    None
                } else {
                    ctx.get_number(var)
                        .and_then(|val| renderer.numeric_label(label, &val))
                        .map(CiteEdgeData::from_number_variable(var, true))
                };
                arena.new_node((IR::Rendered(content), GroupVars::new()))
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
                arena.new_node((IR::Rendered(content), gv))
            }

            Element::Names(ref ns) => ns.intermediate(db, state, ctx, arena),

            //
            // You're going to have to replace sequence() with something more complicated.
            // And pass up information about .any(|v| used variables).
            Element::Group(ref g) => sequence(
                db,
                state,
                ctx,
                arena,
                g.elements.as_ref(),
                g.delimiter.0.clone(),
                g.formatting,
                g.affixes.as_ref(),
                g.display,
                None,
                TextCase::None,
                true,
            ),
            Element::Date(ref dt) => {
                let var = dt.variable();
                let o: Option<NodeId> = state
                    .maybe_suppress_date(var, |state| Some(dt.intermediate(db, state, ctx, arena)));
                o.unwrap_or_else(|| arena.new_node((IR::Rendered(None), GroupVars::Missing)))
            }
        }
    }
}

impl YearSuffixHook {
    pub(crate) fn render<'c, O: OutputFormat, I: OutputFormat>(
        &self,
        ctx: &CiteContext<'c, O, I>,
        suffix_num: u32,
    ) -> IrSum<O> {
        let implicit = plain_text_element(Variable::YearSuffix);
        let text = match self {
            YearSuffixHook::Explicit(text) => text,
            _ => &implicit,
        };
        let renderer = Renderer::cite(ctx);
        let base26 = citeproc_io::utils::to_bijective_base_26(suffix_num);
        let output = renderer
            .text_value(text, &base26)
            .expect("base26 is not empty");
        (
            IR::Rendered(Some(CiteEdgeData::YearSuffix(output))),
            GroupVars::Important,
        )
    }
}

struct ProcWalker<'a, O, I>
where
    O: OutputFormat,
    I: OutputFormat,
{
    db: &'a dyn IrDatabase,
    state: IrState,
    ctx: &'a CiteContext<'a, O, I>,
    arena: &'a mut IrArena<O>,
}

impl<'a, O: OutputFormat, I: OutputFormat> StyleWalker for ProcWalker<'a, O, I> {
    type Output = NodeId;
    type Checker = CiteContext<'a, O, I>;
    fn get_checker(&self) -> Option<&Self::Checker> {
        Some(&self.ctx)
    }

    // Compare with Output = Option<NodeId>, where you wouldn't know the GroupVars of the child.
    fn default(&self) -> Self::Output {
        self.arena.new_node((IR::Rendered(None), GroupVars::Plain))
    }
    fn fold(&mut self, elements: &[Element], fold_type: WalkerFoldType) -> Self::Output {
        let renderer = Renderer::cite(&self.ctx);
        match fold_type {
            WalkerFoldType::Macro(text) => sequence(
                self.db,
                &mut self.state,
                self.ctx,
                self.arena,
                &elements,
                "".into(),
                text.formatting,
                text.affixes.as_ref(),
                text.display,
                renderer.quotes_if(text.quotes),
                text.text_case,
                true,
            ),
            WalkerFoldType::Group(group) => sequence(
                self.db,
                &mut self.state,
                self.ctx,
                self.arena,
                group.elements.as_ref(),
                group.delimiter.0.clone(),
                group.formatting,
                group.affixes.as_ref(),
                group.display,
                None,
                TextCase::None,
                true,
            ),
            WalkerFoldType::Layout(layout) => sequence_basic(
                self.db,
                &mut self.state,
                self.ctx,
                self.arena,
                &layout.elements,
            ),
            WalkerFoldType::IfThen | WalkerFoldType::Else => {
                sequence_basic(self.db, &mut self.state, self.ctx, self.arena, elements)
            }
            WalkerFoldType::Substitute => {
                todo!("use fold() to implement name element substitution")
            }
        }
    }

    fn date(&mut self, body_date: &BodyDate) -> Self::Output {
        let var = body_date.variable();
        let ProcWalker {
            db,
            ctx,
            ref mut state,
            ..
        } = *self;
        let o: Option<NodeId> = state.maybe_suppress_date(var, |state| {
            Some(body_date.intermediate(db, state, ctx, self.arena))
        });
        o.unwrap_or_else(|| {
            self.arena
                .new_node((IR::Rendered(None), GroupVars::Missing))
        })
    }

    fn names(&mut self, names: &Names) -> Self::Output {
        names.intermediate(self.db, &mut self.state, self.ctx, self.arena)
    }

    fn number(&mut self, number: &NumberElement) -> Self::Output {
        let var = number.variable;
        let renderer = Renderer::cite(&self.ctx);
        let state = &mut self.state;
        let content = if state.is_suppressed_num(var) {
            None
        } else {
            state.maybe_suppress_num(var);
            self.ctx
                .get_number(var)
                .map(|val| renderer.number(number, &val))
                .map(CiteEdgeData::Output)
        };
        let gv = GroupVars::rendered_if(content.is_some());
        self.arena.new_node((IR::Rendered(content), gv))
    }

    fn text_value(&mut self, text: &TextElement, value: &Atom) -> Self::Output {
        let renderer = Renderer::cite(&self.ctx);
        let content = renderer.text_value(text, value).map(CiteEdgeData::Output);
        self.arena
            .new_node((IR::Rendered(content), GroupVars::Plain))
    }
}
