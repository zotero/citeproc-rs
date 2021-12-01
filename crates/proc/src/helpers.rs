// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::prelude::*;

use citeproc_io::output::markup::Markup;

pub fn sequence<'c, O, I>(
    db: &dyn IrDatabase,
    state: &mut IrState,
    ctx: &CiteContext<'c, O, I>,
    arena: &mut IrArena<O>,
    els: &[Element],
    implicit_conditional: bool,
    seq_template: Option<&dyn Fn() -> IrSeq>,
) -> NodeId
where
    O: OutputFormat,
    I: OutputFormat,
{
    let mut overall_gv = GroupVars::Plain;
    let mut dropped_gv = GroupVars::Plain;

    // We will edit this later if it turns out it has content & isn't discarded
    let self_node = arena.new_node((IR::Rendered(None), GroupVars::Plain));
    if els.is_empty() {
        return self_node;
    }

    for el in els {
        let child = el.intermediate(db, state, ctx, arena);
        let (ref ch_ir, ch_gv) = *arena.get(child).unwrap().get();
        // we either append the child node to our node, or add it to the "dropped_gv" which means
        // that some element that did not render, but was Missing, may cause a group to hide
        // itself.
        //
        // We will not throw away Unresolved* nodes, because they could change in future, so it is
        // crucial that they stay in the tree so that disambiguation routines can search for them
        // and modify those nodes to have content (or remove the content, etc).
        //
        // Empty child nodes must be thrown out, because "did not render anything" is a property used by
        // e.g. <substitute> to decide whether to render something else, etc. Not rendering is
        // determined by collapsing a seq with no child nodes at all into IR::Rendered(None)
        //
        // keep this in sync with same bit in implementation of `ref_sequence` below
        match ch_ir {
            IR::Rendered(None) if !ch_gv.is_unresolved() => {
                dropped_gv = dropped_gv.neighbour(ch_gv);
            }
            _ => {
                self_node.append(child, arena);
            }
        }
        overall_gv = overall_gv.neighbour(ch_gv)
    }

    let ir = if self_node.children(arena).next().is_none() {
        IR::Rendered(None)
    } else {
        let mut seq = IrSeq {
            dropped_gv: if implicit_conditional {
                Some(dropped_gv)
            } else {
                None
            },
            ..if let Some(tmpl) = seq_template {
                tmpl()
            } else {
                Default::default()
            }
        };
        if !ctx.in_bibliography {
            seq.display = None;
        }
        IR::Seq(seq)
    };

    let (set_ir, set_gv) = if implicit_conditional {
        let is_empty = self_node.children(arena).next().is_none();
        overall_gv.implicit_conditional(ir, is_empty)
    } else {
        (ir, overall_gv)
    };

    let (self_ir, self_gv) = arena.get_mut(self_node).unwrap().get_mut();
    *self_ir = set_ir;
    *self_gv = set_gv;
    self_node
}

pub fn ref_sequence<'c>(
    db: &dyn IrDatabase,
    state: &mut IrState,
    ctx: &RefContext<'c, Markup>,
    els: &[Element],
    implicit_conditional: bool,
    formatting: Option<Formatting>,
    seq_template: Option<&dyn Fn() -> RefIrSeq>,
) -> (RefIR, GroupVars) {
    let _fmt = &ctx.format;

    let mut contents = Vec::with_capacity(els.len());
    let mut overall_gv = GroupVars::new();
    let fmting = formatting.unwrap_or_default();

    for el in els {
        let (ch_ir, ch_gv) = el.ref_ir(db, ctx, state, fmting);
        // keep this in sync with same bit in implementation of `sequence` above
        match &ch_ir {
            RefIR::Edge(None) if !ch_gv.is_unresolved() => {
                // drop these, no need to keep dropped_gv as RefIR does not need to be mutated
                // later so storing overall_gv is sufficient to know if it needs to be output.
            }
            _ => {
                contents.push(ch_ir);
            }
        }
        overall_gv = overall_gv.neighbour(ch_gv);
    }

    if !contents.iter().any(|x| !matches!(x, RefIR::Edge(None))) {
        (RefIR::Edge(None), overall_gv)
    } else {
        let mut seq = if let Some(tmpl) = seq_template {
            tmpl()
        } else {
            Default::default()
        };
        seq.contents = contents;
        seq.formatting = formatting;
        if implicit_conditional {
            let is_empty = seq.contents.is_empty();
            overall_gv.implicit_conditional(RefIR::Seq(seq), is_empty)
        } else {
            (RefIR::Seq(seq), overall_gv)
        }
    }
}

use fnv::FnvHashSet;
pub fn fnv_set_with_cap<T: std::hash::Hash + std::cmp::Eq>(cap: usize) -> FnvHashSet<T> {
    FnvHashSet::with_capacity_and_hasher(cap, fnv::FnvBuildHasher::default())
}

