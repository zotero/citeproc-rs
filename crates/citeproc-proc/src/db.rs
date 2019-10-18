// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use fnv::FnvHashMap;
use std::sync::Arc;

use crate::disamb::{Dfa, DisambName, DisambNameData, Edge, EdgeData, FreeCondSets};
use crate::prelude::*;
use crate::{CiteContext, DisambPass, IrState, Proc, IR};
use citeproc_io::output::{markup::Markup, OutputFormat};
use citeproc_io::{Cite, ClusterId, Name};
use csl::style::Position;
use csl::variables::NameVariable;
use csl::Atom;

use parking_lot::{Mutex, MutexGuard};

pub trait HasFormatter {
    fn get_formatter(&self) -> Markup;
}

#[allow(dead_code)]
type MarkupBuild = <Markup as OutputFormat>::Build;
#[allow(dead_code)]
type MarkupOutput = <Markup as OutputFormat>::Output;

#[salsa::query_group(IrDatabaseStorage)]
pub trait IrDatabase: CiteDatabase + LocaleDatabase + StyleDatabase + HasFormatter {
    fn ref_dfa(&self, key: Atom) -> Option<Arc<Dfa>>;

    // TODO: cache this
    // #[salsa::invoke(crate::disamb::create_ref_ir)]
    // fn ref_ir(&self, key: Atom) -> Arc<Vec<(FreeCond, RefIR)>>;

    // If these don't run any additional disambiguation, they just clone the
    // previous ir's Arc.
    fn ir_gen0(&self, key: CiteId) -> Arc<IrGen>;
    fn ir_gen1_add_names(&self, key: CiteId) -> Arc<IrGen>;
    fn ir_gen2_add_given_name(&self, key: CiteId) -> Arc<IrGen>;
    fn year_suffixes(&self) -> Arc<FnvHashMap<Atom, u32>>;
    fn year_suffix_for(&self, ref_id: Atom) -> Option<u32>;
    fn ir_gen3_add_year_suffix(&self, key: CiteId) -> Arc<IrGen>;
    fn ir_gen4_conditionals(&self, key: CiteId) -> Arc<IrGen>;
    fn built_cluster(&self, key: ClusterId) -> Arc<MarkupOutput>;

    fn bib_item_gen0(&self, ref_id: Atom) -> Option<Arc<IrGen>>;
    fn bib_item(&self, ref_id: Atom) -> Arc<MarkupOutput>;

    fn branch_runs(&self) -> Arc<FreeCondSets>;

    fn all_person_names(&self) -> Arc<Vec<DisambName>>;

    /// The *Data indexed here are ratcheted as far as was required to do global name
    /// disambiguation.
    #[salsa::invoke(crate::disamb::names::disambiguated_person_names)]
    fn disambiguated_person_names(&self) -> Arc<FnvHashMap<DisambName, DisambNameData>>;

    /// The DisambNameData here correspond to "global identity" -- so each DisambName points to
    /// exactly one Ref/NameEl/Variable/PersonName. Even if there are two identical NameEls
    /// rendering the same name, that's fine, because they would each have the same global
    /// disambiguation done.
    ///
    /// After global disambiguation, any modifications to DisambNameData are stored within the IR.
    #[salsa::interned]
    fn disamb_name(&self, e: DisambNameData) -> DisambName;

    #[salsa::interned]
    fn edge(&self, e: EdgeData) -> Edge;
}

fn all_person_names(db: &impl IrDatabase) -> Arc<Vec<DisambName>> {
    let _style = db.style();
    let name_configurations = db.name_configurations();
    let refs = db.disamb_participants();
    let mut collector = Vec::new();
    // -> for each ref
    //    for each <names var="v" />
    //    for each name in ref["v"]
    //    .. push a DisambName
    for ref_id in refs.iter() {
        if let Some(refr) = db.reference(ref_id.clone()) {
            for (var, el) in name_configurations.iter() {
                if let Some(names) = refr.name.get(&var) {
                    let mut seen_one = false;
                    for name in names {
                        if let Name::Person(val) = name {
                            collector.push(db.disamb_name(DisambNameData {
                                ref_id: ref_id.clone(),
                                var: *var,
                                el: el.clone(),
                                value: val.clone(),
                                primary: !seen_one,
                            }))
                        }
                        seen_one = true;
                    }
                }
            }
        }
    }
    Arc::new(collector)
}

