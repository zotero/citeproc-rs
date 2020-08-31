// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

// For the query group macro expansion
#![allow(clippy::large_enum_variant)]

use fnv::FnvHashMap;
use std::sync::Arc;

use crate::disamb::{Dfa, DisambName, DisambNameData, Edge, EdgeData, FreeCondSets};
use crate::prelude::*;
use crate::{CiteContext, DisambPass, IrState, Proc, IR};
use citeproc_io::output::{markup::Markup, OutputFormat};
use citeproc_io::{Cite, ClusterId, Name};
use citeproc_db::{ClusterData, CiteData};
use citeproc_io::{ClusterNumber, IntraNote};
use csl::{Atom, Bibliography, Element, Position, SortKey, TextElement};
use std::sync::Mutex;

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
    fn get_bibliography_map(&self) -> Arc<FnvHashMap<Atom, Arc<MarkupOutput>>>;

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

    // Sorting

    // Includes intra-cluster sorting
    #[salsa::invoke(crate::sort::clusters_cites_sorted)]
    fn clusters_cites_sorted(&self) -> Arc<Vec<ClusterData>>;
    #[salsa::invoke(crate::sort::cluster_data_sorted)]
    fn cluster_data_sorted(&self, id: ClusterId) -> Option<ClusterData>;

    /// Cite positions are mixed in with sorting. You cannot tell the positions of cites within a
    /// cluster until the sorting macros are called. So any cite sorting macros have to be given a
    /// stable, arbitrary/unspecified default position. We'll use Position::First.
    fn cite_positions(&self) -> Arc<FnvHashMap<CiteId, (Position, Option<u32>)>>;

    /// The first element is a [`Position`]; first, ibid, subsequent, etc
    ///
    /// The second is the 'First Reference Note Number' -- the number of the footnote containing the first cite
    /// referring to this cite's reference. This is None for a [`Position::First`].
    fn cite_position(&self, key: CiteId) -> (Position, Option<u32>);

    #[salsa::invoke(crate::sort::sorted_refs)]
    fn sorted_refs(&self) -> Arc<(Vec<Atom>, FnvHashMap<Atom, u32>)>;

    #[salsa::invoke(crate::sort::sort_string_citation)]
    fn sort_string_citation(
        &self,
        cite_id: CiteId,
        macro_name: Atom,
        sort_key: SortKey,
    ) -> Option<Arc<String>>;
    #[salsa::invoke(crate::sort::sort_string_bibliography)]
    fn sort_string_bibliography(
        &self,
        ref_id: Atom,
        macro_name: Atom,
        sort_key: SortKey,
    ) -> Option<Arc<String>>;
    #[salsa::invoke(crate::sort::bib_number)]
    fn bib_number(&self, id: CiteId) -> Option<u32>;
}

fn all_person_names(db: &dyn IrDatabase) -> Arc<Vec<DisambName>> {
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

fn ref_dfa(db: &dyn IrDatabase, key: Atom) -> Option<Arc<Dfa>> {
    if let Some(refr) = db.reference(key) {
        Some(Arc::new(create_dfa::<Markup>(db, &refr)))
    } else {
        None
    }
}

fn branch_runs(db: &dyn IrDatabase) -> Arc<FreeCondSets> {
    use crate::disamb::get_free_conds;
    Arc::new(get_free_conds(db))
}

fn year_suffix_for(db: &dyn IrDatabase, ref_id: Atom) -> Option<u32> {
    let ys = db.year_suffixes();
    ys.get(&ref_id).cloned()
}

/// This deviates from citeproc-js in one important way.
///
/// Since there are no 'groups of ambiguous cites', it is not quite simple
/// to have separate numbering for different such 'groups'.
///
/// .             'Doe 2007,  Doe 2007,  Smith 2008,  Smith 2008'
/// should become 'Doe 2007a, Doe 2007b, Smith 2008a, Smith 2008b'
///
/// The best way to do this is:
///
/// 1. Store the set of 'refs_accepting_cite'
/// 2. Find the distinct transitive closures of the `A.refs intersects B.refs` relation
///    a. Groups = {}
///    b. For each cite A with more than its own, find, if any, a Group whose total refs intersects A.refs
///    c. If found G, add A to that group, and G.total_refs = G.total_refs UNION A.refs
fn year_suffixes(db: &dyn IrDatabase) -> Arc<FnvHashMap<Atom, u32>> {
    use fnv::FnvHashSet;
    let style = db.style();
    if !style.citation.disambiguate_add_year_suffix {
        return Arc::new(FnvHashMap::default());
    }

    let mut groups: Vec<FnvHashSet<Atom>> = db
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

    use std::mem;

    // This gives us year allocations in the order they appear in the bibliography. This is how
    // the spec wants, and conveniently it is also a deterministic ordering of
    // disamb_participants that by default reflects the order they were cited and the uncited
    // ones last.
    let sorted_refs = db.sorted_refs();
    let (refs, _citation_numbers) = &*sorted_refs;
    refs
        .iter()
        .map(|id| {
            let cite = db.ghost_cite(id.clone());
            let cite_id = db.cite(CiteData::BibliographyGhost { cite });
            (id.clone(), db.ir_gen2_add_given_name(cite_id))
        })
        .for_each(|(ref_id, ir2)| {
            if ir2.unambiguous() {
                // no need to check if own id is in a group, it will receive a suffix already
            } else {
                // we make sure ref_id is included, even if there was a bug with RefIR and a
                // cite didn't match its own reference
                let mut coalesce: Option<(usize, FnvHashSet<Atom>)> = None;
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
fn ref_bib_number(db: &dyn IrDatabase, ref_id: &Atom) -> u32 {
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
        // Let's not fail, just give it one after the rest.
        std::u32::MAX
    }
}

#[derive(Debug, Clone)]
pub struct IrGen {
    pub ir: IR<Markup>,
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
            && self.state == other.state
            && self.ir == other.ir
    }
}

fn ref_not_found(db: &dyn IrDatabase, ref_id: &Atom, log: bool) -> Arc<IrGen> {
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
        let (names_delimiter, name_el) = $db.name_info_citation();
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
            in_bibliography: false,
            names_delimiter,
            name_citation: name_el,
            sort_key: None,
            year_suffix: None,
        };
    }};
}

