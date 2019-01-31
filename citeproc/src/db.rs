use csl::locale::Locale;
use fnv::{FnvHashMap, FnvHashSet};
use std::collections::HashSet;
use std::sync::Arc;

use crate::input::{Cite, CiteId, ClusterId, Reference};
use crate::proc::ProcDatabase;
use crate::style::db::StyleDatabase;
use csl::style::{Position, Style};
// use crate::input::{Reference, Cite};
use crate::locale::db::LocaleDatabase;
use crate::output::{OutputFormat, Pandoc};
use crate::proc::{AddDisambTokens, CiteContext, DisambPass, DisambToken, IrState, Proc, IR};
use crate::Atom;

#[salsa::query_group(CiteDatabaseStorage)]
pub trait CiteDatabase: salsa::Database + LocaleDatabase + StyleDatabase {
    #[salsa::input]
    fn reference_input(&self, key: Atom) -> Arc<Reference>;

    #[salsa::input]
    fn all_keys(&self, key: ()) -> Arc<HashSet<Atom>>;

    #[salsa::input]
    fn all_uncited(&self, key: ()) -> Arc<HashSet<Atom>>;
    /// Filters out keys not in the library
    fn uncited(&self, key: ()) -> Arc<HashSet<Atom>>;

    /// Filters out keys not in the library
    fn cited_keys(&self, key: ()) -> Arc<HashSet<Atom>>;

    /// Equal to `all.intersection(cited U uncited)`
    fn disamb_participants(&self, key: ()) -> Arc<HashSet<Atom>>;

    fn reference(&self, key: Atom) -> Option<Arc<Reference>>;

    fn disamb_tokens(&self, key: Atom) -> Arc<HashSet<DisambToken>>;

    fn inverted_index(&self, key: ()) -> Arc<FnvHashMap<DisambToken, HashSet<Atom>>>;

    // priv
    #[salsa::input]
    fn cite(&self, key: CiteId) -> Arc<Cite<Pandoc>>;

    #[salsa::input]
    fn cluster_ids(&self, key: ()) -> Arc<Vec<ClusterId>>;

    #[salsa::input]
    fn cluster_cites(&self, key: ClusterId) -> Arc<Vec<CiteId>>;

    #[salsa::input]
    fn cluster_note_number(&self, key: ClusterId) -> u32;

    // All cite ids, in the order they appear in the document
    fn all_cite_ids(&self, key: ()) -> Arc<Vec<CiteId>>;

    fn cite_positions(&self, key: ()) -> Arc<FnvHashMap<CiteId, (Position, Option<u32>)>>;
    #[salsa::dependencies]
    fn cite_position(&self, key: CiteId) -> (Position, Option<u32>);

    fn year_suffixes(&self, key: ()) -> Arc<FnvHashMap<Atom, u32>>;

    // If these don't run any additional disambiguation, they just clone the
    // previous ir's Arc.
    fn ir_gen0(&self, key: CiteId) -> IrGen;
    fn ir_gen1_add_names(&self, key: CiteId) -> IrGen;
    fn ir_gen2_add_given_name(&self, key: CiteId) -> IrGen;
    fn ir_gen3_add_year_suffix(&self, key: CiteId) -> IrGen;
    fn ir_gen4_conditionals(&self, key: CiteId) -> IrGen;

    fn built_cluster(&self, key: ClusterId) -> Arc<<Pandoc as OutputFormat>::Output>;
}

impl<T> ProcDatabase for T
where
    T: CiteDatabase,
{
    #[inline]
    fn default_locale(&self) -> Arc<Locale> {
        self.merged_locale(self.style(()).default_locale.clone())
    }
    #[inline]
    fn style_el(&self) -> Arc<Style> {
        self.style(())
    }
    #[inline]
    fn cite_pos(&self, id: CiteId) -> csl::style::Position {
        self.cite_position(id).0
    }
    #[inline]
    fn cite_frnn(&self, id: CiteId) -> Option<u32> {
        self.cite_position(id).1
    }
    fn bib_number(&self, _: CiteId) -> Option<u32> {
        // TODO: None if not rendering bibliography
        unimplemented!()
    }
}

fn reference(db: &impl CiteDatabase, key: Atom) -> Option<Arc<Reference>> {
    if db.all_keys(()).contains(&key) {
        Some(db.reference_input(key))
    } else {
        None
    }
}

// only call with real references please
fn disamb_tokens(db: &impl CiteDatabase, key: Atom) -> Arc<HashSet<DisambToken>> {
    let refr = db.reference_input(key);
    let mut set = HashSet::new();
    refr.add_tokens_index(&mut set);
    Arc::new(set)
}

