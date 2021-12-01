// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use self::initials::initialize;

use crate::disamb::names::{
    self as disamb, DisambNameData, DisambNameRatchet, NameIR, PersonDisambNameRatchet,
};
use crate::prelude::*;
use crate::NamesInheritance;
use citeproc_io::utils::Intercalate;
use citeproc_io::{Name, PersonName, Reference};
use csl::{
    Atom, DelimiterPrecedes, DemoteNonDroppingParticle, Name as NameEl, NameAnd, NameAsSortOrder,
    NameEtAl, NameForm, NamePart, NameVariable, Names, Position,
};

mod initials;

impl<B> DisambNameRatchet<B> {
    fn for_person(
        db: &dyn IrDatabase,
        var: NameVariable,
        value: PersonName,
        ref_id: &Atom,
        name_el: &NameEl,
        primary: bool,
        all_same_family_name: bool,
        advance_to_global: bool,
    ) -> Self {
        let mut data = DisambNameData {
            var,
            value,
            ref_id: ref_id.clone(),
            el: name_el.clone(),
            primary,
            all_same_family_name: all_same_family_name && name_el.form == Some(NameForm::Short),
        };
        let id = db.disamb_name(data.clone());
        // test disambiguate_AndreaEg2 decided that we shouldn't do this in RefIR mode.
        //
        if advance_to_global {
            let globally_disambiguated = db.disambiguated_person_names();
            if let Some(&global_pass) = globally_disambiguated.get(&id) {
                data.apply_upto_pass(global_pass);
                // optimise: should apply pass to the ratchet's iterator as well
            }
        }
        let ratchet = PersonDisambNameRatchet::new(&db.style(), id, data);
        DisambNameRatchet::Person(ratchet)
    }
}

/// One NameIR per variable
pub fn to_individual_name_irs<'a, O: OutputFormat, I: OutputFormat>(
    ctx: &'a GenericContext<'a, O, I>,
    names: &'a Names,
    names_inheritance: &'a NamesInheritance,
    db: &'a dyn IrDatabase,
    state: &'a IrState,
    advance_to_global: bool,
) -> impl Iterator<Item = NameIR<O>> + 'a + Clone {
    let fmt = ctx.format();
    let style = ctx.style();
    let locale = ctx.locale();
    let refr = ctx.reference();
    let get_name_ir = move |(var, label_var, value): (NameVariable, NameVariable, Vec<Name>)| {
        // fullstyles_APA.txt
        let all_same_family_name = disamb::all_same_family_name(&value);
        let ratchets = value
            .into_iter()
            .enumerate()
            .map(|(n, value)| {
                // Each variable gets its own 'primary' name.
                let primary = n == 0;
                match value {
                    Name::Person(pn) => DisambNameRatchet::for_person(
                        db,
                        var,
                        pn,
                        &refr.id,
                        &names_inheritance.name,
                        primary,
                        all_same_family_name,
                        advance_to_global,
                    ),
                    Name::Literal {
                        literal,
                        is_latin_cyrillic,
                    } => {
                        warn!("literal names should be normalised into family-only");
                        DisambNameRatchet::Literal {
                            literal: fmt.text_node(literal, None),
                            is_latin_cyrillic,
                        }
                    }
                }
            })
            .collect();
        NameIR::new(
            ctx,
            names_inheritance.clone(),
            var,
            label_var,
            ratchets,
            style,
            locale
                .et_al_term(names_inheritance.et_al.as_ref())
                .map(|(a, b)| (SmartString::from(a), b)),
            locale.and_term(None).map(|x| x.into()),
        )
    };

    // If multiple variables are selected (separated by single spaces, see example below), each
    // variable is independently rendered in the order specified, with one exception: when the
    // selection consists of “editor” and “translator”, and when the contents of these two name
    // variables is identical, then the contents of only one name variable is rendered. In
    // addition, the “editortranslator” term is used if the cs:names element contains a cs:label
    // element, replacing the default “editor” and “translator” terms (e.g. resulting in “Doe
    // (editor & translator)”).

    // Doesn't handle the editortranslator variable used directly (feature-flagged at the
    // moment), but it doesn't need to: that would accept a single list of names, which makes it
    // more convenient to use for people inputting names in a reference manager.

    let mut var_override = None;
    let mut slice_override = None;

    // Note: won't make editortranslator when you're also rendering a third or even more
    // variables.
    let is_editor_translator = &names.variables
        == &[NameVariable::Editor, NameVariable::Translator]
        || &names.variables == &[NameVariable::Translator, NameVariable::Editor];

    // name_EditorTranslatorSameEmptyTerm
    // (Although technically the spec isn't worded that way, it is useful to be able to disable
    // this behaviour.)
    let sel = csl::TextTermSelector::Role(csl::RoleTermSelector(
        csl::RoleTerm::EditorTranslator,
        csl::TermFormExtended::Long,
    ));
    let editortranslator_term_empty = locale.get_text_term(sel, false) == Some("");

    if is_editor_translator && !editortranslator_term_empty {
        let ed_val = refr.name.get(&NameVariable::Editor);
        let tr_val = refr.name.get(&NameVariable::Translator);
        if let (Some(ed), Some(tr)) = (ed_val, tr_val) {
            // identical
            if ed == tr {
                let ed_sup = state.is_suppressed_name(NameVariable::Editor);
                let tran_sup = state.is_suppressed_name(NameVariable::Translator);
                if ed_sup && tran_sup {
                    slice_override = Some(&[][..]);
                } else if ed_sup {
                    var_override = Some(NameVariable::EditorTranslator);
                    slice_override = Some(&[NameVariable::Translator][..]);
                } else {
                    var_override = Some(NameVariable::EditorTranslator);
                    slice_override = Some(&[NameVariable::Editor][..]);
                }
            }
        }
    }

    slice_override
        .unwrap_or(&names.variables[..])
        .iter()
        .filter(move |var| !state.is_suppressed_name(**var))
        .filter_map(move |var| {
            let ovar = var_override.as_ref().unwrap_or(var);
            refr.name.get(var).map(|val| (*var, *ovar, val.clone()))
        })
        .map(get_name_ir)
}

