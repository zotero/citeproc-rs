use super::finite_automata::{Dfa, Nfa, NfaEdge};
use super::mult_identity;
use crate::names::OneNameVar;
use crate::prelude::*;
use citeproc_io::PersonName;
use csl::style::{Citation, GivenNameDisambiguationRule, Name as NameEl, NameForm, Names, Style};
use csl::variables::NameVariable;
use csl::Atom;
use fnv::FnvHashMap;
use std::sync::Arc;

impl Disambiguation<Markup> for Names {
    fn get_free_conds(&self, db: &impl IrDatabase) -> FreeCondSets {
        // TODO: Position may be involved for NASO and primary disambiguation
        // TODO: drill down into the substitute logic here
        if let Some(subst) = &self.substitute {
            cross_product(db, &subst.0)
        } else {
            mult_identity()
        }
    }
    fn ref_ir(
        &self,
        db: &impl IrDatabase,
        ctx: &RefContext<Markup>,
        stack: Formatting,
    ) -> (RefIR, GroupVars) {
        let fmt = ctx.format;
        let style = ctx.style;
        let locale = ctx.locale;
        let name_el = db
            .name_citation()
            .merge(self.name.as_ref().unwrap_or(&NameEl::default()));
        let runner = OneNameVar {
            name_el: &name_el,
            demote_non_dropping_particle: style.demote_non_dropping_particle,
            initialize_with_hyphen: style.initialize_with_hyphen,
            fmt,
        };

        let mut any = false;
        let mut nfa = Nfa::new();
        let mut irs = Vec::new();
        for var in &self.variables {
            if let Some(vec_of_names) = ctx.reference.name.get(var) {
                any = true;
                let iter = runner
                    .names_to_builds(vec_of_names, ctx.position, ctx.locale, &self.et_al)
                    .into_iter()
                    .filter(|x| !fmt.is_empty(&x))
                    .map(|x| fmt.output_in_context(x, stack))
                    .map(|x| db.edge(EdgeData::Output(x)))
                    .map(|e| RefIR::Edge(Some(e)));
                let seq = RefIrSeq {
                    contents: iter.collect(),
                    formatting: runner.name_el.formatting,
                    affixes: runner.name_el.affixes.clone(),
                    // delimiter is built-in
                    delimiter: Atom::from(""),
                };
                if !seq.contents.is_empty() {
                    irs.push(RefIR::Seq(seq));
                }
            }
        }

        if irs.is_empty() {
            // null object pattern
            let start = nfa.graph.add_node(());
            nfa.start.insert(start);
            nfa.accepting.insert(start);
            // TODO: substitute
            // TODO: suppress once substituted
            return (RefIR::Edge(None), GroupVars::OnlyEmpty);
        }

        let seq = RefIR::Seq(RefIrSeq {
            contents: irs,
            formatting: self.formatting,
            affixes: self.affixes.clone(),
            delimiter: self
                .delimiter
                .as_ref()
                .map(|d| d.0.clone())
                .unwrap_or_else(|| Atom::from("")),
        });

        use super::add_to_graph;

        let mut spot = nfa.graph.add_node(());
        nfa.start.insert(spot);
        spot = add_to_graph(db, fmt, &mut nfa, &seq, spot);
        nfa.accepting.insert(spot);

        // TODO: rerun with more names etc

        (
            RefIR::Names(nfa, Box::new(RefIR::Edge(None))),
            GroupVars::DidRender,
        )
    }
}

