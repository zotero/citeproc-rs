// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

// For the query group macro expansion
#![allow(clippy::large_enum_variant)]

use fnv::FnvHashMap;
use std::sync::Arc;

use crate::disamb::names::replace_single_child;
use crate::disamb::{Dfa, DisambName, DisambNameData, Edge, EdgeData, FreeCondSets};
use crate::prelude::*;
use crate::{CiteContext, DisambPass, IrState, Proc, IR};
use citeproc_db::{CiteData, ClusterData};
use citeproc_io::output::{markup::Markup, OutputFormat};
use citeproc_io::{Cite, ClusterId, Name};
use citeproc_io::{ClusterNumber, IntraNote};
use csl::{Atom, Bibliography, Position, SortKey};
use std::sync::Mutex;

use indextree::NodeId;

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
    fn ir_gen2_add_given_name(&self, key: CiteId) -> Arc<IrGen>;
    fn year_suffixes(&self) -> Arc<FnvHashMap<Atom, u32>>;
    fn year_suffix_for(&self, ref_id: Atom) -> Option<u32>;
    fn ir_fully_disambiguated(&self, key: CiteId) -> Arc<IrGen>;
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
    refs.iter()
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

#[derive(Clone)]
pub struct IrGen {
    pub(crate) arena: IrArena<Markup>,
    pub(crate) root: NodeId,
    pub(crate) state: IrState,
    pub(crate) matching_refs: Vec<Atom>,
}

impl Eq for IrGen {}
impl PartialEq<IrGen> for IrGen {
    fn eq(&self, other: &Self) -> bool {
        self.matching_refs == other.matching_refs
            && self.state == other.state
            && self.root == other.root
            && self.arena == other.arena
    }
}

use std::fmt;
impl fmt::Debug for IrGen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn go(
            indent: u32,
            node: NodeId,
            arena: &IrArena<Markup>,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result {
            let pair = arena.get(node).unwrap().get();
            for _ in 0..indent {
                write!(f, "    ")?;
            }
            writeln!(f, " - [{:?}] {:?}", pair.1, pair.0)?;
            node.children(arena)
                .try_for_each(|ch| go(indent + 1, ch, arena, f))
        }
        write!(f, "\n")?;
        go(0, self.root, &self.arena, f)
    }
}

impl IrGen {
    fn new(root: NodeId, arena: IrArena<Markup>, matching_refs: Vec<Atom>, state: IrState) -> Self {
        IrGen {
            root,
            arena,
            state,
            matching_refs,
        }
    }
    fn unambiguous(&self) -> bool {
        self.matching_refs.len() <= 1
    }
    fn fresh_copy(&self) -> (NodeId, IrArena<Markup>, IrState) {
        let root = self.root;
        let arena = self.arena.clone();
        let state = self.state.clone();
        (root, arena, state)
    }
}

