use super::add_to_graph;
use super::finite_automata::{Nfa, NfaEdge};
use super::graph_with_stack;
use crate::names::{NameTokenBuilt, OneNameVar};
use crate::prelude::*;
use citeproc_io::PersonName;
use csl::variables::*;
use csl::{
    Atom, DemoteNonDroppingParticle, GivenNameDisambiguationRule, Name as NameEl, NameForm, Names,
    Position, Style,
};
use fnv::FnvHashMap;
use petgraph::graph::NodeIndex;
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

        let and_term = locale.and_term(None).map(|x| x.to_owned());
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
            delimiter: names_inheritance
                .delimiter
                .clone()
                .unwrap_or_else(|| Atom::from("")),
            ..Default::default()
        };

        let gen = GenericContext::<Markup, Markup>::Ref(ctx);
        let name_irs =
            crate::names::to_individual_name_irs(&gen, self, &names_inheritance, db, state, false);
        for nir in name_irs {
            let mut nfa = Nfa::new();
            let start = nfa.graph.add_node(());
            nfa.start.insert(start);
            let (ntbs, mut ntb_len) = runner.names_to_builds(
                &nir.disamb_names,
                ctx.position,
                &self.et_al,
                false,
                and_term.as_ref(),
                etal_term.as_ref(),
            );
            // We need to use this a couple of times.
            let ntbs = ntbs.collect::<Vec<_>>();
            let mut max_counted_tokens = 0u16;
            let mut counted_tokens = ntb_len;

            while counted_tokens > max_counted_tokens {
                let one_run = graph_with_stack(
                    db,
                    fmt,
                    &mut nfa,
                    runner.name_el.formatting,
                    runner.name_el.affixes.as_ref(),
                    start,
                    |nfa, mut spot| {
                        for ntb in &ntbs {
                            match ntb {
                                NameTokenBuilt::Built(b) => {
                                    if !fmt.is_empty(&b) {
                                        let out =
                                            fmt.output_in_context(b.to_vec(), child_stack, None);
                                        let e = db.edge(EdgeData::Output(out));
                                        let ir = RefIR::Edge(Some(e));
                                        spot = add_to_graph(db, fmt, nfa, &ir, spot);
                                    }
                                }
                                NameTokenBuilt::Ratchet(index) => match &nir.disamb_names[*index] {
                                    DisambNameRatchet::Literal(b) => {
                                        if !fmt.is_empty(b) {
                                            let out =
                                                fmt.output_in_context(b.clone(), child_stack, None);
                                            let e = db.edge(EdgeData::Output(out));
                                            let ir = RefIR::Edge(Some(e));
                                            spot = add_to_graph(db, fmt, nfa, &ir, spot);
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
                        spot
                    },
                );
                if one_run == start {
                    // XXX: not sure about this
                    continue;
                }
                nfa.accepting.insert(one_run);
                runner.bump_name_count += 1;
                let (ntbs, ntb_len) = runner.names_to_builds(
                    &nir.disamb_names,
                    ctx.position,
                    &self.et_al,
                    false,
                    and_term.as_ref(),
                    etal_term.as_ref(),
                );
                max_counted_tokens = counted_tokens;
                counted_tokens = ntb_len;
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct DisambNameData {
    pub ref_id: Atom,
    pub var: NameVariable,
    pub el: NameEl,
    pub value: PersonName,
    pub primary: bool,
}

impl DisambNameData {
    pub fn apply_pass(&mut self, pass: NameDisambPass) {
        match pass {
            NameDisambPass::WithFormLong => self.el.form = Some(NameForm::Long),
            NameDisambPass::WithInitializeFalse => self.el.initialize = Some(false),
        }
    }

    /// This is used directly for *global name disambiguation*
    pub(crate) fn single_name_edge(&self, db: &dyn IrDatabase, stack: Formatting) -> Edge {
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
        db.edge(EdgeData::Output(o))
    }

    pub fn disamb_iter(&self, rule: GivenNameDisambiguationRule) -> SingleNameDisambIter {
        let method = SingleNameDisambMethod::from_rule(rule, self.primary);
        SingleNameDisambIter::new(method, &self.el)
    }
}

/// The GivenNameDisambiguationRule variants are poorly worded. "-with-initials" doesn't *add*
/// steps, it removes steps / limits the expansion. This is a bit clearer to work with, and mixes
/// in the information about whether a name is primary or not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SingleNameDisambMethod {
    None,
    AddInitials,
    AddInitialsThenGivenName,
}

impl SingleNameDisambMethod {
    /// `is_primary` refers to whether this is the first name to be rendered in a Names element.
    pub fn from_rule(rule: GivenNameDisambiguationRule, is_primary: bool) -> Self {
        match (rule, is_primary) {
            (GivenNameDisambiguationRule::ByCite, _)
            | (GivenNameDisambiguationRule::AllNames, _) => {
                SingleNameDisambMethod::AddInitialsThenGivenName
            }
            (GivenNameDisambiguationRule::AllNamesWithInitials, _) => {
                SingleNameDisambMethod::AddInitials
            }
            (GivenNameDisambiguationRule::PrimaryName, true) => {
                SingleNameDisambMethod::AddInitialsThenGivenName
            }
            (GivenNameDisambiguationRule::PrimaryNameWithInitials, true) => {
                SingleNameDisambMethod::AddInitials
            }
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
    pub fn new(method: SingleNameDisambMethod, name_el: &NameEl) -> Self {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameDisambPass {
    WithFormLong,
    WithInitializeFalse,
}

#[cfg(test)]
fn test(name: &NameEl, rule: GivenNameDisambiguationRule, primary: bool) -> Vec<NameDisambPass> {
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
    assert_eq!(
        test(&name, GivenNameDisambiguationRule::AllNames, true),
        vec![]
    );

    name.form = Some(NameForm::Short);
    assert_eq!(
        test(&name, GivenNameDisambiguationRule::AllNames, true),
        vec![NameDisambPass::WithFormLong]
    );

    name.form = Some(NameForm::Short);
    assert_eq!(
        test(&name, GivenNameDisambiguationRule::PrimaryName, true),
        vec![NameDisambPass::WithFormLong]
    );
    assert_eq!(
        test(&name, GivenNameDisambiguationRule::PrimaryName, false),
        vec![]
    );
    name.initialize_with = Some(Atom::from("."));
    assert_eq!(
        test(&name, GivenNameDisambiguationRule::AllNames, true),
        vec![
            NameDisambPass::WithFormLong,
            NameDisambPass::WithInitializeFalse
        ]
    );
    assert_eq!(
        test(
            &name,
            GivenNameDisambiguationRule::AllNamesWithInitials,
            true
        ),
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
    let edge = dn.single_name_edge(db, stack);
    let next_spot = nfa.graph.add_node(());
    let last = add_to_graph(db, fmt, nfa, &RefIR::Edge(Some(edge)), spot);
    nfa.graph.add_edge(last, next_spot, NfaEdge::Epsilon);
    for pass in dn.disamb_iter(rule) {
        dn.apply_pass(pass);
        let first = nfa.graph.add_node(());
        nfa.start.insert(first);
        let edge = dn.single_name_edge(db, stack);
        let last = add_to_graph(db, fmt, nfa, &RefIR::Edge(Some(edge)), spot);
        nfa.graph.add_edge(last, next_spot, NfaEdge::Epsilon);
    }
    next_spot
}

use smallvec::SmallVec;
pub struct NameVariantMatcher(SmallVec<[Edge; 3]>);

impl NameVariantMatcher {
    pub fn accepts(&self, edge: Edge) -> bool {
        self.0.contains(&edge)
    }

    pub fn from_disamb_name(db: &dyn IrDatabase, dn: DisambName) -> Self {
        let style = db.style();
        let _fmt = &db.get_formatter();
        let rule = style.citation.givenname_disambiguation_rule;

        let mut data: DisambNameData = dn.lookup(db);
        let iter = data.disamb_iter(rule);
        let mut edges = SmallVec::new();
        let edge = data.single_name_edge(db, Formatting::default());
        edges.push(edge);
        for pass in iter {
            data.apply_pass(pass);
            let edge = data.single_name_edge(db, Formatting::default());
            edges.push(edge);
        }
        NameVariantMatcher(edges)
    }
}

/// Performs 'global name disambiguation'
pub fn disambiguated_person_names(
    db: &dyn IrDatabase,
) -> Arc<FnvHashMap<DisambName, DisambNameData>> {
    let style = db.style();
    let rule = style.citation.givenname_disambiguation_rule;
    let dagn = style.citation.disambiguate_add_givenname;

    if !dagn || rule == GivenNameDisambiguationRule::ByCite {
        return Arc::new(Default::default());
    }

    let dns = db.all_person_names();
    let _fmt = &db.get_formatter();
    let mut matchers = Vec::new();
    let mut results = FnvHashMap::default();

    // preamble: build all the names
    for &dn in dns.iter() {
        matchers.push(NameVariantMatcher::from_disamb_name(db, dn));
    }
    let is_ambiguous = |edge: Edge| -> bool {
        let mut n = 0;
        for m in &matchers {
            let acc = m.accepts(edge);
            if acc {
                n += 1;
            }
            if n > 1 {
                break;
            }
        }
        n > 1
    };

    for &dn_id in dns.iter() {
        let mut dn: DisambNameData = dn_id.lookup(db);
        let mut edge = dn.single_name_edge(db, Formatting::default());
        let mut iter = dn.disamb_iter(rule);
        while is_ambiguous(edge) {
            if let Some(pass) = iter.next() {
                dn.apply_pass(pass);
                edge = dn.single_name_edge(db, Formatting::default());
            } else {
                // failed, so we must reset
                dn = dn_id.lookup(db);
                break;
            }
        }
        results.insert(dn_id, dn);
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
    Literal(B),
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
        let method = SingleNameDisambMethod::from_rule(rule, data.primary);
        let iter = SingleNameDisambIter::new(method, &data.el);
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
    pub etal_term: Option<(String, Option<Formatting>)>,
    pub and_term: Option<String>,
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
        etal_term: Option<(String, Option<Formatting>)>,
        and_term: Option<String>,
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
        db: &dyn IrDatabase,
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
        db: &dyn IrDatabase,
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