use crate::disamb::create_dfa;

fn ref_dfa<DB: IrDatabase>(db: &DB, key: Atom) -> Option<Arc<Dfa>> {
    if let Some(refr) = db.reference(key) {
        Some(Arc::new(create_dfa::<Markup, DB>(db, &refr)))
    } else {
        None
    }
}

fn branch_runs(db: &impl IrDatabase) -> Arc<FreeCondSets> {
    let style = db.style();
    Arc::new(style.get_free_conds(db))
}

fn year_suffix_for(db: &impl IrDatabase, ref_id: Atom) -> Option<u32> {
    let ys = db.year_suffixes();
    ys.get(&ref_id).cloned()
}

/// This deviates from citeproc-js in one important way.
///
/// Since there are no 'groups of ambiguous cites', it is not quite simple
/// to have separate numbering for different such 'groups'.
///
///               `Doe 2007,  Doe 2007,  Smith 2008,  Smith 2008`
/// should become `Doe 2007a, Doe 2007b, Smith 2008a, Smith 2008b`
///
/// The best way to do this is:
///
/// 1. Store the set of `refs_accepting_cite`
/// 2. Find the distinct transitive closures of the `A.refs intersects B.refs` relation
///    a. Groups = {}
///    b. For each cite A with more than its own, find, if any, a Group whose total refs intersects A.refs
///    c. If found G, add A to that group, and G.total_refs = G.total_refs UNION A.refs
fn year_suffixes(db: &impl IrDatabase) -> Arc<FnvHashMap<Atom, u32>> {
    use fnv::FnvHashSet;

    type Group = FnvHashSet<Atom>;

    let mut groups: Vec<_> = db
        .all_keys()
        .iter()
        .cloned()
        .map(|i| {
            let mut s = FnvHashSet::default();
            s.insert(i);
            s
        })
        .collect();

    // equivalent to `!self.is_disjoint(other)` from std, but with earlier exit
    // enumerating lists results in less allocation than converting Vec to HashSet every time
    fn intersects(set: &FnvHashSet<Atom>, list: &[Atom]) -> bool {
        if set.len() <= list.len() {
            set.iter().any(|v| list.contains(v))
        } else {
            list.iter().any(|v| set.contains(v))
        }
    }

    let style = db.style();
    if !style.citation.disambiguate_add_year_suffix {
        return Arc::new(FnvHashMap::default());
    }

    use std::mem;

    // TODO: sort based on the bibliography's sort mechanism, not just document order
    let all_cites_ordered = db.all_cite_ids();
    all_cites_ordered
        .iter()
        .map(|id| (*id, id.lookup(db)))
        .map(|(id, cite)| (cite.ref_id.clone(), db.ir_gen2_add_given_name(id)))
        .for_each(|(ref_id, ir2)| {
            if ir2.unambiguous() {
                // no need to check if own id is in a group, it will receive a suffix already
            } else {
                // we make sure ref_id is included, even if there was a bug with RefIR and a
                // cite didn't match its own reference
                let mut coalesce: Option<(usize, Group)> = None;
                for (n, group) in groups.iter_mut().enumerate() {
                    if group.contains(&ref_id) || intersects(group, &ir2.matching_refs) {
                        group.insert(ref_id.clone());
                        for id in &ir2.matching_refs {
                            group.insert(id.clone());
                        }
                        if let Some((_n, ref mut already)) = coalesce {
                            let g = mem::replace(group, FnvHashSet::default());
                            *already = already.intersection(&g).cloned().collect();
                        } else {
                            // Move it cheaply out of the iterator to add to it later
                            let g = mem::replace(group, FnvHashSet::default());
                            coalesce = Some((n, g));
                        }
                    }
                }
                if let Some((n, group)) = coalesce {
                    groups[n] = group;
                }
                groups.retain(|x| !x.is_empty());
            }
        });

    let mut suffixes = FnvHashMap::default();
    let mut vec = Vec::new();
    for group in groups {
        vec.clear();
        if group.len() <= 1 {
            continue;
        }
        for atom in group {
            vec.push(atom);
        }
        vec.sort_by_key(|ref_id| ref_bib_number(db, ref_id));
        let mut i = 1; // "a" = 1
        for ref_id in &vec {
            if !suffixes.contains_key(ref_id) {
                suffixes.insert(ref_id.clone(), i);
                i += 1;
            }
        }
    }
    Arc::new(suffixes)
}