fn inverted_index(db: &impl CiteDatabase, _: ()) -> Arc<FnvHashMap<DisambToken, HashSet<Atom>>> {
    let mut index = FnvHashMap::default();
    for key in db.disamb_participants(()).iter() {
        for tok in db.disamb_tokens(key.clone()).iter() {
            let ids = index.entry(tok.clone()).or_insert_with(|| HashSet::new());
            ids.insert(key.clone());
        }
    }
    Arc::new(index)
}

// make sure there are no keys we wouldn't recognise
fn uncited(db: &impl CiteDatabase, _: ()) -> Arc<HashSet<Atom>> {
    let all = db.all_keys(());
    let uncited = db.all_uncited(());
    let merged = all.intersection(&uncited).cloned().collect();
    Arc::new(merged)
}

fn cited_keys(db: &impl CiteDatabase, _: ()) -> Arc<HashSet<Atom>> {
    let all = db.all_keys(());
    let mut keys = HashSet::new();
    let all_cite_ids = db.all_cite_ids(());
    for &id in all_cite_ids.iter() {
        keys.insert(db.cite(id).ref_id.clone());
    }
    // make sure there are no keys we wouldn't recognise
    let merged = all.intersection(&keys).cloned().collect();
    Arc::new(merged)
}

fn disamb_participants(db: &impl CiteDatabase, _: ()) -> Arc<HashSet<Atom>> {
    let cited = db.cited_keys(());
    let uncited = db.uncited(());
    // make sure there are no keys we wouldn't recognise
    let merged = cited.union(&uncited).cloned().collect();
    Arc::new(merged)
}

fn all_cite_ids(db: &impl CiteDatabase, _: ()) -> Arc<Vec<CiteId>> {
    let mut ids = Vec::new();
    let cluster_ids = db.cluster_ids(());
    let clusters = cluster_ids.iter().cloned().map(|id| db.cluster_cites(id));
    for cluster in clusters {
        ids.extend(cluster.iter().cloned());
    }
    Arc::new(ids)
}

#[cfg(test)]
mod test {
    use super::CiteDatabase;
    use crate::db_impl::RootDatabase;
    use crate::input::{Cite, Cluster};
    use csl::style::Position;

    #[test]
    fn cite_positions_ibid() {
        let mut db = RootDatabase::test_db();
        db.init_clusters(vec![
            Cluster {
                id: 1,
                cites: vec![Cite::basic(1, "one")],
                note_number: 1,
            },
            Cluster {
                id: 2,
                cites: vec![Cite::basic(2, "one")],
                note_number: 2,
            },
        ]);
        let poss = db.cite_positions(());
        assert_eq!(poss[&1], (Position::First, None));
        assert_eq!(poss[&2], (Position::Ibid, Some(1)));
    }

    #[test]
    fn cite_positions_near_note() {
        let mut db = RootDatabase::test_db();
        db.init_clusters(vec![
            Cluster {
                id: 1,
                cites: vec![Cite::basic(1, "one")],
                note_number: 1,
            },
            Cluster {
                id: 2,
                cites: vec![Cite::basic(2, "other")],
                note_number: 2,
            },
            Cluster {
                id: 3,
                cites: vec![Cite::basic(3, "one")],
                note_number: 3,
            },
        ]);
        let poss = db.cite_positions(());
        assert_eq!(poss[&1], (Position::First, None));
        assert_eq!(poss[&2], (Position::First, None));
        assert_eq!(poss[&3], (Position::NearNote, Some(1)));
    }

}

