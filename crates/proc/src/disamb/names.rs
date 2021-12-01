use super::add_to_graph;
use super::finite_automata::{Nfa, NfaEdge};
use super::graph_with_stack;
use crate::names::{NameTokenBuilt, OneNameVar};
use crate::prelude::*;
use citeproc_io::{Name, PersonName};
use csl::variables::*;
use csl::{
    Atom, DemoteNonDroppingParticle, GivenNameDisambiguationRule as GNDR, Name as NameEl, NameForm,
    Names, Position, Style,
};
use fnv::FnvHashMap;
use petgraph::graph::NodeIndex;
use smallvec::SmallVec;
use std::sync::Arc;

impl Disambiguation<Markup> for Names {
    fn ref_ir(
        &self,
        db: &dyn IrDatabase,
        ctx: &RefContext<Markup>,
        state: &mut IrState,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        let child_stack = self
            .formatting
            .map_or(stack, |mine| stack.override_with(mine));
        let fmt = ctx.format;
        let style = ctx.style;
        let locale = ctx.locale;
        let names_inheritance =
            state
                .name_override
                .inherited_names_options(&ctx.name_el, &ctx.names_delimiter, &self);

        // TODO: resolve which parts of name_el's Formatting are irrelevant due to 'stack'
        // and get a reduced formatting to work with

        let and_term = locale.and_term(None).map(SmartString::from);
        let etal_term = locale.et_al_term(names_inheritance.et_al.as_ref());
        let mut runner = OneNameVar {
            name_el: &names_inheritance.name,
            bump_name_count: 0,
            fmt,
            demote_non_dropping_particle: style.demote_non_dropping_particle,
            initialize_with_hyphen: style.initialize_with_hyphen,
        };

        let mut seq = RefIrSeq {
            contents: Vec::with_capacity(self.variables.len()),
            formatting: self.formatting,
            affixes: self.affixes.clone(),
            delimiter: names_inheritance.delimiter.clone(),
            ..Default::default()
        };

        let gen = GenericContext::<Markup, Markup>::Ref(ctx);
        let name_irs =
            crate::names::to_individual_name_irs(&gen, self, &names_inheritance, db, state, false);
        for nir in name_irs {
            let mut nfa = Nfa::new();
            let start = nfa.graph.add_node(());
            nfa.start.insert(start);
            let mut max_counted_tokens = 0u16;
            let mut counted_tokens;

            let mut once = false;
            loop {
                if once {
                    runner.bump_name_count += 1;
                }
                once = true;
                let label_after_name = nir
                    .names_inheritance
                    .label
                    .as_ref()
                    .map_or(false, |x| x.after_name);
                let (ntbs, ntb_len) = runner.names_to_builds(
                    &nir.disamb_names,
                    ctx.position,
                    &self.et_al,
                    false,
                    and_term.as_ref(),
                    etal_term
                        .as_ref()
                        .map(|(a, b)| (a.as_str().into(), b.clone()))
                        .as_ref(),
                );
                counted_tokens = ntb_len;
                if counted_tokens <= max_counted_tokens {
                    break;
                }

                let one_run = graph_with_stack(
                    fmt,
                    &mut nfa,
                    runner.name_el.formatting,
                    runner.name_el.affixes.as_ref(),
                    start,
                    |nfa, mut spot| {
                        // Generally, here we must match the behaviour of NameIR::rendered_ntbs_to_node
                        let mut iter = ntbs.into_iter().peekable();
                        let mk_label = |nfa: &mut Nfa, place: &mut NodeIndex| {
                            if let Some(built_label) = nir.built_label.as_ref() {
                                let formatted =
                                    fmt.output_in_context(built_label.clone(), child_stack, None);
                                let new = nfa.graph.add_node(());
                                let edge = NfaEdge::Token(EdgeData::Output(formatted));
                                nfa.graph.add_edge(*place, new, edge);
                                *place = new;
                            }
                        };
                        if !label_after_name {
                            mk_label(nfa, &mut spot);
                        }
                        while let Some(ntb) = iter.next() {
                            match ntb {
                                NameTokenBuilt::Built(b, _lat_cy) => {
                                    if !fmt.is_empty(&b) {
                                        let out =
                                            fmt.output_in_context(b.to_vec(), child_stack, None);
                                        let e = EdgeData::Output(out);
                                        let ir = RefIR::Edge(Some(e));
                                        spot = add_to_graph(fmt, nfa, &ir, spot, None);
                                    }
                                }
                                NameTokenBuilt::Space => {
                                    let next_is_latin = iter
                                        .peek()
                                        .map_or(None, |x| x.is_latin(&nir.disamb_names))
                                        .unwrap_or(false);
                                    if next_is_latin {
                                        let e = EdgeData::Output(" ".into());
                                        let ir = RefIR::Edge(Some(e));
                                        spot = add_to_graph(fmt, nfa, &ir, spot, None);
                                    }
                                }
                                NameTokenBuilt::Ratchet(index) => match &nir.disamb_names[index] {
                                    DisambNameRatchet::Literal {
                                        literal,
                                        is_latin_cyrillic: _,
                                    } => {
                                        if !fmt.is_empty(literal) {
                                            let out = fmt.output_in_context(
                                                literal.clone(),
                                                child_stack,
                                                None,
                                            );
                                            let e = EdgeData::Output(out);
                                            let ir = RefIR::Edge(Some(e));
                                            spot = add_to_graph(fmt, nfa, &ir, spot, None);
                                        }
                                    }
                                    DisambNameRatchet::Person(ratchet) => {
                                        let dn = ratchet.data.clone();
                                        spot = add_expanded_name_to_graph(
                                            db,
                                            nfa,
                                            dn,
                                            spot,
                                            child_stack,
                                        );
                                    }
                                },
                            }
                        }
                        if label_after_name {
                            mk_label(nfa, &mut spot);
                        }
                        spot
                    },
                );
                if one_run == start {
                    // XXX: not sure about this
                    continue;
                }
                nfa.accepting.insert(one_run);
                max_counted_tokens = counted_tokens;
            }
            if !nfa.accepting.is_empty() {
                seq.contents
                    .push(RefIR::Name(RefNameIR::from_name_ir(&nir), nfa))
            }
        }

        state.maybe_suppress_name_vars(&self.variables);

        if seq.contents.is_empty() {
            if let Some(subst) = self.substitute.as_ref() {
                for el in subst.0.iter() {
                    // Need to clone the state so that any ultimately-non-rendering names blocks do not affect
                    // substitution later on
                    let mut new_state = state.clone();
                    let old = new_state
                        .name_override
                        .replace_name_overrides_for_substitute(names_inheritance.clone());
                    let (ir, gv) = el.ref_ir(db, ctx, &mut new_state, stack);
                    if !ir.is_empty() {
                        new_state.name_override.restore_name_overrides(old);
                        *state = new_state;
                        return (ir, gv);
                    }
                }
            }
            return (RefIR::Edge(None), GroupVars::Missing);
        }

        (RefIR::Seq(seq), GroupVars::Important)
    }
}