citeproc_db::intern_key!(pub DisambName);
impl DisambName {
    pub fn lookup(&self, db: &impl IrDatabase) -> Arc<DisambNameData> {
        db.lookup_disamb_name(*self)
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
    pub(crate) fn single_name_ref_ir(
        &self,
        db: &impl IrDatabase,
        fmt: &Markup,
        style: &Style,
        stack: Formatting,
    ) -> RefIR {
        let val = Some(());
        let edge = val
            .map(|val| {
                let builder = OneNameVar {
                    fmt,
                    name_el: &self.el,
                    demote_non_dropping_particle: style.demote_non_dropping_particle,
                    initialize_with_hyphen: style.initialize_with_hyphen,
                };
                builder.render_person_name(
                    &self.value,
                    // XXX: seen_one?
                    false,
                )
            })
            .map(|x| fmt.output_in_context(x, stack))
            .map(|o| db.edge(EdgeData::Output(o)));
        RefIR::Edge(edge)
    }

    pub(crate) fn single_name_ir(
        &self,
        db: &impl IrDatabase,
        fmt: &Markup,
        style: &Style,
        stack: Formatting,
    ) -> IR {
        let val = Some(());
        let val = {
            let builder = OneNameVar {
                fmt,
                name_el: &self.el,
                demote_non_dropping_particle: style.demote_non_dropping_particle,
                initialize_with_hyphen: style.initialize_with_hyphen,
            };
            builder.render_person_name(
                &self.value,
                // XXX: seen_one?
                false,
            )
        };
        IR::Rendered(Some(CiteEdgeData::Output(val)))
    }

    pub fn disamb_iter(&self, rule: GivenNameDisambiguationRule) -> SingleNameDisambIter {
        let method = SingleNameDisambMethod::from_rule(rule, self.primary);
        SingleNameDisambIter::new(method, &self.el)
    }
}

/// The GivenNameDisambiguationRule variants are poorly worded. "-with-initials" doesn't *add*
/// steps, it removes steps / limits the expansion. This is a bit clearer to work with, and mixes
/// in the information about whether a name is primary or not.
#[derive(Clone, Copy, Debug)]
enum SingleNameDisambMethod {
    None,
    AddInitials,
    AddInitialsThenGivenName,
}

impl SingleNameDisambMethod {
    /// `is_primary` refers to whether this is the first name to be rendered in a Names element.
    fn from_rule(rule: GivenNameDisambiguationRule, is_primary: bool) -> Self {
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

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug)]
enum NameDisambState {
    Original,
    AddedInitials,
    AddedGivenName,
}

impl SingleNameDisambIter {
    fn new(method: SingleNameDisambMethod, name_options: &NameEl) -> Self {
        SingleNameDisambIter {
            method,
            initialize_with: name_options.initialize_with.is_some()
                && name_options.initialize == Some(true),
            name_form: name_options.form.unwrap_or(NameForm::Long),
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
    let mut iter = SingleNameDisambIter::new(method, name);
    let passes: Vec<_> = iter.collect();
    passes
}

#[test]
fn test_name_disamb_iter() {
    let mut name = NameEl::default();
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
    db: &impl IrDatabase,
    nfa: &mut Nfa,
    mut dn: DisambNameData,
    spot: NodeIndex,
) -> NodeIndex {
    let style = db.style();
    let rule = style.citation.givenname_disambiguation_rule;
    let fmt = &db.get_formatter();
    let ir = dn.single_name_ref_ir(
        db,
        fmt,
        &style,
        /* TODO: store format stck with DND */ Formatting::default(),
    );
    let next_spot = nfa.graph.add_node(());
    let last = add_to_graph(db, fmt, nfa, &ir, spot);
    nfa.graph.add_edge(last, next_spot, NfaEdge::Epsilon);
    for pass in dn.disamb_iter(rule) {
        dn.apply_pass(pass);
        let first = nfa.graph.add_node(());
        nfa.start.insert(first);
        let ir = dn.single_name_ref_ir(
            db,
            fmt,
            &style,
            /* TODO: store format stck with DND */ Formatting::default(),
        );
        let last = add_to_graph(db, fmt, nfa, &ir, spot);
        nfa.graph.add_edge(last, next_spot, NfaEdge::Epsilon);
    }
    next_spot
}

pub fn disambiguated_person_names(db: &impl IrDatabase) -> Arc<FnvHashMap<DisambName, IR<Markup>>> {
    let dns = db.all_person_names();
    let style = db.style();
    let fmt = &db.get_formatter();
    let rule = style.citation.givenname_disambiguation_rule;
    let mut dfas = Vec::new();
    let mut results = FnvHashMap::default();
    use crate::disamb::add_to_graph;
    use crate::disamb::names::{NameDisambPass, SingleNameDisambIter, SingleNameDisambMethod};
    use csl::style::NameForm;

    // preamble: build all the names
    for dn in dns.iter() {
        let dn: DisambNameData = (*dn.lookup(db)).clone();
        let mut nfa = Nfa::new();
        let first = nfa.graph.add_node(());
        nfa.start.insert(first);
        let last = add_expanded_name_to_graph(db, &mut nfa, dn, first);
        nfa.accepting.insert(last);
        let dfa = nfa.brzozowski_minimise();
        debug! {"{}", dfa.debug_graph(db)};
        dfas.push(dfa);
    }
    let is_ambiguous = |edges: &[EdgeData]| -> bool {
        let mut n = 0;
        for dfa in &dfas {
            let acc = dfa.accepts_data(db, &edges);
            if acc {
                n += 1;
            }
            if n > 1 {
                break;
            }
        }
        debug!("had edges: {:?}", &edges);
        debug!("is it ambiguous: n = {}", n);
        n > 1
    };
    // round 1:
    for &dn_id in dns.iter() {
        let mut dn: DisambNameData = (*dn_id.lookup(db)).clone();
        let mut ir = dn.single_name_ir(
            db,
            fmt,
            &style,
            /* TODO: store format stack */ Formatting::default(),
        );
        let mut edges = ir.to_edge_stream(fmt);
        let mut iter = dn.disamb_iter(rule);
        while is_ambiguous(&edges) {
            if let Some(pass) = iter.next() {
                dn.apply_pass(pass);
                ir = dn.single_name_ir(db, fmt, &style, Formatting::default());
                edges = ir.to_edge_stream(fmt);
            } else {
                break;
            }
        }
        results.insert(dn_id, ir);
    }
    warn!("disambiguated_person_names {:#?}", &results);
    Arc::new(results)
}
