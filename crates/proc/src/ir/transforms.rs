use crate::disamb::names::{replace_single_child, NameIR};
use crate::names::NameToken;
use crate::prelude::*;
use citeproc_io::Cite;
use std::mem;
use std::sync::Arc;

/////////////////////////////////
// capitalize start of cluster //
/////////////////////////////////

impl<O: OutputFormat> IR<O> {
    pub fn capitalize_first_term_of_cluster(root: NodeId, arena: &mut IrArena<O>, fmt: &O) {
        if let Some(node) = IR::find_term_rendered_first(root, arena) {
            let trf = match arena.get_mut(node).unwrap().get_mut().0 {
                IR::Rendered(Some(CiteEdgeData::Term(ref mut b)))
                | IR::Rendered(Some(CiteEdgeData::LocatorLabel(ref mut b)))
                | IR::Rendered(Some(CiteEdgeData::FrnnLabel(ref mut b))) => b,
                _ => return,
            };
            fmt.apply_text_case(
                trf,
                &IngestOptions {
                    text_case: TextCase::CapitalizeFirst,
                    ..Default::default()
                },
            );
        }
    }
    // Gotta find a a CiteEdgeData::Term/LocatorLabel/FrnnLabel
    // (the latter two are also terms, but a different kind for disambiguation).
    fn find_term_rendered_first(node: NodeId, arena: &IrArena<O>) -> Option<NodeId> {
        match arena.get(node)?.get().0 {
            IR::Rendered(Some(CiteEdgeData::Term(_)))
            | IR::Rendered(Some(CiteEdgeData::LocatorLabel(_)))
            | IR::Rendered(Some(CiteEdgeData::FrnnLabel(_))) => Some(node),
            IR::ConditionalDisamb(_) | IR::Seq(_) => node
                .children(arena)
                .next()
                .and_then(|child| IR::find_term_rendered_first(child, arena)),
            _ => None,
        }
    }
}

////////////////////////
// second-field-align //
////////////////////////

impl<O: OutputFormat> IR<O> {
    // If returns Some(id), that ID is the new root node of the whole tree.
    pub fn split_first_field(node: NodeId, arena: &mut IrArena<O>) -> Option<NodeId> {
        // Pull off the first field of self -> [first, ...rest]

        if node.children(arena).take(2).count() != 2 {
            return None;
        }

        // Steal the top seq's IrSeq configuration
        let orig_top = if let (IR::Seq(s), gv) = arena.get_mut(node)?.get_mut() {
            (mem::take(s), *gv)
        } else {
            return None;
        };

        // Detach the first child
        let first = node.children(arena).next().unwrap();
        first.detach(arena);
        let rest = node;

        let (afpre, afsuf) = {
            // Keep this mutable ref inside {}
            // Split the affixes into two sets with empty inside.
            orig_top
                .0
                .affixes
                .map(|mine| {
                    (
                        Some(Affixes {
                            prefix: mine.prefix,
                            suffix: "".into(),
                        }),
                        Some(Affixes {
                            prefix: "".into(),
                            suffix: mine.suffix,
                        }),
                    )
                })
                .unwrap_or((None, None))
        };

        let left_gv = arena.get(first)?.get().1;
        let left = arena.new_node((
            IR::Seq(IrSeq {
                display: Some(DisplayMode::LeftMargin),
                affixes: afpre,
                ..Default::default()
            }),
            left_gv,
        ));
        left.append(first, arena);

        let right_config = (
            IR::Seq(IrSeq {
                display: Some(DisplayMode::RightInline),
                affixes: afsuf,
                ..Default::default()
            }),
            GroupVars::Important,
        );

        // Take the IrSeq that configured the original top-level.
        // Replace the configuration for rest with right_config.
        // This is because we want to move all of the rest node's children to the right
        // half, so the node is the thing that has to move.
        *arena.get_mut(rest)?.get_mut() = right_config;
        let top_seq = (
            IR::Seq(IrSeq {
                display: None,
                affixes: None,
                dropped_gv: None,
                ..orig_top.0
            }),
            orig_top.1,
        );

        // Twist it all into place.
        // We make sure rest is detached, even though ATM it's definitely a detached node.
        let new_toplevel = arena.new_node(top_seq);
        rest.detach(arena);
        new_toplevel.append(left, arena);
        new_toplevel.append(rest, arena);
        return Some(new_toplevel);
    }
}

////////////////////////////////
// Cite Grouping & Collapsing //
////////////////////////////////

impl<O: OutputFormat> IR<O> {
    pub fn first_name_block(node: NodeId, arena: &IrArena<O>) -> Option<NodeId> {
        match arena.get(node)?.get().0 {
            IR::Name(_) => Some(node),
            IR::ConditionalDisamb(_) | IR::Seq(_) => {
                // assumes it's the first one that appears
                node.children(arena)
                    .find_map(|child| IR::first_name_block(child, arena))
            }
            _ => None,
        }
    }

    fn find_locator(node: NodeId, arena: &IrArena<O>) -> Option<NodeId> {
        match arena.get(node)?.get().0 {
            IR::Rendered(Some(CiteEdgeData::Locator(_))) => Some(node),
            IR::ConditionalDisamb(_) | IR::Seq(_) => {
                // Search backwards because it's likely to be near the end
                node.reverse_children(arena)
                    .find_map(|child| IR::find_locator(child, arena))
            }
            _ => None,
        }
    }

