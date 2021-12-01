use crate::helpers::plain_text_element;
use crate::prelude::*;
use csl::{style::*, variables::*};

impl<'c, O, I> Proc<'c, O, I> for Citation
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
        let layout = &self.layout;
        sequence(
            db,
            state,
            ctx,
            arena,
            &layout.elements,
            false,
            Some(&|| IrSeq {
                // enable layout fixups on citation output, for when it is being combined with
                // intext.
                is_layout: true,
                ..Default::default()
            }),
        )
    }
}

impl<'c, O, I> Proc<'c, O, I> for InText
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
        let layout = &self.layout;
        sequence(db, state, ctx, arena, &layout.elements, false, None)
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
            false,
            // no such thing as layout delimiters in a bibliography
            Some(&|| IrSeq {
                formatting: layout.formatting,
                affixes: layout.affixes.clone(),
                is_layout: true,
                ..Default::default()
            }),
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
                        let macro_elements = ctx
                            .style
                            .macros
                            .get(name)
                            .expect("undefined macro should not be valid CSL");
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
                            &macro_elements,
                            // Not sure about this, but it acted like a group before...
                            true,
                            Some(&|| IrSeq {
                                formatting: text.formatting,
                                affixes: text.affixes.clone(),
                                display: text.display,
                                quotes: renderer.quotes_if(text.quotes),
                                text_case: text.text_case,
                                should_inherit_delim: false,
                                ..Default::default()
                            }),
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
                            let vario = state.maybe_suppress(v, |_| {
                                ctx.get_ordinary(v, form).map(|val| {
                                    renderer.text_variable(&plain_text_element(v), var, &val)
                                })
                            });
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
                                        display: text.display,
                                        quotes: renderer.quotes_if(text.quotes),
                                        ..Default::default()
                                    };
                                    // the citation-label is important, so so is the seq
                                    let seq_node =
                                        arena.new_node((IR::Seq(seq), GroupVars::Important));
                                    seq_node.append(label_node, arena);
                                    seq_node.append(hook_node, arena);
                                    seq_node
                                })
                                .unwrap_or_else(|| {
                                    arena.new_node((IR::Rendered(None), GroupVars::Missing))
                                });
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
                        let content = content.map(CiteEdgeData::from_standard_variable(var, false));
                        let gv = GroupVars::rendered_if(content.is_some());
                        arena.new_node((IR::Rendered(content), gv))
                    }
                    TextSource::Term(term_selector, plural) => {
                        let content = renderer
                            .text_term(text, term_selector, plural)
                            .map(CiteEdgeData::Term);
                        let gv = if term_selector == csl::MiscTerm::NoDate {
                            GroupVars::Important
                        } else {
                            GroupVars::Plain
                        };
                        arena.new_node((IR::Rendered(content), gv))
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
                let content = state.maybe_suppress_num(var, |_| {
                    ctx.get_number(var)
                        .map(|val| renderer.number(number, &val))
                        .map(CiteEdgeData::Output)
                });
                let gv = GroupVars::rendered_if(content.is_some());
                arena.new_node((IR::Rendered(content), gv))
            }

            Element::Names(ref ns) => ns.intermediate(db, state, ctx, arena),

            Element::Group(ref g) => sequence(
                db,
                state,
                ctx,
                arena,
                g.elements.as_ref(),
                true,
                Some(&|| IrSeq {
                    delimiter: g.delimiter.clone(),
                    formatting: g.formatting,
                    affixes: g.affixes.clone(),
                    display: g.display,
                    ..Default::default()
                }),
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
    fn default(&mut self) -> Self::Output {
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
                true,
                Some(&|| IrSeq {
                    formatting: text.formatting,
                    affixes: text.affixes.clone(),
                    display: text.display,
                    quotes: renderer.quotes_if(text.quotes),
                    text_case: text.text_case,
                    ..Default::default()
                }),
            ),
            WalkerFoldType::Group(group) => sequence(
                self.db,
                &mut self.state,
                self.ctx,
                self.arena,
                group.elements.as_ref(),
                true,
                Some(&|| IrSeq {
                    delimiter: group.delimiter.clone(),
                    formatting: group.formatting,
                    affixes: group.affixes.clone(),
                    display: group.display,
                    ..Default::default()
                }),
            ),
            WalkerFoldType::Layout(layout) => sequence(
                self.db,
                &mut self.state,
                self.ctx,
                self.arena,
                &layout.elements,
                false,
                None,
            ),
            WalkerFoldType::IfThen | WalkerFoldType::Else => sequence(
                self.db,
                &mut self.state,
                self.ctx,
                self.arena,
                elements,
                false,
                Some(&|| IrSeq {
                    should_inherit_delim: true,
                    ..Default::default()
                }),
            ),
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
            ref mut arena,
            ..
        } = *self;
        let o: Option<NodeId> = state.maybe_suppress_date(var, |state| {
            Some(body_date.intermediate(db, state, ctx, arena))
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
        let Self {
            ctx, state, arena, ..
        } = self;
        let renderer = Renderer::cite(&ctx);
        let content = state.maybe_suppress_num(var, |_| {
            ctx.get_number(var)
                .map(|val| renderer.number(number, &val))
                .map(CiteEdgeData::Output)
        });
        let gv = GroupVars::rendered_if(content.is_some());
        arena.new_node((IR::Rendered(content), gv))
    }

    fn text_value(&mut self, text: &TextElement, value: &SmartString) -> Self::Output {
        let renderer = Renderer::cite(&self.ctx);
        let content = renderer.text_value(text, value).map(CiteEdgeData::Output);
        self.arena
            .new_node((IR::Rendered(content), GroupVars::Plain))
    }
}