use crate::sort::Natural;
use crate::NameOverrider;
use csl::SortKey;

pub(crate) fn sort_strings_for_names(
    db: &dyn IrDatabase,
    refr: &Reference,
    var: NameVariable,
    sort_key: &SortKey,
    loc: CiteOrBib,
) -> Option<Vec<Natural<SmartString>>> {
    let style = db.style();
    let fmt = db.get_formatter();
    let (delim, arc_name_el) = match loc {
        CiteOrBib::Citation => style.name_info_citation(),
        CiteOrBib::Bibliography => style.name_info_bibliography(),
    };
    let name_o = NameOverrider::default();
    // Not clear from the spec whether we need to preserve the contextual name options or not.
    // This code does preserve them, and then forces NASO and form as is definitely required.
    let names_inheritance = name_o.inherited_names_options_sort_key(&arc_name_el, &delim, sort_key);
    let runner = OneNameVar {
        name_el: &names_inheritance.name.merge(&NameEl {
            name_as_sort_order: Some(NameAsSortOrder::All),
            form: Some(NameForm::Long),
            ..Default::default()
        }),
        bump_name_count: 0,
        demote_non_dropping_particle: style.demote_non_dropping_particle,
        initialize_with_hyphen: style.initialize_with_hyphen,
        fmt: &fmt,
    };
    let mut out = Vec::new();
    if let Some(values) = refr.name.get(&var) {
        for value in values {
            match value {
                Name::Person(pn) => {
                    runner.person_name_sort_keys(pn, &mut out);
                }
                Name::Literal { literal, .. } => {
                    if !literal.is_empty() {
                        out.push(Natural::new(literal.clone()));
                    }
                }
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

pub fn intermediate<'c, O: OutputFormat, I: OutputFormat>(
    names: &Names,
    db: &dyn IrDatabase,
    state: &mut IrState,
    ctx: &CiteContext<'c, O, I>,
    arena: &mut IrArena<O>,
) -> NodeId {
    let mut names_inheritance = state.name_override.inherited_names_options(
        &ctx.name_citation,
        &ctx.names_delimiter,
        names,
    );

    if let Some(key) = &ctx.sort_key {
        names_inheritance = names_inheritance.override_with(
            &ctx.name_citation,
            &ctx.names_delimiter,
            NamesInheritance::from_sort_key(key),
        );
    }

    let gen = GenericContext::Cit(ctx);
    let nirs_iterator = to_individual_name_irs(&gen, names, &names_inheritance, db, state, true);

    if names_inheritance.name.form == Some(NameForm::Count) {
        let name_irs = nirs_iterator.collect();
        // TODO: styling with a surrounding IrSeq
        let mut nc = IrNameCounter {
            name_irs,
            group_vars: GroupVars::new(),
        };
        // Substitute
        if nc.count(ctx) == 0 {
            if let Some(subst) = names.substitute.as_ref() {
                for el in subst.0.iter() {
                    // Need to clone the state so that any ultimately-non-rendering names blocks do not affect
                    // substitution later on
                    let mut new_state = state.clone();
                    let old = new_state
                        .name_override
                        .replace_name_overrides_for_substitute(names_inheritance.clone());
                    let node = el.intermediate(db, &mut new_state, ctx, arena);
                    if !IrTree::is_empty(node, arena) {
                        new_state.name_override.restore_name_overrides(old);
                        let wrapper = arena.new_node((IR::Substitute, GroupVars::Important));
                        wrapper.append(node, arena);
                        *state = new_state;
                        return wrapper;
                    }
                }
            }
            return arena.new_node((IR::Rendered(None), GroupVars::Missing));
        }
        let (new_ir, gv) = nc.render_cite(ctx);
        nc.group_vars = gv;
        let nc_node = arena.new_node((IR::NameCounter(nc), GroupVars::Important));
        let sub_node = arena.new_node((new_ir, gv));
        nc_node.append(sub_node, arena);
        return nc_node;
    }

    let seq_node = arena.new_node((IR::Rendered(None), GroupVars::Missing));

    for mut nir in nirs_iterator {
        let is_sort_key = ctx.sort_key.is_some();
        let label_after_name = nir
            .names_inheritance
            .label
            .as_ref()
            .map_or(false, |x| x.after_name);
        let built_label = nir.built_label.clone();
        let node = if let Some(result) = nir.intermediate_custom(
            &ctx.format,
            ctx.position.0,
            is_sort_key,
            ctx.disamb_pass,
            None,
        ) {
            let names_seq = NameIR::rendered_ntbs_to_node(
                result,
                arena,
                is_sort_key,
                label_after_name,
                built_label.as_ref(),
            );
            let nir_node = arena.new_node((IR::Name(nir), GroupVars::Important));
            nir_node.append(names_seq, arena);
            nir_node
        } else {
            // shouldn't happen; intermediate_custom should return Some the first time
            // round in any situation, and only retun None if it's impossible to crank any
            // further for a disamb pass
            error!("nir.intermediate_custom returned None the first time round");
            arena.new_node((IR::Rendered(None), GroupVars::Important))
        };
        seq_node.append(node, arena);
    }

    // Wait until iteration is done to collect
    state.maybe_suppress_name_vars(&names.variables);

    if seq_node.children(arena).next().is_none()
        || seq_node
            .children(arena)
            .filter_map(|id| arena.get(id))
            .map(Node::get)
            .all(|(ir, _)| match ir {
                IR::Name(nir) => nir.disamb_names.is_empty(),
                _ => true,
            })
    {
        if let Some(subst) = names.substitute.as_ref() {
            for el in subst.0.iter() {
                // Need to clone the state so that any ultimately-non-rendering names blocks do not affect
                // substitution later on
                let mut new_state = state.clone();
                let old = new_state
                    .name_override
                    .replace_name_overrides_for_substitute(names_inheritance.clone());
                let node = el.intermediate(db, &mut new_state, ctx, arena);
                if !IrTree::is_empty(node, arena) {
                    new_state.name_override.restore_name_overrides(old);
                    let wrapper = arena.new_node((IR::Substitute, GroupVars::Important));
                    wrapper.append(node, arena);
                    *state = new_state;
                    return wrapper;
                }
            }
        }
        seq_node.remove_subtree(arena);
        return arena.new_node((IR::Rendered(None), GroupVars::Missing));
    }

    // TODO: &[editor, translator] => &[editor], and use editortranslator on
    // the label

    let seq = IrSeq {
        formatting: names_inheritance.formatting,
        affixes: names_inheritance.affixes.clone(),
        delimiter: names_inheritance.delimiter.clone(),
        display: if ctx.in_bibliography {
            names.display
        } else {
            None
        },
        ..Default::default()
    };
    *arena.get_mut(seq_node).unwrap().get_mut() = (IR::Seq(seq), GroupVars::Important);
    seq_node
}

impl<'c, O, I> Proc<'c, O, I> for Names
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
        intermediate(self, db, state, ctx, arena)
    }
}

impl<'c, O: OutputFormat> NameIR<O> {
    pub fn count<I: OutputFormat>(&self, ctx: &CiteContext<'c, O, I>) -> u32 {
        let fmt = &ctx.format;
        let position = ctx.position.0;

        let runner = self.one_name_var(&self.names_inheritance.name, fmt);

        let name_tokens = runner.name_tokens(
            position,
            self.disamb_names.len(),
            ctx.sort_key.is_some(),
            self.etal_term.as_ref(),
        );

        let count: u32 = name_tokens.iter().fold(0, |acc, name| match name {
            NameToken::Name(_) => acc + 1,
            // etal, delimiter, etc
            _ => acc,
        });
        count
    }

    // For subsequent-author-substitute
    pub fn iter_bib_rendered_names<'a>(&'a self, fmt: &'a O) -> Vec<NameToken> {
        let runner = self.one_name_var(&self.names_inheritance.name, fmt);
        let name_tokens = runner.name_tokens(
            Position::First, // All bib entries are First
            self.disamb_names.len(),
            false, // not in sort key, we're transforming bib ir
            self.etal_term.as_ref(),
        );
        name_tokens
    }

    pub fn one_name_var<'a>(&self, name_el: &'a NameEl, fmt: &'a O) -> OneNameVar<'a, O> {
        OneNameVar {
            fmt,
            name_el,
            bump_name_count: self.name_counter.bump,
            demote_non_dropping_particle: self.demote_non_dropping_particle,
            initialize_with_hyphen: self.initialize_with_hyphen,
        }
    }

    /// Render each of the people's names for this NameIR (i.e. for this one variable)
    pub fn intermediate_custom(
        &mut self,
        fmt: &O,
        position: Position,
        is_sort_key: bool,
        pass: Option<DisambPass>,
        substitute: Option<(u32, &str)>,
    ) -> Option<Vec<O::Build>> {
        let (mut subst_count, subst_text) = substitute.unwrap_or((0, ""));
        let mut maybe_subst = |x: O::Build| -> O::Build {
            if subst_count > 0 {
                subst_count -= 1;
                fmt.plain(subst_text)
            } else {
                x
            }
        };

        let runner = self.one_name_var(&self.names_inheritance.name, fmt);
        let count_instead = runner.ntb_count_instead(
            &self.disamb_names,
            position,
            is_sort_key,
            self.etal_term.as_ref(),
        );
        if count_instead.is_some() {
            // Don't care about disambiguation with count. It's for sorting.
            return count_instead;
        }
        let (ntbs, ntb_len) = runner.names_to_builds(
            &self.disamb_names,
            position,
            &self.names_inheritance.et_al,
            is_sort_key,
            self.and_term.as_ref(),
            self.etal_term.as_ref(),
        );

        // TODO: refactor into a method on NameCounter
        self.name_counter.current = ntb_len;
        if pass == Some(DisambPass::AddNames)
            && self.name_counter.current <= self.name_counter.max_recorded
        {
            return None;
        }
        self.name_counter.max_recorded = self.name_counter.current;

        let mut cloned_runner = runner.clone();
        let mut rendered = Vec::new();
        let mut iter = ntbs.into_iter().peekable();
        while let Some(ntb) = iter.next() {
            let renderable = match ntb {
                NameTokenBuilt::Built(b, _lat_cy) => Some(b),
                NameTokenBuilt::Ratchet(index) => match self.disamb_names.get(index)? {
                    DisambNameRatchet::Literal {
                        literal,
                        is_latin_cyrillic: _,
                    } => {
                        if fmt.is_empty(literal) {
                            None
                        } else {
                            Some(maybe_subst(literal.clone()))
                        }
                    }
                    DisambNameRatchet::Person(pn) => {
                        cloned_runner.name_el = &pn.data.el;
                        let ret =
                            cloned_runner.render_person_name(&pn.data.value, !pn.data.primary);
                        cloned_runner.name_el = &self.names_inheritance.name;
                        Some(maybe_subst(ret)).filter(|x| !fmt.is_empty(&x))
                    }
                },
                NameTokenBuilt::Space => {
                    let next_is_latin = iter
                        .peek()
                        .map_or(None, |x| x.is_latin(&self.disamb_names))
                        .unwrap_or(false);
                    if next_is_latin {
                        Some(fmt.plain(" "))
                    } else {
                        None
                    }
                }
            };
            if let Some(r) = renderable {
                rendered.push(r);
            }
        }
        Some(rendered)
    }

    /// This must match the behaviour of Names::ref_ir and the stuff it adds to the Nfa
    /// graph.
    pub(crate) fn rendered_ntbs_to_node(
        rendered_ntbs: Vec<O::Build>,
        arena: &mut IrArena<O>,
        is_sort_key: bool,
        label_after_name: bool,
        built_label: Option<&O::Build>,
    ) -> NodeId {
        // Edit this later if we add anything
        let seq_node = arena.new_node((IR::Rendered(None), GroupVars::Missing));

        for built in rendered_ntbs {
            let node = arena.new_node((
                IR::Rendered(Some(CiteEdgeData::Output(built))),
                GroupVars::Important,
            ));
            seq_node.append(node, arena);
        }

        if seq_node.children(arena).next().is_none() {
            // this is Missing, unchanged from node creation
            seq_node
        } else {
            *arena.get_mut(seq_node).unwrap().get_mut() =
                (IR::Seq(IrSeq::default()), GroupVars::Important);
            if !is_sort_key {
                if let Some(label) = built_label {
                    let label_ir = IR::Rendered(Some(CiteEdgeData::Output(label.clone())));
                    let label = arena.new_node((label_ir, GroupVars::Plain));
                    if label_after_name {
                        seq_node.append(label, arena);
                    } else {
                        seq_node.prepend(label, arena);
                    }
                }
            }
            seq_node
        }
    }
}