// Not cached
fn ref_bib_number(db: &impl CiteDatabase, ref_id: &Atom) -> u32 {
    let srs = db.sorted_refs();
    let (_, ref lookup_ref_ids) = &*srs;
    let ret = lookup_ref_ids.get(ref_id).cloned();
    if let Some(ret) = ret {
        ret
    } else {
        error!(
            "called ref_bib_number on a ref_id {} that is unknown/not in the bibliography",
            ref_id
        );
        std::u32::MAX
    }
}

#[derive(Debug, Clone)]
pub struct IrGen {
    pub(crate) ir: IR<Markup>,
    pub(crate) state: IrState,
    pub(crate) matching_refs: Vec<Atom>,
}

impl IrGen {
    fn new(ir: IR<Markup>, matching_refs: Vec<Atom>, state: IrState) -> Self {
        IrGen {
            ir,
            state,
            matching_refs,
        }
    }
    fn unambiguous(&self) -> bool {
        self.matching_refs.len() <= 1
    }
    fn fresh_copy(&self) -> (IR<Markup>, IrState) {
        let ir = self.ir.clone();
        let state = self.state.clone();
        (ir, state)
    }
}

impl Eq for IrGen {}
impl PartialEq<IrGen> for IrGen {
    fn eq(&self, other: &Self) -> bool {
        self.matching_refs == other.matching_refs
            && &self.state == &other.state
            && &self.ir == &other.ir
    }
}

fn ref_not_found(db: &impl IrDatabase, ref_id: &Atom, log: bool) -> Arc<IrGen> {
    if log {
        eprintln!("citeproc-rs: reference {} not found", ref_id);
    }
    Arc::new(IrGen::new(
        IR::Rendered(Some(CiteEdgeData::Output(db.get_formatter().plain("???")))),
        Vec::new(),
        IrState::new(),
    ))
}

macro_rules! preamble {
    ($style:ident, $locale:ident, $cite:ident, $refr:ident, $ctx:ident, $db:expr, $id:expr, $pass:expr) => {{
        $style = $db.style();
        $locale = $db.locale_by_cite($id);
        $cite = $id.lookup($db);
        $refr = match $db.reference($cite.ref_id.clone()) {
            None => return ref_not_found($db, &$cite.ref_id, true),
            Some(r) => r,
        };
        let ni = $style.names_delimiter.clone();
        let citation_ni = $style.citation.names_delimiter.clone();
        $ctx = CiteContext {
            reference: &$refr,
            format: $db.get_formatter(),
            cite_id: Some($id),
            cite: &$cite,
            position: $db.cite_position($id),
            citation_number: 0,
            disamb_pass: $pass,
            style: &$style,
            locale: &$locale,
            bib_number: $db.bib_number($id),
            name_citation: $db.name_citation(),
            in_bibliography: false,
            names_delimiter: citation_ni.or(ni),
        };
    }};
}

fn disambiguate(
    db: &impl IrDatabase,
    ir: &mut IR<Markup>,
    state: &mut IrState,
    ctx: &mut CiteContext<Markup>,
    _maybe_ys: Option<&FnvHashMap<Atom, u32>>,
    own_id: &Atom,
) -> bool {
    let mut un = is_unambiguous(db, ctx.disamb_pass, ir, ctx.cite_id, own_id);
    info!(
        "{:?} was {}ambiguous in pass {:?}",
        ctx.cite_id,
        if un { "un" } else { "" },
        ctx.disamb_pass
    );
    // disambiguate returns true if it can do more for this DisambPass (i.e. more names to add)
    let mut applied_suffix = false;
    while !un && ir.disambiguate(db, state, ctx, &mut applied_suffix) {
        un = is_unambiguous(db, ctx.disamb_pass, ir, ctx.cite_id, own_id);
    }
    if !un {
        un = is_unambiguous(db, ctx.disamb_pass, ir, ctx.cite_id, own_id);
    }
    un
}