    fn find_first_year(node: NodeId, arena: &IrArena<O>) -> Option<NodeId> {
        match &arena.get(node)?.get().0 {
            IR::Rendered(Some(CiteEdgeData::Year(_b))) => Some(node),
            IR::Seq(_) | IR::ConditionalDisamb(_) => node
                .children(arena)
                .find_map(|child| IR::find_first_year(child, arena)),
            _ => None,
        }
    }

    pub fn find_year_suffix(node: NodeId, arena: &IrArena<O>) -> Option<u32> {
        IR::has_explicit_year_suffix(node, arena)
            .or_else(|| IR::has_implicit_year_suffix(node, arena))
    }

    fn find_first_year_and_suffix(node: NodeId, arena: &IrArena<O>) -> Option<(NodeId, u32)> {
        // if let Some(fy) = IR::find_first_year(node, arena) {
        //     debug!("fy, {:?}", arena.get(fy).unwrap().get().0);
        // }
        // if let Some(ys) = IR::find_year_suffix(node, arena) {
        //     debug!("ys, {:?}", ys);
        // }
        Some((
            IR::find_first_year(node, arena)?,
            IR::find_year_suffix(node, arena)?,
        ))
    }

    /// Rest of the name: "if it has a year suffix"
    fn suppress_first_year(
        node: NodeId,
        arena: &mut IrArena<O>,
        has_explicit: bool,
    ) -> Option<NodeId> {
        match arena.get(node)?.get().0 {
            IR::Rendered(Some(CiteEdgeData::Year(_))) => {
                arena.get_mut(node)?.get_mut().0 = IR::Rendered(None);
                Some(node)
            }
            IR::ConditionalDisamb(_) => {
                // Not sure why this result is thrown away
                IR::suppress_first_year(node, arena, has_explicit);
                None
            }
            IR::Seq(_) => {
                let mut iter = node.children(arena).fuse();
                let first_two = (iter.next(), iter.next());
                // Check for the exact explicit year suffix IR output
                let mut found = if iter.next().is_some() {
                    None
                } else if let (Some(first), Some(second)) = first_two {
                    match arena.get(second).unwrap().get() {
                        (IR::YearSuffix(_), GroupVars::Unresolved) if has_explicit => {
                            IR::suppress_first_year(first, arena, has_explicit)
                        }
                        (IR::YearSuffix(_), GroupVars::Important)
                            if !has_explicit && !IR::is_empty(second, arena) =>
                        {
                            IR::suppress_first_year(first, arena, has_explicit)
                        }
                        _ => None,
                    }
                } else {
                    None
                };

                // Otherwise keep looking in subtrees etc
                if found.is_none() {
                    let child_ids: Vec<_> = node.children(arena).collect();
                    for child in child_ids {
                        found = IR::suppress_first_year(child, arena, has_explicit);
                        if found.is_some() {
                            break;
                        }
                    }
                }
                found
            }
            _ => None,
        }
    }

    pub fn has_implicit_year_suffix(node: NodeId, arena: &IrArena<O>) -> Option<u32> {
        match arena.get(node)?.get().0 {
            IR::YearSuffix(YearSuffix {
                hook: YearSuffixHook::Plain,
                suffix_num: Some(n),
                ..
            }) if !IR::is_empty(node, arena) => Some(n),

            IR::ConditionalDisamb(_) | IR::Seq(_) => {
                // assumes it's the first one that appears
                node.children(arena)
                    .find_map(|child| IR::has_implicit_year_suffix(child, arena))
            }
            _ => None,
        }
    }

    pub fn has_explicit_year_suffix(node: NodeId, arena: &IrArena<O>) -> Option<u32> {
        match arena.get(node)?.get().0 {
            IR::YearSuffix(YearSuffix {
                hook: YearSuffixHook::Explicit(_),
                suffix_num: Some(n),
                ..
            }) if !IR::is_empty(node, arena) => Some(n),

            IR::ConditionalDisamb(_) | IR::Seq(_) => {
                // assumes it's the first one that appears
                node.children(arena)
                    .find_map(|child| IR::has_explicit_year_suffix(child, arena))
            }
            _ => None,
        }
    }

    pub fn suppress_names(node: NodeId, arena: &mut IrArena<O>) {
        if let Some(fnb) = IR::first_name_block(node, arena) {
            // TODO: check interaction of this with GroupVars of the parent seq
            fnb.remove_subtree(arena);
        }
    }

    pub fn suppress_year(node: NodeId, arena: &mut IrArena<O>) {
        let has_explicit = IR::has_explicit_year_suffix(node, arena).is_some();
        let has_implicit = IR::has_implicit_year_suffix(node, arena).is_some();
        if !has_explicit && !has_implicit {
            return;
        }
        IR::suppress_first_year(node, arena, has_explicit);
    }
}

impl<O: OutputFormat<Output = SmartString>> IR<O> {
    pub fn collapse_to_cnum(node: NodeId, arena: &IrArena<O>, fmt: &O) -> Option<u32> {
        match &arena.get(node)?.get().0 {
            IR::Rendered(Some(CiteEdgeData::CitationNumber(build))) => {
                // TODO: just get it from the database
                fmt.output(build.clone(), false).parse().ok()
            }
            IR::ConditionalDisamb(_) => node
                .children(arena)
                .find_map(|child| IR::collapse_to_cnum(child, arena, fmt)),
            IR::Seq(_) => {
                // assumes it's the first one that appears
                if node.children(arena).count() != 1 {
                    None
                } else {
                    node.children(arena)
                        .next()
                        .and_then(|child| IR::collapse_to_cnum(child, arena, fmt))
                }
            }
            _ => None,
        }
    }
}