fn ref_not_found(db: &dyn IrDatabase, ref_id: &Atom, log: bool) -> Arc<IrGen> {
    if log {
        eprintln!("citeproc-rs: reference {} not found", ref_id);
    }
    let mut arena = IrArena::new();
    let root = arena.new_node((
        IR::Rendered(Some(CiteEdgeData::Output(db.get_formatter().plain("???")))),
        GroupVars::Plain,
    ));
    Arc::new(IrGen::new(root, arena, Vec::new(), IrState::new()))
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
    root: NodeId,
    arena: &IrArena<Markup>,
    _cite_id: Option<CiteId>,
    _own_id: &Atom,
) -> bool {
    let edges = IR::to_edge_stream(root, arena, &db.get_formatter());
    let mut n = 0;
    for k in db.disamb_participants().iter() {
        let dfa = db
            .ref_dfa(k.clone())
            .expect("disamb_participants should all exist");
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
    root: NodeId,
    arena: &IrArena<Markup>,
    ctx: &CiteContext<'_, O>,
) -> Vec<Atom> {
    use log::Level::{Info, Warn};
    let edges = IR::to_edge_stream(root, arena, &db.get_formatter());
    let mut v = Vec::with_capacity(1);
    for k in db.disamb_participants().iter() {
        let dfa = db
            .ref_dfa(k.clone())
            .expect("disamb_participants should all exist");
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

fn list_all_name_blocks(root: NodeId, arena: &IrArena<Markup>) -> Vec<NodeId> {
    fn list_all_name_blocks_inner(node: NodeId, arena: &IrArena<Markup>, vec: &mut Vec<NodeId>) {
        let me = match arena.get(node) {
            Some(x) => x.get(),
            None => return,
        };
        match me.0 {
            IR::NameCounter(_) | IR::YearSuffix(..) | IR::Rendered(_) => {}
            IR::Name(_) => {
                vec.push(node);
            }
            IR::ConditionalDisamb(_) | IR::Seq(_) => {
                // assumes it's the first one that appears
                for child in node.children(arena) {
                    list_all_name_blocks_inner(child, arena, vec);
                }
            }
        }
    }
    let mut vec = Vec::new();
    list_all_name_blocks_inner(root, arena, &mut vec);
    vec
}

fn list_all_cond_disambs(root: NodeId, arena: &IrArena<Markup>) -> Vec<NodeId> {
    fn list_all_cd_inner(node: NodeId, arena: &IrArena<Markup>, vec: &mut Vec<NodeId>) {
        let me = match arena.get(node) {
            Some(x) => x.get(),
            None => return,
        };
        match &me.0 {
            IR::NameCounter(_) | IR::YearSuffix(..) | IR::Rendered(_) | IR::Name(_) => {}
            IR::ConditionalDisamb(c) => {
                vec.push(node);
                node.children(arena)
                    .for_each(|child| list_all_cd_inner(child, arena, vec));
            }
            IR::Seq(seq) => {
                node.children(arena)
                    .for_each(|child| list_all_cd_inner(child, arena, vec));
            }
        }
    }
    let mut vec = Vec::new();
    list_all_cd_inner(root, arena, &mut vec);
    vec
}

use crate::disamb::names::{DisambNameRatchet, NameIR, NameVariantMatcher, RefNameIR};

fn get_nir_mut(nid: NodeId, arena: &mut IrArena<Markup>) -> &mut NameIR<Markup> {
    arena.get_mut(nid).unwrap().get_mut().0.unwrap_name_ir_mut()
}
fn get_ys_mut(yid: NodeId, arena: &mut IrArena<Markup>) -> (&mut YearSuffix, &mut GroupVars) {
    let both = arena.get_mut(yid).unwrap().get_mut();
    (both.0.unwrap_year_suffix_mut(), &mut both.1)
}
fn get_cond_mut(cid: NodeId, arena: &mut IrArena) -> (&mut ConditionalDisambIR, &mut GroupVars) {
    let both = arena.get_mut(cid).unwrap().get_mut();
    (both.0.unwrap_cond_disamb_mut(), &mut both.1)
}

fn disambiguate_add_names(
    db: &dyn IrDatabase,
    root: NodeId,
    arena: &mut IrArena<Markup>,
    ctx: &CiteContext<'_, Markup>,
    also_expand: bool,
) -> bool {
    let fmt = &db.get_formatter();
    // We're going to assume, for a bit of a boost, that you can't ever match a ref not in
    // initial_refs after adding names. We'll see how that holds up.
    let initial_refs = refs_accepting_cite(db, root, arena, ctx);
    let mut best = initial_refs.len() as u16;
    let name_refs = list_all_name_blocks(root, arena);

    info!(
        "attempting to disambiguate {:?} ({}) with {:?}",
        ctx.cite_id, &ctx.reference.id, ctx.disamb_pass
    );

    for (n, nid) in name_refs.into_iter().enumerate() {
        if best <= 1 {
            return true;
        }
        let mut dfas = Vec::with_capacity(best as usize);
        for k in &initial_refs {
            let dfa = db
                .ref_dfa(k.clone())
                .expect("disamb_participants should all exist");
            dfas.push(dfa);
        }

        let total_ambiguity_number = |arena: &IrArena<Markup>| -> u16 {
            // unlock the nir briefly, so we can access it during to_edge_stream
            let edges = IR::to_edge_stream(root, arena, fmt);
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
        let nir = get_nir_mut(nid, arena);
        nir.achieved_count(best);

        let is_sort_key = ctx.sort_key.is_some();
        let label_after_name = nir
            .names_inheritance
            .label
            .as_ref()
            .map_or(false, |x| x.after_name);
        // Probably use an Atom for this buddy
        let built_label = nir.built_label.clone();

        while best > 1 {
            let nir = get_nir_mut(nid, arena);
            nir.achieved_count(best);
            // TODO: reuse backing storage when doing this, with a scratch Vec<O::Build>.
            if let Some(built_names) = nir.add_name(db, ctx) {
                let seq = NameIR::rendered_ntbs_to_node(
                    built_names,
                    arena,
                    is_sort_key,
                    label_after_name,
                    built_label.as_ref(),
                );
                replace_single_child(nid, seq, arena);
            } else {
                break;
            }
            if also_expand {
                if let Some(expanded) =
                    expand_one_name_ir(db, ctx, &initial_refs, get_nir_mut(nid, arena), n as u32)
                {
                    let seq = NameIR::rendered_ntbs_to_node(
                        expanded,
                        arena,
                        is_sort_key,
                        label_after_name,
                        built_label.as_ref(),
                    );
                    replace_single_child(nid, seq, arena);
                }
            }
            IR::recompute_group_vars(root, arena);
            let new_count = total_ambiguity_number(arena);
            get_nir_mut(nid, arena).achieved_count(new_count);
            best = std::cmp::min(best, new_count);
        }
        // TODO: simply save the node id of the rolled-back nir, and restore it to position.
        let nir = get_nir_mut(nid, arena);
        if let Some(rolled_back) = get_nir_mut(nid, arena).rollback(db, ctx) {
            let new_seq = NameIR::rendered_ntbs_to_node(
                rolled_back,
                arena,
                is_sort_key,
                label_after_name,
                built_label.as_ref(),
            );
            replace_single_child(nid, new_seq, arena);
        }
        IR::recompute_group_vars(root, arena);
        best = total_ambiguity_number(arena);
    }
    best <= 1
}

fn expand_one_name_ir(
    db: &dyn IrDatabase,
    ctx: &CiteContext<'_, Markup>,
    refs_accepting: &[Atom],
    nir: &mut NameIR<Markup>,
    index: u32,
) -> Option<Vec<MarkupBuild>> {
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
    nir.intermediate_custom(
        &ctx.format,
        ctx.position.0,
        ctx.sort_key.is_some(),
        ctx.disamb_pass,
        None,
    )
}

fn disambiguate_add_givennames(
    db: &dyn IrDatabase,
    root: NodeId,
    arena: &mut IrArena<Markup>,
    ctx: &CiteContext<'_, Markup>,
    also_add: bool,
) -> Option<bool> {
    let _fmt = db.get_formatter();
    let refs = refs_accepting_cite(db, root, arena, ctx);
    let name_refs = list_all_name_blocks(root, arena);

    let is_sort_key = ctx.sort_key.is_some();
    for (n, nid) in name_refs.into_iter().enumerate() {
        let nir = get_nir_mut(nid, arena);

        let label_after_name = nir
            .names_inheritance
            .label
            .as_ref()
            .map_or(false, |x| x.after_name);
        let built_label = nir.built_label.clone();

        if let Some(expanded) = expand_one_name_ir(db, ctx, &refs, nir, n as u32) {
            let seq = NameIR::rendered_ntbs_to_node(
                expanded,
                arena,
                is_sort_key,
                label_after_name,
                built_label.as_ref(),
            );
            replace_single_child(nid, seq, arena);
        }
        // TODO: this is likely unnecessary
        IR::recompute_group_vars(root, arena);
    }
    if also_add {
        disambiguate_add_names(db, root, arena, ctx, true);
    }
    None
}

fn disambiguate_add_year_suffix(
    db: &dyn IrDatabase,
    root: NodeId,
    arena: &mut IrArena<Markup>,
    state: &mut IrState,
    ctx: &CiteContext<'_, Markup>,
    suffix: u32,
) {
    // First see if we can do it with an explicit one
    let hooks = IR::list_year_suffix_hooks(root, arena);
    let mut added_suffix = false;
    for &yid in &hooks {
        let (ys, _) = get_ys_mut(yid, arena);
        let sum: IrSum<Markup> = match &ys.hook {
            YearSuffixHook::Explicit(_) => ys.hook.render(ctx, suffix),
            _ => continue,
        };
        let gv = sum.1;
        let node = arena.new_node(sum);
        replace_single_child(yid, node, arena);
        let (ys, ys_gv) = get_ys_mut(yid, arena);
        *ys_gv = gv;
        ys.suffix_num = Some(suffix);
        added_suffix = true;
        break;
    }
    if added_suffix {
        return;
    }

    // Then attempt to do it for the ones that are embedded in date output
    for yid in hooks {
        let (ys, _) = get_ys_mut(yid, arena);
        let sum: IrSum<Markup> = match &ys.hook {
            YearSuffixHook::Plain => ys.hook.render(ctx, suffix),
            _ => continue,
        };
        let gv = sum.1;
        let node = arena.new_node(sum);
        yid.append(node, arena);
        let (ys, ys_gv) = get_ys_mut(yid, arena);
        *ys_gv = gv;
        ys.suffix_num = Some(suffix);
        break;
    }

    IR::recompute_group_vars(root, arena);
}

#[inline(never)]
fn disambiguate_true(
    db: &dyn IrDatabase,
    root: NodeId,
    arena: &mut IrArena<Markup>,
    state: &mut IrState,
    ctx: &CiteContext<'_, Markup>,
) {
    info!(
        "attempting to disambiguate {:?} ({}) with {:?}",
        ctx.cite_id, &ctx.reference.id, ctx.disamb_pass
    );
    let un = is_unambiguous(
        db,
        ctx.disamb_pass,
        root,
        arena,
        ctx.cite_id,
        &ctx.reference.id,
    );
    if un {
        return;
    }
    let cond_refs = list_all_cond_disambs(root, arena);
    for cid in cond_refs.into_iter() {
        if is_unambiguous(
            db,
            ctx.disamb_pass,
            root,
            arena,
            ctx.cite_id,
            &ctx.reference.id,
        ) {
            debug!("successfully disambiguated with Cond");
            break;
        }
        {
            let (cond, _) = get_cond_mut(cid, arena);
            let choose = cond.choose.clone();
            let new_node = choose.intermediate(db, state, ctx, arena);
            let gv = arena.get(new_node).unwrap().get().1;
            replace_single_child(cid, new_node, arena);
            let (cond, cond_gv) = get_cond_mut(cid, arena);
            cond.done = true;
            *cond_gv = gv;
        }
        IR::recompute_group_vars(root, arena);
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
    let mut arena = IrArena::new();
    let root = style.intermediate(db, &mut state, &ctx, &mut arena);
    let _fmt = db.get_formatter();
    let matching = refs_accepting_cite(db, root, &arena, &ctx);
    let irgen = IrGen::new(root, arena, matching, state);
    Arc::new(irgen)
}

enum IrGenCow {
    Arc(Arc<IrGen>),
    Cloned(IrGen),
}

impl IrGenCow {
    fn to_mut(&mut self) -> &mut IrGen {
        *self = match self {
            IrGenCow::Arc(arc) => {
                let (root, arena, state) = arc.as_ref().fresh_copy();
                IrGenCow::Cloned(IrGen {
                    root,
                    arena,
                    state,
                    matching_refs: Default::default(),
                })
            }
            IrGenCow::Cloned(gen) => return gen,
        };
        // takes Cloned branch next time
        self.to_mut()
    }
    fn into_arc(self) -> Arc<IrGen> {
        match self {
            IrGenCow::Arc(arc) => arc,
            IrGenCow::Cloned(gen) => Arc::new(gen),
        }
    }
}

impl std::ops::Deref for IrGenCow {
    type Target = IrGen;
    fn deref(&self) -> &Self::Target {
        match self {
            IrGenCow::Arc(arc) => arc.as_ref(),
            IrGenCow::Cloned(gen) => gen,
        }
    }
}

impl IrGenCow {
    fn disambiguate_add_names(&mut self, db: &dyn IrDatabase, ctx: &mut CiteContext<Markup>) {
        if ctx.style.citation.disambiguate_add_names {
            ctx.disamb_pass = Some(DisambPass::AddNames);
            // Clone ir0; disambiguate by adding names
            let mut cloned = self.to_mut();
            disambiguate_add_names(db, cloned.root, &mut cloned.arena, &ctx, false);
            cloned.matching_refs = refs_accepting_cite(db, cloned.root, &cloned.arena, &ctx);
        }
    }

    fn disambiguate_add_given_name(&mut self, db: &dyn IrDatabase, ctx: &mut CiteContext<Markup>) {
        if ctx.style.citation.disambiguate_add_givenname {
            let mut cloned = self.to_mut();
            let gndr = ctx.style.citation.givenname_disambiguation_rule;
            ctx.disamb_pass = Some(DisambPass::AddGivenName(gndr));
            let also_add_names = ctx.style.citation.disambiguate_add_names;
            disambiguate_add_givennames(db, cloned.root, &mut cloned.arena, &ctx, also_add_names);
            cloned.matching_refs = refs_accepting_cite(db, cloned.root, &cloned.arena, &ctx);
        }
    }
    fn disambiguate_add_year_suffix(&mut self, db: &dyn IrDatabase, ctx: &mut CiteContext<Markup>) {
        if ctx.style.citation.disambiguate_add_year_suffix {
            let year_suffix = match db.year_suffix_for(ctx.cite.ref_id.clone()) {
                Some(y) => y,
                _ => return,
            };
            let mut cloned = self.to_mut();
            ctx.disamb_pass = Some(DisambPass::AddYearSuffix(year_suffix));
            disambiguate_add_year_suffix(
                db,
                cloned.root,
                &mut cloned.arena,
                &mut cloned.state,
                &ctx,
                year_suffix,
            );
            cloned.matching_refs = refs_accepting_cite(db, cloned.root, &cloned.arena, &ctx);
        }
    }
    fn disambiguate_conditionals(&mut self, db: &dyn IrDatabase, ctx: &mut CiteContext<Markup>) {
        let mut cloned = self.to_mut();
        ctx.disamb_pass = Some(DisambPass::Conditionals);
        disambiguate_true(db, cloned.root, &mut cloned.arena, &mut cloned.state, &ctx);
        // No point recomputing when nothing more can be done.
        cloned.matching_refs = Vec::new();
    }
}

/// Starts with ir_gen0, and disambiguates through add_names and add_givenname
fn ir_gen2_add_given_name(db: &dyn IrDatabase, id: CiteId) -> Arc<IrGen> {
    let style;
    let locale;
    let cite;
    let refr;
    let mut ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);

    let mut irgen = IrGenCow::Arc(db.ir_gen0(id));
    if irgen.unambiguous() {
        return irgen.into_arc();
    }
    irgen.disambiguate_add_names(db, &mut ctx);
    if irgen.unambiguous() {
        return irgen.into_arc();
    }
    irgen.disambiguate_add_given_name(db, &mut ctx);
    irgen.into_arc()
}

fn ir_fully_disambiguated(db: &dyn IrDatabase, id: CiteId) -> Arc<IrGen> {
    let style;
    let locale;
    let cite;
    let refr;
    let mut ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);

    // Start with the given names done.
    let mut irgen = IrGenCow::Arc(db.ir_gen2_add_given_name(id));
    if irgen.unambiguous() {
        return irgen.into_arc();
    }
    irgen.disambiguate_add_year_suffix(db, &mut ctx);
    if irgen.unambiguous() {
        return irgen.into_arc();
    }
    irgen.disambiguate_conditionals(db, &mut ctx);
    irgen.into_arc()
}

fn get_piq(db: &dyn IrDatabase) -> bool {
    // We pant PIQ to be global in a document, not change within a cluster because one cite
    // decided to use a different language. Use the default locale to get it.
    let default_locale = db.default_locale();
    default_locale
        .options_node
        .punctuation_in_quote
        .unwrap_or(false)
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

pub fn built_cluster_preview(
    db: &dyn IrDatabase,
    cluster_id: ClusterId,
    formatter: &Markup,
) -> Arc<<Markup as OutputFormat>::Output> {
    let build = built_cluster_before_output(db, cluster_id);
    let string = formatter.output(build, get_piq(db));
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
    use crate::ir::transforms::{group_and_collapse, CnumIx, RangePiece, Unnamed3};
    let mut irs: Vec<_> = cite_ids
        .iter()
        .map(|&id| {
            let gen4 = db.ir_fully_disambiguated(id);
            let cite = id.lookup(db);
            let (_keys, citation_numbers_by_id) = &*sorted_refs_arc;
            let cnum = citation_numbers_by_id.get(&cite.ref_id).cloned();
            Unnamed3::new(cite, cnum, gen4)
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
        if style.class != csl::StyleClass::InText
            && cite.prefix.as_ref().map_or(true, |pre| {
                // bugreports_CapsAfterOneWordPrefix
                let mut words = Words::new(pre, |s| s.chars().any(|c| c.is_alphanumeric()));
                words.next();
                let is_single_word = words.next().is_none();
                (pre.is_empty() || pre.trim_end().ends_with(".")) && !is_single_word
            })
        {
            let gen_mut = Arc::make_mut(gen4);
            IR::capitalize_first_term_of_cluster(gen_mut.root, &mut gen_mut.arena, &fmt);
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
        let ends_punc = |string: &str| {
            string
                .chars()
                .rev()
                .nth(0)
                .map_or(false, |x| x == ',' || x == '.' || x == '?' || x == '!')
        };
        let starts_punc = |string: &str| {
            string
                .chars()
                .nth(0)
                .map_or(false, |x| x == ',' || x == '.' || x == '?' || x == '!')
        };

        // "2000 is one source,; David Jones" => "2000 is one source, David Jones"
        // "2000;, and David Jones" => "2000, and David Jones"
        ends_punc(this_suffix) || starts_punc(next_prefix)
    };

    let build_cite = |cites: &[Unnamed3<Markup>], ix: usize| -> Option<MarkupBuild> {
        let Unnamed3 { cite, gen4, .. } = cites.get(ix)?;
        use std::borrow::Cow;
        let flattened = IR::flatten(gen4.root, &gen4.arena, &fmt)?;
        let mut pre = Cow::from(cite.prefix.as_ref().map(AsRef::as_ref).unwrap_or(""));
        let mut suf = Cow::from(cite.suffix.as_ref().map(AsRef::as_ref).unwrap_or(""));
        if !pre.is_empty() && !pre.ends_with(' ') {
            let pre_mut = pre.to_mut();
            pre_mut.push(' ');
        }
        let suf_first = suf.chars().nth(0);
        if suf_first.map_or(false, |x| {
            x != ' ' && !citeproc_io::output::markup::is_punc(x)
        }) {
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

    let cgroup_delim = style
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
    let render_range =
        |ranges: &[RangePiece], group_delim: &str, outer_delim: &str| -> (MarkupBuild, usize) {
            let mut advance_to = 0usize;
            let mut group: Vec<MarkupBuild> = Vec::with_capacity(ranges.len());
            for (ix, piece) in ranges.iter().enumerate() {
                let is_last = ix == ranges.len() - 1;
                match *piece {
                    RangePiece::Single(CnumIx {
                        ix, force_single, ..
                    }) => {
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
        let Unnamed3 {
            vanished,
            collapsed_ranges,
            is_first,
            ..
        } = &irs[ix];
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
                    let (built, advance_to) =
                        render_range(&r.collapsed_year_suffixes, ysuf_delim, acol_delim);
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
        fmt.affixed(fmt.group(built_cites, "", None), layout.affixes.as_ref()),
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
            let mut arena = IrArena::new();
            let mut root = bib.intermediate(db, &mut state, &ctx, &mut arena);

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
                disambiguate_add_year_suffix(db, root, &mut arena, &mut state, &ctx, suffix);
            }

            if bib.second_field_align == Some(csl::SecondFieldAlign::Flush) {
                if let Some(new_root) = IR::split_first_field(root, &mut arena) {
                    root = new_root;
                }
            }

            let matching = refs_accepting_cite(db, root, &arena, &ctx);
            Arc::new(IrGen::new(root, arena, matching, state))
        },
    )
}

fn bib_item(db: &dyn IrDatabase, ref_id: Atom) -> Arc<MarkupOutput> {
    let fmt = db.get_formatter();
    let style = db.style();
    if let Some(gen0) = db.bib_item_gen0(ref_id) {
        let layout = &style.bibliography.as_ref().unwrap().layout;
        let ir = &gen0.arena;
        let flat = IR::flatten(gen0.root, &gen0.arena, &fmt).unwrap_or_else(|| fmt.plain(""));
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
    let mut m =
        FnvHashMap::with_capacity_and_hasher(sorted_refs.0.len(), fnv::FnvBuildHasher::default());
    let mut prev: Option<(NodeId, Arc<IrGen>)> = None;
    for key in sorted_refs.0.iter() {
        // TODO: put Nones in there so they can be updated
        if let Some(mut gen0) = db.bib_item_gen0(key.clone()) {
            let layout = &style.bibliography.as_ref().unwrap().layout;
            // in a bibliography, we do the affixes etc inside Layout, so they're not here
            let current = IR::first_name_block(gen0.root, &gen0.arena);
            let sas = style.bibliography.as_ref().and_then(|bib| {
                bib.subsequent_author_substitute
                    .as_ref()
                    .map(|x| (x.as_ref(), bib.subsequent_author_substitute_rule))
            });
            if let (Some(prev_name_block), Some(current_name_block), Some((sas, sas_rule))) = (
                prev.as_ref()
                    .and_then(|(first_block, gen)| gen.arena.get(*first_block)),
                current,
                sas,
            ) {
                let mutated = Arc::make_mut(&mut gen0);
                let did = crate::transforms::subsequent_author_substitute(
                    &fmt,
                    // In order to unwrap this here, you must only replace the NameIR node's
                    // children, not the IR.
                    prev_name_block.get().0.unwrap_name_ir(),
                    current_name_block,
                    &mut mutated.arena,
                    sas,
                    sas_rule,
                );
                if did {
                    IR::recompute_group_vars(mutated.root, &mut mutated.arena);
                }
            }
            let flat = IR::flatten(gen0.root, &gen0.arena, &fmt).unwrap_or_else(|| fmt.plain(""));
            let string = fmt.output(flat, get_piq(db));
            if !string.is_empty() {
                m.insert(key.clone(), Arc::new(string));
            }
            prev = current.map(|cur| (cur, gen0));
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

    for cluster in clusters.iter() {
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
