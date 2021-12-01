use crate::prelude::*;
use core::fmt;
use indextree::Arena;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct IrTree<O: OutputFormat = Markup> {
    pub(crate) root: NodeId,
    pub(crate) arena: IrArena<O>,
}

#[allow(dead_code)]
impl<O: OutputFormat> IrTree<O> {
    pub(crate) fn new(root: NodeId, arena: IrArena<O>) -> Self {
        Self { root, arena }
    }
    pub(crate) fn is_empty(node: NodeId, arena: &IrArena<O>) -> bool {
        (IrTreeRef { node, arena }).is_empty()
    }
    /// Returns the NodeId in self.arena that represents the copy of src.node.
    pub(crate) fn extend(&mut self, src: IrTreeRef<O>) -> Option<NodeId> {
        arena_copy_tree(src.node, src.arena, &mut self.arena)
    }
    pub(crate) fn tree_ref(&self) -> IrTreeRef<O> {
        IrTreeRef {
            node: self.root,
            arena: &self.arena,
        }
    }
    pub(crate) fn tree_at_node(&self, node: NodeId) -> IrTreeRef<O> {
        IrTreeRef {
            node,
            arena: &self.arena,
        }
    }
    pub(crate) fn get_mut(
        &mut self,
        node: NodeId,
    ) -> Option<&mut indextree::Node<(IR<O>, GroupVars)>> {
        self.arena.get_mut(node)
    }
    pub(crate) fn root_mut(&mut self) -> Option<&mut indextree::Node<(IR<O>, GroupVars)>> {
        self.arena.get_mut(self.root)
    }
}

#[allow(dead_code)]
impl<'a, O: OutputFormat> IrTreeRef<'a, O> {
    pub(crate) fn new(node: NodeId, arena: &'a IrArena<O>) -> Self {
        Self { node, arena }
    }
    pub(crate) fn with_node(&self, node: NodeId) -> IrTreeRef<O> {
        IrTreeRef {
            node,
            arena: &self.arena,
        }
    }
}

impl<O: OutputFormat> fmt::Display for IrTree<O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.tree_ref().fmt(f)
    }
}

impl<O: OutputFormat> fmt::Debug for IrTree<O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.tree_ref().fmt(f)
    }
}

impl<O: OutputFormat> fmt::Debug for IrTreeMut<'_, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <IrTreeRef<'_, _> as fmt::Debug>::fmt(&self.as_ref(), f)
    }
}

pub(crate) struct IrTreeRef<'a, O: OutputFormat = Markup> {
    pub(crate) node: NodeId,
    pub(crate) arena: &'a IrArena<O>,
}

impl<'a, O: OutputFormat> Clone for IrTreeRef<'a, O> {
    fn clone(&self) -> Self {
        let Self { node, arena } = *self;
        Self { node, arena }
    }
}
impl<'a, O: OutputFormat> Copy for IrTreeRef<'a, O> {}

pub(crate) struct IrTreeMut<'a, O: OutputFormat = Markup> {
    pub node: NodeId,
    pub arena: &'a mut IrArena<O>,
}

#[allow(dead_code)]
impl<'a, O: OutputFormat> IrTreeRef<'a, O> {
    pub(crate) fn children<'b>(&'b self) -> impl Iterator<Item = IrTreeRef<'b, O>> + 'b {
        let Self { node, arena } = self;
        node.children(arena)
            .map(move |child| IrTreeRef { node: child, arena })
    }
    pub(crate) fn reverse_children<'b>(&'b self) -> impl Iterator<Item = IrTreeRef<'b, O>> + 'b {
        let Self { node, arena } = self;
        node.reverse_children(arena)
            .map(move |child| IrTreeRef { node: child, arena })
    }
    pub(crate) fn get_node(&self) -> Option<&'a indextree::Node<(IR<O>, GroupVars)>> {
        self.arena.get(self.node)
    }
    pub(crate) fn get_node_at(
        &self,
        node: NodeId,
    ) -> Option<&'a indextree::Node<(IR<O>, GroupVars)>> {
        self.arena.get(node)
    }
}

impl<'a> core::ops::Deref for IrTreeMut<'a> {
    type Target = IrTreeRef<'a>;

    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}

#[allow(dead_code)]
impl<'a, O: OutputFormat> IrTreeMut<'a, O> {
    pub(crate) fn root_mut(&mut self) -> Option<&mut indextree::Node<(IR<O>, GroupVars)>> {
        self.arena.get_mut(self.node)
    }
    pub(crate) fn get_mut(
        &mut self,
        node: NodeId,
    ) -> Option<&mut indextree::Node<(IR<O>, GroupVars)>> {
        self.arena.get_mut(node)
    }
    pub(crate) fn get(&self, node: NodeId) -> Option<&indextree::Node<(IR<O>, GroupVars)>> {
        self.arena.get(node)
    }
}