use crate::db::IrGen;
use csl::Collapse;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct CnumIx {
    pub cnum: u32,
    pub ix: usize,
    pub force_single: bool,
}

impl CnumIx {
    fn new(c: u32, ix: usize) -> Self {
        CnumIx {
            cnum: c,
            ix,
            force_single: false,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RangePiece {
    /// If the length of the range is only two, it should be rendered with a comma anyway
    Range(CnumIx, CnumIx),
    Single(CnumIx),
}

impl RangePiece {
    /// Return value is the previous value, to be emitted, if the next it couldn't be appended
    fn attempt_append(&mut self, nxt: CnumIx) -> Option<RangePiece> {
        *self = match self {
            _ if nxt.force_single => return Some(std::mem::replace(self, RangePiece::Single(nxt))),
            RangePiece::Single(prv) if prv.cnum == nxt.cnum - 1 => RangePiece::Range(*prv, nxt),
            RangePiece::Range(_, end) if end.cnum == nxt.cnum - 1 => {
                *end = nxt;
                return None;
            }
            _ => return Some(std::mem::replace(self, RangePiece::Single(nxt))),
        };
        return None;
    }
}

#[test]
fn range_append() {
    let mut range = RangePiece::Single(CnumIx::new(1, 1));
    let emit = range.attempt_append(CnumIx::new(2, 2));
    assert_eq!(
        (range, emit),
        (
            RangePiece::Range(CnumIx::new(1, 1), CnumIx::new(2, 2)),
            None
        )
    );
    let mut range = RangePiece::Single(CnumIx::new(1, 1));
    let emit = range.attempt_append(CnumIx::new(3, 2));
    assert_eq!(
        (range, emit),
        (
            RangePiece::Single(CnumIx::new(3, 2)),
            Some(RangePiece::Single(CnumIx::new(1, 1)))
        )
    );
}

pub fn collapse_ranges(nums: &[CnumIx]) -> Vec<RangePiece> {
    let mut pieces = Vec::new();
    if let Some(init) = nums.first() {
        let mut wip = RangePiece::Single(*init);
        for &num in &nums[1..] {
            if let Some(emit) = wip.attempt_append(num) {
                pieces.push(emit);
            }
        }
        pieces.push(wip);
    }
    pieces
}

#[test]
fn range_collapse() {
    let s = |cnum: u32| CnumIx::new(cnum, cnum as usize);
    assert_eq!(
        collapse_ranges(&[s(1), s(2), s(3)]),
        vec![RangePiece::Range(s(1), s(3))]
    );
    assert_eq!(
        collapse_ranges(&[s(1), s(2), CnumIx::new(4, 3)]),
        vec![
            RangePiece::Range(s(1), s(2)),
            RangePiece::Single(CnumIx::new(4, 3))
        ]
    );
}

type MarkupBuild = <Markup as OutputFormat>::Build;
pub struct Unnamed3<O: OutputFormat> {
    pub cite: Arc<Cite<O>>,
    pub cnum: Option<u32>,
    pub gen4: Arc<IrGen>,
    /// So we can look for punctuation at the end and use the format's quoting abilities
    pub prefix_parsed: Option<MarkupBuild>,
    /// First of a group of cites with the same name
    pub is_first: bool,
    /// Subsequent in a group of cites with the same name
    pub should_collapse: bool,
    /// First of a group of cites with the same year, all with suffixes
    /// (same name implied)
    pub first_of_ys: bool,
    /// Subsequent in a group of cites with the same year, all with suffixes
    /// (same name implied)
    pub collapse_ys: bool,

    pub year_suffix: Option<u32>,

    /// Ranges of year suffixes (not alphabetic, in its base u32 form)
    pub collapsed_year_suffixes: Vec<RangePiece>,

    /// Ranges of citation numbers
    pub collapsed_ranges: Vec<RangePiece>,

    /// Tagging removed cites is cheaper than memmoving the rest of the Vec
    pub vanished: bool,

    pub has_locator: bool,
}

use std::fmt::{Debug, Formatter};

impl<O: OutputFormat<Output = SmartString>> Debug for Unnamed3<O> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let fmt = &Markup::default();
        f.debug_struct("Unnamed3")
            .field("cite", &self.cite)
            .field("cnum", &self.cnum)
            .field(
                "gen4",
                &IR::flatten(self.gen4.root, &self.gen4.arena, fmt, None)
                    .map(|x| fmt.output(x, false)),
            )
            .field("prefix_parsed", &self.prefix_parsed)
            .field("has_locator", &self.has_locator)
            .field("is_first", &self.is_first)
            .field("should_collapse", &self.should_collapse)
            .field("first_of_ys", &self.first_of_ys)
            .field("collapse_ys", &self.collapse_ys)
            .field("year_suffix", &self.year_suffix)
            .field("collapsed_year_suffixes", &self.collapsed_year_suffixes)
            .field("collapsed_ranges", &self.collapsed_ranges)
            .field("vanished", &self.vanished)
            .field("gen4_full", &self.gen4)
            .finish()
    }
}

impl Unnamed3<Markup> {
    pub fn new(cite: Arc<Cite<Markup>>, cnum: Option<u32>, gen4: Arc<IrGen>, fmt: &Markup) -> Self {
        let prefix_parsed = cite
            .prefix
            .as_opt_str()
            .map(|p| fmt.ingest(p, &IngestOptions::default()));
        Unnamed3 {
            has_locator: cite.locators.is_some()
                && IR::find_locator(gen4.root, &gen4.arena).is_some(),
            cite,
            gen4,
            prefix_parsed,
            cnum,
            is_first: false,
            should_collapse: false,
            first_of_ys: false,
            collapse_ys: false,
            year_suffix: None,
            collapsed_year_suffixes: Vec::new(),
            collapsed_ranges: Vec::new(),
            vanished: false,
        }
    }
}

pub fn group_and_collapse<O: OutputFormat<Output = SmartString>>(
    fmt: &Markup,
    collapse: Option<Collapse>,
    cites: &mut Vec<Unnamed3<O>>,
) {
    // Neat trick: same_names[None] tracks cites without a cs:names block, which helps with styles
    // that only include a year. (What kind of style is that?
    // magic_ImplicitYearSuffixExplicitDelimiter.txt, I guess that's the only possible reason, but
    // ok.)
    let mut same_names: HashMap<Option<SmartString>, (usize, bool)> = HashMap::new();
    let mut same_years: HashMap<SmartString, (usize, bool)> = HashMap::new();

    // First, group cites with the same name
    for ix in 0..cites.len() {
        let gen4 = &cites[ix].gen4;
        let rendered = IR::first_name_block(gen4.root, &gen4.arena)
            .and_then(|fnb| IR::flatten(fnb, &gen4.arena, fmt, None))
            .map(|flat| fmt.output(flat, false));
        same_names
            .entry(rendered)
            .and_modify(|(oix, seen_once)| {
                // Keep cites separated by affixes together
                if cites.get(*oix).map_or(false, |u| u.cite.has_suffix())
                    || cites.get(*oix + 1).map_or(false, |u| u.cite.has_prefix())
                    || cites.get(ix - 1).map_or(false, |u| u.cite.has_suffix())
                    || cites.get(ix).map_or(false, |u| u.cite.has_affix())
                {
                    *oix = ix;
                    *seen_once = false;
                    return;
                }
                if *oix < ix {
                    if !*seen_once {
                        cites[*oix].is_first = true;
                    }
                    *seen_once = true;
                    cites[ix].should_collapse = true;
                    let rotation = &mut cites[*oix + 1..ix + 1];
                    rotation.rotate_right(1);
                    *oix += 1;
                }
            })
            .or_insert((ix, false));
    }

    if collapse.map_or(false, |c| {
        c == Collapse::YearSuffixRanged || c == Collapse::YearSuffix
    }) {
        let mut top_ix = 0;
        while top_ix < cites.len() {
            if cites[top_ix].is_first {
                let mut moved = 0;
                let mut ix = top_ix;
                while ix < cites.len() {
                    if ix != top_ix && !cites[ix].should_collapse {
                        break;
                    }
                    moved += 1;
                    let year_and_suf =
                        IR::find_first_year_and_suffix(cites[ix].gen4.root, &cites[ix].gen4.arena)
                            .and_then(|(ys_node, suf)| {
                                let flat = IR::flatten(ys_node, &cites[ix].gen4.arena, fmt, None)?;
                                Some((fmt.output(flat, false), suf))
                            });
                    if let Some((y, suf)) = year_and_suf {
                        cites[ix].year_suffix = Some(suf);
                        same_years
                            .entry(y)
                            .and_modify(|(oix, seen_once)| {
                                if *oix == ix - 1 {
                                    if !*seen_once {
                                        cites[*oix].first_of_ys = true;
                                    }
                                    cites[ix].collapse_ys = true;
                                    *seen_once = true;
                                } else {
                                    *seen_once = false;
                                }
                                *oix = ix;
                            })
                            .or_insert((ix, false));
                    }
                    ix += 1;
                }
                top_ix += moved;
            }
            top_ix += 1;
        }
    }

    if collapse == Some(Collapse::CitationNumber) {
        // XXX: Gotta factor in that some might have prefixes and suffixes
        if let Some((first, rest)) = cites.split_first_mut() {
            first.is_first = true;
            for r in rest {
                r.should_collapse = true;
            }
        }
    }

    if let Some(collapse) = collapse {
        match collapse {
            Collapse::CitationNumber => {
                let mut ix = 0;
                while ix < cites.len() {
                    let slice = &mut cites[ix..];
                    if let Some((u, rest)) = slice.split_first_mut() {
                        if u.is_first {
                            let following = rest.iter_mut().take_while(|u| u.should_collapse);

                            let mut cnums = Vec::new();
                            if let Some(cnum) = u.cnum {
                                cnums.push(CnumIx::new(cnum, ix));
                            }
                            let mut count = 0;
                            for (nix, cite) in following.enumerate() {
                                if let Some(cnum) = cite.cnum {
                                    cnums.push(CnumIx {
                                        cnum,
                                        ix: ix + nix + 1,
                                        force_single: cite.has_locator,
                                    })
                                }
                                cite.vanished = true;
                                count += 1;
                            }
                            ix += count;
                            u.collapsed_ranges = collapse_ranges(&cnums);
                        }
                    }
                    ix += 1;
                }
            }
            Collapse::Year => {
                let mut ix = 0;
                while ix < cites.len() {
                    let slice = &mut cites[ix..];
                    if let Some((u, rest)) = slice.split_first_mut() {
                        if u.is_first {
                            let following = rest.iter_mut().take_while(|u| u.should_collapse);
                            let mut count = 0;
                            for cite in following {
                                let gen4 = Arc::make_mut(&mut cite.gen4);
                                IR::suppress_names(gen4.root, &mut gen4.arena);
                                count += 1;
                            }
                            ix += count;
                        }
                    }
                    ix += 1;
                }
            }
            Collapse::YearSuffixRanged | Collapse::YearSuffix => {
                let mut ix = 0;
                while ix < cites.len() {
                    let slice = &mut cites[ix..];
                    if let Some((u, rest)) = slice.split_first_mut() {
                        if u.is_first {
                            let following = rest.iter_mut().take_while(|u| u.should_collapse);
                            for cite in following {
                                let gen4 = Arc::make_mut(&mut cite.gen4);
                                IR::suppress_names(gen4.root, &mut gen4.arena)
                            }
                        }
                        if u.first_of_ys {
                            let following = rest.iter_mut().take_while(|u| u.collapse_ys);

                            if collapse == Collapse::YearSuffixRanged {
                                // Potentially confusing; 'cnums' here are year suffixes in u32 form.
                                let mut cnums = Vec::new();
                                if let Some(cnum) = u.year_suffix {
                                    cnums.push(CnumIx::new(cnum, ix));
                                }
                                for (nix, cite) in following.enumerate() {
                                    if let Some(cnum) = cite.year_suffix {
                                        cnums.push(CnumIx {
                                            cnum,
                                            ix: ix + nix + 1,
                                            force_single: cite.has_locator,
                                        });
                                    }
                                    cite.vanished = true;
                                    if !cite.has_locator {
                                        let gen4 = Arc::make_mut(&mut cite.gen4);
                                        IR::suppress_year(gen4.root, &mut gen4.arena);
                                    }
                                }
                                u.collapsed_year_suffixes = collapse_ranges(&cnums);
                            } else {
                                if let Some(cnum) = u.year_suffix {
                                    u.collapsed_year_suffixes
                                        .push(RangePiece::Single(CnumIx::new(cnum, ix)));
                                }
                                for (nix, cite) in following.enumerate() {
                                    if let Some(cnum) = cite.year_suffix {
                                        u.collapsed_year_suffixes.push(RangePiece::Single(
                                            CnumIx {
                                                cnum,
                                                ix: ix + nix + 1,
                                                force_single: cite.has_locator,
                                            },
                                        ));
                                    }
                                    cite.vanished = true;
                                    let gen4 = Arc::make_mut(&mut cite.gen4);
                                    IR::suppress_year(gen4.root, &mut gen4.arena);
                                }
                            }
                        }
                    }
                    ix += 1;
                }
            }
        }
    }
}

fn pair_at_mut<T>(mut slice: &mut [T], ix: usize) -> Option<(&mut T, &mut T)> {
    let nix = ix + 1;
    slice = &mut slice[ix..];
    if slice.len() < 2 || nix >= slice.len() {
        return None;
    }
    slice
        .split_first_mut()
        .and_then(|(first, rest)| rest.first_mut().map(|second| (first, second)))
}

////////////////////////////////
// Cite Grouping & Collapsing //
////////////////////////////////

use crate::disamb::names::DisambNameRatchet;
use citeproc_io::PersonName;
use csl::SubsequentAuthorSubstituteRule as SasRule;

#[derive(Eq, PartialEq, Clone)]
pub enum ReducedNameToken<'a, B> {
    Name(&'a PersonName),
    Literal(&'a B),
    EtAl,
    Ellipsis,
    Delimiter,
    And,
    Space,
}

impl<'a, T: Debug> Debug for ReducedNameToken<'a, T> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            ReducedNameToken::Name(p) => write!(f, "{:?}", p.family),
            ReducedNameToken::Literal(b) => write!(f, "{:?}", b),
            ReducedNameToken::EtAl => write!(f, "EtAl"),
            ReducedNameToken::Ellipsis => write!(f, "Ellipsis"),
            ReducedNameToken::Delimiter => write!(f, "Delimiter"),
            ReducedNameToken::And => write!(f, "And"),
            ReducedNameToken::Space => write!(f, "Space"),
        }
    }
}

