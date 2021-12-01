// This Source Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

// For the query group macro expansion
#![allow(clippy::large_enum_variant)]

use fnv::FnvHashMap;
use std::sync::Arc;

use crate::cluster;
use crate::disamb::names::{replace_single_child, NameDisambPass};
use crate::disamb::{Dfa, DisambName, DisambNameData, EdgeData, FreeCondSets};
use crate::prelude::*;
use crate::sort::BibNumber;
use crate::{CiteContext, DisambPass, IrState, Proc, IR};
use citeproc_db::{CiteData, ClusterData, ClusterId, ClusterNumber, IntraNote};
use citeproc_io::output::{markup::Markup, OutputFormat};
use citeproc_io::{Cite, Name, Reference};
use csl::GivenNameDisambiguationRule as GNDR;
use csl::{Atom, Bibliography, Position, SortKey};

use indextree::NodeId;

pub trait ImplementationDetails {
    fn get_formatter(&self) -> Markup;
    fn lookup_cluster_id(&self, symbol: ClusterId) -> Option<SmartString>;
}

// trait ParallelIrDatabase {
//     fn snapshot(&self) -> salsa::Snapshot<&(dyn IrDatabase + 'static)>;
// }

#[salsa::query_group(IrDatabaseStorage)]
pub trait IrDatabase:
    CiteDatabase + LocaleDatabase + StyleDatabase + ImplementationDetails
{
    fn ref_dfa(&self, key: Atom) -> Option<Arc<Dfa>>;
    #[salsa::transparent]
    fn all_ref_dfas(&self) -> Arc<FnvHashMap<Atom, Arc<Dfa>>>;

    // TODO: cache this
    // #[salsa::invoke(crate::disamb::create_ref_ir)]
    // fn ref_ir(&self, key: Atom) -> Arc<Vec<(FreeCond, RefIR)>>;

    // Cache the most expensive thing, dfa.accepts_data() on the same edge streams over and over
    fn edge_stream_matches_ref(&self, edges: Vec<EdgeData>, ref_id: Atom) -> bool;

    // If these don't run any additional disambiguation, they just clone the
    // previous ir's Arc.
    fn ir_gen0(&self, key: CiteId) -> Arc<IrGen>;
    fn ir_gen2_add_given_name(&self, key: CiteId) -> Arc<IrGen>;
    fn ir_gen2_matching_refs(&self, id: CiteId) -> Arc<Vec<Atom>>;
    fn year_suffixes(&self) -> Arc<FnvHashMap<Atom, u32>>;
    fn year_suffix_for(&self, ref_id: Atom) -> Option<u32>;
    fn ir_fully_disambiguated(&self, key: CiteId) -> Arc<IrGen>;
    fn built_cluster(&self, key: ClusterId) -> Arc<MarkupOutput>;

    /// render the `<intext>` element on demand
    fn intext(&self, key: CiteId) -> Option<Arc<IrGen>>;

    fn bib_item_gen0(&self, ref_id: Atom) -> Option<Arc<IrGen>>;
    fn bib_item(&self, ref_id: Atom) -> Arc<MarkupOutput>;
    fn get_bibliography_map(&self) -> Arc<FnvHashMap<Atom, Arc<MarkupOutput>>>;

    fn branch_runs(&self) -> Arc<FreeCondSets>;

    /// For all refs, for all name configurations, for each name, produce one DisambNameData.
    fn all_person_names(&self) -> Arc<Vec<DisambNameData>>;

    /// The *Data indexed here are ratcheted as far as was required to do global name
    /// disambiguation.
    #[salsa::invoke(crate::disamb::names::disambiguated_person_names)]
    fn disambiguated_person_names(&self) -> Arc<FnvHashMap<DisambName, NameDisambPass>>;

    /// The DisambNameData here correspond to "global identity" -- so each DisambName points to
    /// exactly one Ref/NameEl/Variable/PersonName. Even if there are two identical NameEls
    /// rendering the same name, that's fine, because they would each have the same global
    /// disambiguation done.
    ///
    /// After global disambiguation, any modifications to DisambNameData are stored within the IR.
    #[salsa::interned]
    fn disamb_name(&self, e: DisambNameData) -> DisambName;

    // Sorting

    // Includes intra-cluster sorting
    #[salsa::invoke(crate::sort::clusters_cites_sorted)]
    fn clusters_cites_sorted(&self) -> Arc<Vec<ClusterData>>;

    #[salsa::invoke(crate::sort::cluster_data_sorted)]
    fn cluster_data_sorted(&self, id: ClusterId) -> Option<ClusterData>;

    /// Masks changes in note number
    fn cluster_cites_sorted(&self, id: ClusterId) -> Option<Arc<Vec<CiteId>>>;

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
    fn sorted_refs(&self) -> Arc<(Vec<Atom>, FnvHashMap<Atom, BibNumber>)>;
    #[salsa::input]
    fn bibliography_no_sort(&self) -> bool;

    #[salsa::invoke(crate::sort::bib_number)]
    fn bib_number(&self, id: CiteId) -> Option<BibNumber>;
}

pub fn safe_default(db: &mut dyn IrDatabase) {
    db.set_bibliography_no_sort_with_durability(false, salsa::Durability::HIGH);
}