fn is_unambiguous(
    db: &impl IrDatabase,
    pass: Option<DisambPass>,
    ir: &IR<Markup>,
    cite_id: Option<CiteId>,
    own_id: &Atom,
) -> bool {
    use log::Level::{Info, Warn};
    let edges = ir.to_edge_stream(&db.get_formatter());
    let mut n = 0;
    for k in db.cited_keys().iter() {
        let dfa = db.ref_dfa(k.clone()).expect("cited_keys should all exist");
        let acc = dfa.accepts_data(db, &edges);
        if acc {
            n += 1;
        }
        if n > 1 {
            break;
        }
    }
    n <= 1
}

/// Returns the set of Reference IDs that could have produced a cite's IR
fn refs_accepting_cite<O: OutputFormat>(
    db: &impl IrDatabase,
    ir: &IR<Markup>,
    ctx: &CiteContext<'_, O>,
) -> Vec<Atom> {
    use log::Level::{Info, Warn};
    let edges = ir.to_edge_stream(&db.get_formatter());
    let mut v = Vec::with_capacity(1);
    for k in db.cited_keys().iter() {
        let dfa = db.ref_dfa(k.clone()).expect("cited_keys should all exist");
        let acc = dfa.accepts_data(db, &edges);
        if acc {
            v.push(k.clone());
        }
        if log_enabled!(Warn) && ctx.cite_id.is_some() && k == &ctx.reference.id && !acc {
            warn!(
                "{:?}: own reference {} did not match during pass {:?}:\n{}\n{:?}",
                ctx.cite_id,
                k,
                ctx.disamb_pass,
                dfa.debug_graph(db),
                edges
            );
        }
        if log_enabled!(Info) && ctx.cite_id.is_some() && k != &ctx.reference.id && acc {
            info!(
                "{:?}: matched other reference {} during pass {:?}",
                ctx.cite_id, k, ctx.disamb_pass
            );
        }
    }
    v
}

///
/// 1. We assume you only get clashes from the exact same name_el (even if it could be slightly
///    different but produce clashing results).
///
/// 2. We construct the specific RefContext that would have produced the <names/> that would have
///    made the Names NFA that accepted this cite's IR. This is the 'exact same name_el' referred
///    to in step 1. (This is technically redundant, but it's not possible to pull it back out of a
///    minimised DFA.) We know the structure of the NFA, so we can avoid constructing one by just
///    having a Vec<Edge> of options.
///
///    This step is done by `make_identical_name_formatter`.
///
/// 3. We can then use this narrowed-down matcher to test, locally, whether name expansions are narrowing
///    down the cite's ambiguity, without having to zip in and out or use a mutex.

fn make_identical_name_formatter<'a, DB: IrDatabase>(
    db: &DB,
    ref_id: Atom,
    cite_ctx: &'a CiteContext<'a, Markup>,
    index: u32,
) -> Option<RefNameIR> {
    use crate::disamb::create_single_ref_ir;
    let refr = db.reference(ref_id)?;
    let ref_ctx = RefContext::from_cite_context(&refr, cite_ctx);
    let ref_ir = create_single_ref_ir::<Markup, DB>(db, &ref_ctx);
    fn find_name_block<'a>(ref_ir: &'a RefIR, nth: &mut u32) -> Option<&'a RefNameIR> {
        match ref_ir {
            RefIR::Edge(_) => None,
            RefIR::Name(nir, ref nfa) => {
                if *nth == 0 {
                    Some(nir)
                } else {
                    *nth = nth.saturating_sub(1);
                    None
                }
            }
            RefIR::Seq(seq) => {
                // assumes it's the first one that appears
                seq.contents
                    .iter()
                    .filter_map(|x| find_name_block(x, nth))
                    .nth(0)
            }
        }
    }
    info!("searching for the nth {} name block", index);
    let mut nth = index;
    find_name_block(&ref_ir, &mut nth).map(|rnir| rnir.clone())
}

type NameRef = Arc<Mutex<NameIR<Markup>>>;