// See https://github.com/jgm/pandoc-citeproc/blob/e36c73ac45c54dec381920e92b199787601713d1/src/Text/CSL/Reference.hs#L910
fn cite_positions(
    db: &impl CiteDatabase,
    _: (),
) -> Arc<FnvHashMap<CiteId, (Position, Option<u32>)>> {
    let cluster_ids = db.cluster_ids(());
    let clusters: Vec<_> = cluster_ids
        .iter()
        .map(|&id| (id, db.cluster_cites(id)))
        .collect();

    let mut map = FnvHashMap::default();

    // TODO: configure
    let near_note_distance = 5;

    let mut seen = FnvHashMap::default();

    for (i, (cluster_id, cluster)) in clusters.iter().enumerate() {
        let note_number = db.cluster_note_number(*cluster_id);
        for (j, &cite_id) in cluster.iter().enumerate() {
            let cite = db.cite(cite_id);
            let prev_cite = cluster
                .get(j.wrapping_sub(1))
                .map(|&prev_id| db.cite(prev_id));
            let matching_prev = prev_cite
                .filter(|p| p.ref_id == cite.ref_id)
                .or_else(|| {
                    if let Some((_, prev_cluster)) = clusters.get(i.wrapping_sub(1)) {
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
                .map(|prev| match (&prev.locators[..], &cite.locators[..]) {
                    (&[], &[]) => Position::Ibid,
                    (&[], _cur) => Position::IbidWithLocator,
                    (_pre, &[]) => Position::Subsequent,
                    (pre, cur) if pre == cur => Position::Ibid,
                    _ => Position::IbidWithLocator,
                });
            if let Some(&first_note_number) = seen.get(&cite.ref_id) {
                if let Some(pos) = matching_prev {
                    map.insert(cite_id, (pos, Some(first_note_number)));
                } else if note_number - first_note_number < near_note_distance {
                    map.insert(cite_id, (Position::NearNote, Some(first_note_number)));
                } else {
                    map.insert(cite_id, (Position::FarNote, Some(first_note_number)));
                }
            } else {
                map.insert(cite_id, (Position::First, None));
                seen.insert(cite.ref_id.clone(), note_number);
            }
        }
    }

    Arc::new(map)
}

fn cite_position(db: &impl CiteDatabase, key: CiteId) -> (Position, Option<u32>) {
    db.cite_positions(())
        .get(&key)
        .expect("called cite_position on unknown cite id")
        .clone()
}

fn built_cluster(
    db: &impl CiteDatabase,
    cluster_id: ClusterId,
) -> Arc<<Pandoc as OutputFormat>::Output> {
    let fmt = Pandoc::default();
    let cite_ids = db.cluster_cites(cluster_id);
    let style = db.style(());
    let layout = &style.citation.layout;
    let built_cites: Vec<_> = cite_ids
        .iter()
        .map(|&id| {
            let ir = &db.ir_gen4_conditionals(id).0;
            ir.flatten(&fmt).unwrap_or(fmt.plain(""))
        })
        .collect();
    let build = fmt.affixed(
        fmt.group(built_cites, &layout.delimiter.0, layout.formatting),
        &layout.affixes,
    );
    Arc::new(fmt.output(build))
}

use crate::utils::to_bijective_base_26;

/// the inverted index is constant for a particular set of cited+uncited references
/// year_suffixes should not be present before ir_gen3_add_year_suffix, because that would mean you would mess up
/// the parallelization of IR <= 2
fn is_unambiguous(
    index: &FnvHashMap<DisambToken, HashSet<Atom>>,
    year_suffixes: Option<&FnvHashMap<Atom, u32>>,
    state: &IrState,
) -> bool {
    let mut refs = FnvHashSet::default();
    let invert_ysuffix: Option<FnvHashMap<_, _>> = year_suffixes.map(|ys| {
        ys.iter()
            .map(|(a, &b)| (Atom::from(to_bijective_base_26(b)), a))
            .collect()
    });
    let lookup_ysuffix = |tok: &DisambToken| match tok {
        DisambToken::Str(s) => invert_ysuffix.as_ref().and_then(|iys| iys.get(&s)),
        _ => None,
    };
    // Build up all possible citekeys it could match
    for tok in state.tokens.iter() {
        if let Some(ids) = index.get(tok) {
            for x in ids {
                refs.insert(x.clone());
            }
        }
        if let Some(id) = lookup_ysuffix(tok) {
            refs.insert((*id).clone());
        }
    }
    // Remove any that didn't appear in the index for ALL tokens
    for tok in state.tokens.iter() {
        if let Some(ids) = index.get(tok) {
            refs.retain(|already| ids.contains(already));
        }
        if let Some(id) = lookup_ysuffix(tok) {
            refs.retain(|already| *id == already);
        }
    }
    // dbg!(&state.tokens);
    // dbg!(&refs);
    // ignore tokens which matched NO references; they are just part of the style,
    // like <text value="xxx"/>. Of course:
    //   - <text value="xxx"/> WILL match any references that have a field with
    //     "xxx" in it.
    //   - You have to make sure all text is transformed equivalently.
    //   So TODO: make all text ASCII uppercase first!

    // len == 0 is for "ibid" or "[1]", etc. They are clearly unambiguous, and we will assume
    // that any time it happens is intentional.
    // len == 1 means there was only one ref. Great!
    //
    // TODO Of course, that whole 'compare IR output for ambiguous cites' thing.
    let len = refs.len();
    len < 2
}

fn year_suffixes(db: &impl CiteDatabase, _: ()) -> Arc<FnvHashMap<Atom, u32>> {
    let style = db.style(());
    if !style.citation.disambiguate_add_year_suffix {
        return Arc::new(FnvHashMap::default());
    }

    let all_cites_ordered = db.all_cite_ids(());
    let refs_to_add_suffixes_to = all_cites_ordered
        .iter()
        .map(|&id| db.cite(id))
        .map(|cite| (cite.ref_id.clone(), db.ir_gen2_add_given_name(cite.id)))
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

fn disambiguate<O: OutputFormat>(
    db: &impl CiteDatabase,
    ir: &mut IR<O>,
    state: &mut IrState,
    ctx: &mut CiteContext<O>,
    maybe_ys: Option<&FnvHashMap<Atom, u32>>,
) -> bool {
    let index = db.inverted_index(());
    let is_unambig = |state: &IrState| is_unambiguous(&index, maybe_ys, state);
    // TODO: (BUG) Restore original IrState before running again?
    // Maybe maintain token sets per-name-el. Add an ID to each <names> and reuse IrStates, but
    // clear the relevant names tokens when you're re-evaluating one.
    // Currently, the state being reset means disambiguate doesn't add many tokens at all,
    // and suddently is_unambiguous is running on less than its full range of tokens.
    ir.disambiguate(db, state, ctx, &is_unambig);
    let un = is_unambiguous(&index, maybe_ys, state);
    eprintln!("{:?} trying to disam {}", ctx.disamb_pass, ctx.cite.id);
    if un {
        eprintln!("{:?} disambiguated {}", ctx.disamb_pass, ctx.cite.id);
    }
    un
}

fn ctx_for<'c, O: OutputFormat>(
    db: &impl CiteDatabase,
    cite: &'c Cite<O>,
    reference: &'c Reference,
) -> CiteContext<'c, O> {
    CiteContext {
        cite,
        reference,
        format: O::default(),
        position: db.cite_position(cite.id).0,
        citation_number: 0, // XXX: from db
        disamb_pass: None,
    }
}

type IrGen = Arc<(IR<Pandoc>, bool, IrState)>;

fn ref_not_found(ref_id: &Atom, log: bool) -> IrGen {
    if log {
        eprintln!("citeproc-rs: reference {} not found", ref_id);
    }
    return Arc::new((
        IR::Rendered(Some(Pandoc::default().plain("???"))),
        true,
        IrState::new(),
    ));
}

fn ir_gen0(db: &impl CiteDatabase, id: CiteId) -> IrGen {
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

    let un = is_unambiguous(&index, None, &state);
    Arc::new((ir, un, state))
}

fn ir_gen1_add_names(db: &impl CiteDatabase, id: CiteId) -> IrGen {
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
    let mut state = ir0.2.clone();
    let mut ir = ir0.0.clone();

    ctx.disamb_pass = Some(DisambPass::AddNames);
    let un = disambiguate(db, &mut ir, &mut state, &mut ctx, None);
    Arc::new((ir, un, state))
}

fn ir_gen2_add_given_name(db: &impl CiteDatabase, id: CiteId) -> IrGen {
    let style = db.style(());
    let ir1 = db.ir_gen1_add_names(id);
    if ir1.1 || !style.citation.disambiguate_add_givenname {
        return ir1.clone();
    }
    let cite = db.cite(id);
    let refr = db
        .reference(cite.ref_id.clone())
        .expect("already handled missing ref");
    let mut ctx = ctx_for(db, &cite, &refr);
    let mut state = ir1.2.clone();
    let mut ir = ir1.0.clone();

    let gndr = style.citation.givenname_disambiguation_rule;
    ctx.disamb_pass = Some(DisambPass::AddGivenName(gndr));
    let un = disambiguate(db, &mut ir, &mut state, &mut ctx, None);
    Arc::new((ir, un, state))
}

fn ir_gen3_add_year_suffix(db: &impl CiteDatabase, cite_id: CiteId) -> IrGen {
    let style = db.style(());
    let ir2 = db.ir_gen2_add_given_name(cite_id);
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
    let mut state = ir2.2.clone();
    let mut ir = ir2.0.clone();

    let year_suffix = suffixes[&cite.ref_id];
    ctx.disamb_pass = Some(DisambPass::AddYearSuffix(year_suffix));
    let un = disambiguate(db, &mut ir, &mut state, &mut ctx, Some(&suffixes));
    Arc::new((ir, un, state))
}

fn ir_gen4_conditionals(db: &impl CiteDatabase, cite_id: CiteId) -> IrGen {
    let ir3 = db.ir_gen3_add_year_suffix(cite_id);
    if ir3.1 {
        return ir3.clone();
    }
    let cite = db.cite(cite_id);
    let refr = db
        .reference(cite.ref_id.clone())
        .expect("already handled missing ref");
    let mut ctx = ctx_for(db, &cite, &refr);
    let mut state = ir3.2.clone();
    let mut ir = ir3.0.clone();

    ctx.disamb_pass = Some(DisambPass::Conditionals);
    let un = disambiguate(db, &mut ir, &mut state, &mut ctx, None);
    Arc::new((ir, un, state))
}
