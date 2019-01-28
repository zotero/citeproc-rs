use fnv::{FnvHashMap, FnvHashSet};
use std::collections::HashSet;
use std::sync::Arc;

use crate::input::{Cite, CiteContext, CiteId, ClusterId, Reference};
use crate::style::db::StyleDatabase;
use crate::style::element::Position;
// use crate::input::{Reference, Cite};
use crate::locale::db::LocaleDatabase;
use crate::output::{OutputFormat, Pandoc};
use crate::proc::{AddDisambTokens, DisambToken, IrState, ReEvaluation, IR};
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

    fn ir_gen(&self, key: CiteId) -> Arc<IR<Pandoc>>;

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

fn ir_gen(db: &impl ReferenceDatabase, id: CiteId) -> Arc<IR<Pandoc>> {
    use crate::proc::Proc;
    let style = db.style(());
    let cite = &db.cite(id);
    let fmt = Pandoc::default();
    let refr = db.reference(cite.ref_id.clone());
    if refr.is_none() {
        eprintln!("citeproc-rs: reference {} not found", &cite.ref_id);
        return Arc::new(IR::Rendered(Some(fmt.plain("???"))));
    }
    let refr = refr.unwrap();
    let mut ctx = CiteContext {
        cite,
        format: &fmt,
        // TODO: handle missing references
        reference: &refr,
        position: db.cite_position(id),
        citation_number: 0, // XXX: from db
        re_evaluation: None,
    };
    let index = db.inverted_index(());
    let is_unambig = |state: &IrState| {
        let (_, unambiguous) = matching_refs(&index, &state);
        unambiguous
    };

    // TODO: use this to apply the same transforms to other cites with the same ref_id
    // in another pass
    let mut biggest_reeval = None;

    let mut state = IrState::new();
    let (mut ir, _) = style.intermediate(db, &mut state, &ctx);
    // if the cite is ibid (etc), we are assuming it produces <= the first one's DisambTokens.
    // So the first pass ignores ibids, and the second just reapplies any transforms done on the
    // first one.
    if is_unambig(&state) || ctx.position != Position::First {
        return Arc::new(ir);
    }

    let mut reeval = |re: ReEvaluation| {
        ctx.re_evaluation = Some(re);
        biggest_reeval = ctx.re_evaluation;
        ir.re_evaluate(db, &mut state, &ctx, &is_unambig);
        let (_, unambiguous) = matching_refs(&index, &state);
        unambiguous
    };

    if style.citation.disambiguate_add_names {
        if reeval(ReEvaluation::AddNames) {
            return Arc::new(ir);
        }
    }

    if style.citation.disambiguate_add_givenname {
        let gndr = style.citation.givenname_disambiguation_rule;
        if reeval(ReEvaluation::AddGivenName(gndr)) {
            return Arc::new(ir);
        }
    }

    if style.citation.disambiguate_add_year_suffix {
        if reeval(ReEvaluation::AddYearSuffix) {
            return Arc::new(ir);
        }
    }

    reeval(ReEvaluation::Conditionals);

    Arc::new(ir)
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
            let ir = db.ir_gen(id);
            ir.flatten(&fmt)
        })
        .collect();
    let build = fmt.affixed(
        fmt.group(built_cites, &layout.delimiter.0, layout.formatting),
        &layout.affixes,
    );
    Arc::new(fmt.output(build))
}