fn list_all_name_blocks(ir: &IR<Markup>) -> Vec<NameRef> {
    fn list_all_name_blocks_inner(ir: &IR<Markup>, vec: &mut Vec<NameRef>) {
        match ir {
            IR::YearSuffix(..) | IR::Rendered(_) => {}
            IR::Name(ref nir) => {
                vec.push(nir.clone());
            }
            IR::ConditionalDisamb(_, boxed) => list_all_name_blocks_inner(&**boxed, vec),
            IR::Seq(seq) => {
                // assumes it's the first one that appears
                for x in &seq.contents {
                    list_all_name_blocks_inner(x, vec)
                }
            }
        }
    }
    let mut vec = Vec::new();
    list_all_name_blocks_inner(ir, &mut vec);
    vec
}

use crate::disamb::names::{
    DisambNameRatchet, NameIR, NameVariantMatcher, PersonDisambNameRatchet, RefNameIR,
};

fn disambiguate_add_names(
    db: &impl IrDatabase,
    ir: &mut IR<Markup>,
    ctx: &CiteContext<'_, Markup>,
    also_expand: bool,
) -> bool {
    let fmt = &db.get_formatter();
    let style = db.style();
    // We're going to assume, for a bit of a boost, that you can't ever match a ref not in
    // initial_refs after adding names. We'll see how that holds up.
    let initial_refs = refs_accepting_cite(db, ir, ctx);
    let mut best = initial_refs.len() as u16;
    let name_refs = list_all_name_blocks(ir);

    info!(
        "attempting to disambiguate {:?} ({}) with {:?}",
        ctx.cite_id, &ctx.reference.id, ctx.disamb_pass
    );

    for (n, nir_arc) in name_refs.into_iter().enumerate() {
        if best <= 1 {
            return true;
        }
        let mut dfas = Vec::with_capacity(best as usize);
        for k in &initial_refs {
            let dfa = db.ref_dfa(k.clone()).expect("cited_keys should all exist");
            dfas.push(dfa);
        }

        let total_ambiguity_number = |this_nir: &mut MutexGuard<'_, NameIR<Markup>>| -> u16 {
            // unlock the nir briefly, so we can access it during to_edge_stream
            let edges = MutexGuard::unlocked(this_nir, || ir.to_edge_stream(fmt));
            let count = dfas
                .iter()
                .filter(|dfa| dfa.accepts_data(db, &edges))
                .count() as u16;
            if count == 0 {
                warn!("should not get to zero matching refs");
            }
            count
        };

        let mut nir = nir_arc.lock();
        // So we can roll back to { bump = 0 }
        nir.achieved_count(best);

        // TODO: save, within NameIR, new_count and the lowest bump_name_count to achieve it,
        // so that it can roll back to that number easily
        while best > 1 {
            let ret = nir.add_name(db, ctx);
            if !ret {
                break;
            }
            if also_expand {
                expand_one_name_ir(db, ir, ctx, &initial_refs, &mut nir, n as u32);
            }
            let new_count = total_ambiguity_number(&mut nir);
            nir.achieved_count(new_count);
            best = std::cmp::min(best, new_count);
        }
        nir.rollback(db, ctx);
        best = total_ambiguity_number(&mut nir);
    }
    best <= 1
}

type NameMutexGuard<'a> = MutexGuard<'a, NameIR<Markup>>;