/// For a given display order, not all the name parts will have data in them at the end. So for
/// this PersonName, reduce the DisplayOrdering to include only those parts that will end up
/// with content.
///
/// For example, for a last-name-only name like "Megalodon", `NamePartToken::Given` is removed,
/// which for `&[Family, SortSeparator, Given]` would leave `&[Family, SortSeparator]` and
/// render "Megalodon, ", so SortSeparator also has to be removed.
pub fn pn_filtered_parts(pn: &PersonName, order: DisplayOrdering) -> Vec<NamePartToken> {
    let parts: Vec<NamePartToken> = order
        .iter()
        .cloned()
        .filter(|npt| pn_filter_part(pn, *npt))
        .collect();

    // don't include leading or trailing spaces or delimiters
    let len = parts.len();
    let take = if let Some(last) = parts.iter().rposition(|t| t.not_delim()) {
        last + 1
    } else {
        len
    };
    // We may have dropped some of the namey name parts, leaving some stylistic tokens that
    // are incorrect or redundant. So we need to drop stuff like 'two spaces in a row'.
    // It *could* be done without a new Vec, but this is easier.
    parts
        .into_iter()
        .take(take)
        .fold(Vec::with_capacity(len), |mut acc, token| {
            use self::ord::NamePartToken::*;
            match (acc.last(), token) {
                (None, Space)
                | (None, SortSeparator)
                | (Some(Space), Space)
                | (Some(SortSeparator), SortSeparator)
                | (Some(SortSeparator), Space) => {
                    // do not add the token
                }
                (Some(Space), SortSeparator) => {
                    // recall that separator includes a space
                    // "Doe , John" is wrong
                    acc.pop();
                    acc.push(SortSeparator);
                }
                (_, t) => {
                    acc.push(t);
                }
            }
            acc
        })
}

