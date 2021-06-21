use std::sync::Arc;

use citeproc_db::ClusterId;

use crate::ir::transforms;
use crate::prelude::*;

const CSL_STYLE_ERROR: &'static str = "[CSL STYLE ERROR: reference with no printed form.]";


pub fn built_cluster_before_output(
    db: &dyn IrDatabase,
    cluster_id: ClusterId,
    fmt: &Markup,
) -> <Markup as OutputFormat>::Build {
    let cite_ids = if let Some(x) = db.cluster_cites_sorted(cluster_id) {
        x
    } else {
        return fmt.plain("");
    };
    let style = db.style();
    let layout = &style.citation.layout;
    let sorted_refs_arc = db.sorted_refs();
    use transforms::{CnumIx, RangePiece, Unnamed3};
    let mut irs: Vec<_> = cite_ids
        .iter()
        .map(|&id| {
            let gen4 = db.ir_fully_disambiguated(id);
            let cite = id.lookup(db);
            let (_keys, citation_numbers_by_id) = &*sorted_refs_arc;
            let cnum = citation_numbers_by_id.get(&cite.ref_id).cloned();
            Unnamed3::new(id, cite, cnum.map(|x| x.get()), gen4, &fmt)
        })
        .collect();

    if let Some((_cgd, collapse)) = style.citation.group_collapsing() {
        transforms::group_and_collapse(&fmt, collapse, &mut irs);
    }

    if let Some(mode) = db.cluster_mode(cluster_id) {
        transforms::apply_cluster_mode(db, &fmt, mode, &mut irs);
    }

    // Cite capitalization
    // TODO: allow clients to pass a flag to prevent this (on ix==0) when a cluster is in the
    // middle of an existing footnote, and isn't preceded by a period (or however else a client
    // wants to judge that).
    // We capitalize all cites whose prefixes end with full stops.
    for (
        ix,
        Unnamed3 {
            gen4,
            prefix_parsed,
            ..
        },
    ) in irs.iter_mut().enumerate()
    {
        if style.class != csl::StyleClass::InText
            && prefix_parsed
                .as_ref()
                .map_or(ix == 0, |pre| fmt.ends_with_full_stop(pre))
        {
            // dbg!(ix, prefix_parsed);
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

    fn flatten_arena(root: NodeId, arena: &IrArena<Markup>, fmt: &Markup, if_empty: &str) -> MarkupBuild {
        match IR::flatten(root, arena, fmt, None) {
            Some(x) => x,
            None => fmt.plain(if_empty),
        }
    }
    let flatten_affix_unnamed = |unnamed: &Unnamed3<Markup>, cite_is_last: bool| -> MarkupBuild {
        let Unnamed3 { cite, gen4, .. } = unnamed;
        use std::borrow::Cow;
        let flattened = flatten_arena(gen4.root, &gen4.arena, &fmt, CSL_STYLE_ERROR);
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
        if suf_last_punc && !cite_is_last {
            let suf_mut = suf.to_mut();
            suf_mut.push(' ');
        }
        let opts = IngestOptions {
            is_external: true,
            ..Default::default()
        };
        let prefix_parsed = fmt.ingest(&pre, &opts);
        let suffix_parsed = fmt.ingest(&suf, &opts);
        // TODO: custom procedure for joining user-supplied cite affixes, which should interact
        // with terminal punctuation by overriding rather than joining in the usual way.
        use std::iter::once;
        fmt.seq(
            once(prefix_parsed)
            .chain(once(flattened))
            .chain(once(suffix_parsed)),
        )
    };
    let flatten_affix_cite = |cites: &[Unnamed3<Markup>], ix: usize| -> Option<MarkupBuild> {
        Some(flatten_affix_unnamed(cites.get(ix)?, ix == cites.len() - 1))
    };

    let cgroup_delim = style
        .citation
        .cite_group_delimiter
        .as_opt_str()
        .unwrap_or(", ");
    let ysuf_delim = style
        .citation
        .year_suffix_delimiter
        .as_opt_str()
        .or(style.citation.layout.delimiter.as_opt_str())
        .unwrap_or("");
    let acol_delim = style
        .citation
        .after_collapse_delimiter
        .as_opt_str()
        .or(style.citation.layout.delimiter.as_opt_str())
        .unwrap_or("");
    let layout_delim = style.citation.layout.delimiter.as_ref();

    let intext_el = style.intext.as_ref();
    let intext_delim = intext_el.map_or("", |x| x.layout.delimiter.as_opt_str().unwrap_or(""));
    let intext_pre = intext_el.map_or("", |x| {
        x.layout
            .affixes
            .as_ref()
            .map_or("", |af| af.prefix.as_str())
    });
    let intext_suf = intext_el.map_or("", |x| {
        x.layout
            .affixes
            .as_ref()
            .map_or("", |af| af.suffix.as_str())
    });

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
                        if let Some(one) = flatten_affix_cite(&irs, ix) {
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
                        if let Some(start) = flatten_affix_cite(&irs, start.ix) {
                            g.push(start);
                        }
                        if let Some(end) = flatten_affix_cite(&irs, end.ix) {
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
            let (built, advance_to) = render_range(
                collapsed_ranges,
                layout_delim.as_opt_str().unwrap_or(""),
                acol_delim,
            );
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
                    if let Some(b) = flatten_affix_cite(&irs, rix) {
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
            if let Some(built) = flatten_affix_cite(&irs, ix) {
                built_cites.push(built);
                if !suppress_delimiter(&irs, ix) {
                    built_cites.push(fmt.plain(layout_delim.as_opt_str().unwrap_or("")));
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