use csl::{StandardVariable, TextCase, TextElement, TextSource, Variable, VariableForm};
pub fn plain_text_element(v: Variable) -> TextElement {
    TextElement {
        source: TextSource::Variable(StandardVariable::Ordinary(v), VariableForm::Long),
        formatting: None,
        affixes: None,
        quotes: false,
        strip_periods: false,
        text_case: TextCase::None,
        display: None,
    }
}

/// Unstable `#[feature(slice_group_by)]`: <https://github.com/rust-lang/rust/issues/80552>
///
/// Used under the MIT license from the implementation PR by Kerollmops
pub(crate) mod slice_group_by {
    use core::fmt;
    use core::iter::FusedIterator;
    use core::mem;

    /// An iterator over slice in (non-overlapping) chunks separated by a predicate.
    ///
    /// This struct is created by the [`group_by`] method on [slices].
    ///
    /// [`group_by`]: ../../std/primitive.slice.html#method.group_by
    /// [slices]: ../../std/primitive.slice.html
    // #[unstable(feature = "slice_group_by", issue = "80552")]
    pub struct GroupBy<'a, T: 'a, P> {
        slice: &'a [T],
        predicate: P,
    }

    // #[unstable(feature = "slice_group_by", issue = "80552")]
    impl<'a, T: 'a, P> GroupBy<'a, T, P> {
        pub(super) fn new(slice: &'a [T], predicate: P) -> Self {
            GroupBy { slice, predicate }
        }
    }

    // #[unstable(feature = "slice_group_by", issue = "80552")]
    impl<'a, T: 'a, P> Iterator for GroupBy<'a, T, P>
    where
        P: FnMut(&T, &T) -> bool,
    {
        type Item = &'a [T];

        #[inline]
        fn next(&mut self) -> Option<Self::Item> {
            if self.slice.is_empty() {
                None
            } else {
                let mut len = 1;
                let mut iter = self.slice.windows(2);
                while let Some([l, r]) = iter.next() {
                    if (self.predicate)(l, r) {
                        len += 1
                    } else {
                        break;
                    }
                }
                let (head, tail) = self.slice.split_at(len);
                self.slice = tail;
                Some(head)
            }
        }

        #[inline]
        fn size_hint(&self) -> (usize, Option<usize>) {
            if self.slice.is_empty() {
                (0, Some(0))
            } else {
                (1, Some(self.slice.len()))
            }
        }

        #[inline]
        fn last(mut self) -> Option<Self::Item> {
            self.next_back()
        }
    }

    // #[unstable(feature = "slice_group_by", issue = "80552")]
    impl<'a, T: 'a, P> DoubleEndedIterator for GroupBy<'a, T, P>
    where
        P: FnMut(&T, &T) -> bool,
    {
        #[inline]
        fn next_back(&mut self) -> Option<Self::Item> {
            if self.slice.is_empty() {
                None
            } else {
                let mut len = 1;
                let mut iter = self.slice.windows(2);
                while let Some([l, r]) = iter.next_back() {
                    if (self.predicate)(l, r) {
                        len += 1
                    } else {
                        break;
                    }
                }
                let (head, tail) = self.slice.split_at(self.slice.len() - len);
                self.slice = head;
                Some(tail)
            }
        }
    }

    // #[unstable(feature = "slice_group_by", issue = "80552")]
    impl<'a, T: 'a, P> FusedIterator for GroupBy<'a, T, P> where P: FnMut(&T, &T) -> bool {}

    // #[unstable(feature = "slice_group_by", issue = "80552")]
    impl<'a, T: 'a + fmt::Debug, P> fmt::Debug for GroupBy<'a, T, P> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("GroupBy")
                .field("slice", &self.slice)
                .finish()
        }
    }

    /// An iterator over slice in (non-overlapping) mutable chunks separated
    /// by a predicate.
    ///
    /// This struct is created by the [`group_by_mut`] method on [slices].
    ///
    /// [`group_by_mut`]: ../../std/primitive.slice.html#method.group_by_mut
    /// [slices]: ../../std/primitive.slice.html
    // #[unstable(feature = "slice_group_by", issue = "80552")]
    pub struct GroupByMut<'a, T: 'a, P> {
        slice: &'a mut [T],
        predicate: P,
    }

    // #[unstable(feature = "slice_group_by", issue = "80552")]
    impl<'a, T: 'a, P> GroupByMut<'a, T, P> {
        pub(super) fn new(slice: &'a mut [T], predicate: P) -> Self {
            GroupByMut { slice, predicate }
        }
    }

    // #[unstable(feature = "slice_group_by", issue = "80552")]
    impl<'a, T: 'a, P> Iterator for GroupByMut<'a, T, P>
    where
        P: FnMut(&T, &T) -> bool,
    {
        type Item = &'a mut [T];

        #[inline]
        fn next(&mut self) -> Option<Self::Item> {
            if self.slice.is_empty() {
                None
            } else {
                let mut len = 1;
                let mut iter = self.slice.windows(2);
                while let Some([l, r]) = iter.next() {
                    if (self.predicate)(l, r) {
                        len += 1
                    } else {
                        break;
                    }
                }
                let slice = mem::take(&mut self.slice);
                let (head, tail) = slice.split_at_mut(len);
                self.slice = tail;
                Some(head)
            }
        }

        #[inline]
        fn size_hint(&self) -> (usize, Option<usize>) {
            if self.slice.is_empty() {
                (0, Some(0))
            } else {
                (1, Some(self.slice.len()))
            }
        }

        #[inline]
        fn last(mut self) -> Option<Self::Item> {
            self.next_back()
        }
    }

    // #[unstable(feature = "slice_group_by", issue = "80552")]
    impl<'a, T: 'a, P> DoubleEndedIterator for GroupByMut<'a, T, P>
    where
        P: FnMut(&T, &T) -> bool,
    {
        #[inline]
        fn next_back(&mut self) -> Option<Self::Item> {
            if self.slice.is_empty() {
                None
            } else {
                let mut len = 1;
                let mut iter = self.slice.windows(2);
                while let Some([l, r]) = iter.next_back() {
                    if (self.predicate)(l, r) {
                        len += 1
                    } else {
                        break;
                    }
                }
                let slice = mem::take(&mut self.slice);
                let (head, tail) = slice.split_at_mut(slice.len() - len);
                self.slice = head;
                Some(tail)
            }
        }
    }

    // #[unstable(feature = "slice_group_by", issue = "80552")]
    impl<'a, T: 'a, P> FusedIterator for GroupByMut<'a, T, P> where P: FnMut(&T, &T) -> bool {}

    // #[unstable(feature = "slice_group_by", issue = "80552")]
    impl<'a, T: 'a + fmt::Debug, P> fmt::Debug for GroupByMut<'a, T, P> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("GroupByMut")
                .field("slice", &self.slice)
                .finish()
        }
    }

    /// Returns an iterator over the slice producing non-overlapping runs
    /// of elements using the predicate to separate them.
    ///
    /// The predicate is called on two elements following themselves,
    /// it means the predicate is called on `slice[0]` and `slice[1]`
    /// then on `slice[1]` and `slice[2]` and so on.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(slice_group_by)]
    ///
    /// let slice = &[1, 1, 1, 3, 3, 2, 2, 2];
    ///
    /// let mut iter = slice.group_by(|a, b| a == b);
    ///
    /// assert_eq!(iter.next(), Some(&[1, 1, 1][..]));
    /// assert_eq!(iter.next(), Some(&[3, 3][..]));
    /// assert_eq!(iter.next(), Some(&[2, 2, 2][..]));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// This method can be used to extract the sorted subslices:
    ///
    /// ```
    /// #![feature(slice_group_by)]
    ///
    /// let slice = &[1, 1, 2, 3, 2, 3, 2, 3, 4];
    ///
    /// let mut iter = slice.group_by(|a, b| a <= b);
    ///
    /// assert_eq!(iter.next(), Some(&[1, 1, 2, 3][..]));
    /// assert_eq!(iter.next(), Some(&[2, 3][..]));
    /// assert_eq!(iter.next(), Some(&[2, 3, 4][..]));
    /// assert_eq!(iter.next(), None);
    /// ```
    // #[unstable(feature = "slice_group_by", issue = "80552")]
    #[inline]
    pub fn group_by<T, F>(slice: &[T], pred: F) -> GroupBy<'_, T, F>
    where
        F: FnMut(&T, &T) -> bool,
    {
        GroupBy::new(slice, pred)
    }

    /// Returns an iterator over the slice producing non-overlapping mutable
    /// runs of elements using the predicate to separate them.
    ///
    /// The predicate is called on two elements following themselves,
    /// it means the predicate is called on `slice[0]` and `slice[1]`
    /// then on `slice[1]` and `slice[2]` and so on.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(slice_group_by)]
    ///
    /// let slice = &mut [1, 1, 1, 3, 3, 2, 2, 2];
    ///
    /// let mut iter = slice.group_by_mut(|a, b| a == b);
    ///
    /// assert_eq!(iter.next(), Some(&mut [1, 1, 1][..]));
    /// assert_eq!(iter.next(), Some(&mut [3, 3][..]));
    /// assert_eq!(iter.next(), Some(&mut [2, 2, 2][..]));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// This method can be used to extract the sorted subslices:
    ///
    /// ```
    /// #![feature(slice_group_by)]
    ///
    /// let slice = &mut [1, 1, 2, 3, 2, 3, 2, 3, 4];
    ///
    /// let mut iter = slice.group_by_mut(|a, b| a <= b);
    ///
    /// assert_eq!(iter.next(), Some(&mut [1, 1, 2, 3][..]));
    /// assert_eq!(iter.next(), Some(&mut [2, 3][..]));
    /// assert_eq!(iter.next(), Some(&mut [2, 3, 4][..]));
    /// assert_eq!(iter.next(), None);
    /// ```
    // #[unstable(feature = "slice_group_by", issue = "80552")]
    #[inline]
    pub fn group_by_mut<T, F>(slice: &mut [T], pred: F) -> GroupByMut<'_, T, F>
    where
        F: FnMut(&T, &T) -> bool,
    {
        GroupByMut::new(slice, pred)
    }
}
