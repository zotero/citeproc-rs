use fnv::{FnvHashMap, FnvHashSet};
use std::collections::HashSet;
use std::sync::Arc;

use crate::input::{Cite, CiteContext, CiteId, ClusterId, Reference};
use crate::style::db::StyleDatabase;
use crate::style::element::Position;
// use crate::input::{Reference, Cite};
use crate::locale::db::LocaleDatabase;
use crate::output::{OutputFormat, Pandoc};
use crate::proc::{AddDisambTokens, DisambToken, IrState, Proc, ReEvaluation, IR};
use crate::Atom;

#[salsa::query_group(ReferenceDatabaseStorage)]
pub trait ReferenceDatabase: salsa::Database + LocaleDatabase + StyleDatabase {
    #[salsa::input]
    fn reference_input(&self, key: Atom) -> Arc<Reference>;

    #[salsa::input]
    fn all_keys(&self, key: ()) -> Arc<HashSet<Atom>>;

    /// Filters out keys not in the library
    fn citekeys(&self, key: ()) -> Arc<HashSet<Atom>>;

    #[salsa::input]
    fn all_uncited(&self, key: ()) -> Arc<HashSet<Atom>>;
    /// Filters out keys not in the library
    fn uncited(&self, key: ()) -> Arc<HashSet<Atom>>;

    fn reference(&self, key: Atom) -> Option<Arc<Reference>>;

    fn disamb_tokens(&self, key: Atom) -> Arc<HashSet<DisambToken>>;

    fn inverted_index(&self, key: ()) -> Arc<FnvHashMap<DisambToken, HashSet<Atom>>>;

    fn cite_positions(&self, key: ()) -> Arc<FnvHashMap<CiteId, Position>>;
    #[salsa::dependencies]
    fn cite_position(&self, key: CiteId) -> Position;

    #[salsa::input]
    fn cite(&self, key: CiteId) -> Arc<Cite<Pandoc>>;

    #[salsa::input]
    fn cluster_cites(&self, key: ClusterId) -> Arc<Vec<CiteId>>;

    #[salsa::input]
    fn cluster_ids(&self, key: ()) -> Arc<Vec<ClusterId>>;

    fn year_suffixes(&self, key: ()) -> Arc<FnvHashMap<Atom, u32>>;

    // If these don't run any additional disambiguation, they just clone the
    // previous ir's Arc.
    fn ir_gen0(&self, key: CiteId) -> Arc<(IR<Pandoc>, bool)>;
    fn ir_gen1(&self, key: CiteId) -> Arc<(IR<Pandoc>, bool)>;
    fn ir_gen2(&self, key: CiteId) -> Arc<(IR<Pandoc>, bool)>;
    fn ir_gen3(&self, key: CiteId) -> Arc<(IR<Pandoc>, bool)>;
    fn ir_gen4(&self, key: CiteId) -> Arc<(IR<Pandoc>, bool)>;

    fn built_cluster(&self, key: ClusterId) -> Arc<<Pandoc as OutputFormat>::Output>;
}

fn reference(db: &impl ReferenceDatabase, key: Atom) -> Option<Arc<Reference>> {
    if db.all_keys(()).contains(&key) {
        Some(db.reference_input(key))
    } else {
        None
    }
}

// only call with real references please
fn disamb_tokens(db: &impl ReferenceDatabase, key: Atom) -> Arc<HashSet<DisambToken>> {
    let refr = db.reference_input(key);
    let mut set = HashSet::new();
    refr.add_tokens_index(&mut set);
    Arc::new(set)
}

fn inverted_index(
    db: &impl ReferenceDatabase,
    _: (),
) -> Arc<FnvHashMap<DisambToken, HashSet<Atom>>> {
    let mut index = FnvHashMap::default();
    // TODO: build index from (cited U uncited), not ALL keys.
    for key in db.citekeys(()).iter() {
        for tok in db.disamb_tokens(key.clone()).iter() {
            let ids = index.entry(tok.clone()).or_insert_with(|| HashSet::new());
            ids.insert(key.clone());
        }
    }
    Arc::new(index)
}