fn all_person_names(db: &dyn IrDatabase) -> Arc<Vec<DisambNameData>> {
    let style = db.style();
    let rule = style.citation.givenname_disambiguation_rule;
    let name_configurations = db.name_configurations();
    let refs = db.disamb_participants();
    let mut collector = Vec::new();
    // -> for each ref
    //    for each <names var="v" />
    //    for each name in ref["v"]
    //    .. push a DisambNameData
    for ref_id in refs.iter() {
        if let Some(refr) = db.reference(ref_id.clone()) {
            for (var, el) in name_configurations.iter() {
                if let Some(names) = refr.name.get(&var) {
                    let mut seen_one = false;
                    // fullstyles_APA.txt
                    let all_same_family_name = (rule == GNDR::PrimaryName
                        || rule == GNDR::PrimaryNameWithInitials)
                        && el.form == Some(csl::NameForm::Short)
                        && crate::disamb::names::all_same_family_name(names);
                    for name in names {
                        if let Name::Person(val) = name {
                            collector.push(DisambNameData {
                                ref_id: ref_id.clone(),
                                var: *var,
                                el: el.clone(),
                                value: val.clone(),
                                primary: !seen_one,
                                all_same_family_name,
                            })
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

fn all_ref_dfas(db: &dyn IrDatabase) -> Arc<FnvHashMap<Atom, Arc<Dfa>>> {
    let map = db
        .disamb_participants()
        .iter()
        .filter_map(|key| db.ref_dfa(key.clone()).map(|v| (key.clone(), v)))
        .collect();
    Arc::new(map)
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
    let (refs, bib_numbers) = &*sorted_refs;
    refs.iter()
        .map(|id| {
            let cite = db.ghost_cite(id.clone());
            let cite_id = db.cite(CiteData::BibliographyGhost { cite });
            (id.clone(), db.ir_gen2_matching_refs(cite_id))
        })
        .for_each(|(ref_id, ir2_matching_refs)| {
            // if matching refs <= 1, then it's unambiguous
            if ir2_matching_refs.len() <= 1 {
                // no need to check if own id is in a group, it will receive a suffix already
            } else {
                // we make sure ref_id is included, even if there was a bug with RefIR and a
                // cite didn't match its own reference
                let mut coalesce: Option<(usize, FnvHashSet<Atom>)> = None;
                for (n, group) in groups.iter_mut().enumerate() {
                    if group.contains(&ref_id) || intersects(group, &ir2_matching_refs) {
                        group.insert(ref_id.clone());
                        for id in ir2_matching_refs.iter() {
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
        vec.sort_by_key(|ref_id| ref_bib_number(bib_numbers, ref_id));
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
fn ref_bib_number(bib_numbers: &FnvHashMap<Atom, BibNumber>, ref_id: &Atom) -> u32 {
    let ret = bib_numbers.get(ref_id).cloned();
    if let Some(ret) = ret {
        ret.get()
    } else {
        error!(
            "called ref_bib_number on a ref_id {} that is unknown/not in the bibliography",
            ref_id
        );
        // Let's not fail, just give it one after the rest.
        std::u32::MAX
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct IrGen {
    pub(crate) tree: IrTree<Markup>,
    pub(crate) state: IrState,
    pub(crate) used_disambiguate_true: bool,
    pub(crate) disambiguation_finished: bool,
}

use std::fmt;
impl fmt::Debug for IrGen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("IrGen");
        dbg.field("tree", &self.tree);
        dbg.field("state", &self.state);
        dbg.finish()
    }
}

impl IrGen {
    pub(crate) fn new(tree: IrTree<Markup>, state: IrState, disambiguation_finished: bool) -> Self {
        IrGen {
            tree,
            state,
            used_disambiguate_true: false,
            disambiguation_finished,
        }
    }
    pub(crate) fn tree(&self) -> &IrTree {
        &self.tree
    }
    pub(crate) fn tree_ref(&self) -> IrTreeRef {
        self.tree.tree_ref()
    }
    pub(crate) fn tree_mut(&mut self) -> &mut IrTree {
        &mut self.tree
    }
}

fn ref_not_found(db: &dyn IrDatabase, ref_id: &Atom, log: bool) -> Arc<IrGen> {
    if log {
        info!("citeproc-rs: reference {} not found", ref_id);
    }
    let mut arena = IrArena::new();
    let root = arena.new_node((
        IR::Rendered(Some(CiteEdgeData::Output(db.get_formatter().plain("???")))),
        GroupVars::Plain,
    ));
    Arc::new(IrGen::new(IrTree::new(root, arena), IrState::new(), true))
}

// IR gen0 depends on:
// style
// cite
// reference
// cite_position
//  -
// bib_number
//  - sorted_refs
macro_rules! preamble {
    ($style:ident, $locale:ident, $cite:ident, $refr:ident, $ctx:ident, $db:expr, $id:expr, $pass:expr) => {{
        $style = $db.style();
        $locale = $db.default_locale();
        // Avoid making bibliography ghosts all depend any positional / note num info
        let cite_stuff = match $db.lookup_cite($id) {
            CiteData::RealCite { cite, .. } => (cite, $db.cite_position($id)),
            // Subsequent because: disambiguate_BasedOnEtAlSubsequent.txt
            // The position being Some(1) is so the ghost entries don't emit nothing where every
            // normal reference would emit a first-reference-note-number. You'll never see this
            // value as output.
            CiteData::BibliographyGhost { cite, .. } => (cite, (Position::Subsequent, Some(1))),
        };
        $cite = cite_stuff.0;
        let position = cite_stuff.1;
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
            position,
            disamb_pass: $pass,
            style: &$style,
            locale: &$locale,
            bib_number: $db.bib_number($id).map(|x| x.get()),
            in_bibliography: false,
            names_delimiter,
            name_citation: name_el,
            sort_key: None,
            year_suffix: None,
        };
    }};
}

macro_rules! cfg_par_iter {
    ($expr:expr) => {{
        #[cfg(feature = "rayon")]
        {
            use rayon::prelude::*;
            ($expr).par_iter()
        }
        #[cfg(not(feature = "rayon"))]
        {
            ($expr).iter()
        }
    }};
}

macro_rules! cfg_rayon {
    ($rayon:expr, $not:expr) => {{
        #[cfg(feature = "rayon")]
        {
            $rayon
        }
        #[cfg(not(feature = "rayon"))]
        {
            $not
        }
    }};
}

fn is_unambiguous(db: &dyn IrDatabase, tree: IrTreeRef, self_id: &Atom) -> bool {
    struct OtherRef;

    let fmt = db.get_formatter();
    let edges = tree.to_edge_stream(&fmt);

    // Participants could be 100 different references, each with quite a lot of CPU work to do.
    // A possible improvement would be to check the ones that are likely to collide first, so
    // that the short circuit can be quicker.

    #[cfg(feature = "rayon")]
    use rayon::prelude::*;

    let ref_dfas = db.all_ref_dfas();

    #[allow(unused_mut)]
    let mut iter = cfg_par_iter!(ref_dfas);

    // THe bool -> true means matched self
    let res = iter.try_fold(cfg_rayon!(|| false, false), |accumulate: bool, (k, dfa)| {
        let accepts = dfa.accepts_data(&edges);
        if accepts && k == self_id {
            Ok(true)
        } else if accepts {
            Err(OtherRef)
        } else {
            Ok(false || accumulate)
        }
    });
    let res = cfg_rayon!(res.try_reduce(|| false, |a, b| Ok(a || b)), res);
    res.is_ok()
}

fn edge_stream_matches_ref(db: &dyn IrDatabase, edges: Vec<EdgeData>, ref_id: Atom) -> bool {
    if let Some(dfa) = db.ref_dfa(ref_id) {
        dfa.accepts_data(&edges)
    } else {
        false
    }
}

/// Returns the set of Reference IDs that could have produced a cite's IR
fn refs_accepting_cite(
    db: &dyn IrDatabase,
    tree: IrTreeRef,
    cite_id: Option<CiteId>,
    ref_id: &Atom,
    disamb_pass: Option<DisambPass>,
) -> Vec<Atom> {
    // Out of ctx, we need:
    // - cite_id
    // - reference.id
    // - disamb_pass (for debug)
    let edges = tree.to_edge_stream(&db.get_formatter());
    let participants = db.disamb_participants();
    // #[cfg(feature = "rayon")]
    // use rayon::prelude::*;

    let iter = participants.iter();

    let ret: Vec<Atom> = iter
        .filter_map(|k| {
            let acc = db.edge_stream_matches_ref(edges.clone(), k.clone());
            if log_enabled!(log::Level::Trace) && k != ref_id && acc {
                trace!(
                    "{:?}: matched other reference {} during pass {:?}",
                    cite_id,
                    k,
                    disamb_pass
                );
            }
            if acc {
                Some(k.clone())
            } else {
                None
            }
        })
        .collect();

    if !ret.contains(ref_id) {
        let dfa = db.ref_dfa(ref_id.clone()).unwrap();
        error!(
            "{:?}: own reference {} did not match during pass {:?}:\n{}\n{:?}",
            cite_id,
            ref_id,
            disamb_pass,
            dfa.debug_graph(db),
            edges
        );
    }
    ret
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
    trace!("searching for the nth {} name block", index);
    let mut nth = index;
    find_name_block(&ref_ir, &mut nth).cloned()
}

fn list_all_name_blocks(tree: IrTreeRef) -> Vec<NodeId> {
    fn list_all_name_blocks_inner(tree: IrTreeRef, vec: &mut Vec<NodeId>) {
        let me = match tree.get_node() {
            Some(x) => x.get(),
            None => return,
        };
        match me.0 {
            IR::NameCounter(_) | IR::YearSuffix(..) | IR::Rendered(_) => {}
            IR::Name(_) => {
                vec.push(tree.node);
            }
            IR::ConditionalDisamb(_) | IR::Seq(_) | IR::Substitute => {
                // assumes it's the first one that appears
                for child in tree.children() {
                    list_all_name_blocks_inner(child, vec);
                }
            }
        }
    }
    let mut vec = Vec::new();
    list_all_name_blocks_inner(tree, &mut vec);
    vec
}

fn list_all_cond_disambs(tree: IrTreeRef) -> Vec<NodeId> {
    fn list_all_cd_inner(tree: IrTreeRef, vec: &mut Vec<NodeId>) {
        let me = match tree.get_node() {
            Some(x) => x.get(),
            None => return,
        };
        match &me.0 {
            IR::NameCounter(_) | IR::YearSuffix(..) | IR::Rendered(_) | IR::Name(_) => {
                return;
            }
            IR::ConditionalDisamb(_c) => {
                vec.push(tree.node);
            }
            IR::Seq(_) | IR::Substitute => {}
        }
        tree.children()
            .for_each(|child| list_all_cd_inner(child, vec));
    }
    let mut vec = Vec::new();
    list_all_cd_inner(tree, &mut vec);
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
    tree: &mut IrTree,
    ctx: &mut CiteContext<'_, Markup>,
    also_expand: bool,
) -> bool {
    ctx.disamb_pass = Some(DisambPass::AddNames);

    let fmt = &db.get_formatter();
    // We're going to assume, for a bit of a boost, that you can't ever match a ref not in
    // initial_refs after adding names. We'll see how that holds up.
    let initial_refs = refs_accepting_cite(
        db,
        tree.tree_ref(),
        ctx.cite_id,
        &ctx.reference.id,
        ctx.disamb_pass,
    );
    let mut best = initial_refs.len() as u16;
    let name_refs = list_all_name_blocks(tree.tree_ref());

    debug!(
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

        let total_ambiguity_number = |tree: IrTreeRef<Markup>| -> u16 {
            // unlock the nir briefly, so we can access it during to_edge_stream
            let edges = tree.to_edge_stream(fmt);
            let count = dfas.iter().filter(|dfa| dfa.accepts_data(&edges)).count() as u16;
            if count == 0 {
                warn!("should not get to zero matching refs");
            }
            count
        };

        // So we can roll back to { bump = 0 }
        let nir = get_nir_mut(nid, &mut tree.arena);
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
            let nir = get_nir_mut(nid, &mut tree.arena);
            nir.achieved_count(best);
            // TODO: reuse backing storage when doing this, with a scratch Vec<O::Build>.
            if let Some(built_names) = nir.add_name(db, ctx) {
                let seq = NameIR::rendered_ntbs_to_node(
                    built_names,
                    &mut tree.arena,
                    is_sort_key,
                    label_after_name,
                    built_label.as_ref(),
                );
                tree.replace_single_child(nid, seq);
            } else {
                break;
            }
            if also_expand {
                if let Some(expanded) = expand_one_name_ir(
                    db,
                    ctx,
                    &initial_refs,
                    get_nir_mut(nid, &mut tree.arena),
                    n as u32,
                ) {
                    let seq = NameIR::rendered_ntbs_to_node(
                        expanded,
                        &mut tree.arena,
                        is_sort_key,
                        label_after_name,
                        built_label.as_ref(),
                    );
                    tree.replace_single_child(nid, seq);
                }
            }
            tree.recompute_group_vars();
            let new_count = total_ambiguity_number(tree.tree_ref());
            get_nir_mut(nid, &mut tree.arena).achieved_count(new_count);
            best = std::cmp::min(best, new_count);
        }
        // TODO: simply save the node id of the rolled-back nir, and restore it to position.
        if let Some(rolled_back) = get_nir_mut(nid, &mut tree.arena).rollback(db, ctx) {
            let new_seq = NameIR::rendered_ntbs_to_node(
                rolled_back,
                &mut tree.arena,
                is_sort_key,
                label_after_name,
                built_label.as_ref(),
            );
            tree.replace_single_child(nid, new_seq);
        }
        tree.recompute_group_vars();
        best = total_ambiguity_number(tree.tree_ref());
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
                // This is ad-hoc RefIR, so we don't want it to have global disamb applied already.
                // disambiguage_AndreaEg2
                let dn = id.lookup(db);
                let matcher = NameVariantMatcher::from_disamb_name(db, dn);
                if let Some(slot) = double_vec.get_mut(n) {
                    slot.push(matcher);
                }
            }
        }
    }
    use crate::disamb::names::MatchKey;

    let name_ambiguity_number =
        |edge: &EdgeData, match_key: Option<&MatchKey>, slot: &[NameVariantMatcher]| -> u32 {
            slot.iter()
                .filter(|matcher| matcher.accepts(edge, None))
                .count() as u32
        };

    let mut n = 0usize;
    for dnr in nir.disamb_names.iter_mut() {
        if let DisambNameRatchet::Person(ratchet) = dnr {
            if let Some(ref slot) = double_vec.get(n) {
                // First, get the initial count
                /* TODO: store format stack */
                let mut edge = ratchet.data.single_name_edge(db, Formatting::default());
                let key = ratchet.data.family_match_key();
                let mut min = name_ambiguity_number(&edge, key.as_ref(), slot);
                trace!("nan for {}-th ({:?}) initially {}", n, edge, min);
                let mut stage_dn = ratchet.data.clone();
                // Then, try to improve it
                let mut iter = ratchet.iter;
                while min > 1 {
                    if let Some(next) = iter.next() {
                        stage_dn.apply_upto_pass(next);
                        edge = stage_dn.single_name_edge(db, Formatting::default());
                        let new_count = name_ambiguity_number(&edge, key.as_ref(), slot);
                        if new_count < min {
                            // save the improvement
                            min = new_count;
                            ratchet.data = stage_dn.clone();
                            ratchet.iter = iter;
                        }
                        trace!("nan for {}-th ({:?}) got to {}", n, edge, min);
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
    tree: &mut IrTree,
    ctx: &mut CiteContext<'_, Markup>,
    also_add: bool,
) -> Option<bool> {
    ctx.disamb_pass = Some(DisambPass::AddGivenName(
        ctx.style.citation.givenname_disambiguation_rule,
    ));
    let _fmt = db.get_formatter();
    let refs = refs_accepting_cite(
        db,
        tree.tree_ref(),
        ctx.cite_id,
        &ctx.reference.id,
        ctx.disamb_pass,
    );
    let name_refs = list_all_name_blocks(tree.tree_ref());

    let is_sort_key = ctx.sort_key.is_some();
    for (n, nid) in name_refs.into_iter().enumerate() {
        let nir = get_nir_mut(nid, &mut tree.arena);

        let label_after_name = nir
            .names_inheritance
            .label
            .as_ref()
            .map_or(false, |x| x.after_name);
        let built_label = nir.built_label.clone();

        if let Some(expanded) = expand_one_name_ir(db, ctx, &refs, nir, n as u32) {
            let seq = NameIR::rendered_ntbs_to_node(
                expanded,
                &mut tree.arena,
                is_sort_key,
                label_after_name,
                built_label.as_ref(),
            );
            tree.replace_single_child(nid, seq);
        }
        // TODO: this is likely unnecessary
        tree.recompute_group_vars();
    }
    if also_add {
        disambiguate_add_names(db, tree, ctx, true);
    }
    None
}

fn disambiguate_add_year_suffix(tree: &mut IrTree, ctx: &CiteContext<'_, Markup>, suffix: u32) {
    // First see if we can do it with an explicit one
    let hooks = tree.tree_ref().list_year_suffix_hooks();
    let mut added_suffix = false;
    for &yid in &hooks {
        let (ys, _) = get_ys_mut(yid, &mut tree.arena);
        let sum: IrSum<Markup> = match &ys.hook {
            YearSuffixHook::Explicit(_) => ys.hook.render(ctx, suffix),
            _ => continue,
        };
        let gv = sum.1;
        let node = tree.arena.new_node(sum);
        tree.replace_single_child(yid, node);
        let (ys, ys_gv) = get_ys_mut(yid, &mut tree.arena);
        *ys_gv = gv;
        ys.suffix_num = Some(suffix);
        added_suffix = true;
        break;
    }
    if added_suffix {
        tree.recompute_group_vars();
        return;
    }

    // Then attempt to do it for the ones that are embedded in date output
    for yid in hooks {
        let (ys, _) = get_ys_mut(yid, &mut tree.arena);
        let sum: IrSum<Markup> = match &ys.hook {
            // This produces GroupVars::Important
            YearSuffixHook::Plain => ys.hook.render(ctx, suffix),
            _ => continue,
        };
        let gv = sum.1;
        let node = tree.arena.new_node(sum);
        yid.append(node, &mut tree.arena);
        let (ys, ys_gv) = get_ys_mut(yid, &mut tree.arena);
        *ys_gv = gv;
        ys.suffix_num = Some(suffix);
        break;
    }

    tree.recompute_group_vars();
}

#[inline(never)]
fn disambiguate_true(
    db: &dyn IrDatabase,
    tree: &mut IrTree,
    state: &mut IrState,
    ctx: &CiteContext<'_, Markup>,
) {
    debug!(
        "attempting to disambiguate {:?} ({}) with {:?}",
        ctx.cite_id, &ctx.reference.id, ctx.disamb_pass
    );
    let un = is_unambiguous(db, tree.tree_ref(), &ctx.reference.id);
    if un {
        return;
    }
    let cond_refs = list_all_cond_disambs(tree.tree_ref());
    for cid in cond_refs.into_iter() {
        if is_unambiguous(db, tree.tree_ref(), &ctx.reference.id) {
            debug!("successfully disambiguated with Cond");
            break;
        }
        {
            let arena = &mut tree.arena;
            let (cond, _) = get_cond_mut(cid, arena);
            let choose = cond.choose.clone();
            let new_node = choose.intermediate(db, state, ctx, arena);
            let gv = arena.get(new_node).unwrap().get().1;
            replace_single_child(cid, new_node, arena);
            let (cond, cond_gv) = get_cond_mut(cid, arena);
            cond.done = true;
            *cond_gv = gv;
        }
        tree.recompute_group_vars();
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
    let root = style
        .citation
        .intermediate(db, &mut state, &ctx, &mut arena);
    let irgen = IrGen::new(IrTree::new(root, arena), state, false);
    log::debug!("ir_gen0: {}", irgen.tree);
    Arc::new(irgen)
}

fn ir_gen2_matching_refs(db: &dyn IrDatabase, id: CiteId) -> Arc<Vec<Atom>> {
    let style = db.style();
    let gndr = style.citation.givenname_disambiguation_rule;
    let cite = id.lookup(db);
    let gen2 = db.ir_gen2_add_given_name(id);
    let refs = refs_accepting_cite(
        db,
        gen2.tree_ref(),
        Some(id),
        &cite.ref_id,
        Some(DisambPass::AddGivenName(gndr)),
    );
    Arc::new(refs)
}

struct IrGenCow {
    arc: Arc<IrGen>,
}

impl IrGenCow {
    fn to_mut(&mut self) -> &mut IrGen {
        Arc::make_mut(&mut self.arc)
    }
    fn into_arc(self) -> Arc<IrGen> {
        self.arc
    }
}

use std::ops::Deref;
impl Deref for IrGenCow {
    type Target = IrGen;
    fn deref(&self) -> &Self::Target {
        self.arc.deref()
    }
}

impl IrGenCow {
    fn new(arc: Arc<IrGen>) -> Self {
        Self { arc }
    }
    /// Returned true indicates the cite is now unambiguous.
    fn disambiguate_add_names(&mut self, db: &dyn IrDatabase, ctx: &mut CiteContext<Markup>) {
        if self.disambiguation_finished {
            return;
        }
        if ctx.style.citation.disambiguate_add_names {
            // Clone ir0; disambiguate by adding names
            let cloned = self.to_mut();
            cloned.disambiguation_finished =
                disambiguate_add_names(db, cloned.tree_mut(), ctx, false);
        }
    }

    fn disambiguate_add_given_name(&mut self, db: &dyn IrDatabase, ctx: &mut CiteContext<Markup>) {
        if self.disambiguation_finished {
            return;
        }
        if ctx.style.citation.disambiguate_add_givenname {
            let cloned = self.to_mut();
            let also_add_names = ctx.style.citation.disambiguate_add_names;
            disambiguate_add_givennames(db, cloned.tree_mut(), ctx, also_add_names);
        }
    }
    fn disambiguate_add_year_suffix(&mut self, db: &dyn IrDatabase, ctx: &mut CiteContext<Markup>) {
        // the other disambiguate_ routines would exit here if disambiguation_finished was true,
        // but whether we apply year suffixes is actually unconditional at this point.
        // Year suffixes have been produced already through db.year_suffix_for(refId).
        if ctx.style.citation.disambiguate_add_year_suffix {
            let year_suffix = match db.year_suffix_for(ctx.cite.ref_id.clone()) {
                Some(y) => y,
                _ => return,
            };
            let cloned = self.to_mut();
            ctx.disamb_pass = Some(DisambPass::AddYearSuffix(year_suffix));
            disambiguate_add_year_suffix(cloned.tree_mut(), &ctx, year_suffix);
            // if it's already unambiguous on names alone, then adding year suffixes is hardly
            // going to improve it. So avoid the cost.
            if !self.disambiguation_finished {
                self.update_is_ambiguous(db, ctx);
            }
        }
    }

    fn update_is_ambiguous(&mut self, db: &dyn IrDatabase, ctx: &CiteContext<Markup>) {
        let gen = self.arc.deref();
        let unambiguous = is_unambiguous(db, gen.tree_ref(), &ctx.reference.id);
        if unambiguous != self.disambiguation_finished {
            self.to_mut().disambiguation_finished = unambiguous;
        }
    }

    fn disambiguate_conditionals(&mut self, db: &dyn IrDatabase, ctx: &mut CiteContext<Markup>) {
        if self.disambiguation_finished {
            return;
        }
        let cloned = self.to_mut();
        ctx.disamb_pass = Some(DisambPass::Conditionals);
        cloned.used_disambiguate_true = true;
        disambiguate_true(db, &mut cloned.tree, &mut cloned.state, &ctx);
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

    let mut irgen = IrGenCow::new(db.ir_gen0(id));
    irgen.update_is_ambiguous(db, &ctx);
    irgen.disambiguate_add_names(db, &mut ctx);
    irgen.disambiguate_add_given_name(db, &mut ctx);
    log::debug!("ir_gen2_add_given_name: {}", irgen.deref().tree);
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
    let mut irgen = IrGenCow::new(db.ir_gen2_add_given_name(id));
    irgen.disambiguate_add_year_suffix(db, &mut ctx);
    log::debug!("ir_add_year_suffix: {}", irgen.deref().tree);
    irgen.disambiguate_conditionals(db, &mut ctx);
    log::debug!("ir_fully_disambiguated: {}", irgen.deref().tree);
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
    let build = cluster::built_cluster_before_output(db, cluster_id, &fmt);
    let string = fmt.output(build, get_piq(db));
    Arc::new(string)
}

pub fn built_cluster_preview(
    db: &dyn IrDatabase,
    cluster_id: ClusterId,
    fmt: &Markup,
) -> Arc<<Markup as OutputFormat>::Output> {
    let build = cluster::built_cluster_before_output(db, cluster_id, &fmt);
    let string = fmt.output(build, get_piq(db));
    Arc::new(string)
}

#[test]
pub fn test_preview_unicode_escape_issue_91() {
    use crate::test::{test_style_layout, MockProcessor};
    use citeproc_io::{NumberLike, Reference};
    use csl::{CslType, NameVariable, NumberVariable, Variable};

    // ugh. this should be easier.

    let mut proc = MockProcessor::rtf();

    let style = test_style_layout(
        r#"
        <group delimiter=", ">
        <text prefix="text: " variable="title" />
        <names prefix="name: " variable="author" />
        <number prefix="number: " variable="page" />
        </group>
    "#,
    );
    proc.set_style_text(&style);

    let mut r = Reference::empty("id".into(), CslType::Book);
    r.ordinary.insert(Variable::Title, "Čotar".into());
    r.name.insert(
        NameVariable::Author,
        vec![citeproc_io::Name::Person(citeproc_io::PersonName {
            family: Some("Čotar".into()),
            ..Default::default()
        })],
    );
    r.number
        .insert(NumberVariable::Page, NumberLike::Str("Čotar".into()));
    proc.insert_references(vec![r]);

    let mut interner = string_interner::StringInterner::<ClusterId>::new();
    let cluster = interner.get_or_intern("cluster");
    proc.init_clusters(vec![(
        cluster,
        ClusterNumber::Note(IntraNote::Single(1)),
        vec![Cite::basic("id")],
    )]);

    // check the rtf (default)
    let built = proc.built_cluster(cluster);
    println!("{}", built);
    assert_eq!(
        built.as_str(),
        "text: \\uc0\\u268 otar, name: \\uc0\\u268 otar, number: \\uc0\\u268 otar"
    );

    let plain = Markup::plain();
    let preview = built_cluster_preview(&proc, cluster, &plain);
    println!("{}", preview);
    assert_eq!(preview.as_str(), "text: Čotar, name: Čotar, number: Čotar");
}

fn cluster_cites_sorted(db: &dyn IrDatabase, cluster_id: ClusterId) -> Option<Arc<Vec<CiteId>>> {
    db.cluster_data_sorted(cluster_id)
        .map(|data| data.cites.clone())
}

/// None if the reference being cited does not exist
pub fn with_cite_context<T>(
    db: &dyn IrDatabase,
    id: CiteId,
    bib_number: Option<u32>,
    sort_key: Option<SortKey>,
    default_position: bool,
    year_suffix: Option<u32>,
    f: impl FnOnce(CiteContext) -> T,
) -> Option<T> {
    let style = db.style();
    let locale = db.default_locale();
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
    refr: Option<&Reference>,
    bib_number: Option<u32>,
    sort_key: Option<SortKey>,
    year_suffix: Option<u32>,
    ref_present: impl FnOnce(&Bibliography, CiteContext) -> Option<T>,
    ref_missing: impl FnOnce(&Bibliography, CiteContext, bool) -> Option<T>,
) -> Option<T> {
    let style = db.style();
    let bib = style.bibliography.as_ref()?;
    let locale = db.default_locale();
    let cite = Cite::basic(ref_id.clone());
    let null_ref = citeproc_io::Reference::empty("empty_ref".into(), csl::CslType::Article);
    let (refr, is_ref_missing) = if let Some(r) = refr {
        (r, false)
    } else {
        (&null_ref, true)
    };
    let (names_delimiter, name_el) = db.name_info_bibliography();
    let ctx = CiteContext {
        reference: &refr,
        format: db.get_formatter(),
        cite_id: None,
        cite: &cite,
        position: (Position::First, None),
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
    if is_ref_missing {
        ref_missing(bib, ctx, false)
    } else {
        ref_present(bib, ctx.clone()).or_else(|| ref_missing(bib, ctx, true))
    }
}

fn bib_item_gen0(db: &dyn IrDatabase, ref_id: Atom) -> Option<Arc<IrGen>> {
    let sorted_refs_arc = db.sorted_refs();
    let (_keys, citation_numbers_by_id) = &*sorted_refs_arc;
    let bib_number = citation_numbers_by_id
        .get(&ref_id)
        .expect("sorted_refs should contain a bib_item key")
        .get();

    let refr_arc = db.reference(ref_id.clone());

    bib_item_gen0_acontextual(db, ref_id, refr_arc.as_deref(), Some(bib_number))
}

fn format_single_bib_item(ir_gen: Option<&IrGen>, fmt: &Markup, piq: bool) -> SmartString {
    ir_gen
        .and_then(|ir_gen| {
            let flat = ir_gen.tree_ref().flatten(&fmt, None)?;
            let string = fmt.output(flat, piq);
            if string.is_empty() {
                return None;
            }
            Some(string)
        })
        .unwrap_or_else(|| CSL_STYLE_ERROR.into())
}

fn bib_item(db: &dyn IrDatabase, ref_id: Atom) -> Arc<MarkupOutput> {
    let fmt = db.get_formatter();
    let gen0_arc = db.bib_item_gen0(ref_id);
    Arc::new(format_single_bib_item(
        gen0_arc.as_deref(),
        &fmt,
        get_piq(db),
    ))
}

/// Similar to bib_item, but uses a given Reference instead of a ref_id known to the db
/// And doesn't cache. And allows custom fmt arg.
pub fn bib_item_preview(
    db: &dyn IrDatabase,
    ref_id: Atom,
    refr: &Reference,
    fmt: &Markup,
) -> SmartString {
    // Pretend it's the first item in the bibliography
    let gen0_arc = bib_item_gen0_acontextual(db, ref_id, Some(refr), Some(1));
    format_single_bib_item(gen0_arc.as_deref(), fmt, get_piq(db))
}

fn bib_item_gen0_acontextual(
    db: &dyn IrDatabase,
    ref_id: Atom,
    refr: Option<&Reference>,
    bib_number: Option<u32>,
) -> Option<Arc<IrGen>> {
    with_bib_context(
        db,
        ref_id.clone(),
        refr,
        bib_number,
        None,
        None,
        |bib, mut ctx| {
            let mut state = IrState::new();
            let mut arena = IrArena::new();
            let root = bib.intermediate(db, &mut state, &ctx, &mut arena);
            let mut tree = IrTree { root, arena };

            // Immediately apply year suffixes.
            // Early-gen cites determine whether these exist -- but in the bibliography, we are already
            // aware of this, so they just need to be mirrored.
            //
            // Can't apply them the first time round, because IR may contain many suffix hooks, and we
            // need to only supply the first appearing explicit one, or the first appearing implicit one.
            // TODO: comply with the spec where "hook in cite == explicit => no implicit in bib" and "vice
            // versa"
            log::debug!("bib_ir_gen0: {}", tree);
            if let Some(suffix) = db.year_suffix_for(ref_id.clone()) {
                ctx.disamb_pass = Some(DisambPass::AddYearSuffix(suffix));
                disambiguate_add_year_suffix(&mut tree, &ctx, suffix);
                log::debug!("bib_ir add_year_suffix: {}", tree);
            }

            if first_cite_used_disambiguate_true(db, ref_id.clone()) {
                ctx.disamb_pass = Some(DisambPass::Conditionals);
                disambiguate_true(db, &mut tree, &mut state, &ctx);
                log::debug!("bib_ir disambiguate_true: {}", tree);
            }

            if bib.second_field_align == Some(csl::SecondFieldAlign::Flush) {
                if let Some(new_root) = IR::split_first_field(tree.root, &mut tree.arena) {
                    tree.root = new_root;
                }
            }

            // Pull affixes off layout into the right-inlines etc, after we may have created those
            // divs in split_first_field
            transforms::fix_left_right_layout_affixes(tree.root, &mut tree.arena);

            if tree.tree_ref().is_empty() {
                log::warn!("tree empty for bibliography ref {}", &ref_id);
                None
            } else {
                // Disambiguation is over already
                Some(Arc::new(IrGen::new(tree, state, true)))
            }
        },
        |bib, ctx, _just_empty_output| {
            let mut state = IrState::new();

            // Re the ? operator here: sort_omittedBibRefMixedNonNumericStyle.txt
            // If no citation-number found, simply exclude it from the bibliography.
            let (el_ref, maybe_delim) =
                transforms::style_is_mixed_numeric(ctx.style, CiteOrBib::Bibliography)?;
            // Render it as "1. [CSL STYLE ERROR ...]"
            let mut tree = {
                let mut arena = IrArena::new();
                let msg = ctx.format.plain(CSL_STYLE_ERROR);
                let msg_node = arena.new_node((
                    IR::Rendered(Some(CiteEdgeData::Output(msg))),
                    GroupVars::Important,
                ));
                let n = el_ref.intermediate(db, &mut state, &ctx, &mut arena);
                let seq = IrSeq {
                    delimiter: maybe_delim.map(Into::into),
                    ..Default::default()
                };
                let seq_node = arena.new_node((IR::Seq(seq), GroupVars::Important));
                seq_node.append(n, &mut arena);
                seq_node.append(msg_node, &mut arena);
                IrTree {
                    root: seq_node,
                    arena,
                }
            };

            if bib.second_field_align == Some(csl::SecondFieldAlign::Flush) {
                if let Some(new_root) = IR::split_first_field(tree.root, &mut tree.arena) {
                    tree.root = new_root;
                }
            }

            // Pull affixes off layout into the right-inlines etc, after we may have created those
            // divs in split_first_field
            transforms::fix_left_right_layout_affixes(tree.root, &mut tree.arena);

            // Disambiguation is over already
            Some(Arc::new(IrGen::new(tree, state, true)))
        },
    )
}

fn first_cite_used_disambiguate_true(db: &dyn IrDatabase, ref_id: Atom) -> bool {
    let all_cites = db.all_cite_ids();
    let first = all_cites.iter().find(|cite_id| {
        let cite = cite_id.lookup(db);
        cite.ref_id == ref_id
    });
    first.map_or(false, |&id| {
        let gen4 = db.ir_fully_disambiguated(id);
        gen4.used_disambiguate_true
    })
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
            // in a bibliography, we do the affixes etc inside Layout, so they're not here
            let current = gen0.tree_ref().first_names_block();
            let sas = style.bibliography.as_ref().and_then(|bib| {
                bib.subsequent_author_substitute
                    .as_ref()
                    .map(|x| (x.as_ref(), bib.subsequent_author_substitute_rule))
            });
            if let (Some(prev_name_block), Some(current_name_block), Some((sas, sas_rule))) = (
                prev.as_ref()
                    .and_then(|(first_block, gen)| gen.tree.arena.get(*first_block)),
                current,
                sas,
            ) {
                let mutated = Arc::make_mut(&mut gen0);
                let did = transforms::subsequent_author_substitute(
                    &fmt,
                    // In order to unwrap this here, you must only replace the NameIR node's
                    // children, not the IR.
                    prev_name_block.get().0.unwrap_name_ir(),
                    current_name_block,
                    &mut mutated.tree.arena,
                    sas,
                    sas_rule,
                );
                if did {
                    mutated.tree_mut().recompute_group_vars();
                }
            }
            let flat = gen0
                .tree_ref()
                .flatten(&fmt, None)
                .unwrap_or_else(|| fmt.plain(""));
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
    let mut clusters_in_last_note: Vec<ClusterId> = Vec::new();

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
            ClusterNumber::InText(_n) => true,
            ClusterNumber::OutsideFlow => true,
            _ => false,
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
            let matching_prev: Option<Position> = prev_cite
                .and_then(|p| {
                    if p.ref_id == cite.ref_id {
                        Some(Where::SameCluster(p))
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    if let Some(prev_cluster) = match cluster.number {
                        ClusterNumber::OutsideFlow => None,
                        ClusterNumber::InText(_) => prev_in_text,
                        ClusterNumber::Note(_) => prev_note,
                    } {
                        let prev_number = match prev_cluster.number {
                            ClusterNumber::Note(intra) => Some(intra.note_number()),
                            _ => None,
                        };
                        let cites_all_same = if prev_in_group && !in_text {
                            // { id: 1, note: 4, cites: [A] },
                            // { id: 2, note: 4, cites: [B] },
                            // { id: 3: note: 5, cites: [B] } => subsequent
                            // (because prev note wasn't homogenous)
                            clusters_in_last_note
                                .iter()
                                .filter_map(|&cluster_id| db.cluster_cites_sorted(cluster_id))
                                .all(|cites| {
                                    cites
                                        .iter()
                                        .all(|cite_id| cite_id.lookup(db).ref_id == cite.ref_id)
                                })
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
                        // no locators
                        (None, None, false) => Position::Ibid,
                        (None, None, true) => Position::IbidNear,
                        // prev no locator, cur has locator
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
                        // both have locator, but it's the same locator
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
                    match cluster.number {
                        ClusterNumber::Note(this_intranote) => {
                            debug_assert!(
                                // the PartialOrd impl sometimes returns None => false
                                // so !
                                !(this_intranote < first_note_number),
                                "note numbers not monotonic: {:?} came after but was less than {:?}",
                                cluster.number,
                                first_note_number,
                            );
                            let unsigned = first_note_number.note_number();
                            if let Some(pos) = matching_prev {
                                map.insert(cite_id, (pos, Some(unsigned)));
                            } else if this_intranote == first_note_number || is_near(unsigned) {
                                // XXX: not sure about this one
                                // unimplemented!("cite position for same number, but different cluster");
                                map.insert(cite_id, (Position::NearNote, Some(unsigned)));
                            } else {
                                map.insert(cite_id, (Position::FarNote, Some(unsigned)));
                            }
                        }
                        ClusterNumber::InText(_this_intext) => {
                            log::warn!("InText should not be sorted after a Note");
                            // this won't happen; InText is sorted before Note. Nevertheless:
                            map.insert(cite_id, (Position::First, None));
                        }
                        ClusterNumber::OutsideFlow => {
                            map.insert(cite_id, (Position::First, None));
                        }
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
                        ClusterNumber::OutsideFlow => {
                            map.insert(cite_id, (Position::First, None));
                        }
                    }
                }
                Some(ClusterNumber::OutsideFlow) | None => {
                    match cluster.number {
                        ClusterNumber::Note(_) | ClusterNumber::InText(_) => {
                            first_seen.insert(cite.ref_id.clone(), cluster.number);
                        }
                        ClusterNumber::OutsideFlow => {}
                    }
                    map.insert(cite_id, (Position::First, None));
                }
            }
        }

        match cluster.number {
            ClusterNumber::Note(n) => {
                let n = n.note_number();
                if last_note_num != Some(n) {
                    last_note_num = Some(n);
                    clusters_in_last_note.clear();
                }
                clusters_in_last_note.push(cluster.id);
            }
            ClusterNumber::InText(_) => {}
            ClusterNumber::OutsideFlow => {}
        }
        prev_in_text = match cluster.number {
            ClusterNumber::InText(_) => Some(cluster),
            _ => None,
        };
        prev_note = match cluster.number {
            ClusterNumber::Note(_) => Some(cluster),
            _ => None,
        };
    }

    Arc::new(map)
}

fn cite_position(db: &dyn IrDatabase, key: CiteId) -> (Position, Option<u32>) {
    if let Some(x) = db.cite_positions().get(&key) {
        *x
    } else {
        // Assume this cite is a ghost cite.
        (Position::Subsequent, None)
    }
}

fn intext(db: &dyn IrDatabase, id: CiteId) -> Option<Arc<IrGen>> {
    let style = db.style();
    style.intext.as_ref().map(|intext| {
        let style;
        let locale;
        let cite;
        let refr;
        let ctx;
        preamble!(style, locale, cite, refr, ctx, db, id, None);
        let mut state = IrState::new();
        let mut arena = IrArena::new();
        let root = intext.intermediate(db, &mut state, &ctx, &mut arena);
        // disambiguation cannot be done on <intext>
        let irgen = IrGen::new(IrTree::new(root, arena), state, true);
        Arc::new(irgen)
    })
}
