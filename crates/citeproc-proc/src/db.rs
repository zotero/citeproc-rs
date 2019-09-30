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
            match ir2.1 {
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

type IrGen = Arc<(IR<Markup>, bool, IrState)>;

fn ref_not_found(db: &impl IrDatabase, ref_id: &Atom, log: bool) -> IrGen {
    if log {
        eprintln!("citeproc-rs: reference {} not found", ref_id);
    }
    Arc::new((
        IR::Rendered(Some(CiteEdgeData::Output(db.get_formatter().plain("???")))),
        true,
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
    while !un && ir.disambiguate(db, state, ctx) {
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
///    minimised DFA.)
///
///    This step is done by `make_identical_name_formatter`.
///
/// 3. We can then use this narrowed-down Dfa to test, locally, whether name expansions are narrowing
///    down the cite's ambiguity, without having to zip in and out or use a mutex.

fn make_identical_name_formatter<'a, DB: IrDatabase>(
    db: &DB,
    ref_id: Atom,
    cite_ctx: &'a CiteContext<'a, Markup>,
    nvar: NameVariable,
) -> Option<(RefNameIR, Dfa)> {
    use crate::disamb::create_single_ref_ir;
    let refr = db.reference(ref_id)?;
    let ref_ctx = RefContext::from_cite_context(&refr, cite_ctx);
    let ref_ir = create_single_ref_ir::<Markup, DB>(db, &ref_ctx);
    use crate::disamb::Nfa;
    fn find_name_block(nvar: NameVariable, ref_ir: &RefIR) -> Option<(&RefNameIR, &Nfa)> {
        match ref_ir {
            RefIR::Edge(_) => None,
            RefIR::Name(nir, ref nfa) => {
                if nir.variable == nvar {
                    Some((nir, nfa))
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
    find_name_block(nvar, &ref_ir)
        .map(|(rnir, nfa)| (rnir.clone(), Nfa::brzozowski_minimise(nfa.clone())))
}

/// This should be refactored to produce an iterator of mutable NameIRs, one per variable
fn find_cite_name_block(
    nvar: NameVariable,
    ir: &mut IR<Markup>,
) -> Option<&mut NameIR<<Markup as OutputFormat>::Build>> {
    match ir {
        IR::YearSuffix(..) | IR::Rendered(_) => None,
        IR::Names(ref mut nir, _) => {
            if nir.variable == nvar {
                Some(nir)
            } else {
                None
            }
        }
        IR::ConditionalDisamb(_, boxed) => find_cite_name_block(nvar, &mut *boxed),
        IR::Seq(seq) => {
            // assumes it's the first one that appears
            seq.contents
                .iter_mut()
                .filter_map(|x| find_cite_name_block(nvar, x))
                .nth(0)
        }
    }
}

fn stamp_modified_name_ir(
    db: &impl IrDatabase,
    ctx: &CiteContext<'_, Markup>,
    nvar: NameVariable,
    ir: &mut IR<Markup>,
) {
    match ir {
        IR::YearSuffix(..) | IR::Rendered(_) => {}
        IR::Names(ref mut nir, ref mut boxed) => {
            if nir.variable == nvar {
                if let Some((new_ir, _gv)) = nir.intermediate_custom(db, ctx) {
                    **boxed = new_ir;
                }
            }
        }
        IR::ConditionalDisamb(_, boxed) => stamp_modified_name_ir(db, ctx, nvar, &mut *boxed),
        IR::Seq(seq) => {
            seq.contents
                .iter_mut()
                .for_each(|x| stamp_modified_name_ir(db, ctx, nvar, x));
        }
    }
}

type MarkupBuild = <Markup as OutputFormat>::Build;

use crate::disamb::names::{
    single_name_matcher, DisambNameRatchet, NameIR, PersonDisambNameRatchet, RefNameIR,
};
fn disambiguate_add_givennames(
    db: &impl IrDatabase,
    ir: &mut IR<Markup>,
    ctx: &CiteContext<'_, Markup>,
) -> Option<bool> {
    let fmt = db.get_formatter();
    let style = db.style();
    let refs = refs_accepting_cite(db, ir);
    let name_ir: &mut NameIR<MarkupBuild> = find_cite_name_block(NameVariable::Author, ir)?;
    // This should be done for each NameIR/variable found in the cite IR rather than once for
    // Author!
    let mut double_vec: Vec<Vec<Vec<Edge>>> = Vec::new();

    for r in refs {
        if let Some((rnir, dfa)) =
            make_identical_name_formatter(db, r, ctx, /* XXX */ NameVariable::Author)
        {
            let var = rnir.variable;
            let len = rnir.disamb_name_ids.len();
            if len > double_vec.len() {
                double_vec.resize_with(len, || Vec::with_capacity(name_ir.disamb_names.len()));
            }
            for (n, id) in rnir.disamb_name_ids.into_iter().enumerate() {
                let matcher = single_name_matcher(db, id);
                if let Some(slot) = double_vec.get_mut(n) {
                    slot.push(matcher);
                }
            }
        }
    }

    let name_ambiguity_number = |edge: Edge, slot: &[Vec<Edge>]| -> u32 {
        slot.iter()
            .filter(|matcher| matcher.contains(&edge))
            .count() as u32
    };

    let rule = style.citation.givenname_disambiguation_rule;

    let mut n = 0usize;
    for dnr in name_ir.disamb_names.iter_mut() {
        match dnr {
            DisambNameRatchet::Person(ratchet) => {
                if let Some(ref slot) = double_vec.get(n) {
                    // First, get the initial count
                    /* TODO: store format stack */
                    let mut edge =
                        ratchet
                            .data
                            .single_name_edge(db, Formatting::default());
                    let mut min = name_ambiguity_number(edge, slot);
                    debug!("nan for {}-th ({:?}) initially {}", n, edge, min);
                    let initial_min = min;
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
    stamp_modified_name_ir(db, ctx, NameVariable::Author, ir);
    None
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
    Arc::new((ir, un, state))
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
    if ir0.1 || !style.citation.disambiguate_add_names {
        return ir0.clone();
    }
    let mut state = ir0.2.clone();
    let mut ir = ir0.0.clone();

    let un = disambiguate(db, &mut ir, &mut state, &mut ctx, None, &refr.id);
    Arc::new((ir, un, state))
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
    if ir1.1 || !style.citation.disambiguate_add_givenname {
        return ir1.clone();
    }
    let mut state = ir1.2.clone();
    let mut ir = ir1.0.clone();

    // let un = disambiguate(db, &mut ir, &mut state, &mut ct&x, None, &refr.id);
    disambiguate_add_givennames(db, &mut ir, &ctx);
    let un = is_unambiguous(db, ctx.disamb_pass, &ir, id, &refr.id);
    Arc::new((ir, un, state))
}

fn ir_gen3_add_year_suffix(db: &impl IrDatabase, id: CiteId) -> IrGen {
    let style;
    let locale;
    let cite;
    let refr;
    let mut ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);
    let ir2 = db.ir_gen2_add_given_name(id);
    if ir2.1 || !style.citation.disambiguate_add_year_suffix {
        return ir2.clone();
    }
    // TODO: remove
    // splitting the ifs means we only compute year suffixes if it's enabled
    let suffixes = db.year_suffixes();
    if !suffixes.contains_key(&cite.ref_id) {
        return ir2.clone();
    }
    let mut state = ir2.2.clone();
    let mut ir = ir2.0.clone();

    let year_suffix = suffixes[&cite.ref_id];
    ctx.disamb_pass = Some(DisambPass::AddYearSuffix(year_suffix));

    let un = disambiguate(db, &mut ir, &mut state, &mut ctx, Some(&suffixes), &refr.id);
    Arc::new((ir, un, state))
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
    if ir3.1 {
        return ir3.clone();
    }
    let mut state = ir3.2.clone();
    let mut ir = ir3.0.clone();

    let un = disambiguate(db, &mut ir, &mut state, &mut ctx, None, &refr.id);
    Arc::new((ir, un, state))
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
            let ir = &db.ir_gen4_conditionals(id).0;
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