impl<'a, O: OutputFormat> fmt::Display for IrTreeRef<'a, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn go<O2: OutputFormat>(
            indent: u32,
            node: NodeId,
            arena: &IrArena<O2>,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result {
            let pair = arena.get(node).unwrap().get();
            for _ in 0..indent {
                write!(f, "  ")?;
            }
            writeln!(f, " - [{:?}] {}", pair.1, pair.0)?;
            node.children(arena)
                .try_for_each(|ch| go(indent + 1, ch, arena, f))
        }
        write!(f, "\n")?;
        go(0, self.node, &self.arena, f)
    }
}

impl<'a, O: OutputFormat> fmt::Debug for IrTreeRef<'a, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn go<O2: OutputFormat>(
            indent: u32,
            node: NodeId,
            arena: &IrArena<O2>,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result {
            let pair = arena.get(node).unwrap().get();
            for _ in 0..indent {
                write!(f, "  ")?;
            }
            writeln!(f, " - [{:?}] {:?}", pair.1, pair.0)?;
            node.children(arena)
                .try_for_each(|ch| go(indent + 1, ch, arena, f))
        }
        write!(f, "\n")?;
        go(0, self.node, &self.arena, f)
    }
}

fn arena_copy_node<T: Clone>(
    src_node: NodeId,
    src_arena: &Arena<T>,
    dst_arena: &mut Arena<T>,
) -> Option<NodeId> {
    let node = src_arena.get(src_node)?;
    let new_node = dst_arena.new_node(node.get().clone());
    Some(new_node)
}

fn arena_copy_children<T: Clone>(
    src_node: NodeId,
    src_arena: &Arena<T>,
    dst_node: NodeId,
    dst_arena: &mut Arena<T>,
) {
    for src_child in src_node.children(src_arena) {
        let dst_child = arena_copy_node(src_child, src_arena, dst_arena)
            .expect("children are always valid node ids");
        dst_node.append(dst_child, dst_arena);
        arena_copy_children(src_child, src_arena, dst_child, dst_arena);
    }
}

fn arena_copy_tree<T: Clone>(
    src_root: NodeId,
    src_arena: &Arena<T>,
    dst_arena: &mut Arena<T>,
) -> Option<NodeId> {
    let dst_root = arena_copy_node(src_root, src_arena, dst_arena)?;
    arena_copy_children(src_root, src_arena, dst_root, dst_arena);
    Some(dst_root)
}

#[allow(dead_code)]
impl<'a, O: OutputFormat> IrTreeRef<'a, O> {
    pub(crate) fn list_year_suffix_hooks(&self) -> Vec<NodeId> {
        fn list_ysh_inner<O: OutputFormat>(tree: IrTreeRef<O>, vec: &mut Vec<NodeId>) {
            let me = match tree.arena.get(tree.node) {
                Some(x) => x.get(),
                None => return,
            };
            match &me.0 {
                IR::YearSuffix(..) => vec.push(tree.node),
                IR::NameCounter(_) | IR::Rendered(_) | IR::Name(_) => {}
                IR::ConditionalDisamb(_) | IR::Seq(_) | IR::Substitute => {
                    tree.children().for_each(|child| list_ysh_inner(child, vec));
                }
            }
        }
        let mut vec = Vec::new();
        list_ysh_inner(*self, &mut vec);
        vec
    }
}

#[allow(dead_code)]
impl<O: OutputFormat> IrTree<O> {
    pub(crate) fn mutable(&mut self) -> IrTreeMut<O> {
        IrTreeMut {
            node: self.root,
            arena: &mut self.arena,
        }
    }
    pub(crate) fn recompute_group_vars(&mut self) {
        self.mutable().recompute_group_vars()
    }
}

#[allow(dead_code)]
impl<'a, O: OutputFormat> IrTreeMut<'a, O> {
    pub(crate) fn tree_at_node(&self, node: NodeId) -> IrTreeRef<O> {
        IrTreeRef {
            node,
            arena: self.arena,
        }
    }
    pub(crate) fn as_ref(&self) -> IrTreeRef<O> {
        IrTreeRef {
            node: self.node,
            arena: self.arena,
        }
    }
    pub(crate) fn with_node<R>(
        &mut self,
        node: NodeId,
        f: impl FnOnce(&mut IrTreeMut<'_, O>) -> R,
    ) -> R {
        let my_node = core::mem::replace(&mut self.node, node);
        let res = f(self);
        self.node = my_node;
        res
    }
    pub(crate) fn recompute_group_vars(&mut self) {
        if self.root_mut().is_none() {
            return;
        }
        // todo: share this allocation over all nodes
        let mut queue = Vec::new();
        for node in self.node.descendants(&self.arena) {
            match &self.get(node).unwrap().get().0 {
                IR::Seq(seq) => {
                    queue.push((node, seq.dropped_gv));
                }
                _ => {}
            }
        }
        // Reverse, such that descendants are recalculated first
        for (seq_node, dropped_gv) in queue.into_iter().rev() {
            let seq_tree = self.tree_at_node(seq_node);
            let force = IrSeq::overall_group_vars(dropped_gv, seq_tree);
            let existing = self.arena.get(seq_node).unwrap().get().1;
            if existing != force {
                log::debug!("recompute rewriting gv to {:?} {}", force, seq_tree);
            }
            self.arena.get_mut(seq_node).unwrap().get_mut().1 = force;
        }
    }
}
