use crate::db::StyleDatabase;
use crate::input::{CiteId, Locator};
use crate::output::OutputFormat;
use crate::Atom;
use csl::locale::Locale;
use csl::style::{Affixes, Element, Style};
use csl::terms::{GenderedTermSelector, TextTermSelector};
use csl::variables::*;
use std::collections::HashSet;
use std::sync::Arc;

mod cite_context;
pub use cite_context::CiteContext;
mod choose;
mod date;
mod disamb;
mod group;
mod helpers;
mod ir;
mod names;
pub use self::disamb::*;
use self::helpers::sequence;
pub use self::ir::*;
pub use group::GroupVars;

pub trait ProcDatabase: StyleDatabase {
    // TODO: get locales based on the current reference's language field
    fn default_locale(&self) -> Arc<Locale>;
    fn style_el(&self) -> Arc<Style>;
    fn cite_pos(&self, id: CiteId) -> csl::style::Position;
    /// 'First Reference Note Number' -- the number of the footnote containing the first cite
    /// referring to this cite's reference.
    fn cite_frnn(&self, id: CiteId) -> Option<u32>;
    fn bib_number(&self, id: CiteId) -> Option<u32>;
}

use fnv::FnvHashMap;

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct IrState {
    pub tokens: HashSet<DisambToken>,
    pub name_tokens: FnvHashMap<u64, HashSet<DisambToken>>,
    /// This can be a set because macros are strictly non-recursive.
    /// So the same macro name anywhere above indicates attempted recursion.
    /// When you exit a frame, delete from the set.
    pub macro_stack: HashSet<Atom>,
}

impl IrState {
    pub fn new() -> Self {
        IrState::default()
    }
}

// TODO: function to walk the entire tree for a <text variable="year-suffix"> to work out which
// nodes are possibly disambiguate-able in year suffix mode and if such a node should be inserted
// at the end of the layout block before the suffix. (You would only insert an IR node, not in the
// actual style, to keep it immutable and plain-&borrow-thread-shareable).
// TODO: also to figure out which macros are needed
// TODO: juris-m module loading in advance? probably in advance.

// Levels 1-3 will also have to update the ConditionalDisamb's current render

//
// * `'c`: [Cite]
// * `'ci`: [Cite]
// * `'r`: [Reference][]
//
// [Style]: ../style/element/struct.Style.html
// [Reference]: ../input/struct.Reference.html
pub trait Proc<'c, O>
where
    O: OutputFormat,
{
    /// `'s` (the self lifetime) must live longer than the IR it generates, because the IR will
    /// often borrow from self to be recomputed during disambiguation.
    fn intermediate(
        &self,
        db: &impl ProcDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O>,
    ) -> IrSum<O>;
}

impl<'c, O> Proc<'c, O> for Style
where
    O: OutputFormat,
{
    fn intermediate(
        &self,
        db: &impl ProcDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O>,
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

impl<'c, O> Proc<'c, O> for Element
where
    O: OutputFormat,
{
    fn intermediate(
        &self,
        db: &impl ProcDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O>,
    ) -> IrSum<O> {
        let fmt = &ctx.format;
        match *self {
            Element::Choose(ref ch) => ch.intermediate(db, state, ctx),

            Element::Text(ref source, f, ref af, quo, _sp, _tc, _disp) => {
                use crate::output::LocalizedQuotes;
                use csl::style::TextSource;
                let q = LocalizedQuotes::Single(Atom::from("'"), Atom::from("'"));
                let quotes = if quo { Some(&q) } else { None };
                match *source {
                    TextSource::Macro(ref name) => {
                        // TODO: be able to return errors
                        let style = db.style_el();
                        let macro_unsafe = style
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
                        let out = sequence(db, state, ctx, &macro_unsafe, "".into(), f, af.clone());
                        state.macro_stack.remove(&name);
                        out
                    }
                    TextSource::Value(ref value) => {
                        state.tokens.insert(DisambToken::Str(value.clone()));
                        (
                            IR::Rendered(Some(fmt.affixed_text_quoted(
                                value.to_string(),
                                f,
                                &af,
                                quotes,
                            ))),
                            GroupVars::new(),
                        )
                    }
                    TextSource::Variable(var, form) => {
                        if var == StandardVariable::Ordinary(Variable::YearSuffix) {
                            if let Some(DisambPass::AddYearSuffix(i)) = ctx.disamb_pass {
                                let base26 = crate::utils::to_bijective_base_26(i);
                                state
                                    .tokens
                                    .insert(DisambToken::Str(base26.as_str().into()));
                                return (
                                    IR::Rendered(Some(fmt.text_node(base26, None))),
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
                                let s = if v.should_replace_hyphens() {
                                    val.replace('-', "\u{2013}")
                                } else {
                                    val.to_string()
                                };
                                let maybe_link = v.hyperlink(val);
                                let txt = fmt.text_node(s, f);
                                let linked = fmt.hyperlinked(txt, maybe_link);
                                fmt.affixed_quoted(linked, &af, quotes)
                            }),
                            StandardVariable::Number(v) => ctx.get_number(v, db).map(|val| {
                                state.tokens.insert(DisambToken::Num(val.clone()));
                                fmt.affixed_text_quoted(
                                    val.verbatim(v.should_replace_hyphens()),
                                    f,
                                    &af,
                                    quotes,
                                )
                            }),
                        };
                        let gv = GroupVars::rendered_if(content.is_some());
                        (IR::Rendered(content), gv)
                    }
                    TextSource::Term(term_selector, plural) => {
                        let locale = db.default_locale();
                        let content = locale
                            .get_text_term(term_selector, plural)
                            .map(|val| fmt.affixed_text_quoted(val.to_owned(), f, &af, quotes));
                        (IR::Rendered(content), GroupVars::new())
                    }
                }
            }

            Element::Label(var, form, f, ref af, _tc, _sp, pl) => {
                use csl::style::Plural;
                let selector = GenderedTermSelector::from_number_variable(
                    &ctx.cite.locators.get(0).map(Locator::type_of),
                    var,
                    form,
                );
                let num_val = ctx.get_number(var, db);
                let plural = match (num_val, pl) {
                    (None, _) => None,
                    (Some(ref val), Plural::Contextual) => Some(val.is_multiple()),
                    (Some(_), Plural::Always) => Some(true),
                    (Some(_), Plural::Never) => Some(false),
                };
                let content = plural.and_then(|p| {
                    selector.and_then(|sel| {
                        let locale = db.default_locale();
                        locale
                            .get_text_term(TextTermSelector::Gendered(sel), p)
                            .map(|val| fmt.affixed_text(val.to_owned(), f, &af))
                    })
                });
                (IR::Rendered(content), GroupVars::new())
            }

            Element::Number(var, _form, f, ref af, ref _tc, _disp) => {
                let content = ctx.get_number(var, db).map(|val| {
                    fmt.affixed_text(val.as_number(var.should_replace_hyphens()), f, &af)
                });
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
                dt.intermediate(db, state, ctx)
                // IR::YearSuffix(YearSuffixHook::Date(dt.clone()), fmt.plain("date"))
            }
        }
    }
}