// make sure there are no keys we wouldn't recognise
fn uncited(db: &impl ReferenceDatabase, _: ()) -> Arc<HashSet<Atom>> {
    let all = db.all_keys(());
    let uncited = db.all_uncited(());
    let merged = all.intersection(&uncited).cloned().collect();
    Arc::new(merged)
}

// make sure there are no keys we wouldn't recognise
// TODO: compute citekeys from the clusters
fn citekeys(db: &impl ReferenceDatabase, _: ()) -> Arc<HashSet<Atom>> {
    let all = db.all_keys(());
    let mut citekeys = HashSet::new();
    let cluster_ids = db.cluster_ids(());
    let clusters = cluster_ids.iter().cloned().map(|id| db.cluster_cites(id));
    for cluster in clusters {
        citekeys.extend(cluster.iter().map(|&id| db.cite(id).ref_id.clone()));
    }
    let merged = all.intersection(&citekeys).cloned().collect();
    Arc::new(merged)
}

// See https://github.com/jgm/pandoc-citeproc/blob/e36c73ac45c54dec381920e92b199787601713d1/src/Text/CSL/Reference.hs#L910
fn cite_positions(db: &impl ReferenceDatabase, _: ()) -> Arc<FnvHashMap<CiteId, Position>> {
    let cluster_ids = db.cluster_ids(());
    let clusters: Vec<_> = cluster_ids
        .iter()
        .cloned()
        .map(|id| db.cluster_cites(id))
        .collect();

    let mut map = FnvHashMap::default();

    // TODO: configure
    let near_note_distance = 5;

    for (i, cluster) in clusters.iter().enumerate() {
        let mut seen = FnvHashMap::default();

        for (j, &id) in cluster.iter().enumerate() {
            let cite = db.cite(id);
            let prev_cite = cluster
                .get(j.wrapping_sub(1))
                .map(|&prev_id| db.cite(prev_id));
            let matching_prev = prev_cite
                .filter(|p| p.ref_id == cite.ref_id)
                .or_else(|| {
                    if let Some(prev_cluster) = clusters.get(i.wrapping_sub(1)) {
                        if prev_cluster.len() > 0
                            && prev_cluster
                                .iter()
                                .all(|&pid| db.cite(pid).ref_id == cite.ref_id)
                        {
                            // Pick the last one to match locators against
                            prev_cluster.last().map(|&pid| db.cite(pid))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .map(|prev| {
                    if cite.locators.len() > 0 {
                        Position::Subsequent
                    } else if prev.locators == cite.locators {
                        Position::Ibid
                    } else {
                        Position::IbidWithLocator
                    }
                });
            if let Some(pos) = matching_prev {
                map.insert(id, pos);
            } else if let Some(last_id) = seen.get(&cite.ref_id) {
                // TODO: read position="subsequent" as Ibid || FarNote || NearNote
                if i - last_id < near_note_distance {
                    map.insert(id, Position::NearNote);
                } else {
                    map.insert(id, Position::FarNote);
                }
            } else {
                map.insert(id, Position::First);
            }
            seen.insert(cite.ref_id.clone(), i);
        }
    }

    Arc::new(map)
}

fn cite_position(db: &impl ReferenceDatabase, key: CiteId) -> Position {
    db.cite_positions(())
        .get(&key)
        .expect("called cite_position on unknown cite id")
        .clone()
}

fn built_cluster(
    db: &impl ReferenceDatabase,
    cluster_id: ClusterId,
) -> Arc<<Pandoc as OutputFormat>::Output> {
    let fmt = Pandoc::default();
    let cite_ids = db.cluster_cites(cluster_id);
    let style = db.style(());
    let layout = &style.citation.layout;
    let built_cites: Vec<_> = cite_ids
        .iter()
        .map(|&id| {
            let ir = &db.ir_gen4(id).0;
            ir.flatten(&fmt)
        })
        .collect();
    let build = fmt.affixed(
        fmt.group(built_cites, &layout.delimiter.0, layout.formatting),
        &layout.affixes,
    );
    Arc::new(fmt.output(build))
}

fn matching_refs(
    index: &FnvHashMap<DisambToken, HashSet<Atom>>,
    state: &IrState,
) -> (FnvHashSet<Atom>, bool) {
    let mut matching_ids = FnvHashSet::default();
    for tok in state.tokens.iter() {
        // ignore tokens which matched NO references; they are just part of the style,
        // like <text value="xxx"/>. Of course:
        //   - <text value="xxx"/> WILL match any references that have a field with
        //     "xxx" in it.
        //   - You have to make sure all text is transformed equivalently.
        //   So TODO: make all text ASCII uppercase first!
        if let Some(ids) = index.get(tok) {
            for x in ids {
                matching_ids.insert(x.clone());
            }
        }
    }
    // dbg!(&state.tokens);
    // dbg!(&matching_ids);
    // len == 0 is for "ibid" or "[1]", etc. They are clearly unambiguous, and we will assume
    // that any time it happens is intentional.
    // len == 1 means there was only one ref. Great!
    //
    // TODO Of course, that whole 'compare IR output for ambiguous cites' thing.
    let len = matching_ids.len();
    (matching_ids, len == 0 || len == 1)
}

fn year_suffixes(_db: &impl ReferenceDatabase, _: ()) -> Arc<FnvHashMap<Atom, u32>> {
    return Arc::new(FnvHashMap::default());
    // unimplemented!();
    // let refs_to_add_suffixes_to = all_cites_ordered
    //     .map(|cite| (&cite.ref_id, db.ir2(cite.id)))
    //     .filter_map(|(ref_id, (_, is_date_ambig))| {
    //         match is_date_ambig {
    //             true => Some(ref_id),
    //             _ => None
    //         }
    //     });
    //
    // let mut suffixes = FnvHashMap::new();
    // let mut i = 1; // "a" = 1
    // for ref_id in refs_to_add_suffixes_to {
    //     if !suffixes.contains(ref_id) {
    //         suffixes.insert(ref_id.clone(), i);
    //         i += 1;
    //     }
    // }
    // Arc::new(suffixes)
}

fn disambiguate<O: OutputFormat>(
    db: &impl ReferenceDatabase,
    ir: &mut IR<O>,
    state: &mut IrState,
    ctx: &mut CiteContext<O>,
    re: ReEvaluation,
) -> (FnvHashSet<Atom>, bool) {
    let index = db.inverted_index(());
    ctx.re_evaluation = Some(re);
    let is_unambig = |state: &IrState| {
        let (_, unambiguous) = matching_refs(&index, state);
        unambiguous
    };
    ir.re_evaluate(db, state, ctx, &is_unambig);
    matching_refs(&index, state)
}

fn ctx_for<'c, O: OutputFormat>(
    db: &impl ReferenceDatabase,
    cite: &'c Cite<O>,
    reference: &'c Reference,
) -> CiteContext<'c, O> {
    CiteContext {
        cite,
        reference,
        format: O::default(),
        position: db.cite_position(cite.id),
        citation_number: 0, // XXX: from db
        re_evaluation: None,
    }
}

fn ref_not_found(ref_id: &Atom, log: bool) -> Arc<(IR<Pandoc>, bool)> {
    if log {
        eprintln!("citeproc-rs: reference {} not found", ref_id);
    }
    return Arc::new((IR::Rendered(Some(Pandoc::default().plain("???"))), true));
}

fn ir_gen0(db: &impl ReferenceDatabase, id: CiteId) -> Arc<(IR<Pandoc>, bool)> {
    let style = db.style(());
    let index = db.inverted_index(());
    let cite = db.cite(id);
    let refr = match db.reference(cite.ref_id.clone()) {
        None => return ref_not_found(&cite.ref_id, true),
        Some(r) => r,
    };
    let ctx = ctx_for(db, &cite, &refr);
    let mut state = IrState::new();
    let ir = style.intermediate(db, &mut state, &ctx).0;

    let (_, un) = matching_refs(&index, &state);
    Arc::new((ir, un))
}

fn ir_gen1(db: &impl ReferenceDatabase, id: CiteId) -> Arc<(IR<Pandoc>, bool)> {
    let style = db.style(());
    let ir0 = db.ir_gen0(id);
    // XXX: keep going if there is global name disambig to perform?
    if ir0.1 || !style.citation.disambiguate_add_names {
        return ir0.clone();
    }
    let cite = db.cite(id);
    let refr = db
        .reference(cite.ref_id.clone())
        .expect("already handled missing ref");
    let mut ctx = ctx_for(db, &cite, &refr);
    let mut state = IrState::new();
    let mut ir = ir0.0.clone();

    let (_, un) = disambiguate(db, &mut ir, &mut state, &mut ctx, ReEvaluation::AddNames);
    Arc::new((ir, un))
}

fn ir_gen2(db: &impl ReferenceDatabase, id: CiteId) -> Arc<(IR<Pandoc>, bool)> {
    let style = db.style(());
    let ir1 = db.ir_gen1(id);
    if ir1.1 || !style.citation.disambiguate_add_givenname {
        return ir1.clone();
    }
    let cite = db.cite(id);
    let refr = db
        .reference(cite.ref_id.clone())
        .expect("already handled missing ref");
    let mut ctx = ctx_for(db, &cite, &refr);
    let mut state = IrState::new();
    let mut ir = ir1.0.clone();

    let gndr = style.citation.givenname_disambiguation_rule;
    let (_, un) = disambiguate(
        db,
        &mut ir,
        &mut state,
        &mut ctx,
        ReEvaluation::AddGivenName(gndr),
    );
    Arc::new((ir, un))
}

fn ir_gen3(db: &impl ReferenceDatabase, cite_id: CiteId) -> Arc<(IR<Pandoc>, bool)> {
    let style = db.style(());
    let ir2 = db.ir_gen2(cite_id);
    if ir2.1 || !style.citation.disambiguate_add_year_suffix {
        return ir2.clone();
    }
    // splitting the ifs means we only compute year suffixes if it's enabled
    let cite = db.cite(cite_id);
    let suffixes = db.year_suffixes(());
    if !suffixes.contains_key(&cite.ref_id) {
        return ir2.clone();
    }
    let refr = db
        .reference(cite.ref_id.clone())
        .expect("already handled missing ref");
    let mut ctx = ctx_for(db, &cite, &refr);
    let mut state = IrState::new();
    let mut ir = ir2.0.clone();

    let year_suffix = suffixes[&cite.ref_id];
    let (_, un) = disambiguate(
        db,
        &mut ir,
        &mut state,
        &mut ctx,
        ReEvaluation::AddYearSuffix(year_suffix),
    );
    Arc::new((ir, un))
}

fn ir_gen4(db: &impl ReferenceDatabase, cite_id: CiteId) -> Arc<(IR<Pandoc>, bool)> {
    let ir3 = db.ir_gen3(cite_id);
    if ir3.1 {
        return ir3.clone();
    }
    let cite = db.cite(cite_id);
    let refr = db
        .reference(cite.ref_id.clone())
        .expect("already handled missing ref");
    let mut ctx = ctx_for(db, &cite, &refr);
    let mut state = IrState::new();
    let mut ir = ir3.0.clone();

    let (_, un) = disambiguate(
        db,
        &mut ir,
        &mut state,
        &mut ctx,
        ReEvaluation::Conditionals,
    );
    Arc::new((ir, un))
}