impl<'a, T> ReducedNameToken<'a, T> {
    fn from_token(token: &NameToken, names: &'a [DisambNameRatchet<T>]) -> Self {
        match token {
            NameToken::Name(dnr_index) => match &names[*dnr_index] {
                DisambNameRatchet::Person(p) => ReducedNameToken::Name(&p.data.value),
                DisambNameRatchet::Literal { literal, .. } => ReducedNameToken::Literal(literal),
            },
            NameToken::Ellipsis => ReducedNameToken::Ellipsis,
            NameToken::EtAl(..) => ReducedNameToken::EtAl,
            NameToken::Space => ReducedNameToken::Space,
            NameToken::Delimiter => ReducedNameToken::Delimiter,
            NameToken::And => ReducedNameToken::And,
        }
    }
    fn relevant(&self) -> bool {
        match self {
            ReducedNameToken::Name(_) | ReducedNameToken::Literal(_) => true,
            _ => false,
        }
    }
}

impl<O: OutputFormat> IR<O> {
    pub(crate) fn unwrap_name_ir(&self) -> &NameIR<O> {
        match self {
            IR::Name(nir) => nir,
            _ => panic!("Called unwrap_name_ir on a {:?}", self),
        }
    }
    pub(crate) fn unwrap_name_ir_mut(&mut self) -> &mut NameIR<O> {
        match self {
            IR::Name(nir) => nir,
            _ => panic!("Called unwrap_name_ir_mut on a {:?}", self),
        }
    }
    pub(crate) fn unwrap_year_suffix(&self) -> &YearSuffix {
        match self {
            IR::YearSuffix(ys) => ys,
            _ => panic!("Called unwrap_year_suffix on a {:?}", self),
        }
    }
    pub(crate) fn unwrap_year_suffix_mut(&mut self) -> &mut YearSuffix {
        match self {
            IR::YearSuffix(ys) => ys,
            _ => panic!("Called unwrap_year_suffix_mut on a {:?}", self),
        }
    }
    pub(crate) fn unwrap_cond_disamb(&self) -> &ConditionalDisambIR {
        match self {
            IR::ConditionalDisamb(cond) => cond,
            _ => panic!("Called unwrap_cond_disamb on a {:?}", self),
        }
    }
    pub(crate) fn unwrap_cond_disamb_mut(&mut self) -> &mut ConditionalDisambIR {
        match self {
            IR::ConditionalDisamb(cond) => cond,
            _ => panic!("Called unwrap_cond_disamb_mut on a {:?}", self),
        }
    }
}