citeproc_db::intern_key!(pub DisambName);
impl DisambName {
    pub fn lookup(self, db: &dyn IrDatabase) -> DisambNameData {
        db.lookup_disamb_name(self)
    }
}

/// A name, with enough context to perform global name disambiguation.
/// Created mainly with `crate::db::all_person_names`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DisambNameData {
    /// The reference the name appears in
    pub(crate) ref_id: Atom,
    /// The variable in the reference that the name appears in
    pub(crate) var: NameVariable,
    /// The element that it is to be rendered with. This has to contain the inherited name options,
    /// so it encapsulates a single "rendering context".
    pub(crate) el: NameEl,
    /// The actual name itself
    pub(crate) value: PersonName,
    /// Whether the name is the primary name for this name variable
    pub(crate) primary: bool,

    /// Hack for fullstyles_APA.txt
    /// This is something of an APA-specific hack, but it's OK to apply everywhere
    /// as the APA rule is sane enough. We want to avoid adding initials to a the
    /// first of a bunch of the same last names.
    /// Just stores the result of calling `crate::disamb::all_same_family_name`.
    pub(crate) all_same_family_name: bool,
}

/// fullstyles_APA.txt
/// An optimisation of "do all these names render the same under NameForm::Short"
pub(crate) fn all_same_family_name(names: &[Name]) -> bool {
    if names.len() <= 1 {
        return false;
    }
    let mut family_names = names.iter().map(|name| match name {
        Name::Person(PersonName {
            family: Some(f), ..
        }) => Some(f),
        _ => None,
    });
    if let Some(Some(first)) = family_names.next() {
        !family_names.any(|x| x.map(|x| x != first).unwrap_or(true))
    } else {
        false
    }
}