fn pn_filter_part(pn: &PersonName, token: NamePartToken) -> bool {
    match token {
        NamePartToken::Given | NamePartToken::GivenAndDropping | NamePartToken::GivenAndBoth => {
            pn.given.as_ref().map_or(false, |s| !s.is_empty())
        }
        NamePartToken::Family | NamePartToken::FamilyDropped | NamePartToken::FamilyFull => {
            pn.family.as_ref().map_or(false, |s| !s.is_empty())
        }
        NamePartToken::NonDroppingParticle => pn
            .non_dropping_particle
            .as_ref()
            .map_or(false, |s| !s.is_empty()),
        NamePartToken::DroppingParticle => pn
            .dropping_particle
            .as_ref()
            .map_or(false, |s| !s.is_empty()),
        NamePartToken::Suffix => pn.suffix.as_ref().map_or(false, |s| !s.is_empty()),
        NamePartToken::Space => true,
        NamePartToken::SortSeparator => true,
    }
}

fn should_delimit_after<O: OutputFormat>(
    prec: DelimiterPrecedes,
    name: &OneNameVar<'_, O>,
    count_before_spot: usize,
) -> bool {
    match prec {
        DelimiterPrecedes::Contextual => count_before_spot >= 2,
        // anticipate whether name_as_sort_order would kick in for the
        // name just before the delimiter would go
        DelimiterPrecedes::AfterInvertedName => name.naso(count_before_spot > 1),
        DelimiterPrecedes::Always => true,
        DelimiterPrecedes::Never => false,
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum NameToken {
    /// Index of a DisambNameRatchet in the disamb_names array
    Name(usize),
    // Name(&'a DisambNameRatchet<B>),
    EtAl(SmartString, Option<Formatting>),
    Ellipsis,
    Delimiter,
    And,
    Space,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct OneNameVar<'a, O: OutputFormat> {
    pub name_el: &'a NameEl,
    pub bump_name_count: u16,
    // From Style
    pub demote_non_dropping_particle: DemoteNonDroppingParticle,
    pub initialize_with_hyphen: bool,
    pub fmt: &'a O,
}

impl<'a, O: OutputFormat> OneNameVar<'a, O> {
    #[inline]
    fn naso(&self, seen_one: bool) -> bool {
        match self.name_el.name_as_sort_order {
            None => false,
            Some(NameAsSortOrder::First) => !seen_one,
            Some(NameAsSortOrder::All) => true,
        }
    }

    #[inline]
    fn ea_min(&self, pos: Position) -> usize {
        let first = self.name_el.et_al_min.unwrap_or(0);
        let min = if pos == Position::First {
            first as usize
        } else {
            self.name_el.et_al_subsequent_min.unwrap_or(first) as usize
        };
        let use_first = self.ea_use_first(pos);
        std::cmp::max(min, use_first + 1)
    }

    #[inline]
    fn ea_use_first(&self, pos: Position) -> usize {
        let first = self.name_el.et_al_use_first.unwrap_or(1);
        let use_first = if pos == Position::First {
            first as usize
        } else {
            self.name_el.et_al_subsequent_use_first.unwrap_or(first) as usize
        };
        use_first + self.bump_name_count as usize
    }

    /// Any returned NameToken::Name(ix) will index into the names_slice.
    fn name_tokens(
        &self,
        position: Position,
        name_count: usize,
        is_sort_key: bool,
        etal_term: Option<&(SmartString, Option<Formatting>)>,
    ) -> Vec<NameToken> {
        let ea_min = self.ea_min(position);
        let ea_use_first = self.ea_use_first(position);
        if self.name_el.enable_et_al() && name_count >= ea_min {
            // etal_UseZeroFirst
            if ea_use_first == 0 {
                return Vec::new();
            }
            if self.name_el.et_al_use_last == Some(true) && ea_use_first + 2 <= name_count {
                let last = name_count - 1;
                let mut nms = (0..name_count)
                    .map(NameToken::Name)
                    .take(ea_use_first)
                    .intercalate(&NameToken::Delimiter);
                nms.push(NameToken::Delimiter);
                nms.push(NameToken::Ellipsis);
                nms.push(NameToken::Space);
                nms.push(NameToken::Name(last));
                nms
            } else {
                let mut nms = (0..name_count)
                    .map(NameToken::Name)
                    .take(ea_use_first)
                    .intercalate(&NameToken::Delimiter);
                if !is_sort_key {
                    if let Some((term_text, formatting)) = etal_term {
                        let dpea = self
                            .name_el
                            .delimiter_precedes_et_al
                            .unwrap_or(DelimiterPrecedes::Contextual);
                        if should_delimit_after(dpea, self, ea_use_first) {
                            nms.push(NameToken::Delimiter);
                        } else {
                            nms.push(NameToken::Space);
                        }
                        nms.push(NameToken::EtAl(term_text.clone(), *formatting));
                    }
                }
                nms
            }
        } else {
            let mut nms = (0..name_count)
                .map(NameToken::Name)
                .intercalate(&NameToken::Delimiter);
            // "delimiter-precedes-last" would be better named as "delimiter-precedes-and",
            // because it only has any effect when "and" is set.
            if self.name_el.and.is_some() && !is_sort_key {
                if let Some(last_delim) = nms.iter().rposition(|t| *t == NameToken::Delimiter) {
                    let dpl = self
                        .name_el
                        .delimiter_precedes_last
                        .unwrap_or(DelimiterPrecedes::Contextual);
                    if should_delimit_after(dpl, self, name_count - 1) {
                        nms.insert(last_delim + 1, NameToken::And);
                    } else {
                        nms[last_delim] = NameToken::Space;
                        nms.insert(last_delim + 1, NameToken::And);
                    }
                }
            }
            nms
        }
    }

    // TODO: strip html/markup for sort keys.
    pub(crate) fn person_name_sort_keys(
        &self,
        pn: &PersonName,
        out: &mut Vec<Natural<SmartString>>,
    ) {
        let order = get_sort_order(
            pn.is_latin_cyrillic,
            self.name_el.form == Some(NameForm::Long),
            self.demote_non_dropping_particle,
        );
        for sort_token in order {
            let mut s = SmartString::new();
            for token in sort_token
                .iter()
                .cloned()
                .filter(|npt| pn_filter_part(pn, *npt))
            {
                match token {
                    NamePartToken::Given
                    | NamePartToken::GivenAndDropping
                    | NamePartToken::GivenAndBoth => {
                        if let Some(ref given) = pn.given {
                            // TODO: parametrize for disambiguation
                            let string = initialize(
                                &given,
                                self.name_el.initialize.unwrap_or(true),
                                // name_OnlyGivenname.txt
                                if pn.family.is_some() {
                                    self.name_el.initialize_with.as_ref().map(|s| s.as_ref())
                                } else {
                                    None
                                },
                                self.initialize_with_hyphen,
                            );
                            s.push_str(&string);
                            if token != NamePartToken::Given {
                                if let Some(dp) = pn.dropping_particle.as_ref() {
                                    s.push_str(" ");
                                    s.push_str(dp);
                                }
                            }
                            if token == NamePartToken::GivenAndBoth {
                                if let Some(ndp) = pn.non_dropping_particle.as_ref() {
                                    s.push_str(" ");
                                    s.push_str(ndp);
                                }
                            }
                        }
                    }
                    NamePartToken::Family
                    | NamePartToken::FamilyDropped
                    | NamePartToken::FamilyFull => {
                        if let Some(fam) = pn.family.as_ref() {
                            let dp = pn
                                .dropping_particle
                                .as_ref()
                                .filter(|_| token == NamePartToken::FamilyFull);
                            let ndp = pn
                                .non_dropping_particle
                                .as_ref()
                                .filter(|_| token != NamePartToken::Family);
                            if let Some(dp) = dp {
                                s.push_str(dp);
                                if dp_should_append_space(dp) {
                                    s.push_str(" ");
                                }
                            }
                            if let Some(ndp) = ndp {
                                s.push_str(ndp);
                                if dp_should_append_space(ndp) {
                                    s.push_str(" ");
                                }
                            }
                            s.push_str(fam);
                        }
                    }
                    NamePartToken::NonDroppingParticle => {
                        s.push_str(&pn.non_dropping_particle.as_ref().unwrap());
                    }
                    NamePartToken::DroppingParticle => {
                        s.push_str(&pn.dropping_particle.as_ref().unwrap());
                    }
                    NamePartToken::Suffix => {
                        s.push_str(&pn.suffix.as_ref().unwrap());
                    }
                    NamePartToken::Space => {}
                    NamePartToken::SortSeparator => {}
                }
            }
            if !s.is_empty() {
                // UCD category is to catch \u{2019} etc.
                let is_punc = |c| unic_ucd_category::GeneralCategory::of(c).is_punctuation();
                let options = IngestOptions {
                    no_parse_quotes: true,
                    ..Default::default()
                };
                let fmt = crate::sort::SortStringFormat;
                let strip_it = fmt.ingest(&s, &options);
                let mut stripped = fmt.output(strip_it, false);
                if stripped.starts_with(is_punc) {
                    stripped = SmartString::from(stripped.trim_start_matches(is_punc));
                }
                out.push(crate::sort::Natural::new(stripped));
            }
        }
    }

    fn format_with_part(&self, o_part: &Option<NamePart>, s: impl AsRef<str>) -> O::Build {
        let fmt = self.fmt;
        // We don't want quotes to be parsed in names, so don't leave MicroNodes; we just
        // want InlineElement::Text but with text-casing applied.
        let mut options = IngestOptions {
            no_parse_quotes: true,
            ..Default::default()
        };
        match o_part {
            None => fmt.ingest(s.as_ref(), &options),
            Some(ref part) => {
                let NamePart {
                    text_case,
                    formatting,
                    // Don't apply affixes here; that has to be done separately for the weirdo
                    // name-part-formatting part of the spec.
                    ..
                } = *part;
                options.text_case = text_case;
                let mut b = fmt.ingest(s.as_ref(), &options);
                fmt.apply_text_case(&mut b, &options);
                fmt.with_format(b, formatting)
            }
        }
    }

    pub(crate) fn render_person_name(&self, pn: &PersonName, seen_one: bool) -> O::Build {
        let fmt = self.fmt;

        let order = get_display_order(
            pn.is_latin_cyrillic,
            self.name_el.form == Some(NameForm::Long),
            self.naso(seen_one),
            self.demote_non_dropping_particle,
        );

        let filtered_tokens = pn_filtered_parts(pn, order);
        let mut build = Vec::with_capacity(2 * filtered_tokens.len());
        for token in filtered_tokens {
            // We already tested is_some() for all these Some::unwrap() calls
            match token {
                NamePartToken::Given
                | NamePartToken::GivenAndDropping
                | NamePartToken::GivenAndBoth => {
                    if let Some(ref given) = pn.given {
                        let given_part = &self.name_el.name_part_given;
                        let family_part = &self.name_el.name_part_family;
                        let mut parts = Vec::new();
                        // TODO: parametrize for disambiguation
                        let initialized = initialize(
                            &given,
                            self.name_el.initialize.unwrap_or(true),
                            // name_OnlyGivenname.txt
                            if pn.family.is_some() {
                                self.name_el.initialize_with.as_ref().map(|s| s.as_ref())
                            } else {
                                None
                            },
                            self.initialize_with_hyphen,
                        );
                        parts.push(self.format_with_part(given_part, initialized.as_ref()));
                        if token != NamePartToken::Given {
                            if let Some(dp) = pn.dropping_particle.as_ref() {
                                parts.push(fmt.plain(" "));
                                parts.push(self.format_with_part(given_part, dp.clone()));
                            }
                        }
                        if token == NamePartToken::GivenAndBoth {
                            if let Some(ndp) = pn.non_dropping_particle.as_ref() {
                                parts.push(fmt.plain(" "));
                                parts.push(self.format_with_part(family_part, ndp.clone()));
                            }
                        }
                        let b = fmt.group(parts, "", None);
                        build.push(
                            fmt.affixed(
                                b,
                                given_part.as_ref().map_or(None, |p| p.affixes.as_ref()),
                            ),
                        );
                    }
                }
                NamePartToken::Family
                | NamePartToken::FamilyDropped
                | NamePartToken::FamilyFull => {
                    let family_part = &self.name_el.name_part_family;
                    let given_part = &self.name_el.name_part_given;
                    let fam = pn.family.as_ref().unwrap();
                    let dp = pn
                        .dropping_particle
                        .as_ref()
                        .filter(|_| token == NamePartToken::FamilyFull);
                    let ndp = pn
                        .non_dropping_particle
                        .as_ref()
                        .filter(|_| token != NamePartToken::Family);
                    let suffix = pn
                        .suffix
                        .as_ref()
                        .filter(|_| token == NamePartToken::FamilyFull);
                    let mut parts = Vec::new();
                    if let Some(dp) = dp {
                        let string = dp.clone();
                        parts.push(self.format_with_part(given_part, string));
                        if dp_should_append_space(dp) {
                            parts.push(fmt.plain(" "));
                        }
                    }
                    let mut casing = Vec::new();
                    if let Some(ndp) = ndp {
                        let string = ndp.clone();
                        casing.push(self.format_with_part(family_part, string));
                        if dp_should_append_space(ndp) {
                            casing.push(fmt.plain(" "));
                        }
                    }
                    casing.push(self.format_with_part(family_part, fam.clone()));
                    let mut casing = fmt.group(casing, "", None);
                    let options = IngestOptions {
                        no_parse_quotes: true,
                        text_case: family_part.as_ref().map_or(TextCase::None, |p| p.text_case),
                        ..Default::default()
                    };
                    fmt.apply_text_case(&mut casing, &options);
                    parts.push(casing);
                    if let Some(suffix) = suffix {
                        let mut string = SmartString::new();
                        if pn.comma_suffix {
                            string.push_str(", ");
                        } else {
                            string.push_str(" ");
                        }
                        string.push_str(suffix);
                        parts.push(fmt.text_node(string, None));
                    }
                    let b = fmt.group(parts, "", None);
                    build.push(
                        fmt.affixed(b, family_part.as_ref().map_or(None, |p| p.affixes.as_ref())),
                    );
                }
                NamePartToken::NonDroppingParticle => {
                    let family_part = &self.name_el.name_part_family;
                    build.push(self.format_with_part(
                        family_part,
                        pn.non_dropping_particle.as_ref().unwrap().clone(),
                    ));
                }
                NamePartToken::DroppingParticle => {
                    let given_part = &self.name_el.name_part_given;
                    build.push(self.format_with_part(
                        given_part,
                        pn.dropping_particle.as_ref().unwrap().clone(),
                    ));
                }
                NamePartToken::Suffix => {
                    build.push(fmt.plain(pn.suffix.as_ref().unwrap()));
                }
                NamePartToken::Space => {
                    build.push(fmt.plain(" "));
                }
                NamePartToken::SortSeparator => {
                    build.push(if let Some(sep) = &self.name_el.sort_separator {
                        fmt.plain(&sep)
                    } else {
                        fmt.plain(", ")
                    });
                }
            }
        }

        fmt.affixed(
            fmt.with_format(fmt.seq(build.into_iter()), self.name_el.formatting),
            self.name_el.affixes.as_ref(),
        )
    }

    fn ntb_count_instead(
        &self,
        names_slice: &[DisambNameRatchet<O::Build>],
        position: Position,
        is_sort_key: bool,
        etal_term: Option<&(SmartString, Option<Formatting>)>,
    ) -> Option<Vec<O::Build>> {
        if self.name_el.form == Some(NameForm::Count) {
            let name_tokens = self.name_tokens(position, names_slice.len(), is_sort_key, etal_term);
            let count: u32 = name_tokens.iter().fold(0, |acc, name| match name {
                NameToken::Name(_) => acc + 1,
                _ => acc,
            });
            if is_sort_key {
                let b = self.fmt.affixed_text(
                    smart_format!("{:08}", count),
                    None,
                    Some(&crate::sort::natural_sort::num_affixes()),
                );
                return Some(vec![b]);
            } else {
                // This isn't sort-mode, you can render NameForm::Count as text.
                return Some(vec![self.fmt.text_node(smart_format!("{}", count), None)]);
            }
        }
        None
    }

    /// without the <name /> formatting and affixes applied
    pub(crate) fn names_to_builds(
        &'a self,
        names_slice: &[DisambNameRatchet<O::Build>],
        position: Position,
        _et_al: &Option<NameEtAl>,
        is_sort_key: bool,
        and_term: Option<&SmartString>,
        etal_term: Option<&(SmartString, Option<Formatting>)>,
    ) -> (impl Iterator<Item = NameTokenBuilt<O::Build>> + 'a, u16) {
        let fmt = self.fmt.clone();
        let name_tokens = self.name_tokens(position, names_slice.len(), is_sort_key, etal_term);

        let ntb_len = name_tokens.iter().fold(0, |acc, n| match n {
            NameToken::Name(_ratchet) => acc + 1,
            _ => acc,
        });

        let and_term = and_term.cloned();

        let iterator = name_tokens.into_iter().filter_map(move |n| {
            Some(match n {
                NameToken::Name(ratchet) => NameTokenBuilt::Ratchet(ratchet),
                NameToken::Delimiter => {
                    let s = self.name_el.delimiter.as_opt_str().unwrap_or(", ");
                    NameTokenBuilt::Built(fmt.plain(s), citeproc_io::unicode::is_latin_cyrillic(s))
                }
                NameToken::EtAl(text, formatting) => {
                    if is_sort_key {
                        return None;
                    }
                    let lat_cy = citeproc_io::unicode::is_latin_cyrillic(&text);
                    NameTokenBuilt::Built(fmt.text_node(text, formatting), lat_cy)
                }
                NameToken::Ellipsis => NameTokenBuilt::Built(fmt.plain("…"), true),
                NameToken::Space => NameTokenBuilt::Space,
                NameToken::And => {
                    // If an And token shows up, we already know self.name_el.and is Some.
                    let form = match self.name_el.and {
                        Some(NameAnd::Symbol) => "&",
                        _ => and_term.as_ref().map(|x| x.as_ref()).unwrap_or("and"),
                    };
                    let mut string: SmartString = form.into();
                    let lat_cy = citeproc_io::unicode::is_latin_cyrillic(form);
                    if lat_cy {
                        string.push(' ');
                    }
                    NameTokenBuilt::Built(fmt.text_node(string, None), lat_cy)
                }
            })
        });
        (iterator, ntb_len)
    }
}

#[derive(Debug)]
pub enum NameTokenBuilt<B> {
    Ratchet(usize),
    Built(B, bool /* is_latin_cyrillic */),
    // So we can refuse to insert it after a non-latin-cyrillic name
    Space,
}

impl<B> NameTokenBuilt<B> {
    pub(crate) fn is_latin(&self, ratchets: &[DisambNameRatchet<B>]) -> Option<bool> {
        match self {
            NameTokenBuilt::Built(_, lat_cy) => Some(*lat_cy),
            NameTokenBuilt::Space => None,
            NameTokenBuilt::Ratchet(index) => match &ratchets[*index] {
                DisambNameRatchet::Literal {
                    is_latin_cyrillic, ..
                } => Some(*is_latin_cyrillic),
                DisambNameRatchet::Person(ratchet) => Some(ratchet.data.value.is_latin_cyrillic),
            },
        }
    }
}

use self::ord::{get_display_order, get_sort_order, DisplayOrdering, NamePartToken};

#[allow(dead_code)]
mod ord {
    //! Latin here means latin or cyrillic.
    //! TODO: use the regex crate with \\p{Cyrillic} and \\p{Latin}

    use csl::DemoteNonDroppingParticle as DNDP;

    pub type DisplayOrdering = &'static [NamePartToken];

    #[derive(Clone, Copy, PartialEq, Debug)]
    pub enum NamePartToken {
        Given,
        GivenAndDropping,
        GivenAndBoth,
        Family,
        FamilyFull,
        FamilyDropped,
        NonDroppingParticle,
        DroppingParticle,
        Suffix,
        SortSeparator,
        Space,
    }

    impl NamePartToken {
        pub fn not_delim(self) -> bool {
            match self {
                SortSeparator => false,
                Space => false,
                _ => true,
            }
        }
    }

    pub type SortOrdering = &'static [SortToken];
    pub type SortToken = &'static [NamePartToken];

    use self::NamePartToken::*;

    pub fn get_display_order(latin: bool, long: bool, naso: bool, demote: DNDP) -> DisplayOrdering {
        match (latin, long, naso, demote) {
            (false, long, ..) => {
                if long {
                    NON_LATIN_LONG
                } else {
                    NON_LATIN_SHORT
                }
            }
            (true, false, ..) => LATIN_SHORT,
            (true, true, false, _) => LATIN_LONG,
            (true, true, true, demote) => {
                if demote == DNDP::DisplayAndSort {
                    LATIN_LONG_NASO_DEMOTED
                } else {
                    LATIN_LONG_NASO
                }
            }
        }
    }

    pub fn get_sort_order(latin: bool, long: bool, demote: DNDP) -> SortOrdering {
        match (latin, long, demote) {
            (false, long, _) => {
                if long {
                    NON_LATIN_SORT_LONG
                } else {
                    NON_LATIN_SORT_SHORT
                }
            }
            (true, _, demote) => {
                if demote == DNDP::Never {
                    LATIN_SORT_NEVER
                } else {
                    LATIN_SORT
                }
            }
        }
    }

    /// [Jean] [de] [La] [Fontaine] [III]
    static LATIN_LONG: DisplayOrdering = &[Given, Space, FamilyFull];
    /// [La] [Fontaine], [Jean] [de], [III]
    static LATIN_LONG_NASO: DisplayOrdering = &[
        FamilyDropped,
        SortSeparator,
        GivenAndDropping,
        SortSeparator,
        Suffix,
    ];
    /// [Fontaine], [Jean] [de] [La], [III]
    static LATIN_LONG_NASO_DEMOTED: DisplayOrdering =
        &[Family, SortSeparator, GivenAndBoth, SortSeparator, Suffix];
    /// [La] [Fontaine]
    static LATIN_SHORT: DisplayOrdering = &[FamilyDropped];

    /// [La Fontaine] [de] [Jean] [III]
    static LATIN_SORT_NEVER: SortOrdering = &[
        &[NonDroppingParticle, Family],
        &[DroppingParticle],
        &[Given],
        &[Suffix],
    ];
    /// [Fontaine] [de La] [Jean] [III]
    static LATIN_SORT: SortOrdering = &[
        &[Family],
        &[DroppingParticle, NonDroppingParticle],
        &[Given],
        &[Suffix],
    ];

    /// 毛泽东 [Mao Zedong]
    static NON_LATIN_LONG: DisplayOrdering = &[
        Family, // TODO: how do we determine if spaces are required?
        Given,
    ];
    /// 毛 [Mao]
    static NON_LATIN_SHORT: DisplayOrdering = &[Family];
    /// 毛泽东 [Mao Zedong]
    static NON_LATIN_SORT_LONG: SortOrdering = &[&[Family], &[Given]];
    /// 毛 [Mao]
    static NON_LATIN_SORT_SHORT: SortOrdering = &[&[Family]];
}

/// we usually want to append a space to a non-dropping particle
///
/// "von" + "Crumb" = "von Crumb"
///
/// but not always; not when we have an apostrophe:
///
/// "d'" + "Angelo" = "d'Angelo"
///
/// see io/src/names.rs split_nondrop_family for the protocol for forcing the ndp to have a space
/// (input { family: "d' Lastname" } and the space will be preserved).
///
fn dp_should_append_space(s: &str) -> bool {
    !s.chars().rev().nth(0).map_or(true, |last| {
        matches!(last, '\u{2019}' | '\u{2018}' | '-' | '\'')
    })
}