pub fn subsequent_author_substitute<O: OutputFormat>(
    fmt: &O,
    previous: &NameIR<O>,
    current_id: NodeId,
    arena: &mut IrArena<O>,
    sas: &str,
    sas_rule: SasRule,
) -> bool {
    let pre_tokens = previous.iter_bib_rendered_names(fmt);
    let pre_reduced = pre_tokens
        .iter()
        .map(|tok| ReducedNameToken::from_token(tok, &previous.disamb_names))
        .filter(|x| x.relevant());

    let cur = arena.get(current_id).unwrap().get().0.unwrap_name_ir();
    let label_after_name = cur
        .names_inheritance
        .label
        .as_ref()
        .map_or(false, |l| l.after_name);
    let built_label = cur.built_label.clone();

    let cur_tokens = cur.iter_bib_rendered_names(fmt);
    let cur_reduced = cur_tokens
        .iter()
        .map(|tok| ReducedNameToken::from_token(tok, &cur.disamb_names))
        .filter(|x| x.relevant());
    debug!(
        "{:?} vs {:?}",
        pre_reduced.clone().collect::<Vec<_>>(),
        cur_reduced.clone().collect::<Vec<_>>()
    );

    match sas_rule {
        SasRule::CompleteAll | SasRule::CompleteEach => {
            if Iterator::eq(pre_reduced, cur_reduced) {
                let (current_ir, _current_gv) = arena.get_mut(current_id).unwrap().get_mut();
                if sas_rule == SasRule::CompleteEach {
                    let current_nir = current_ir.unwrap_name_ir_mut();
                    // let nir handle it
                    // u32::MAX so ALL names get --- treatment
                    if let Some(rebuilt) =
                        current_nir.subsequent_author_substitute(fmt, std::u32::MAX, sas)
                    {
                        let node = NameIR::rendered_ntbs_to_node(
                            rebuilt,
                            arena,
                            false,
                            label_after_name,
                            built_label.as_ref(),
                        );
                        replace_single_child(current_id, node, arena);
                    }
                } else if sas.is_empty() {
                    let empty_node = arena.new_node((IR::Rendered(None), GroupVars::Important));
                    replace_single_child(current_id, empty_node, arena);
                } else {
                    // Remove all children
                    let children: Vec<_> = current_id.children(arena).collect();
                    children.into_iter().for_each(|ch| ch.remove_subtree(arena));

                    // Add the sas ---
                    let sas_ir = arena.new_node((
                        IR::Rendered(Some(CiteEdgeData::Output(fmt.plain(sas)))),
                        GroupVars::Important,
                    ));
                    current_id.append(sas_ir, arena);

                    // Add a name label
                    if let Some(label) = built_label.as_ref() {
                        let label_node = arena.new_node((
                            IR::Rendered(Some(CiteEdgeData::Output(label.clone()))),
                            GroupVars::Plain,
                        ));
                        if label_after_name {
                            current_id.append(label_node, arena)
                        } else {
                            current_id.prepend(label_node, arena)
                        }
                    }
                };
                return true;
            }
        }
        SasRule::PartialEach => {
            let count = pre_reduced
                .zip(cur_reduced)
                .take_while(|(p, c)| p == c)
                .count();
            let current = arena.get_mut(current_id).unwrap().get_mut();
            let current_nir = current.0.unwrap_name_ir_mut();
            if let Some(rebuilt) = current_nir.subsequent_author_substitute(fmt, count as u32, sas)
            {
                let node = NameIR::rendered_ntbs_to_node(
                    rebuilt,
                    arena,
                    false,
                    label_after_name,
                    built_label.as_ref(),
                );
                replace_single_child(current_id, node, arena);
            }
        }
        SasRule::PartialFirst => {
            let count = pre_reduced
                .zip(cur_reduced)
                .take_while(|(p, c)| p == c)
                .count();
            if count > 0 {
                let current = arena.get_mut(current_id).unwrap().get_mut();
                let current_nir = current.0.unwrap_name_ir_mut();
                if let Some(rebuilt) = current_nir.subsequent_author_substitute(fmt, 1, sas) {
                    let node = NameIR::rendered_ntbs_to_node(
                        rebuilt,
                        arena,
                        false,
                        label_after_name,
                        built_label.as_ref(),
                    );
                    replace_single_child(current_id, node, arena);
                }
            }
        }
    }
    false
}

