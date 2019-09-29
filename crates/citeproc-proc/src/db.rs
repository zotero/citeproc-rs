// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use crate::disamb::{Dfa, Edge, EdgeData, FreeCondSets};
use crate::prelude::*;

use fnv::FnvHashMap;

use std::sync::Arc;

use crate::disamb::{DisambName, DisambNameData};

use crate::{CiteContext, DisambPass, IrState, Proc, IR};
use citeproc_io::output::{markup::Markup, OutputFormat};
use citeproc_io::{ClusterId, Name};

use csl::Atom;

pub trait HasFormatter {
    fn get_formatter(&self) -> Markup;
}

#[salsa::query_group(IrDatabaseStorage)]
pub trait IrDatabase: CiteDatabase + LocaleDatabase + StyleDatabase + HasFormatter {
    fn ref_dfa(&self, key: Atom) -> Option<Arc<Dfa>>;

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

    #[salsa::invoke(crate::disamb::names::disambiguated_person_names)]
    fn disambiguated_person_names(&self) -> Arc<FnvHashMap<DisambName, IR<Markup>>>;

    // fn name_expansions(&self) -> Arc<NameExpansions>;

    #[salsa::interned]
    fn disamb_name(&self, e: Arc<DisambNameData>) -> DisambName;

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
                            collector.push(db.disamb_name(Arc::new(DisambNameData {
                                ref_id: ref_id.clone(),
                                var: *var,
                                el: el.clone(),
                                value: val.clone(),
                                primary: !seen_one,
                            })))
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
    let mut un = is_unambiguous(db, ctx.disamb_pass, ir, own_id);
    // disambiguate returns true if it can do more for this DisambPass (i.e. more names to add)
    while !un && ir.disambiguate(db, state, ctx) {
        un = is_unambiguous(db, ctx.disamb_pass, ir, own_id);
    }
    un
}

/// the inverted index is constant for a particular set of cited+uncited references
/// year_suffixes should not be present before ir_gen3_add_year_suffix, because that would mean you would mess up
/// the parallelization of IR <= 2
fn is_unambiguous(
    db: &impl IrDatabase,
    pass: Option<DisambPass>,
    ir: &IR<Markup>,
    own_id: &Atom,
) -> bool {
    use log::Level::Warn;
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
                "Own reference {} did not match during {:?}:\n{}",
                k,
                pass,
                dfa.debug_graph(db)
            );
            warn!("{:#?}", &edges);
        }
        if n > 1 {
            break;
        }
    }
    n <= 1
}

fn ir_gen0(db: &impl IrDatabase, id: CiteId) -> IrGen {
    let style;
    let locale;
    let cite;
    let refr;
    let ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);
    let mut state = IrState::new();
    let ir = style.intermediate(&mut state, &ctx).0;
    let _fmt = db.get_formatter();
    let un = is_unambiguous(db, None, &ir, &refr.id);
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

    let un = disambiguate(db, &mut ir, &mut state, &mut ctx, None, &refr.id);
    Arc::new((ir, un, state))
}

fn ir_gen3_add_year_suffix(db: &impl IrDatabase, id: CiteId) -> IrGen {
    let style;
    let locale;
    let cite;
    let refr;
    let mut ctx;
    preamble!(style, locale, cite, refr, ctx, db, id, None);
    db.disambiguated_person_names();

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