#[test]
fn test_all_same_family_name() {
    let n1 = Name::Person(PersonName {
        family: Some("Oblinger".into()),
        ..Default::default()
    });
    let n2 = Name::Person(PersonName {
        family: Some("Oblinger".into()),
        ..Default::default()
    });
    let n3 = Name::Person(PersonName {
        family: Some("Other".into()),
        ..Default::default()
    });
    assert!(all_same_family_name(&[n1.clone(), n2.clone()]));
    assert!(!all_same_family_name(&[n1.clone()]));
    // fullstyles_APA.txt
    assert!(!all_same_family_name(&[n1, n2, n3]));
}

impl DisambNameData {
    /// Sets options on the NameEl such that rendering the name again will
    /// produce an expanded form.
    pub fn apply_upto_pass(&mut self, pass: NameDisambPass) {
        if pass == NameDisambPass::Initial {
            // noop, but also won't reverse higher settings.
        }
        if pass >= NameDisambPass::WithFormLong {
            self.el.form = Some(NameForm::Long);
        }
        if pass >= NameDisambPass::WithInitializeFalse {
            self.el.initialize = Some(false);
        }
    }

    /// Render the name to an EdgeData, using the db's default formatter,
    /// in a formatting context `stack`.
    /// This is used directly for *global name disambiguation*, and for ratcheting one name
    /// forward in NameIR expansion.
    pub(crate) fn single_name_edge(&self, db: &dyn IrDatabase, stack: Formatting) -> EdgeData {
        let fmt = &db.get_formatter();
        let style = db.style();
        let builder = OneNameVar {
            fmt,
            name_el: &self.el,
            bump_name_count: 0,
            demote_non_dropping_particle: style.demote_non_dropping_particle,
            initialize_with_hyphen: style.initialize_with_hyphen,
        };
        let built = builder.render_person_name(&self.value, !self.primary);
        let o = fmt.output_in_context(built, stack, None);
        EdgeData::Output(o)
    }

    fn disamb_iter(&self, rule: GNDR) -> SingleNameDisambIter {
        let method = SingleNameDisambMethod::from_rule(rule, self.primary);
        SingleNameDisambIter::new(method, &self.el)
    }
}

/// The GNDR variants are poorly worded. "-with-initials" doesn't *add*
/// steps, it removes steps / limits the expansion. This is a bit clearer to work with, and mixes
/// in the information about whether a name is primary or not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SingleNameDisambMethod {
    None,
    AddInitials,
    AddInitialsThenGivenName,
}