///////////////////////
// MixedNumericStyle //
///////////////////////

pub fn style_is_mixed_numeric(
    style: &csl::Style,
    cite_or_bib: CiteOrBib,
) -> Option<(&Element, Option<&str>)> {
    use csl::style::{Element as El, TextSource as TS, *};
    use csl::variables::{NumberVariable::CitationNumber, StandardVariable as SV};
    fn cnum_renders_first<'a>(
        els: &'a [El],
        maybe_delim: Option<&'a str>,
    ) -> Option<(&'a Element, Option<&'a str>)> {
        for el in els {
            match el {
                El::Text(TextElement {
                    source: TS::Variable(SV::Number(CitationNumber), _),
                    ..
                }) => return Some((el, maybe_delim)),
                El::Number(NumberElement {
                    variable: CitationNumber,
                    ..
                }) => return Some((el, maybe_delim)),
                El::Group(Group {
                    elements,
                    delimiter,
                    ..
                }) => {
                    return cnum_renders_first(elements, delimiter.as_opt_str());
                }
                El::Choose(c) => {
                    let Choose(if_, ifthens_, else_) = c.as_ref();

                    // You could have a citation number appear first in the bibliography in an else
                    // block. You wouldn't, but you could.
                    let either = cnum_renders_first(&if_.1, maybe_delim).or_else(|| {
                        ifthens_
                            .iter()
                            .find_map(|ifthen| cnum_renders_first(&ifthen.1, maybe_delim))
                    });
                    if either.is_some() {
                        return either;
                    } else if else_.0.is_empty() {
                        // No else block? The choose could be empty.
                        continue;
                    } else {
                        let else_found = cnum_renders_first(&else_.0, maybe_delim);
                        if else_found.is_some() {
                            return else_found;
                        }
                    }
                }
                _ => break,
            }
        }
        None
    }
    style
        .get_layout(cite_or_bib)
        .and_then(|layout| cnum_renders_first(&layout.elements, None))
}