fn expand_one_name_ir(
    db: &impl IrDatabase,
    ir: &IR<Markup>,
    ctx: &CiteContext<'_, Markup>,
    refs_accepting: &[Atom],
    nir: &mut NameMutexGuard,
    index: u32,
) {
    let mut double_vec: Vec<Vec<NameVariantMatcher>> = Vec::new();

    for r in refs_accepting {
        if let Some(rnir) =
            make_identical_name_formatter(db, r.clone(), ctx, index)
        {
            let var = rnir.variable;
            let len = rnir.disamb_name_ids.len();
            if len > double_vec.len() {
                double_vec.resize_with(len, || Vec::with_capacity(nir.disamb_names.len()));
            }
            for (n, id) in rnir.disamb_name_ids.into_iter().enumerate() {
                let matcher = NameVariantMatcher::from_disamb_name(db, id);
                if let Some(slot) = double_vec.get_mut(n) {
                    slot.push(matcher);
                }
            }
        }
    }

    let name_ambiguity_number = |edge: Edge, slot: &[NameVariantMatcher]| -> u32 {
        slot.iter().filter(|matcher| matcher.accepts(edge)).count() as u32
    };

    let mut n = 0usize;
    for dnr in nir.disamb_names.iter_mut() {
        match dnr {
            DisambNameRatchet::Person(ratchet) => {
                if let Some(ref slot) = double_vec.get(n) {
                    // First, get the initial count
                    /* TODO: store format stack */
                    let mut edge = ratchet.data.single_name_edge(db, Formatting::default());
                    let mut min = name_ambiguity_number(edge, slot);
                    debug!("nan for {}-th ({:?}) initially {}", n, edge, min);
                    let mut stage_dn = ratchet.data.clone();
                    // Then, try to improve it
                    let mut iter = ratchet.iter.clone();
                    while min > 1 {
                        if let Some(next) = iter.next() {
                            stage_dn.apply_pass(next);
                            edge = stage_dn.single_name_edge(db, Formatting::default());
                            let new_count = name_ambiguity_number(edge, slot);
                            if new_count < min {
                                // save the improvement
                                min = new_count;
                                ratchet.data = stage_dn.clone();
                                ratchet.iter = iter.clone();
                            }
                            debug!("nan for {}-th ({:?}) got to {}", n, edge, min);
                        } else {
                            break;
                        }
                    }
                } else {
                    // We've gone past the end of the slots.
                    // None of the ambiguous references had this many names
                    // so it's impossible to improve disamb by expanding this one (though adding it would
                    // help. Since this name block was ambiguous, we know this name wasn't
                    // initially rendered.)
                }
                n += 1;
            }
            _ => {}
        }
    }
    if let Some((new_ir, _gv)) = nir.intermediate_custom(db, ctx, ctx.disamb_pass) {
        *nir.ir = new_ir;
    }
}

fn disambiguate_add_givennames(
    db: &impl IrDatabase,
    ir: &mut IR<Markup>,
    ctx: &CiteContext<'_, Markup>,
    also_add: bool,
) -> Option<bool> {
    let fmt = db.get_formatter();
    let refs = refs_accepting_cite(db, ir, ctx);
    let name_refs = list_all_name_blocks(ir);
    for (n, nir_arc) in name_refs.into_iter().enumerate() {
        let mut nir = nir_arc.lock();
        expand_one_name_ir(db, ir, ctx, &refs, &mut nir, n as u32);
    }
    if also_add {
        disambiguate_add_names(db, ir, ctx, true);
    }
    None
}

use csl::style::Element;
fn plain_suffix_element() -> Element {
    use csl::style::{Affixes, Element, TextCase, TextSource, VariableForm};
    use csl::variables::{StandardVariable, Variable};
    Element::Text(
        TextSource::Variable(
            StandardVariable::Ordinary(Variable::YearSuffix),
            VariableForm::Long,
        ),
        None,
        Default::default(),
        false,
        false,
        TextCase::None,
        None,
    )
}

fn disambiguate_add_year_suffix(
    db: &impl IrDatabase,
    ir: &mut IR<Markup>,
    state: &mut IrState,
    ctx: &CiteContext<'_, Markup>,
) {
    use csl::style::{Affixes, Element, TextCase, TextSource, VariableForm};
    use csl::variables::{StandardVariable, Variable};
    // First see if we can do it with an explicit one
    let asuf = ir.visit_year_suffix_hooks(&mut |piece| {
        *piece = match piece {
            IR::YearSuffix(ref mut hook, ref mut _built) => match hook {
                YearSuffixHook::Explicit(ref el) => {
                    let (new_ir, _gv) = el.intermediate(db, state, ctx);
                    new_ir
                }
                _ => return false,
            },
            _ => unreachable!(),
        };
        true
    });

    if asuf {
        return;
    }

    // Then attempt to do it for the ones that are embedded in date output
    ir.visit_year_suffix_hooks(&mut |piece| {
        *piece = match piece {
            IR::YearSuffix(ref mut hook, ref mut _built) => match hook {
                YearSuffixHook::Plain => {
                    let (new_ir, _gv) = plain_suffix_element().intermediate(db, state, ctx);
                    new_ir
                }
                _ => return false,
            },
            _ => unreachable!(),
        };
        true
    });
}