impl SingleNameDisambMethod {
    /// `is_primary` refers to whether this is the first name to be rendered in a Names element.
    fn from_rule(rule: GNDR, is_primary: bool) -> Self {
        match (rule, is_primary) {
            (GNDR::ByCite, _) | (GNDR::AllNames, _) => {
                SingleNameDisambMethod::AddInitialsThenGivenName
            }
            (GNDR::AllNamesWithInitials, _) => SingleNameDisambMethod::AddInitials,
            (GNDR::PrimaryName, true) => SingleNameDisambMethod::AddInitialsThenGivenName,
            (GNDR::PrimaryNameWithInitials, true) => SingleNameDisambMethod::AddInitials,
            _ => SingleNameDisambMethod::None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SingleNameDisambIter {
    /// If this is None, the iterator won't produce anything. Essentially the null object
    /// pattern.
    method: SingleNameDisambMethod,
    /// Whether to use part 1 or part 2 of the name expansion steps (confusing, because you are
    /// never running both in sequence, it's a choice)
    initialize_with: bool,
    name_form: NameForm,
    state: NameDisambState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NameDisambState {
    Original,
    AddedInitials,
    AddedGivenName,
}

impl SingleNameDisambIter {
    fn new(method: SingleNameDisambMethod, name_el: &NameEl) -> Self {
        SingleNameDisambIter {
            method,
            initialize_with: name_el.initialize_with.is_some() && name_el.initialize == Some(true),
            name_form: name_el.form.unwrap_or(NameForm::Long),
            state: NameDisambState::Original,
        }
    }
}

impl Iterator for SingleNameDisambIter {
    type Item = NameDisambPass;
    fn next(&mut self) -> Option<Self::Item> {
        match self.method {
            SingleNameDisambMethod::None => None,
            SingleNameDisambMethod::AddInitials => {
                if self.initialize_with {
                    match self.state {
                        NameDisambState::Original => {
                            if self.name_form == NameForm::Short {
                                self.state = NameDisambState::AddedInitials;
                                Some(NameDisambPass::WithFormLong)
                            } else {
                                None
                            }
                        }
                        NameDisambState::AddedInitials => None,
                        NameDisambState::AddedGivenName => unreachable!(),
                    }
                } else {
                    None
                }
            }
            SingleNameDisambMethod::AddInitialsThenGivenName => {
                if self.initialize_with {
                    match (self.state, self.name_form) {
                        (NameDisambState::Original, NameForm::Short) => {
                            self.state = NameDisambState::AddedInitials;
                            Some(NameDisambPass::WithFormLong)
                        }
                        (NameDisambState::Original, _) | (NameDisambState::AddedInitials, _) => {
                            self.state = NameDisambState::AddedGivenName;
                            Some(NameDisambPass::WithInitializeFalse)
                        }
                        (NameDisambState::AddedGivenName, _) => None,
                    }
                } else {
                    match self.state {
                        NameDisambState::Original => {
                            self.state = NameDisambState::AddedGivenName;
                            if self.name_form == NameForm::Short {
                                Some(NameDisambPass::WithFormLong)
                            } else {
                                None
                            }
                        }
                        NameDisambState::AddedInitials => unreachable!(),
                        NameDisambState::AddedGivenName => None,
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NameDisambPass {
    Initial,
    WithFormLong,
    WithInitializeFalse,
}

#[cfg(test)]
fn test(name: &NameEl, rule: GNDR, primary: bool) -> Vec<NameDisambPass> {
    let method = SingleNameDisambMethod::from_rule(rule, primary);
    let iter = SingleNameDisambIter::new(method, name);
    let passes: Vec<_> = iter.collect();
    passes
}

#[test]
fn test_name_disamb_iter() {
    let mut name = NameEl::root_default();
    name.form = Some(NameForm::Long); // default
    name.initialize = Some(true); // default
    assert_eq!(test(&name, GNDR::AllNames, true), vec![]);

    name.form = Some(NameForm::Short);
    assert_eq!(
        test(&name, GNDR::AllNames, true),
        vec![NameDisambPass::WithFormLong]
    );

    name.form = Some(NameForm::Short);
    assert_eq!(
        test(&name, GNDR::PrimaryName, true),
        vec![NameDisambPass::WithFormLong]
    );
    assert_eq!(test(&name, GNDR::PrimaryName, false), vec![]);
    name.initialize_with = Some(".".into());
    assert_eq!(
        test(&name, GNDR::AllNames, true),
        vec![
            NameDisambPass::WithFormLong,
            NameDisambPass::WithInitializeFalse
        ]
    );
    assert_eq!(
        test(&name, GNDR::AllNamesWithInitials, true),
        vec![NameDisambPass::WithFormLong]
    );
}

/// Original + expansions
fn add_expanded_name_to_graph(
    db: &dyn IrDatabase,
    nfa: &mut Nfa,
    mut dn: DisambNameData,
    spot: NodeIndex,
    stack: Formatting,
) -> NodeIndex {
    let style = db.style();
    let rule = style.citation.givenname_disambiguation_rule;
    let fmt = &db.get_formatter();
    let next_spot = nfa.graph.add_node(());

    // first, the original form
    let edge = dn.single_name_edge(db, stack);
    let last = add_to_graph(fmt, nfa, &RefIR::Edge(Some(edge)), spot, None);
    nfa.graph.add_edge(last, next_spot, NfaEdge::Epsilon);

    // then all the expansions the name can have
    for pass in dn.disamb_iter(rule) {
        dn.apply_upto_pass(pass);
        let first = nfa.graph.add_node(());
        nfa.start.insert(first);
        let edge = dn.single_name_edge(db, stack);
        let last = add_to_graph(fmt, nfa, &RefIR::Edge(Some(edge)), spot, None);
        nfa.graph.add_edge(last, next_spot, NfaEdge::Epsilon);
    }
    next_spot
}

/// The bool means 'is_primary'
pub(crate) type MatchKey = (Atom, NameVariable, bool);

impl DisambNameData {
    /// fullstyles_APA.txt
    /// The bool means 'is_primary'
    pub(crate) fn family_match_key(&self) -> Option<MatchKey> {
        if self.primary && self.all_same_family_name {
            Some((self.ref_id.clone(), self.var, true))
        } else {
            None
        }
    }
}

/// Matches a name against its possible disambiguated forms, each encoded as a single EdgeData.
/// Used for testing the ambiguity of names, both while doing global name disambiguation and when
/// disambiguating in other phases.
#[derive(Debug)]
pub struct NameVariantMatcher {
    edges: SmallVec<[EdgeData; 3]>,
    /// The bool means 'is_primary'
    family_match_key: Option<MatchKey>,
}

impl NameVariantMatcher {
    pub fn accepts(&self, edge: &EdgeData, match_key: Option<&MatchKey>) -> bool {
        let result = self.edges.contains(edge);
        if result
            && match_key
                .and_then(|m| self.family_match_key.as_ref().map(|me| (m, me)))
                .map_or(false, |(key, self_key)| {
                    key.0 == self_key.0
                    && key.1 == self_key.1
                    // Can't have the primary name falsely not matching itself.
                    // That would ruin primary name disambiguation in general, by under-reporting the
                    // number of matches (should be >=1!)
                    && key.2 == true
                    && self_key.2 == false
                })
        {
            log::debug!(
                "Ignored primary name clash between primary and secondary name ({:?})",
                edge
            );
            return false;
        }
        result
    }

    /// Construct from a DisambNameData, which
    pub fn from_disamb_name(db: &dyn IrDatabase, mut data: DisambNameData) -> Self {
        let style = db.style();
        let rule = style.citation.givenname_disambiguation_rule;

        let mut edges = SmallVec::new();
        let edge = data.single_name_edge(db, Formatting::default());
        edges.push(edge);
        for pass in data.disamb_iter(rule) {
            data.apply_upto_pass(pass);
            let edge = data.single_name_edge(db, Formatting::default());
            edges.push(edge);
        }
        NameVariantMatcher {
            edges,
            family_match_key: Some((data.ref_id.clone(), data.var, data.primary)),
        }
    }
}

/// Performs 'global name disambiguation'
pub fn disambiguated_person_names(
    db: &dyn IrDatabase,
) -> Arc<FnvHashMap<DisambName, NameDisambPass>> {
    let style = db.style();
    let rule = style.citation.givenname_disambiguation_rule;
    let dagn = style.citation.disambiguate_add_givenname;

    if !dagn || rule == GNDR::ByCite {
        return Arc::new(Default::default());
    }

    let dns = db.all_person_names();
    let _fmt = &db.get_formatter();
    let mut matchers = Vec::new();
    let mut results = FnvHashMap::default();

    // preamble: build all the names
    for dn in dns.iter().cloned() {
        matchers.push(NameVariantMatcher::from_disamb_name(db, dn));
    }
    let is_ambiguous = |edge: &EdgeData, same: Option<&MatchKey>| -> bool {
        let mut n = 0;
        for m in &matchers {
            let acc = m.accepts(edge, same);
            if acc {
                n += 1;
            }
            if n > 1 {
                break;
            }
        }
        n > 1
    };

    for orig in dns.iter() {
        let dn_id = db.disamb_name(orig.clone());
        let mut dn = orig.clone();
        let mut edge = dn.single_name_edge(db, Formatting::default());
        let mut iter = dn.disamb_iter(rule);
        let key = dn.family_match_key();
        let mut max_pass = NameDisambPass::Initial;
        while is_ambiguous(&edge, key.as_ref()) {
            if let Some(pass) = iter.next() {
                dn.apply_upto_pass(pass);
                max_pass = pass;
                edge = dn.single_name_edge(db, Formatting::default());
            } else {
                max_pass = NameDisambPass::Initial;
                break;
            }
        }
        if max_pass > NameDisambPass::Initial {
            results.insert(dn_id, max_pass);
        }
    }
    Arc::new(results)
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RefNameIR {
    pub variable: NameVariable,
    pub disamb_name_ids: Vec<DisambName>,
}

impl RefNameIR {
    fn from_name_ir<O: OutputFormat>(name_ir: &NameIR<O>) -> Self {
        let mut vec = Vec::with_capacity(name_ir.disamb_names.len());
        for dnr in &name_ir.disamb_names {
            if let DisambNameRatchet::Person(PersonDisambNameRatchet { id, .. }) = dnr {
                vec.push(*id);
            }
            // TODO: do it for literals as well
        }
        RefNameIR {
            variable: name_ir.variable,
            disamb_name_ids: vec,
        }
    }
    pub fn count(&self) -> u32 {
        500
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisambNameRatchet<B> {
    Literal { literal: B, is_latin_cyrillic: bool },
    Person(PersonDisambNameRatchet),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonDisambNameRatchet {
    pub id: DisambName,
    pub data: DisambNameData,
    pub iter: SingleNameDisambIter,
}

impl PersonDisambNameRatchet {
    pub fn new(style: &Style, id: DisambName, data: DisambNameData) -> Self {
        let rule = style.citation.givenname_disambiguation_rule;
        let iter = data.disamb_iter(rule);
        // debug!("{} ratchet started with state {:?}", &data.ref_id, iter);
        PersonDisambNameRatchet { id, iter, data }
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct NameCounter {
    /// How much to increase ea_use_first by
    pub bump: u16,
    /// Max recorded number of names rendered
    pub max_recorded: u16,
    ///
    pub current: u16,
}

use crate::NamesInheritance;

/// The full Names block has-many NameIRs when it is rendered. Each NameIR represents one variable
/// to be rendered in a Names block. So each NameIR can have multiple actual people's names in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameIR<O: OutputFormat> {
    // has IR children

    // TODO: make most fields private
    pub names_inheritance: NamesInheritance,

    variable: NameVariable,
    label_variable: NameVariable,

    pub name_counter: NameCounter,
    achieved_at: (u16, NameCounter),

    pub disamb_names: Vec<DisambNameRatchet<O::Build>>,
    pub built_label: Option<O::Build>,

    // These three avoid having to pass in style & locale every time you want to recompute the IR
    // or make name tokens.
    pub demote_non_dropping_particle: DemoteNonDroppingParticle,
    pub initialize_with_hyphen: bool,
    pub etal_term: Option<(SmartString, Option<Formatting>)>,
    pub and_term: Option<SmartString>,
}

impl<O> NameIR<O>
where
    O: OutputFormat,
{
    pub fn new<I: OutputFormat>(
        gen_ctx: &GenericContext<'_, O, I>,
        names_inheritance: NamesInheritance,
        variable: NameVariable,
        label_variable: NameVariable,
        ratchets: Vec<DisambNameRatchet<O::Build>>,
        style: &Style,
        etal_term: Option<(SmartString, Option<Formatting>)>,
        and_term: Option<SmartString>,
    ) -> Self {
        let built_label = names_inheritance.label.as_ref().and_then(|label| {
            let renderer = Renderer::gen(gen_ctx.clone());
            renderer.name_label(&label.concrete(), variable, label_variable)
        });
        NameIR {
            names_inheritance,
            variable,
            label_variable,
            disamb_names: ratchets,
            name_counter: NameCounter::default(),
            achieved_at: (std::u16::MAX, NameCounter::default()),
            demote_non_dropping_particle: style.demote_non_dropping_particle,
            initialize_with_hyphen: style.initialize_with_hyphen,
            etal_term,
            and_term,
            built_label,
        }
    }

    pub fn achieved_count(&mut self, count: u16) {
        let (prev_best, _at) = self.achieved_at;
        if count < prev_best {
            self.achieved_at = (count, self.name_counter);
        }
    }
    pub fn rollback(
        &mut self,
        _db: &dyn IrDatabase,
        ctx: &CiteContext<'_, O>,
    ) -> Option<Vec<O::Build>> {
        let (_prev_best, at) = self.achieved_at;
        if self.name_counter.bump > at.bump {
            info!("{:?} rolling back to {:?} names", ctx.cite_id, at);
        }
        self.name_counter = at;
        self.intermediate_custom(
            &ctx.format,
            ctx.position.0,
            ctx.sort_key.is_some(),
            None,
            None,
        )
    }

    // returns false if couldn't add any more names
    pub fn add_name(
        &mut self,
        _db: &dyn IrDatabase,
        ctx: &CiteContext<'_, O>,
    ) -> Option<Vec<O::Build>> {
        self.name_counter.bump += 1;
        self.intermediate_custom(
            &ctx.format,
            ctx.position.0,
            ctx.sort_key.is_some(),
            Some(DisambPass::AddNames),
            None,
        )
    }

    pub fn subsequent_author_substitute(
        &mut self,
        fmt: &O,
        subst_count: u32,
        replace_with: &str,
    ) -> Option<Vec<O::Build>> {
        // We will only need to replace the first element, since names only ever get one child.
        // But keep the arena separate from this.
        self.intermediate_custom(
            fmt,
            Position::First,
            false,
            None,
            Some((subst_count, replace_with)),
        )
    }
}

/// Useful since names blocks only ever have an IrSeq under them.
/// (Except when doing subsequent-author-substitute, but that's after suppression.)
pub fn replace_single_child<O: OutputFormat>(
    of_node: NodeId,
    with: NodeId,
    arena: &mut IrArena<O>,
) {
    if let Some(existing) = of_node.children(arena).next() {
        existing.remove_subtree(arena);
    }
    of_node.append(with, arena);
}

impl<O: OutputFormat> IrTree<O> {
    /// Useful since names blocks only ever have an IrSeq under them.
    /// (Except when doing subsequent-author-substitute, but that's after suppression.)
    pub fn replace_single_child(&mut self, of_node: NodeId, with: NodeId) {
        if let Some(existing) = of_node.children(&self.arena).next() {
            existing.remove_subtree(&mut self.arena);
        }
        of_node.append(with, &mut self.arena);
    }
}