#[test]
fn test_mixed_numeric() {
    use csl::style::{Element as El, TextSource as TS, *};
    use csl::variables::{NumberVariable::CitationNumber, StandardVariable as SV};
    let mk = |layout: &str| {
        let txt = format!(
            r#"
            <style class="in-text" version="1.0">
                <citation><layout></layout></citation>
                <bibliography><layout>
                    {}
                </layout></bibliography>
            </style>
        "#,
            layout
        );
        Style::parse_for_test(&txt).unwrap()
    };
    let style = mk(r#"<group delimiter=". "> <text variable="citation-number" /> </group>"#);
    let found = style_is_mixed_numeric(&style, CiteOrBib::Bibliography);
    let model_el = El::Text(TextElement {
        source: TS::Variable(SV::Number(CitationNumber), VariableForm::Long),
        ..Default::default()
    });
    assert_eq!(found, Some((&model_el, Some(". "))));
    let style = mk(r#"
       <group delimiter=". ">
           <choose>
               <if type="book">
                   <text variable="citation-number" />
                   <text variable="title" />
               </if>
           </choose>
       </group>"#);
    let found = style_is_mixed_numeric(&style, CiteOrBib::Bibliography);
    assert_eq!(found, Some((&model_el, Some(". "))));
    let style = mk(r#"
       <choose>
           <if type="book">
               <group delimiter=". ">
                   <text variable="citation-number" />
               </group>
           </if>
       </choose>
       <text variable="title" />
       "#);
    let found = style_is_mixed_numeric(&style, CiteOrBib::Bibliography);
    assert_eq!(found, Some((&model_el, Some(". "))));
    let style = mk(r#"
       <choose>
           <if type="book">
               <group delimiter=". ">
                   <number variable="citation-number" />
                   <text variable="title" />
               </group>
           </if>
       </choose>
       "#);
    let found = style_is_mixed_numeric(&style, CiteOrBib::Bibliography);
    assert!(matches!(found, Some((_, Some(". ")))));
}

////////////////////////////////////////////////////
// Layout affixes inside left-margin/right-inline //
////////////////////////////////////////////////////

#[derive(Debug, PartialEq)]
struct LeftRightLayout {
    left: Option<NodeId>,
    right: Option<NodeId>,
    layout: NodeId,
}

fn find_left_right_layout<O: OutputFormat>(
    root: NodeId,
    arena: &IrArena<O>,
) -> Option<LeftRightLayout> {
    let node = arena.get(root)?;
    match &node.get().0 {
        IR::Seq(seq)
            if seq.is_layout
                && seq
                    .affixes
                    .as_ref()
                    .map_or(false, |af| !af.prefix.is_empty() || !af.suffix.is_empty()) =>
        {
            let left = node.first_child()
                .filter(|c| matches!(arena.get(*c).map(|x| &x.get().0), Some(IR::Seq(IrSeq { display: Some(DisplayMode::LeftMargin), .. }))));
            let right = node.last_child()
                .filter(|c| matches!(arena.get(*c).map(|x| &x.get().0), Some(IR::Seq(IrSeq { display: Some(DisplayMode::RightInline), .. }))));
            Some(LeftRightLayout {
                left,
                right,
                layout: root,
            })
        }
        _ => None,
    }
}

pub fn fix_left_right_layout_affixes<O: OutputFormat>(
    root: NodeId,
    arena: &mut IrArena<O>,
    fmt: &O,
) {
    let LeftRightLayout {
        left,
        right,
        layout,
    } = match find_left_right_layout(root, arena) {
        Some(lrl) => lrl,
        None => return,
    };

    fn get_af<O: OutputFormat>(node_id: NodeId, suf: bool, arena: &IrArena<O>) -> &str {
        match &arena[node_id].get().0 {
            IR::Seq(s) => s
                .affixes
                .as_ref()
                .map(|af| if suf { &af.suffix } else { &af.prefix })
                .map_or("", |af| af.as_str()),
            _ => "",
        }
    }
    fn write_af<O: OutputFormat>(
        node_id: NodeId,
        suf: bool,
        content: SmartString,
        arena: &mut IrArena<O>,
    ) {
        match &mut arena[node_id].get_mut().0 {
            IR::Seq(s) => {
                match &mut s.affixes {
                    Some(af) => {
                        let which = if suf { &mut af.suffix } else { &mut af.prefix };
                        *which = content;
                        if af.prefix.is_empty() && af.suffix.is_empty() {
                            s.affixes = None;
                        }
                    }
                    None if !content.is_empty() => {
                        let mut af = Affixes::default();
                        let which = if suf { &mut af.suffix } else { &mut af.prefix };
                        *which = content;
                        s.affixes = Some(af);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    if let Some(left) = left {
        let layout_prefix = get_af(layout, false, arena);
        if !layout_prefix.is_empty() {
            let left_prefix = get_af(left, false, arena);
            let mut new_prefix = SmartString::new();
            new_prefix.push_str(layout_prefix);
            new_prefix.push_str(left_prefix);
            write_af(left, false, new_prefix, arena);
            write_af(layout, false, "".into(), arena);
        }
    }
    if let Some(right) = right {
        let layout_suffix = get_af(layout, true, arena);
        if !layout_suffix.is_empty() {
            let right_suffix = get_af(right, true, arena);
            let mut new_suffix = SmartString::new();
            new_suffix.push_str(right_suffix);
            new_suffix.push_str(layout_suffix);
            write_af(right, true, new_suffix, arena);
            write_af(layout, true, "".into(), arena);
        }
    }
}

#[test]
fn test_left_right_layout() {
    use csl::style::{Element as El, TextSource as TS, *};
    use csl::variables::{NumberVariable::CitationNumber, StandardVariable as SV};
    let mut arena = IrArena::<Markup>::new();
    let fmt = Markup::html();

    let left = arena.seq(
        IrSeq {
            display: Some(DisplayMode::LeftMargin),
            ..Default::default()
        },
        |arena, seq| {
            let cnum = arena.blob(
                CiteEdgeData::CitationNumber(fmt.plain("2. ")),
                GroupVars::Important,
            );
            seq.append(cnum, arena);
        },
    );
    let right = arena.seq(
        IrSeq {
            display: Some(DisplayMode::RightInline),
            ..Default::default()
        },
        |arena, seq| {
            let title = arena.blob(
                CiteEdgeData::Output(fmt.plain("title")),
                GroupVars::Important,
            );
            seq.append(title, arena);
        },
    );
    let layout = arena.seq(
        IrSeq {
            is_layout: true,
            affixes: Some(Affixes {
                prefix: "".into(),
                suffix: ".".into(),
            }),
            ..Default::default()
        },
        |arena, seq| {
            seq.append(left, arena);
            seq.append(right, arena);
        },
    );

    let mut irgen = IrGen {
        root: layout,
        arena,
        state: IrState::new(),
    };
    dbg!(&irgen);

    let found = find_left_right_layout(layout, &mut irgen.arena);
    assert_eq!(
        found,
        Some(LeftRightLayout {
            left: Some(left),
            right: Some(right),
            layout
        })
    );

    let blob = irgen
        .arena
        .blob(CiteEdgeData::Output(fmt.plain("blob")), GroupVars::Plain);
    right.insert_before(blob, &mut irgen.arena);

    dbg!(&irgen);

    let found = find_left_right_layout(layout, &mut irgen.arena);
    assert_eq!(
        found,
        Some(LeftRightLayout {
            left: Some(left),
            right: Some(right),
            layout
        })
    );

    fix_left_right_layout_affixes(layout, &mut irgen.arena, &fmt);

    let flat = IR::flatten(layout, &irgen.arena, &fmt, None).unwrap();
    let s = fmt.output(flat, false);
    assert_eq!(
        &s,
        r#"<div class="csl-left-margin">2. </div>blob<div class="csl-right-inline">title.</div>"#
    );
}

#[cfg(test)]
trait ArenaExtensions<O: OutputFormat> {
    fn blob(&mut self, edge: CiteEdgeData<O>, gv: GroupVars) -> NodeId;
    fn seq<F: FnOnce(&mut Self, NodeId)>(&mut self, seq_tmpl: IrSeq, f: F) -> NodeId;
}

#[cfg(test)]
impl<O: OutputFormat> ArenaExtensions<O> for IrArena<O> {
    fn blob(&mut self, edge: CiteEdgeData<O>, gv: GroupVars) -> NodeId {
        self.new_node((IR::Rendered(Some(edge)), gv))
    }
    fn seq<F: FnOnce(&mut Self, NodeId)>(&mut self, seq_tmpl: IrSeq, f: F) -> NodeId {
        let seq_node = self.new_node((IR::Seq(seq_tmpl), GroupVars::Important));
        f(self, seq_node);
        seq_node
    }
}