fn ir_gen0(db: &impl IrDatabase, id: CiteId) -> Arc<IrGen> {
    let style;
    let locale;
    let cite;
    let refr;
    let ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);
    let mut state = IrState::new();
    let ir = style.intermediate(db, &mut state, &ctx).0;
    let _fmt = db.get_formatter();
    let matching = refs_accepting_cite(db, &ir, &ctx);
    Arc::new(IrGen::new(ir, matching, state))
}

fn ir_gen1_add_names(db: &impl IrDatabase, id: CiteId) -> Arc<IrGen> {
    let style;
    let locale;
    let cite;
    let refr;
    let mut ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);
    ctx.disamb_pass = Some(DisambPass::AddNames);

    let ir0 = db.ir_gen0(id);
    // XXX: keep going if there is global name disambig to perform?
    if ir0.unambiguous() || !style.citation.disambiguate_add_names {
        return ir0.clone();
    }
    let (mut ir, state) = ir0.fresh_copy();

    disambiguate_add_names(db, &mut ir, &ctx, false);
    let matching = refs_accepting_cite(db, &ir, &ctx);
    Arc::new(IrGen::new(ir, matching, state))
}

fn ir_gen2_add_given_name(db: &impl IrDatabase, id: CiteId) -> Arc<IrGen> {
    let style;
    let locale;
    let cite;
    let refr;
    let mut ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);
    let gndr = style.citation.givenname_disambiguation_rule;
    ctx.disamb_pass = Some(DisambPass::AddGivenName(gndr));

    let ir1 = db.ir_gen1_add_names(id);
    if ir1.unambiguous() || !style.citation.disambiguate_add_givenname {
        return ir1.clone();
    }
    let (mut ir, mut state) = ir1.fresh_copy();

    let also_add_names = style.citation.disambiguate_add_names;
    disambiguate_add_givennames(db, &mut ir, &ctx, also_add_names);
    let matching = refs_accepting_cite(db, &ir, &ctx);
    Arc::new(IrGen::new(ir, matching, state))
}

fn ir_gen3_add_year_suffix(db: &impl IrDatabase, id: CiteId) -> Arc<IrGen> {
    let style;
    let locale;
    let cite;
    let refr;
    let mut ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);
    let ir2 = db.ir_gen2_add_given_name(id);
    if !style.citation.disambiguate_add_year_suffix {
        return ir2.clone();
    }
    let year_suffix = match db.year_suffix_for(cite.ref_id.clone()) {
        Some(y) => y,
        _ => return ir2.clone(),
    };
    let (mut ir, mut state) = ir2.fresh_copy();

    ctx.disamb_pass = Some(DisambPass::AddYearSuffix(year_suffix));

    disambiguate_add_year_suffix(db, &mut ir, &mut state, &ctx);
    let matching = refs_accepting_cite(db, &ir, &ctx);
    Arc::new(IrGen::new(ir, matching, state))
}

fn ir_gen4_conditionals(db: &impl IrDatabase, id: CiteId) -> Arc<IrGen> {
    let style;
    let locale;
    let cite;
    let refr;
    let mut ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);
    ctx.disamb_pass = Some(DisambPass::Conditionals);

    let ir3 = db.ir_gen3_add_year_suffix(id);
    if ir3.unambiguous() {
        return ir3.clone();
    }
    let (mut ir, mut state) = ir3.fresh_copy();

    disambiguate(db, &mut ir, &mut state, &mut ctx, None, &refr.id);
    // No point recomputing when nothing more can be done.
    let matching = Vec::new();
    Arc::new(IrGen::new(ir, matching, state))
}