fn is_unambiguous(
    db: &dyn IrDatabase,
    _pass: Option<DisambPass>,
    ir: &IR<Markup>,
    _cite_id: Option<CiteId>,
    _own_id: &Atom,
) -> bool {
    let edges = ir.to_edge_stream(&db.get_formatter());
    let mut n = 0;
    for k in db.disamb_participants().iter() {
        let dfa = db.ref_dfa(k.clone()).expect("disamb_participants should all exist");
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
    db: &dyn IrDatabase,
    ir: &IR<Markup>,
    ctx: &CiteContext<'_, O>,
) -> Vec<Atom> {
    use log::Level::{Info, Warn};
    let edges = ir.to_edge_stream(&db.get_formatter());
    let mut v = Vec::with_capacity(1);
    for k in db.disamb_participants().iter() {
        let dfa = db.ref_dfa(k.clone()).expect("disamb_participants should all exist");
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

fn make_identical_name_formatter<'a>(
    db: &dyn IrDatabase,
    ref_id: Atom,
    cite_ctx: &'a CiteContext<'a, Markup>,
    index: u32,
) -> Option<RefNameIR> {
    use crate::disamb::create_single_ref_ir;
    let refr = db.reference(ref_id)?;
    let ref_ctx = RefContext::from_cite_context(&refr, cite_ctx);
    let ref_ir = create_single_ref_ir::<Markup>(db, &ref_ctx);
    fn find_name_block<'a>(ref_ir: &'a RefIR, nth: &mut u32) -> Option<&'a RefNameIR> {
        match ref_ir {
            RefIR::Edge(_) => None,
            RefIR::Name(nir, ref _nfa) => {
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
    find_name_block(&ref_ir, &mut nth).cloned()
}

type NameRef = Arc<Mutex<NameIR<Markup>>>;

fn list_all_name_blocks(ir: &IR<Markup>) -> Vec<NameRef> {
    fn list_all_name_blocks_inner(ir: &IR<Markup>, vec: &mut Vec<NameRef>) {
        match ir {
            IR::NameCounter(_) | IR::YearSuffix(..) | IR::Rendered(_) => {}
            IR::Name(ref nir) => {
                vec.push(nir.clone());
            }
            IR::ConditionalDisamb(c) => {
                let lock = c.lock().unwrap();
                list_all_name_blocks_inner(&*lock.ir, vec);
            }
            IR::Seq(seq) => {
                // assumes it's the first one that appears
                for (ir, _gv) in &seq.contents {
                    list_all_name_blocks_inner(ir, vec)
                }
            }
        }
    }
    let mut vec = Vec::new();
    list_all_name_blocks_inner(ir, &mut vec);
    vec
}

type CondDisambRef = Arc<Mutex<ConditionalDisambIR<Markup>>>;

fn list_all_cond_disambs(ir: &IR<Markup>) -> Vec<CondDisambRef> {
    fn list_all_cd_inner(ir: &IR<Markup>, vec: &mut Vec<CondDisambRef>) {
        match ir {
            IR::NameCounter(_) | IR::YearSuffix(..) | IR::Rendered(_) | IR::Name(_) => {}
            IR::ConditionalDisamb(c) => {
                vec.push(c.clone());
                let lock = c.lock().unwrap();
                list_all_cd_inner(&*lock.ir, vec);
            }
            IR::Seq(seq) => {
                // assumes it's the first one that appears
                for (ir, _gv) in &seq.contents {
                    list_all_cd_inner(ir, vec)
                }
            }
        }
    }
    let mut vec = Vec::new();
    list_all_cd_inner(ir, &mut vec);
    vec
}

use crate::disamb::names::{DisambNameRatchet, NameIR, NameVariantMatcher, RefNameIR};

fn disambiguate_add_names(
    db: &dyn IrDatabase,
    ir: &mut IR<Markup>,
    ctx: &CiteContext<'_, Markup>,
    also_expand: bool,
) -> bool {
    let fmt = &db.get_formatter();
    let _style = db.style();
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
            let dfa = db.ref_dfa(k.clone()).expect("disamb_participants should all exist");
            dfas.push(dfa);
        }

        let total_ambiguity_number = |ir: &IR<Markup>| -> u16 {
            // unlock the nir briefly, so we can access it during to_edge_stream
            let edges = ir.to_edge_stream(fmt);
            let count = dfas
                .iter()
                .filter(|dfa| dfa.accepts_data(db, &edges))
                .count() as u16;
            if count == 0 {
                warn!("should not get to zero matching refs");
            }
            count
        };

        // So we can roll back to { bump = 0 }
        nir_arc.lock().unwrap().achieved_count(best);

        // TODO: save, within NameIR, new_count and the lowest bump_name_count to achieve it,
        // so that it can roll back to that number easily
        while best > 1 {
            let ret = nir_arc.lock().unwrap().add_name(db, ctx);
            if !ret {
                break;
            }
            if also_expand {
                expand_one_name_ir(
                    db,
                    ir,
                    ctx,
                    &initial_refs,
                    &mut nir_arc.lock().unwrap(),
                    n as u32,
                );
            }
            ir.recompute_group_vars();
            let new_count = total_ambiguity_number(ir);
            nir_arc.lock().unwrap().achieved_count(new_count);
            best = std::cmp::min(best, new_count);
        }
        nir_arc.lock().unwrap().rollback(db, ctx);
        ir.recompute_group_vars();
        best = total_ambiguity_number(ir);
    }
    best <= 1
}

fn expand_one_name_ir(
    db: &dyn IrDatabase,
    _ir: &IR<Markup>,
    ctx: &CiteContext<'_, Markup>,
    refs_accepting: &[Atom],
    nir: &mut NameIR<Markup>,
    index: u32,
) {
    let mut double_vec: Vec<Vec<NameVariantMatcher>> = Vec::new();

    for r in refs_accepting {
        if let Some(rnir) = make_identical_name_formatter(db, r.clone(), ctx, index) {
            let _var = rnir.variable;
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
        if let DisambNameRatchet::Person(ratchet) = dnr {
            if let Some(ref slot) = double_vec.get(n) {
                // First, get the initial count
                /* TODO: store format stack */
                let mut edge = ratchet.data.single_name_edge(db, Formatting::default());
                let mut min = name_ambiguity_number(edge, slot);
                debug!("nan for {}-th ({:?}) initially {}", n, edge, min);
                let mut stage_dn = ratchet.data.clone();
                // Then, try to improve it
                let mut iter = ratchet.iter;
                while min > 1 {
                    if let Some(next) = iter.next() {
                        stage_dn.apply_pass(next);
                        edge = stage_dn.single_name_edge(db, Formatting::default());
                        let new_count = name_ambiguity_number(edge, slot);
                        if new_count < min {
                            // save the improvement
                            min = new_count;
                            ratchet.data = stage_dn.clone();
                            ratchet.iter = iter;
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
    }
    if let Some((new_ir, _gv)) = nir.intermediate_custom(&ctx.format, ctx.position.0, ctx.sort_key.is_some(), ctx.disamb_pass, None) {
        *nir.ir = new_ir;
    }
}

fn disambiguate_add_givennames(
    db: &dyn IrDatabase,
    ir: &mut IR<Markup>,
    ctx: &CiteContext<'_, Markup>,
    also_add: bool,
) -> Option<bool> {
    let _fmt = db.get_formatter();
    let refs = refs_accepting_cite(db, ir, ctx);
    let name_refs = list_all_name_blocks(ir);
    for (n, nir_arc) in name_refs.into_iter().enumerate() {
        let mut nir = nir_arc.lock().unwrap();
        expand_one_name_ir(db, ir, ctx, &refs, &mut nir, n as u32);
        ir.recompute_group_vars();
    }
    if also_add {
        disambiguate_add_names(db, ir, ctx, true);
    }
    None
}

fn disambiguate_add_year_suffix(
    db: &dyn IrDatabase,
    ir: &mut IR<Markup>,
    state: &mut IrState,
    ctx: &CiteContext<'_, Markup>,
    suffix: u32,
) {
    // First see if we can do it with an explicit one
    let asuf = ir.visit_year_suffix_hooks(&mut |piece| {
        match &piece.hook {
            YearSuffixHook::Explicit(_) => {
                let (new_ir, gv) = piece.hook.render(ctx, suffix);
                *piece.ir = new_ir;
                piece.group_vars = gv;
                piece.suffix_num = Some(suffix);
                true
            }
            _ => false,
        }
    });

    if asuf {
        return;
    }

    // Then attempt to do it for the ones that are embedded in date output
    ir.visit_year_suffix_hooks(&mut |piece| {
        match &piece.hook {
            YearSuffixHook::Plain => {
                let (new_ir, gv) = piece.hook.render(ctx, suffix);
                *piece.ir = new_ir;
                piece.group_vars = gv;
                piece.suffix_num = Some(suffix);
                true
            }
            _ => false,
        }
    });
    ir.recompute_group_vars()
}

#[inline(never)]
fn disambiguate_true(
    db: &dyn IrDatabase,
    ir: &mut IR<Markup>,
    state: &mut IrState,
    ctx: &CiteContext<'_, Markup>,
) {
    info!(
        "attempting to disambiguate {:?} ({}) with {:?}",
        ctx.cite_id, &ctx.reference.id, ctx.disamb_pass
    );
    let un = is_unambiguous(db, ctx.disamb_pass, ir, ctx.cite_id, &ctx.reference.id);
    if un {
        return;
    }
    let cond_refs = list_all_cond_disambs(ir);
    for (_n, cir_arc) in cond_refs.into_iter().enumerate() {
        if is_unambiguous(db, ctx.disamb_pass, ir, ctx.cite_id, &ctx.reference.id) {
            info!("successfully disambiguated");
            break;
        }
        {
            let mut lock = cir_arc.lock().unwrap();
            let (new_ir, gv) = lock.choose.intermediate(db, state, ctx);
            lock.done = true;
            lock.ir = Box::new(new_ir);
            lock.group_vars = gv;
        }
        ir.recompute_group_vars()
    }
}

fn ir_gen0(db: &dyn IrDatabase, id: CiteId) -> Arc<IrGen> {
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

fn ir_gen1_add_names(db: &dyn IrDatabase, id: CiteId) -> Arc<IrGen> {
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
        return ir0;
    }
    let (mut ir, state) = ir0.fresh_copy();

    disambiguate_add_names(db, &mut ir, &ctx, false);
    let matching = refs_accepting_cite(db, &ir, &ctx);
    Arc::new(IrGen::new(ir, matching, state))
}

fn ir_gen2_add_given_name(db: &dyn IrDatabase, id: CiteId) -> Arc<IrGen> {
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
        return ir1;
    }
    let (mut ir, state) = ir1.fresh_copy();

    let also_add_names = style.citation.disambiguate_add_names;
    disambiguate_add_givennames(db, &mut ir, &ctx, also_add_names);
    let matching = refs_accepting_cite(db, &ir, &ctx);
    Arc::new(IrGen::new(ir, matching, state))
}

fn ir_gen3_add_year_suffix(db: &dyn IrDatabase, id: CiteId) -> Arc<IrGen> {
    let style;
    let locale;
    let cite;
    let refr;
    let mut ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);
    let ir2 = db.ir_gen2_add_given_name(id);
    if !style.citation.disambiguate_add_year_suffix {
        return ir2;
    }
    let year_suffix = match db.year_suffix_for(cite.ref_id.clone()) {
        Some(y) => y,
        _ => return ir2,
    };
    let (mut ir, mut state) = ir2.fresh_copy();

    ctx.disamb_pass = Some(DisambPass::AddYearSuffix(year_suffix));

    disambiguate_add_year_suffix(db, &mut ir, &mut state, &ctx, year_suffix);
    let matching = refs_accepting_cite(db, &ir, &ctx);
    Arc::new(IrGen::new(ir, matching, state))
}

fn ir_gen4_conditionals(db: &dyn IrDatabase, id: CiteId) -> Arc<IrGen> {
    let style;
    let locale;
    let cite;
    let refr;
    let mut ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);
    ctx.disamb_pass = Some(DisambPass::Conditionals);

    let ir3 = db.ir_gen3_add_year_suffix(id);
    if ir3.unambiguous() {
        return ir3;
    }
    let (mut ir, mut state) = ir3.fresh_copy();

    disambiguate_true(db, &mut ir, &mut state, &ctx);
    // No point recomputing when nothing more can be done.
    let matching = Vec::new();
    Arc::new(IrGen::new(ir, matching, state))
}

fn get_piq(db: &dyn IrDatabase) -> bool {
    // We pant PIQ to be global in a document, not change within a cluster because one cite
    // decided to use a different language. Use the default locale to get it.
    let default_locale = db.default_locale();
    default_locale.options_node.punctuation_in_quote.unwrap_or(false)
}

fn built_cluster(
    db: &dyn IrDatabase,
    cluster_id: ClusterId,
) -> Arc<<Markup as OutputFormat>::Output> {
    let fmt = db.get_formatter();
    let build = built_cluster_before_output(db, cluster_id);
    let string = fmt.output(build, get_piq(db));
    Arc::new(string)
}

pub fn built_cluster_before_output(
    db: &dyn IrDatabase,
    cluster_id: ClusterId,
) -> <Markup as OutputFormat>::Build {
    let fmt = db.get_formatter();
    let cluster = if let Some(data) = db.cluster_data_sorted(cluster_id) {
        data
    } else {
        return fmt.plain("");
    };
    let cite_ids = &cluster.cites;
    let style = db.style();
    let layout = &style.citation.layout;
    let sorted_refs_arc = db.sorted_refs();
    use crate::ir::transforms::{Unnamed3, RangePiece, group_and_collapse, CnumIx};
    let mut irs: Vec<_> = cite_ids
        .iter()
        .map(|&id| {
            let gen4 = db.ir_gen4_conditionals(id);
            let cite = id.lookup(db);
            let (_keys, citation_numbers_by_id) = &*sorted_refs_arc;
            let cnum = citation_numbers_by_id
                .get(&cite.ref_id)
                .cloned();
            Unnamed3::new(cite, cnum, gen4.clone())
        })
        .collect();

    if let Some((cgd, collapse)) = style.citation.group_collapsing() {
        group_and_collapse(db, &fmt, cgd, collapse, &mut irs);
    }

    // Cite capitalization
    // TODO: allow clients to pass a flag to prevent this when a cluster is in the middle of an
    // existing footnote, and isn't preceded by a period (or however else a client wants to judge
    // that).
    if let Some(Unnamed3 { cite, gen4, .. }) = irs.first_mut() {
        use unic_segment::Words;
        if style.class != csl::StyleClass::InText && cite.prefix.as_ref().map_or(true, |pre| {
            // bugreports_CapsAfterOneWordPrefix
            let mut words = Words::new(pre, |s| s.chars().any(|c| c.is_alphanumeric()));
            words.next();
            let is_single_word = words.next().is_none();
            (pre.is_empty() || pre.trim_end().ends_with(".")) && !is_single_word
        }) {
            let gen_mut = Arc::make_mut(gen4);
            gen_mut.ir.capitalize_first_term_of_cluster(&fmt);
        }
    }
    // debug!("group_and_collapse made: {:#?}", irs);

    // csl_test_suite::affix_WithCommas.txt
    let suppress_delimiter = |cites: &[Unnamed3<Markup>], ix: usize| -> bool {
        let this_suffix = match cites.get(ix) {
            Some(x) => x.cite.suffix.as_ref().map(AsRef::as_ref).unwrap_or(""),
            None => "",
        };
        let next_prefix = match cites.get(ix + 1) {
            Some(x) => x.cite.prefix.as_ref().map(AsRef::as_ref).unwrap_or(""),
            None => "",
        };
        let ends_punc = |string: &str| string.chars()
            .rev()
            .nth(0)
            .map_or(false, |x| x == ',' || x == '.' || x == '?' || x == '!');
        let starts_punc = |string: &str| string.chars()
            .nth(0)
            .map_or(false, |x| x == ',' || x == '.' || x == '?' || x == '!');

        // "2000 is one source,; David Jones" => "2000 is one source, David Jones"
        // "2000;, and David Jones" => "2000, and David Jones"
        ends_punc(this_suffix) || starts_punc(next_prefix)
    };

    let build_cite = |cites: &[Unnamed3<Markup>], ix: usize| -> Option<MarkupBuild> {
        let Unnamed3 { cite, gen4, .. } = cites.get(ix)?;
        use std::borrow::Cow;
        let ir = &gen4.ir;
        let flattened = ir.flatten(&fmt)?;
        let mut pre = Cow::from(cite.prefix.as_ref().map(AsRef::as_ref).unwrap_or(""));
        let mut suf = Cow::from(cite.suffix.as_ref().map(AsRef::as_ref).unwrap_or(""));
        if !pre.is_empty() && !pre.ends_with(' ') {
            let pre_mut = pre.to_mut();
            pre_mut.push(' ');
        }
        let suf_first = suf.chars().nth(0);
        if suf_first.map_or(false, |x| x != ' ' && !citeproc_io::output::markup::is_punc(x)) {
            let suf_mut = suf.to_mut();
            suf_mut.insert_str(0, " ");
        }
        let suf_last_punc = suf.chars().rev().nth(0).map_or(false, |x| {
            x == ',' || x == '.' || x == '!' || x == '?' || x == ':'
        });
        let cite_is_last = ix == cites.len() - 1;
        if suf_last_punc && !cite_is_last {
            let suf_mut = suf.to_mut();
            suf_mut.push(' ');
        }
        // TODO: custom procedure for joining user-supplied cite affixes, which should interact
        // with terminal punctuation by overriding rather than joining in the usual way.
        let aff = Affixes {
            prefix: Atom::from(pre),
            suffix: Atom::from(suf),
        };
        Some(fmt.affixed(flattened, Some(&aff)))
    };

    let mut cgroup_delim = style
        .citation
        .cite_group_delimiter
        .as_ref()
        .map(|atom| atom.as_ref())
        .unwrap_or(", ");
    let ysuf_delim = style
        .citation
        .year_suffix_delimiter
        .as_ref()
        .map(|atom| atom.as_ref())
        .unwrap_or(style.citation.layout.delimiter.0.as_ref());
    let acol_delim = style
        .citation
        .after_collapse_delimiter
        .as_ref()
        .map(|atom| atom.as_ref())
        .unwrap_or(style.citation.layout.delimiter.0.as_ref());
    let layout_delim = style.citation.layout.delimiter.0.as_ref();

    // returned usize is advance len
    let render_range = |ranges: &[RangePiece], group_delim: &str, outer_delim: &str| -> (MarkupBuild, usize) {
        let mut advance_to = 0usize;
        let mut group: Vec<MarkupBuild> = Vec::with_capacity(ranges.len());
        for (ix, piece) in ranges.iter().enumerate() {
            let is_last = ix == ranges.len() - 1;
            match *piece {
                RangePiece::Single(CnumIx { ix, force_single, .. }) => {
                    advance_to = ix;
                    if let Some(one) = build_cite(&irs, ix) {
                        group.push(one);
                        if !is_last && !suppress_delimiter(&irs, ix) {
                            group.push(fmt.plain(if force_single {
                                outer_delim
                            } else {
                                group_delim
                            }));
                        }
                    }
                }
                RangePiece::Range(start, end) => {
                    advance_to = end.ix;
                    let mut delim = "\u{2013}";
                    if start.cnum == end.cnum - 1 {
                        // Not represented as a 1-2, just two sequential numbers 1,2
                        delim = group_delim;
                    }
                    let mut g = vec![];
                    if let Some(start) = build_cite(&irs, start.ix) {
                        g.push(start);
                    }
                    if let Some(end) = build_cite(&irs, end.ix) {
                        g.push(end);
                    }
                    // Delimiters here are never suppressed by build_cite, as they wouldn't be part
                    // of the range if they had affixes on the inside
                    group.push(fmt.group(g, delim, None));
                    if !is_last && !suppress_delimiter(&irs, end.ix) {
                        group.push(fmt.plain(group_delim));
                    }
                }
            }
        }
        (fmt.group(group, "", None), advance_to)
    };

    let mut built_cites = Vec::with_capacity(irs.len() * 2);

    let mut ix = 0;
    while ix < irs.len() {
        let Unnamed3 { cite, gen4, vanished, collapsed_ranges, collapsed_year_suffixes, is_first, .. } = &irs[ix];
        if *vanished {
            ix += 1;
            continue;
        }
        if !collapsed_ranges.is_empty() {
            let (built, advance_to) = render_range(collapsed_ranges, layout_delim, acol_delim);
            built_cites.push(built);
            if !suppress_delimiter(&irs, ix) {
                built_cites.push(fmt.plain(acol_delim));
            } else {
                built_cites.push(fmt.plain(""));
            }
            ix = advance_to + 1;
        } else if *is_first {
            let mut group = Vec::with_capacity(4);
            let mut rix = ix;
            while rix < irs.len() {
                let r = &irs[rix];
                if rix != ix && !r.should_collapse {
                    break;
                }
                if !r.collapsed_year_suffixes.is_empty() {
                    let (built, advance_to) = render_range(&r.collapsed_year_suffixes, ysuf_delim, acol_delim);
                    group.push(built);
                    if !suppress_delimiter(&irs, ix) {
                        group.push(fmt.plain(cgroup_delim));
                    } else {
                        group.push(fmt.plain(""));
                    }
                    rix = advance_to;
                } else {
                    if let Some(b) = build_cite(&irs, rix) {
                        group.push(b);
                        if !suppress_delimiter(&irs, ix) {
                            group.push(fmt.plain(if irs[rix].has_locator {
                                acol_delim
                            } else {
                                cgroup_delim
                            }));
                        } else {
                            group.push(fmt.plain(""));
                        }
                    }
                }
                rix += 1;
            }
            group.pop();
            built_cites.push(fmt.group(group, "", None));
            if !suppress_delimiter(&irs, ix) {
                built_cites.push(fmt.plain(acol_delim));
            } else {
                built_cites.push(fmt.plain(""));
            }
            ix = rix;
        } else {
            if let Some(built) = build_cite(&irs, ix) {
                built_cites.push(built);
                if !suppress_delimiter(&irs, ix) {
                    built_cites.push(fmt.plain(layout_delim));
                } else {
                    built_cites.push(fmt.plain(""));
                }
            }
            ix += 1;
        }
    }
    built_cites.pop();

    fmt.with_format(
        fmt.affixed(
            fmt.group(built_cites, "", None),
            layout.affixes.as_ref(),
        ),
        layout.formatting,
    )
}

/// None if the reference being cited does not exist
pub fn with_cite_context<T>(
    db: &dyn IrDatabase,
    id: CiteId,
    bib_number: Option<u32>,
    sort_key: Option<SortKey>,
    default_position: bool,
    year_suffix: Option<u32>,
    f: impl Fn(CiteContext) -> T,
) -> Option<T> {
    let style = db.style();
    let locale = db.locale_by_cite(id);
    let cite = id.lookup(db);
    let refr = db.reference(cite.ref_id.clone())?;
    let (names_delimiter, name_el) = db.name_info_citation();
    let ctx = CiteContext {
        reference: &refr,
        format: db.get_formatter(),
        cite_id: Some(id),
        cite: &cite,
        position: if default_position {
            (Position::First, None)
        } else {
            db.cite_position(id)
        },
        citation_number: 0,
        disamb_pass: None,
        style: &style,
        locale: &locale,
        bib_number,
        in_bibliography: false,
        names_delimiter,
        name_citation: name_el,
        sort_key,
        year_suffix,
    };
    Some(f(ctx))
}

// TODO: intermediate layer before bib_item, which is before subsequent-author-substitute. Then
// mutate.

pub fn with_bib_context<T>(
    db: &dyn IrDatabase,
    ref_id: Atom,
    bib_number: Option<u32>,
    sort_key: Option<SortKey>,
    year_suffix: Option<u32>,
    f: impl Fn(&Bibliography, CiteContext) -> T,
) -> Option<T> {
    let style = db.style();
    let locale = db.locale_by_reference(ref_id.clone());
    let cite = Cite::basic(ref_id.clone());
    let refr = db.reference(ref_id)?;
    let (names_delimiter, name_el) = db.name_info_bibliography();
    let bib = style.bibliography.as_ref()?;
    let ctx = CiteContext {
        reference: &refr,
        format: db.get_formatter(),
        cite_id: None,
        cite: &cite,
        position: (Position::First, None),
        citation_number: 0,
        disamb_pass: None,
        style: &style,
        locale: &locale,
        bib_number,
        in_bibliography: true,
        names_delimiter,
        name_citation: name_el,
        sort_key,
        year_suffix,
    };
    Some(f(bib, ctx))
}

fn bib_item_gen0(db: &dyn IrDatabase, ref_id: Atom) -> Option<Arc<IrGen>> {
    let sorted_refs_arc = db.sorted_refs();
    let (_keys, citation_numbers_by_id) = &*sorted_refs_arc;
    let bib_number = *citation_numbers_by_id
        .get(&ref_id)
        .expect("sorted_refs should contain a bib_item key");

    with_bib_context(
        db,
        ref_id.clone(),
        Some(bib_number),
        None,
        None,
        |bib, mut ctx| {
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
                disambiguate_add_year_suffix(db, &mut ir, &mut state, &ctx, suffix);
            }

            if bib.second_field_align == Some(csl::SecondFieldAlign::Flush) {
                ir.split_first_field();
            }

            let matching = refs_accepting_cite(db, &ir, &ctx);
            Arc::new(IrGen::new(ir, matching, state))
        },
    )
}

fn bib_item(db: &dyn IrDatabase, ref_id: Atom) -> Arc<MarkupOutput> {
    let fmt = db.get_formatter();
    let style = db.style();
    if let Some(gen0) = db.bib_item_gen0(ref_id) {
        let layout = &style.bibliography.as_ref().unwrap().layout;
        let ir = &gen0.ir;
        let flat = ir.flatten(&fmt).unwrap_or_else(|| fmt.plain(""));
        // in a bibliography, we do the affixes etc inside Layout, so they're not here
        let string = fmt.output(flat, get_piq(db));
        Arc::new(string)
    } else {
        // Whatever
        Arc::new(fmt.output(fmt.plain(""), get_piq(db)))
    }
}

fn get_bibliography_map(db: &dyn IrDatabase) -> Arc<FnvHashMap<Atom, Arc<MarkupOutput>>> {
    let fmt = db.get_formatter();
    let style = db.style();
    let sorted_refs = db.sorted_refs();
    let mut m = FnvHashMap::with_capacity_and_hasher(
        sorted_refs.0.len(),
        fnv::FnvBuildHasher::default(),
    );
    let mut prev: Option<Arc<Mutex<NameIR<Markup>>>> = None;
    for key in sorted_refs.0.iter() {
        // TODO: put Nones in there so they can be updated
        if let Some(mut gen0) = db.bib_item_gen0(key.clone()) {
            let layout = &style.bibliography.as_ref().unwrap().layout;
            // in a bibliography, we do the affixes etc inside Layout, so they're not here
            let current = gen0.ir.first_name_block();
            let sas = style
                .bibliography
                .as_ref()
                .and_then(|bib| bib
                    .subsequent_author_substitute
                    .as_ref()
                    .map(|x| (x.as_ref(), bib.subsequent_author_substitute_rule)));
            if let (Some(prev), Some(current), Some((sas, sas_rule))) = (prev.as_ref(), current.as_ref(), sas) {
                let did = crate::transforms::subsequent_author_substitute(&fmt, prev, current, sas, sas_rule);
                if did {
                    let mutated = Arc::make_mut(&mut gen0);
                    mutated.ir.recompute_group_vars()
                }
            }
            let flat = gen0.ir.flatten(&fmt).unwrap_or_else(|| fmt.plain(""));
            let string = fmt.output(flat, get_piq(db));
            if !string.is_empty() {
                m.insert(key.clone(), Arc::new(string));
            }
            prev = current;
        }
    }
    Arc::new(m)
}

// See https://github.com/jgm/pandoc-citeproc/blob/e36c73ac45c54dec381920e92b199787601713d1/src/Text/CSL/Reference.hs#L910
fn cite_positions(db: &dyn IrDatabase) -> Arc<FnvHashMap<CiteId, (Position, Option<u32>)>> {
    let clusters = db.clusters_cites_sorted();

    let mut map = FnvHashMap::default();

    let style = db.style();
    let near_note_distance = style.citation.near_note_distance;

    // Backref table for FRNN
    // No entries for first ref == an in-text reference, only first time it appeared in a
    // footnote. This makes sense because note styles usually have a near-bibliography level of
    // detail, but in-text styles are often just author-date or a bibligraphy item number.
    let mut first_seen: FnvHashMap<Atom, ClusterNumber> = FnvHashMap::default();

    let mut last_note_num = None;
    let mut clusters_in_last_note: Vec<u32> = Vec::new();

    let mut prev_in_text: Option<&ClusterData> = None;
    let mut prev_note: Option<&ClusterData> = None;

    for (i, cluster) in clusters.iter().enumerate() {
        let prev_in_group = if let ClusterNumber::Note(_) = cluster.number {
            !clusters_in_last_note.is_empty()
        } else {
            false
        };
        let is_near = move |n: u32| {
            cluster
                .number
                .sub_note(IntraNote::Single(n))
                .map_or(false, |d| d <= near_note_distance)
        };
        let in_text = match cluster.number {
            ClusterNumber::InText(n) => Some(n),
            _ => None,
        };
        for (j, &cite_id) in cluster.cites.iter().enumerate() {
            let cite = cite_id.lookup(db);
            let prev_cite = cluster
                .cites
                // 0 - 1 == usize::MAX is never going to come up with anything
                .get(j.wrapping_sub(1))
                .map(|&prev_id| prev_id.lookup(db));
            enum Where {
                SameCluster(Arc<Cite<Markup>>),
                // Note Number here, so we can selectively apply near-note
                // There could be a bunch of non-cluster footnotes in between,
                // so we can't just assume two neighbouring clusters are actually next to each
                // other in the document.
                PrevCluster(Arc<Cite<Markup>>, Option<u32>),
            }
            let matching_prev = prev_cite
                .and_then(|p| {
                    if p.ref_id == cite.ref_id {
                        Some(Where::SameCluster(p))
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    if let Some(prev_cluster) = match cluster.number {
                        ClusterNumber::InText(_) => prev_in_text,
                        ClusterNumber::Note(_) => prev_note,
                    } {
                        let prev_number = match prev_cluster.number {
                            ClusterNumber::Note(intra) => Some(intra.note_number()),
                            _ => None,
                        };
                        let cites_all_same = if prev_in_group && in_text.is_none() {
                            // { id: 1, note: 4, cites: [A] },
                            // { id: 2, note: 4, cites: [B] },
                            // { id: 3: note: 5, cites: [B] } => subsequent
                            // (because prev note wasn't homogenous)
                            clusters_in_last_note
                                .iter()
                                .filter_map(|&cluster_id| db.cluster_data_sorted(cluster_id))
                                .flat_map(|cluster| (*cluster.cites).clone().into_iter())
                                .all(|cite_id| cite_id.lookup(db).ref_id == cite.ref_id)
                        } else {
                            prev_cluster
                                .cites
                                .iter()
                                .all(|&pid| pid.lookup(db).ref_id == cite.ref_id)
                        };
                        // Even if len was 0, fine because last() will end up with None anyway
                        if cites_all_same {
                            // Pick the last one to match locators against
                            prev_cluster
                                .cites
                                .last()
                                .map(|&pid| Where::PrevCluster(pid.lookup(db), prev_number))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .map(|prev| {
                    enum Num {
                        SameCluster,
                        PrevButInText,
                        PrevNote(u32),
                    }
                    let nn = match &prev {
                        Where::SameCluster(_) => Num::SameCluster,
                        Where::PrevCluster(_, None) => Num::PrevButInText,
                        Where::PrevCluster(_, Some(n)) => Num::PrevNote(*n),
                    };
                    let near = match nn {
                        Num::SameCluster => true,
                        Num::PrevButInText => false,
                        Num::PrevNote(n) => is_near(n),
                    };
                    let prev = match &prev {
                        Where::SameCluster(prev) | Where::PrevCluster(prev, _) => prev,
                    };
                    match (prev.locators.as_ref(), cite.locators.as_ref(), near) {
                        (None, None, false) => Position::Ibid,
                        (None, None, true) => Position::IbidNear,
                        (None, Some(_cur), false) => Position::IbidWithLocator,
                        (None, Some(_cur), true) => Position::IbidWithLocatorNear,
                        // Despite "position can only be subsequent", we get
                        // near/far note, as they imply subsequent.
                        (Some(_pre), None, x) => {
                            if x {
                                Position::NearNote
                            } else {
                                Position::FarNote
                            }
                        }
                        (Some(pre), Some(cur), x) if pre == cur => {
                            if x {
                                Position::IbidNear
                            } else {
                                Position::Ibid
                            }
                        }
                        (_, _, x) => {
                            if x {
                                Position::IbidWithLocatorNear
                            } else {
                                Position::IbidWithLocator
                            }
                        }
                    }
                });
            let seen = first_seen.get(&cite.ref_id).cloned();
            match seen {
                Some(ClusterNumber::Note(first_note_number)) => {
                    let first_number = ClusterNumber::Note(first_note_number);
                    assert!(
                        cluster.number >= first_number,
                        "note numbers not monotonic: {:?} came after but was less than {:?}",
                        cluster.number,
                        first_note_number,
                    );
                    let unsigned = first_note_number.note_number();
                    if let Some(pos) = matching_prev {
                        map.insert(cite_id, (pos, Some(unsigned)));
                    } else if cluster.number == first_number || is_near(unsigned) {
                        // XXX: not sure about this one
                        // unimplemented!("cite position for same number, but different cluster");
                        map.insert(cite_id, (Position::NearNote, Some(unsigned)));
                    } else {
                        map.insert(cite_id, (Position::FarNote, Some(unsigned)));
                    }
                }
                Some(ClusterNumber::InText(seen_in_text_num)) => {
                    // First seen was an in-text reference. Can be overwritten with a note cluster.
                    match cluster.number {
                        ClusterNumber::Note(_) => {
                            // Overwrite
                            first_seen.insert(cite.ref_id.clone(), cluster.number);
                            // First 'full' cite.
                            map.insert(cite_id, (Position::First, None));
                        }
                        ClusterNumber::InText(itnum) => {
                            let diff = itnum.wrapping_sub(seen_in_text_num);
                            let pos = if let Some(pos) = matching_prev {
                                pos
                            } else if diff <= near_note_distance {
                                Position::NearNote
                            } else {
                                Position::FarNote
                            };
                            map.insert(cite_id, (pos, None));
                        }
                    }
                }
                None => {
                    map.insert(cite_id, (Position::First, None));
                    first_seen.insert(cite.ref_id.clone(), cluster.number);
                }
            }
        }

        if let ClusterNumber::Note(n) = cluster.number {
            let n = n.note_number();
            if last_note_num != Some(n) {
                last_note_num = Some(n);
                clusters_in_last_note.clear();
            }
            clusters_in_last_note.push(cluster.id);
        }
        prev_in_text = if let ClusterNumber::InText(_i) = cluster.number {
            Some(cluster)
        } else {
            None
        };
        prev_note = if let ClusterNumber::Note(_i) = cluster.number {
            Some(cluster)
        } else {
            None
        };
    }

    Arc::new(map)
}

fn cite_position(db: &dyn IrDatabase, key: CiteId) -> (Position, Option<u32>) {
    if let Some(x) = db.cite_positions().get(&key) {
        *x
    } else {
        // Assume this cite is a ghost cite.
        (Position::First, None)
    }
}

