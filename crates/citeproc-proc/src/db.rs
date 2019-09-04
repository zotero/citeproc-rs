// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use crate::disamb::{Edge, EdgeData, FreeCondSets};
use crate::prelude::*;

use fnv::{FnvHashMap, FnvHashSet};
use std::collections::HashSet;
use std::sync::Arc;

use crate::helpers::to_bijective_base_26;
use crate::{CiteContext, DisambPass, DisambToken, IrState, Proc, IR};
use citeproc_io::output::{
    html::{Html, HtmlOptions},
    OutputFormat,
};
use citeproc_io::ClusterId;
use csl::Atom;

#[salsa::query_group(IrDatabaseStorage)]
pub trait IrDatabase: CiteDatabase + LocaleDatabase + StyleDatabase {
    fn disamb_tokens(&self, key: Atom) -> Arc<HashSet<DisambToken>>;
    fn inverted_index(&self) -> Arc<FnvHashMap<DisambToken, HashSet<Atom>>>;

    // If these don't run any additional disambiguation, they just clone the
    // previous ir's Arc.
    fn ir_gen0(&self, key: CiteId) -> IrGen;
    fn ir_gen1_add_names(&self, key: CiteId) -> IrGen;
    fn ir_gen2_add_given_name(&self, key: CiteId) -> IrGen;
    fn ir_gen3_add_year_suffix(&self, key: CiteId) -> IrGen;
    fn ir_gen4_conditionals(&self, key: CiteId) -> IrGen;

    fn built_cluster(&self, key: ClusterId) -> Arc<<Html as OutputFormat>::Output>;

    fn year_suffixes(&self) -> Arc<FnvHashMap<Atom, u32>>;

    #[salsa::interned]
    fn edge(&self, e: EdgeData) -> Edge;

    fn branch_runs(&self) -> Arc<FreeCondSets>;
}

fn branch_runs(db: &impl IrDatabase) -> Arc<FreeCondSets> {
    let style = db.style();
    Arc::new(style.get_free_conds(db))
}

// only call with real references please
fn disamb_tokens(db: &impl CiteDatabase, key: Atom) -> Arc<HashSet<DisambToken>> {
    let refr = db.reference_input(key);
    let mut set = HashSet::new();
    refr.add_tokens_index(&mut set);
    Arc::new(set)
}

fn inverted_index(db: &impl IrDatabase) -> Arc<FnvHashMap<DisambToken, HashSet<Atom>>> {
    let mut index = FnvHashMap::default();
    for key in db.disamb_participants().iter() {
        for tok in db.disamb_tokens(key.clone()).iter() {
            let ids = index.entry(tok.clone()).or_insert_with(|| HashSet::new());
            ids.insert(key.clone());
        }
    }
    Arc::new(index)
}

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

fn disambiguate<O: OutputFormat>(
    db: &impl IrDatabase,
    ir: &mut IR<O>,
    state: &mut IrState,
    ctx: &mut CiteContext<O>,
    maybe_ys: Option<&FnvHashMap<Atom, u32>>,
) -> bool {
    let index = db.inverted_index();
    let is_unambig = |state: &IrState| is_unambiguous(&index, maybe_ys, state);
    // TODO: (BUG) Restore original IrState before running again?
    // Maybe maintain token sets per-name-el. Add an ID to each <names> and reuse IrStates, but
    // clear the relevant names tokens when you're re-evaluating one.
    // Currently, the state being reset means disambiguate doesn't add many tokens at all,
    // and suddently is_unambiguous is running on less than its full range of tokens.
    ir.disambiguate(db, state, ctx, &is_unambig);
    let un = is_unambiguous(&index, maybe_ys, state);
    debug!("{:?} trying to disam {:?}", ctx.disamb_pass, ctx.cite_id);
    if un {
        debug!("{:?} disambiguated {:?}", ctx.disamb_pass, ctx.cite_id);
    }
    un
}

type IrGen = Arc<(IR<Html>, bool, IrState)>;

fn ref_not_found(ref_id: &Atom, log: bool) -> IrGen {
    if log {
        eprintln!("citeproc-rs: reference {} not found", ref_id);
    }
    Arc::new((
        IR::Rendered(Some(Html::default().plain("???"))),
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
            None => return ref_not_found(&$cite.ref_id, true),
            Some(r) => r,
        };
        $ctx = CiteContext {
            reference: &$refr,
            format: Html::default(),
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

fn ir_gen0(db: &impl IrDatabase, id: CiteId) -> IrGen {
    let style;
    let locale;
    let cite;
    let refr;
    let ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);
    let index = db.inverted_index();
    let mut state = IrState::new();
    let ir = style.intermediate(&mut state, &ctx).0;
    let un = is_unambiguous(&index, None, &state);
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

    let un = disambiguate(db, &mut ir, &mut state, &mut ctx, None);
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

    let un = disambiguate(db, &mut ir, &mut state, &mut ctx, None);
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
    // splitting the ifs means we only compute year suffixes if it's enabled
    let suffixes = db.year_suffixes();
    if !suffixes.contains_key(&cite.ref_id) {
        return ir2.clone();
    }
    let mut state = ir2.2.clone();
    let mut ir = ir2.0.clone();

    let year_suffix = suffixes[&cite.ref_id];
    ctx.disamb_pass = Some(DisambPass::AddYearSuffix(year_suffix));

    let un = disambiguate(db, &mut ir, &mut state, &mut ctx, Some(&suffixes));
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

    let un = disambiguate(db, &mut ir, &mut state, &mut ctx, None);
    Arc::new((ir, un, state))
}

cfg_if::cfg_if! {
    if #[cfg(feature = "test")] {
        fn html_options() -> HtmlOptions { HtmlOptions::test_suite() }
    } else {
        fn html_options() -> HtmlOptions { HtmlOptions::default() }
    }
}

fn built_cluster(
    db: &impl IrDatabase,
    cluster_id: ClusterId,
) -> Arc<<Html as OutputFormat>::Output> {
    let fmt = Html::Html(html_options());
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