fn built_cluster(
    db: &impl IrDatabase,
    cluster_id: ClusterId,
) -> Arc<<Markup as OutputFormat>::Output> {
    let fmt = db.get_formatter();
    let cite_ids = db.cluster_cites(cluster_id);
    let style = db.style();
    let layout = &style.citation.layout;
    let built_cites: Vec<_> = cite_ids
        .iter()
        .map(|&id| {
            let gen4 = db.ir_gen4_conditionals(id);
            let ir = &gen4.ir;
            let cite = id.lookup(db);
            let flattened = ir.flatten(&fmt).unwrap_or(fmt.plain(""));
            // TODO: strip punctuation on these
            let prefix = cite
                .prefix
                .as_ref()
                .map(|pre| fmt.ingest(pre, Default::default()));
            let suffix = cite
                .suffix
                .as_ref()
                .map(|pre| fmt.ingest(pre, Default::default()));
            use std::iter::once;
            match (prefix, suffix) {
                (Some(pre), Some(suf)) => {
                    fmt.seq(once(pre).chain(once(flattened)).chain(once(suf)))
                }
                (Some(pre), None) => fmt.seq(once(pre).chain(once(flattened))),
                (None, Some(suf)) => fmt.seq(once(flattened).chain(once(suf))),
                (None, None) => flattened,
            }
        })
        .collect();
    let build = fmt.with_format(
        fmt.affixed(
            fmt.group(built_cites, &layout.delimiter.0, None),
            &layout.affixes,
        ),
        layout.formatting,
    );
    Arc::new(fmt.output(build))
}

// TODO: intermediate layer before bib_item, which is before subsequent-author-substitute. Then
// mutate.

fn bib_item_gen0(db: &impl IrDatabase, ref_id: Atom) -> Option<Arc<IrGen>> {
    let sorted_refs_arc = db.sorted_refs();
    let (_keys, citation_numbers_by_id) = &*sorted_refs_arc;
    let bib_number = citation_numbers_by_id
        .get(&ref_id)
        .expect("sorted_refs should contain a bib_item key")
        .clone();
    let style = db.style();
    let locale = db.locale_by_reference(ref_id.clone());
    let cite = Cite::basic(ref_id.clone());
    let refr = db.reference(ref_id.clone())?;

    if let Some(bib) = &style.bibliography {
        let ni = style.names_delimiter.clone();
        let bib_ni = bib.names_delimiter.clone();
        let mut ctx = CiteContext {
            reference: &refr,
            format: db.get_formatter(),
            cite_id: None,
            cite: &cite,
            position: (Position::First, None),
            citation_number: 0,
            disamb_pass: None,
            style: &style,
            locale: &locale,
            bib_number: Some(bib_number),
            name_citation: db.name_bibliography(),
            in_bibliography: true,
            names_delimiter: bib_ni.or(ni),
        };
        let layout = &bib.layout;
        let mut state = IrState::new();
        let mut ir = bib.intermediate(db, &mut state, &ctx).0;

        // Immediately apply year suffixes.
        // Early-gen cites determine whether these exist -- but in the bibliography, we are already
        // aware of this, so they just need to be mirrored.
        //
        // Can't apply them the first time round, because IR may contain many suffix hooks, and we
        // need to only supply the first appearing explicit one, or the first appearing implicit one.
        // TODO: comply with the spec where "hook in cite == explicit => no implicit in bib" and "vice
        // versa"
        if let Some(suffix) = db.year_suffix_for(ref_id.clone()) {
            ctx.disamb_pass = Some(DisambPass::AddYearSuffix(suffix));
            disambiguate_add_year_suffix(db, &mut ir, &mut state, &ctx);
        }

        let matching = refs_accepting_cite(db, &ir, &ctx);
        Some(Arc::new(IrGen::new(ir, matching, state)))
    } else {
        None
    }
}

fn bib_item(db: &impl IrDatabase, ref_id: Atom) -> Arc<MarkupOutput> {
    let fmt = db.get_formatter();
    let style = db.style();
    if let Some(gen0) = db.bib_item_gen0(ref_id) {
        let layout = &style.bibliography.as_ref().unwrap().layout;
        let ir = &gen0.ir;
        let flat = ir.flatten(&fmt).unwrap_or(fmt.plain(""));
        let build = fmt.with_format(fmt.affixed(flat, &layout.affixes), layout.formatting);
        Arc::new(fmt.output(build))
    } else {
        // Whatever
        Arc::new(fmt.output(fmt.plain("")))
    }
}
