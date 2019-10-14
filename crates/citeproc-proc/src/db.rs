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
use citeproc_io::{ClusterId, Name};
use csl::variables::NameVariable;
use csl::Atom;

use parking_lot::{Mutex, MutexGuard};

pub trait HasFormatter {
    fn get_formatter(&self) -> Markup;
}

#[salsa::query_group(IrDatabaseStorage)]
pub trait IrDatabase: CiteDatabase + LocaleDatabase + StyleDatabase + HasFormatter {
    fn ref_dfa(&self, key: Atom) -> Option<Arc<Dfa>>;

    // TODO: cache this
    // #[salsa::invoke(crate::disamb::create_ref_ir)]
    // fn ref_ir(&self, key: Atom) -> Arc<Vec<(FreeCond, RefIR)>>;

    // If these don't run any additional disambiguation, they just clone the
    // previous ir's Arc.
    fn ir_gen0(&self, key: CiteId) -> IrGen;
    fn ir_gen1_add_names(&self, key: CiteId) -> IrGen;
    fn ir_gen2_add_given_name(&self, key: CiteId) -> IrGen;
    fn ir_gen3_add_year_suffix(&self, key: CiteId) -> IrGen;
    fn ir_gen4_conditionals(&self, key: CiteId) -> IrGen;

    fn built_cluster(&self, key: ClusterId) -> Arc<<Markup as OutputFormat>::Output>;

    fn year_suffixes(&self) -> Arc<FnvHashMap<Atom, u32>>;
    fn year_suffix_for(&self, ref_id: Atom) -> Option<u32>;

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

fn year_suffixes(db: &impl IrDatabase) -> Arc<FnvHashMap<Atom, u32>> {

    let style = db.style();
    if !style.citation.disambiguate_add_year_suffix {
        return Arc::new(FnvHashMap::default());
    }

    let all_cites_ordered = db.all_cite_ids();
    let refs_to_add_suffixes_to = all_cites_ordered
        .iter()
        .map(|&id| (id, id.lookup(db)))
        .map(|(id, cite)| (cite.ref_id.clone(), db.ir_gen2_add_given_name(id)))
        .filter_map(|(ref_id, ir2)| {
            match ir2.unambiguous {
                // if ambiguous (false), add a suffix
                false => Some(ref_id),
                _ => None,
            }
        });

    let mut suffixes = FnvHashMap::default();
    let mut i = 1; // "a" = 1
    for ref_id in refs_to_add_suffixes_to {
        if !suffixes.contains_key(&ref_id) {
            suffixes.insert(ref_id, i);
            i += 1;
        }
    }
    Arc::new(suffixes)
}

#[derive(Debug, Clone)]
pub struct IrGen {
    unambiguous: bool,
    ir_and_state: Arc<(IR<Markup>, IrState)>,
}

impl IrGen {
    fn new(ir: IR<Markup>, unambiguous: bool, state: IrState) -> Self {
        IrGen {
            unambiguous,
            ir_and_state: Arc::new((ir, state)),
        }
    }
    fn fresh_copy(&self) -> (IR<Markup>, IrState) {
        let ir = self.ir_and_state.0.clone();
        let state = self.ir_and_state.1.clone();
        (ir, state)
    }
    pub(crate) fn ir<'a>(&'a self) -> &'a IR<Markup> {
        &self.ir_and_state.0
    }
}

impl Eq for IrGen {}
impl PartialEq<IrGen> for IrGen {
    fn eq(&self, other: &Self) -> bool {
        self.unambiguous == other.unambiguous
            && &self.ir_and_state.1 == &other.ir_and_state.1
            && &self.ir_and_state.0 == &other.ir_and_state.0
    }
}

fn ref_not_found(db: &impl IrDatabase, ref_id: &Atom, log: bool) -> IrGen {
    if log {
        eprintln!("citeproc-rs: reference {} not found", ref_id);
    }
    IrGen::new(
        IR::Rendered(Some(CiteEdgeData::Output(db.get_formatter().plain("???")))),
        true,
        IrState::new(),
    )
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
        $ctx = CiteContext {
            reference: &$refr,
            format: $db.get_formatter(),
            cite_id: $id,
            cite: &$cite,
            position: $db.cite_position($id),
            citation_number: 0,
            disamb_pass: $pass,
            style: &$style,
            locale: &$locale,
            bib_number: $db.bib_number($id),
            name_citation: $db.name_citation(),
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
    cite_id: CiteId,
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
        if k == own_id && !acc && log_enabled!(Warn) {
            warn!(
                "{:?} Own reference {} did not match during {:?}:\n{}",
                cite_id,
                k,
                pass,
                dfa.debug_graph(db)
            );
            warn!("{:#?}", &edges);
        }
        if k != own_id && acc && log_enabled!(Info) {
            info!(
                "cite {:?} matched other reference {} during {:?}",
                cite_id, k, pass
            );
        }
        if n > 1 {
            break;
        }
    }
    n <= 1
}

