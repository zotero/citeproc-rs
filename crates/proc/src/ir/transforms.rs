use crate::cluster::CiteInCluster;
use crate::disamb::names::{replace_single_child, NameIR};
use crate::helpers::slice_group_by::group_by_mut;
use crate::names::NameToken;
use crate::prelude::*;
use citeproc_io::{CiteMode, ClusterMode};
use std::mem;
use std::sync::Arc;

/////////////////////////////////
// capitalize start of cluster //
/////////////////////////////////

impl<O: OutputFormat> IrTree<O> {
    pub fn capitalize_first_term_of_cluster(&mut self, fmt: &O) {
        if let Some(node) = self.tree_ref().find_term_rendered_first() {
            let trf = match self.arena.get_mut(node).unwrap().get_mut().0 {
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
}

impl<O: OutputFormat> IrTreeRef<'_, O> {
    // Gotta find a a CiteEdgeData::Term/LocatorLabel/FrnnLabel
    // (the latter two are also terms, but a different kind for disambiguation).
    fn find_term_rendered_first(&self) -> Option<NodeId> {
        match self.arena.get(self.node)?.get().0 {
            IR::Rendered(Some(CiteEdgeData::Term(_)))
            | IR::Rendered(Some(CiteEdgeData::LocatorLabel(_)))
            | IR::Rendered(Some(CiteEdgeData::FrnnLabel(_))) => Some(self.node),
            IR::ConditionalDisamb(_) | IR::Seq(_) | IR::Substitute => self
                .node
                .children(self.arena)
                .next()
                .and_then(|child| self.with_node(child).find_term_rendered_first()),
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

impl<'a, O: OutputFormat> IrTreeRef<'a, O> {
    pub fn find_locator(&self) -> Option<NodeId> {
        match self.get_node()?.get().0 {
            IR::Rendered(Some(CiteEdgeData::Locator(_))) => Some(self.node),
            IR::ConditionalDisamb(_) | IR::Seq(_) | IR::Substitute => {
                // Search backwards because it's likely to be near the end
                self.reverse_children()
                    .find_map(|child| child.find_locator())
            }
            _ => None,
        }
    }

    pub fn first_names_block(&self) -> Option<NodeId> {
        match self.get_node()?.get().0 {
            IR::Name(_) => Some(self.node),
            IR::ConditionalDisamb(_) | IR::Seq(_) | IR::Substitute => {
                // assumes it's the first one that appears
                self.children().find_map(|child| child.first_names_block())
            }
            _ => None,
        }
    }

    fn find_first_year(&self) -> Option<NodeId> {
        match &self.get_node()?.get().0 {
            IR::Rendered(Some(CiteEdgeData::Year(_b))) => Some(self.node),
            IR::Seq(_) | IR::ConditionalDisamb(_) | IR::Substitute => {
                self.children().find_map(|child| child.find_first_year())
            }
            _ => None,
        }
    }

    pub fn has_explicit_year_suffix(&self) -> Option<u32> {
        match self.get_node()?.get().0 {
            IR::YearSuffix(YearSuffix {
                hook: YearSuffixHook::Explicit(_),
                suffix_num: Some(n),
                ..
            }) if !self.is_empty() => Some(n),
            IR::ConditionalDisamb(_) | IR::Seq(_) | IR::Substitute => {
                // assumes it's the first one that appears
                self.children()
                    .find_map(|child| child.has_explicit_year_suffix())
            }
            _ => None,
        }
    }

    pub fn has_implicit_year_suffix(&self) -> Option<u32> {
        match self.get_node()?.get().0 {
            IR::YearSuffix(YearSuffix {
                hook: YearSuffixHook::Plain,
                suffix_num: Some(n),
                ..
            }) if !self.is_empty() => Some(n),
            IR::ConditionalDisamb(_) | IR::Seq(_) | IR::Substitute => {
                // assumes it's the first one that appears
                self.children()
                    .find_map(|child| child.has_implicit_year_suffix())
            }
            _ => None,
        }
    }

    /// Rendered(None), empty YearSuffix or empty seq
    pub fn is_empty(&self) -> bool {
        let me = match self.get_node() {
            Some(x) => x.get(),
            None => return false,
        };
        match &me.0 {
            IR::Rendered(opt) => opt.is_none(),
            IR::Seq(_)
            | IR::Name(_)
            | IR::ConditionalDisamb(_)
            | IR::YearSuffix(_)
            | IR::Substitute => self.children().next().is_none(),
            IR::NameCounter(_nc) => false,
        }
    }

    pub fn find_year_suffix(&self) -> Option<u32> {
        self.has_explicit_year_suffix()
            .or_else(|| self.has_implicit_year_suffix())
    }

    pub fn find_first_year_and_suffix(&self) -> Option<(NodeId, u32)> {
        // if let Some(fy) = IR::find_first_year(node, arena) {
        //     debug!("fy, {:?}", arena.get(fy).unwrap().get().0);
        // }
        // if let Some(ys) = IR::find_year_suffix(node, arena) {
        //     debug!("ys, {:?}", ys);
        // }
        Some((self.find_first_year()?, self.find_year_suffix()?))
    }
}

impl<O: OutputFormat> IrTree<O> {
    pub fn suppress_names(&mut self) {
        if let Some(fnb) = self.tree_ref().first_names_block() {
            // TODO: check interaction of this with GroupVars of the parent seq
            fnb.remove_subtree(&mut self.arena);
        }
    }

    pub fn suppress_year(&mut self) {
        let has_explicit = self.tree_ref().has_explicit_year_suffix().is_some();
        let has_implicit = self.tree_ref().has_implicit_year_suffix().is_some();
        if !has_explicit && !has_implicit {
            return;
        }
        self.mutable().suppress_first_year(has_explicit);
    }
}

impl<'a, O: OutputFormat> IrTreeMut<'a, O> {
    /// Rest of the name: "if it has a year suffix"
    fn suppress_first_year(&mut self, has_explicit: bool) -> Option<NodeId> {
        match self.root_mut()?.get().0 {
            IR::Rendered(Some(CiteEdgeData::Year(_))) => {
                self.root_mut()?.get_mut().0 = IR::Rendered(None);
                Some(self.node)
            }
            IR::ConditionalDisamb(_) => None,
            IR::Seq(_) | IR::Substitute => {
                let mut iter = self.node.children(self.arena).fuse();
                let first_two = (iter.next(), iter.next());
                // Check for the exact explicit year suffix IR output
                let mut found = if iter.next().is_some() {
                    None
                } else if let (Some(first), Some(second)) = first_two {
                    match self.arena.get(second).unwrap().get() {
                        (IR::YearSuffix(_), GroupVars::UnresolvedImportant) if has_explicit => {
                            self.with_node(first, |f| f.suppress_first_year(has_explicit))
                        }
                        (IR::YearSuffix(_), GroupVars::Important)
                            if !has_explicit && !self.tree_at_node(second).is_empty() =>
                        {
                            self.with_node(first, |f| f.suppress_first_year(has_explicit))
                        }
                        _ => None,
                    }
                } else {
                    None
                };

                // Otherwise keep looking in subtrees etc
                if found.is_none() {
                    let child_ids: Vec<_> = self.node.children(self.arena).collect();
                    for child in child_ids {
                        found = self.with_node(child, |ch| ch.suppress_first_year(has_explicit));
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
}

impl<O: OutputFormat> IrTree<O> {
    pub fn suppress_author(&mut self) -> Option<NodeId> {
        if let Some(node) = self.tree_ref().leading_names_block_or_title(false) {
            // TODO: check interaction of this with GroupVars of the parent seq
            node.detach(&mut self.arena);
            Some(node)
        } else {
            None
        }
    }
    pub fn author_only_strip_formatting(
        &mut self,
        suppress_author_orig_node: NodeId,
        cite_pos: csl::Position,
        fmt: &O,
    ) -> Option<()> {
        let arena = &mut self.arena;
        match &mut arena.get_mut(suppress_author_orig_node)?.get_mut().0 {
            IR::Seq(seq) if seq.formatting.is_some() => {
                seq.formatting = None;
            }
            IR::Name(nir) => {
                log::debug!("NIR found, stripping formatting");
                if let Some(rebuilt) = nir.strip_formatting(fmt, cite_pos) {
                    log::debug!("strip formatting rebuilt as {:?}", rebuilt);
                    let label_after_name = nir
                        .names_inheritance
                        .label
                        .as_ref()
                        .map_or(false, |l| l.after_name);
                    let built_label = nir.built_label.clone();
                    let new_node = NameIR::rendered_ntbs_to_node(
                        rebuilt,
                        arena,
                        false,
                        label_after_name,
                        built_label.as_ref(),
                    );
                    replace_single_child(suppress_author_orig_node, new_node, arena);
                }
            }
            _ => {}
        }
        Some(())
    }
}

impl<O: OutputFormat> NameIR<O> {
    fn strip_formatting(&mut self, fmt: &O, cite_pos: csl::Position) -> Option<Vec<O::Build>> {
        use core::mem::replace;
        let old_ni = replace(&mut self.names_inheritance.formatting, None)
            .or(replace(&mut self.names_inheritance.name.formatting, None))
            .or(self
                .names_inheritance
                .name
                .name_part_family
                .as_mut()
                .and_then(|x| replace(&mut x.formatting, None)))
            .or(self
                .names_inheritance
                .name
                .name_part_given
                .as_mut()
                .and_then(|x| replace(&mut x.formatting, None)));
        for dn in self.disamb_names.iter_mut() {
            match dn {
                DisambNameRatchet::Person(pdnr) => {
                    pdnr.data.el = self.names_inheritance.name.clone();
                }
                _ => {}
            }
        }
        log::debug!(
            "NameIR::strip_formatting found some formatting to remove ({:?}), rebuilding names block",
            old_ni
        );
        if old_ni.is_some() {
            let rebuilt = self.intermediate_custom(fmt, cite_pos, false, None, None)?;
            Some(rebuilt)
        } else {
            None
        }
    }
}

impl<'a, O: OutputFormat> IrTreeRef<'a, O> {
    /// For author-only
    ///
    /// Returns the new node that should be considered the root.
    ///
    /// This strips away any formatting defined on any parent seq nodes, but that is acceptable; if
    /// a style wishes to have text appear formatted even in an in-text reference, then it should
    /// define an `<intext>` node.
    pub fn leading_names_block_or_title(&self, in_substitute: bool) -> Option<NodeId> {
        match self.get_node()?.get().0 {
            // Name block? Yes.
            IR::Name(_) => Some(self.node),

            // to filter down to titles and match the text of the citeproc-js docs (2021-07-02)
            // this would need to be IR::Rendered(Some(CiteEdgeData::Title(_))) => Some(self.node)
            // however citeproc-js appears to allow any content rendered as a substitute of any
            // names block to be extracted as author-only
            //
            // So we will simply return the Substitute node, which is returned by <Names as Proc>
            // in place of a Seq of NameIR nodes.
            IR::Substitute => Some(self.node),

            IR::ConditionalDisamb(_) | IR::Seq(_) => {
                // it must be at the start of the cite
                self.children()
                    .filter(|x| !x.is_empty())
                    .nth(0)
                    .and_then(|child| child.leading_names_block_or_title(in_substitute))
            }
            _ => None,
        }
    }
}

fn apply_author_only(
    db: &dyn IrDatabase,
    cite: &mut CiteInCluster<Markup>,
    position: csl::Position,
    fmt: &Markup,
) {
    let mut success = true;
    if let Some(intext) = db.intext(cite.cite_id) {
        // completely replace with the intext arena, no need to copy
        // into the old arena in gen4.
        cite.gen4 = intext;
    } else if let Some(new_root) = cite.gen4.tree_ref().leading_names_block_or_title(false) {
        let gen4 = Arc::make_mut(&mut cite.gen4);
        let tree = gen4.tree_mut();
        new_root.detach(&mut tree.arena);
        tree.root.remove_subtree(&mut tree.arena);
        tree.root = new_root;

        tree.author_only_strip_formatting(new_root, position, fmt);
    } else {
        success = false;
    }
    cite.destination = WhichStream::MainToIntext { success };
}

pub(crate) fn apply_cite_modes(
    db: &dyn IrDatabase,
    cites: &mut [CiteInCluster<Markup>],
    fmt: &Markup,
) {
    for cite in cites {
        match cite.cite.mode {
            Some(CiteMode::AuthorOnly) => {
                apply_author_only(db, cite, cite.position, fmt);
            }
            Some(CiteMode::SuppressAuthor) => {
                let gen4 = Arc::make_mut(&mut cite.gen4);
                let _discard = gen4.tree_mut().suppress_author();
            }
            None => {}
        }
    }
}

pub(crate) fn apply_cluster_mode(
    db: &dyn IrDatabase,
    mode: &ClusterMode,
    cites: &mut [CiteInCluster<Markup>],
    class: csl::StyleClass,
    fmt: &Markup,
) {
    fn first_n_authors<'a>(
        cites: &'a mut [CiteInCluster<Markup>],
        suppress_first: u32,
    ) -> impl Iterator<Item = &'a mut CiteInCluster<Markup>> + 'a {
        let by_name = group_by_mut(cites, |a, b| a.by_name() == b.by_name());
        let take = if suppress_first > 0 {
            suppress_first as usize
        } else {
            core::usize::MAX
        };
        by_name.take(take).flatten()
    }
    match *mode {
        ClusterMode::AuthorOnly => {
            for cite in cites.iter_mut() {
                apply_author_only(db, cite, cite.position, fmt);
            }
        }
        ClusterMode::SuppressAuthor { suppress_first } => {
            if class == csl::StyleClass::Note {
                log::warn!(
                    "attempt to use cluster mode suppress-author with a note style, will be no-op"
                );
                return;
            }
            let suppress_it = |cite: &mut CiteInCluster<Markup>| {
                let gen4 = Arc::make_mut(&mut cite.gen4);
                let _discard = gen4.tree_mut().suppress_author();
                cite.destination = WhichStream::MainToCitation;
            };

            first_n_authors(cites, suppress_first).for_each(suppress_it);
        }
        ClusterMode::Composite { suppress_first, .. } => {
            if class == csl::StyleClass::Note {
                log::warn!(
                    "attempt to use cluster mode composite with a note style, will be no-op"
                );
                return;
            }
            for CiteInCluster {
                cite_id,
                gen4,
                destination,
                position,
                ..
            } in first_n_authors(cites, suppress_first)
            {
                let gen4 = Arc::make_mut(gen4);
                log::debug!("called Composite, with tree {:?}", gen4.tree_ref());
                let intext_part = if let Some(removed_node) = gen4.tree_mut().suppress_author() {
                    log::debug!(
                        "removed node from composite: {}",
                        gen4.tree().tree_at_node(removed_node),
                    );
                    if let Some(intext) = db.intext(*cite_id) {
                        log::debug!("using <intext> node for composite: {}", intext.tree());
                        let gen4_tree = gen4.tree_mut();
                        removed_node.remove_subtree(&mut gen4_tree.arena);
                        // this only fails if the tree root is not a valid node, but we need an
                        // Option<NodeId> anyway, so leave it
                        gen4.tree_mut().extend(intext.tree().tree_ref())
                    } else {
                        gen4.tree_mut()
                            .author_only_strip_formatting(removed_node, *position, fmt);
                        log::debug!(
                            "reformatted node from composite: {}",
                            gen4.tree().tree_at_node(removed_node),
                        );
                        Some(removed_node)
                    }
                } else {
                    None
                };
                *destination = WhichStream::MainToCitationPlusIntext(intext_part);
            }
        }
    }
}

use crate::cluster::WhichStream;
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

impl<'a, T: core::fmt::Debug> core::fmt::Debug for ReducedNameToken<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
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

#[allow(dead_code)]
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
    #[allow(dead_code)]
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
        Style::parse_for_test(&txt, None).unwrap()
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
            let left = node.first_child().filter(|c| {
                matches!(
                    arena.get(*c).map(|x| &x.get().0),
                    Some(IR::Seq(IrSeq {
                        display: Some(DisplayMode::LeftMargin),
                        ..
                    }))
                )
            });
            let right = node.last_child().filter(|c| {
                matches!(
                    arena.get(*c).map(|x| &x.get().0),
                    Some(IR::Seq(IrSeq {
                        display: Some(DisplayMode::RightInline),
                        ..
                    }))
                )
            });
            Some(LeftRightLayout {
                left,
                right,
                layout: root,
            })
        }
        _ => None,
    }
}

pub fn fix_left_right_layout_affixes<O: OutputFormat>(root: NodeId, arena: &mut IrArena<O>) {
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
            IR::Seq(s) => match &mut s.affixes {
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
            },
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

    let mut tree = dbg!(IrTree::new(layout, arena));

    let found = find_left_right_layout(tree.root, &mut tree.arena);
    assert_eq!(
        found,
        Some(LeftRightLayout {
            left: Some(left),
            right: Some(right),
            layout
        })
    );

    let blob = tree
        .arena
        .blob(CiteEdgeData::Output(fmt.plain("blob")), GroupVars::Plain);
    right.insert_before(blob, &mut tree.arena);

    dbg!(&tree);

    let found = find_left_right_layout(tree.root, &mut tree.arena);
    assert_eq!(
        found,
        Some(LeftRightLayout {
            left: Some(left),
            right: Some(right),
            layout
        })
    );

    fix_left_right_layout_affixes(layout, &mut tree.arena);

    let flat = tree.tree_ref().flatten(&fmt, None).unwrap();
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