/// Returns the set of Reference IDs that could have produced a cite's IR
fn refs_accepting_cite(db: &impl IrDatabase, ir: &IR<Markup>) -> Vec<Atom> {
    let edges = ir.to_edge_stream(&db.get_formatter());
    let mut v = Vec::with_capacity(1);
    for k in db.cited_keys().iter() {
        let dfa = db.ref_dfa(k.clone()).expect("cited_keys should all exist");
        let acc = dfa.accepts_data(db, &edges);
        if acc {
            v.push(k.clone());
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
    nvar: NameVariable,
) -> Option<RefNameIR> {
    use crate::disamb::create_single_ref_ir;
    let refr = db.reference(ref_id)?;
    let ref_ctx = RefContext::from_cite_context(&refr, cite_ctx);
    let ref_ir = create_single_ref_ir::<Markup, DB>(db, &ref_ctx);
    fn find_name_block(nvar: NameVariable, ref_ir: &RefIR) -> Option<&RefNameIR> {
        match ref_ir {
            RefIR::Edge(_) => None,
            RefIR::Name(nir, ref nfa) => {
                if nir.variable == nvar {
                    Some(nir)
                } else {
                    None
                }
            }
            RefIR::Seq(seq) => {
                // assumes it's the first one that appears
                seq.contents
                    .iter()
                    .filter_map(|x| find_name_block(nvar, x))
                    .nth(0)
            }
        }
    }
    find_name_block(nvar, &ref_ir).map(|rnir| rnir.clone())
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
    let initial_refs = refs_accepting_cite(db, ir);
    let mut best = initial_refs.len() as u16;
    let name_refs = list_all_name_blocks(ir);

    info!(
        "attempting to disambiguate {:?} ({}) with {:?}",
        ctx.cite_id, &ctx.reference.id, ctx.disamb_pass
    );

    for nir_arc in name_refs {
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
                expand_one_name_ir(db, ir, ctx, &initial_refs, &mut nir);
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
) {
    let mut double_vec: Vec<Vec<NameVariantMatcher>> = Vec::new();

    for r in refs_accepting {
        if let Some(rnir) =
            make_identical_name_formatter(db, r.clone(), ctx, /* XXX */ NameVariable::Author)
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
    let refs = refs_accepting_cite(db, ir);
    let name_refs = list_all_name_blocks(ir);
    for nir_arc in name_refs {
        let mut nir = nir_arc.lock();
        expand_one_name_ir(db, ir, ctx, &refs, &mut nir);
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

fn ir_gen0(db: &impl IrDatabase, id: CiteId) -> IrGen {
    let style;
    let locale;
    let cite;
    let refr;
    let ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);
    let mut state = IrState::new();
    let ir = style.intermediate(db, &mut state, &ctx).0;
    let _fmt = db.get_formatter();
    let un = is_unambiguous(db, None, &ir, id, &refr.id);
    IrGen::new(ir, un, state)
}

fn ir_gen1_add_names(db: &impl IrDatabase, id: CiteId) -> IrGen {
    let style;
    let locale;
    let cite;
    let refr;
    let mut ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);
    ctx.disamb_pass = Some(DisambPass::AddNames);

    let ir0 = db.ir_gen0(id);
    // XXX: keep going if there is global name disambig to perform?
    if ir0.unambiguous || !style.citation.disambiguate_add_names {
        return ir0.clone();
    }
    let (mut ir, mut state) = ir0.fresh_copy();

    let un = disambiguate_add_names(db, &mut ir, &ctx, false);
    IrGen::new(ir, un, state)
}

fn ir_gen2_add_given_name(db: &impl IrDatabase, id: CiteId) -> IrGen {
    let style;
    let locale;
    let cite;
    let refr;
    let mut ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);
    let gndr = style.citation.givenname_disambiguation_rule;
    ctx.disamb_pass = Some(DisambPass::AddGivenName(gndr));

    let ir1 = db.ir_gen1_add_names(id);
    if ir1.unambiguous || !style.citation.disambiguate_add_givenname {
        return ir1.clone();
    }
    let (mut ir, mut state) = ir1.fresh_copy();

    let also_add_names = style.citation.disambiguate_add_names;
    disambiguate_add_givennames(db, &mut ir, &ctx, also_add_names);
    let un = is_unambiguous(db, ctx.disamb_pass, &ir, id, &refr.id);
    IrGen::new(ir, un, state)
}

fn ir_gen3_add_year_suffix(db: &impl IrDatabase, id: CiteId) -> IrGen {
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
    let un = is_unambiguous(db, ctx.disamb_pass, &ir, id, &refr.id);
    IrGen::new(ir, un, state)
}

fn ir_gen4_conditionals(db: &impl IrDatabase, id: CiteId) -> IrGen {
    let style;
    let locale;
    let cite;
    let refr;
    let mut ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);
    ctx.disamb_pass = Some(DisambPass::Conditionals);

    let ir3 = db.ir_gen3_add_year_suffix(id);
    if ir3.unambiguous {
        return ir3.clone();
    }
    let (mut ir, mut state) = ir3.fresh_copy();

    let un = disambiguate(db, &mut ir, &mut state, &mut ctx, None, &refr.id);
    IrGen::new(ir, un, state)
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
            let ir = gen4.ir();
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
