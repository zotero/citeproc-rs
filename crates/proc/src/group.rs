// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

// cs:group implicitly acts as a conditional: cs:group and its child elements are suppressed if a)
// at least one rendering element in cs:group calls a variable (either directly or via a macro),
// and b) all variables that are called are empty. This accommodates descriptive cs:text elements.
//
// Make a new one of these per <group> subtree.

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GroupVars {
    /// A group has only seen stuff like `<text value=""/>` so far
    Plain,

    /// Renderer encountered >= 1 variables, but did not render any of them
    Missing,

    /// Renderer encountered >= 1 variables that it did render
    Important,

    /// Initial value given to disambiguate="true" conditionals that are initially empty, as their
    /// content could go either way later, and their currently-empty output shouldn't affect
    /// whether the surrounding group should render before disambiguation comes around.
    UnresolvedImportant,

    /// For e.g. an explicit `<text variable="year-suffix" />`, which would otherwise cause a
    /// surrounding group to be Missing initially and be discarded too soon. Just means "don't
    /// render, but also don't throw it out yet."
    UnresolvedMissing,

    /// Not instantiated directly; computed value for groups containing only Plain and Unresolved
    /// children. A group with this overall GV should render as it is 'leaning towards Plain', but
    /// could still change later.
    ///
    /// "Do render this one, but don't rely on it being plain to discard an outer group."
    UnresolvedPlain,
}

#[test]
fn test_important_seq() {
    let f = |slice: &[GroupVars]| {
        slice
            .iter()
            .fold(GroupVars::Plain, |a, b| a.neighbour(*b))
            .promote_plain()
    };
    assert_eq!(f(&[Important, Missing]), Important);
    assert_eq!(f(&[UnresolvedImportant, Missing]), UnresolvedMissing);
    assert_eq!(f(&[UnresolvedImportant, Plain]), UnresolvedPlain);
    assert_eq!(f(&[UnresolvedImportant, Plain, Important]), Important);
    assert_eq!(f(&[UnresolvedImportant, Missing, Important]), Important);
    assert_eq!(f(&[Important, UnresolvedImportant, Missing]), Important);
    // plains in a group end up being important.
    assert_eq!(f(&[Plain, Plain, Plain]), Important);
    assert_eq!(f(&[UnresolvedImportant, Plain, Plain]), UnresolvedPlain);
    assert_eq!(f(&[UnresolvedMissing, Plain, Plain]), UnresolvedMissing);
}

impl Default for GroupVars {
    fn default() -> Self {
        GroupVars::new()
    }
}

use self::GroupVars::*;

impl GroupVars {
    #[inline]
    pub fn new() -> Self {
        Plain
    }

    #[inline]
    pub fn rendered_if(b: bool) -> Self {
        if b {
            GroupVars::Important
        } else {
            GroupVars::Missing
        }
    }

    // pub fn with_subtree(self, subtree: Self) -> Self {
    //     match subtree {
    //         Plain => self,
    //         Missing => self.did_not_render(),
    //         Important => Important,
    //     }
    // }

    /// Say you have
    ///
    /// ```xml
    /// <group>
    ///   <text value="tag" />
    ///   <text variable="var" />
    /// </group>
    /// ```
    ///
    /// The tag is `Plain`, the var has `Important`, so the group is `Important`.
    ///
    /// ```text
    /// assert_eq!(Plain.neighbour(Important), Important);
    /// assert_eq!(Plain.neighbour(Missing), Missing);
    /// assert_eq!(Important.neighbour(Missing), Important);
    /// ```
    pub fn neighbour(self, other: Self) -> Self {
        match (self, other) {
            // if either is Important, the parent group will be too. For sure. Don't need to track
            // Unresolved any further than this.
            (Important, _) | (_, Important) => Important,

            // Unresolved + Missing has to stay Unresolved until disambiguation is done
            (UnresolvedImportant, Missing)
            | (Missing, UnresolvedImportant)
            | (UnresolvedMissing, Missing)
            | (Missing, UnresolvedMissing)
            | (UnresolvedMissing, UnresolvedImportant)
            | (UnresolvedImportant, UnresolvedMissing)
            | (Plain, UnresolvedMissing)
            | (UnresolvedMissing, Plain)
            | (UnresolvedMissing, UnresolvedMissing)
            | (UnresolvedMissing, UnresolvedPlain)
            | (UnresolvedPlain, UnresolvedMissing)
            | (UnresolvedPlain, Missing)
            | (Missing, UnresolvedPlain) => UnresolvedMissing,

            (UnresolvedPlain, UnresolvedPlain)
            | (UnresolvedPlain, UnresolvedImportant)
            | (UnresolvedImportant, UnresolvedPlain)
            | (UnresolvedPlain, Plain)
            | (Plain, UnresolvedPlain)
            | (UnresolvedImportant, Plain)
            | (Plain, UnresolvedImportant) => UnresolvedPlain,

            // promote Missing over Plain; the style tried and failed to render a variable,
            // so we must take note of this.
            (Missing, Missing) | (Missing, Plain) | (Plain, Missing) => Missing,

            (Plain, Plain) => Plain,

            (UnresolvedImportant, UnresolvedImportant) => UnresolvedImportant,
        }
    }

    /// Resets the group vars so that G(Missing, G(Plain)) will
    /// render the Plain part. Groups shouldn't look inside inner
    /// groups to make themselves not render.
    ///
    /// https://discourse.citationstyles.org/t/groups-variables-and-missing-dates/1529/18
    #[inline]
    pub fn promote_plain(self) -> Self {
        match self {
            Plain | Important => Important,
            _ => self,
        }
    }

    #[inline]
    pub fn should_render_tree(self, is_implicit_conditional: bool) -> bool {
        match self {
            Missing | UnresolvedMissing if is_implicit_conditional => false,
            _ => true,
        }
    }

    #[inline]
    pub fn is_unresolved(self) -> bool {
        match self {
            UnresolvedMissing | UnresolvedPlain | UnresolvedImportant => true,
            _ => false,
        }
    }

    #[inline]
    pub fn implicit_conditional<T: Default>(self, seq_ir: T, is_empty: bool) -> (T, Self) {
        // self here is children_gvs.fold(Plain, neighbour).
        match self {
            // if it's missing, we replace any (clearly Plain-only) nodes we wrote into the seq,
            // with the default for the seq type.
            //
            // Note also that this will, for T = IR, give IR::Rendered(None).
            Missing => (T::default(), GroupVars::Missing),

            // If it's empty, throw it out.
            //
            // If it's Unresolved*, we keep it, but you shouldn't be running implicit_conditional
            // in the construction of an unresolved seq.
            // hook in the tree to maybe render something later.
            //
            // if it's empty (== default implies empty), then we treat the seq node as Plain for
            // the purposes of groups higher up.
            //
            // If we have Important but an empty seq, then we've made a mistake coding, because
            // Important should have some content in it. Fine to throw out.
            Plain | Important if is_empty => (T::default(), GroupVars::Plain),

            // otherwise, if it's Plain, make it Important. This means G(Missing, G(Plain)) will
            // render the Plain part.
            _ => (seq_ir, self.promote_plain()),
        }
    }

    /// We need a seq that ISN'T an implicit-conditional to be rendered, but also carry variable
    /// missing-ness information upwards. This has a very simple implementation, with respect to
    /// the group vars: change nothing, simply set the gv of such a seq to this value. Crucially,
    /// you also have to render (i.e. flatten, or add_to_graph) everything except
    /// `!gv.should_render_tree()` nodes.
    ///
    /// Basically here we are referring to if/else-if/else branches.
    ///
    /// ```xml,ignore
    /// <group>
    ///   <text value="PLAIN" />
    ///   <choose><if ...>
    ///     <text variable="MISSING" />
    ///   </if></choose>
    /// </group>
    /// ```
    ///
    /// The variable is Missing, the if-branch is (because of this method) Missing, the outer group
    /// is Missing + Plain = Missing, so nothing renders.
    #[inline]
    pub fn unconditional(self) -> Self {
        self
    }

    /// Changes Unresolved* variants into their normal counterparts.
    #[inline]
    pub fn resolve(self) -> Self {
        match self {
            UnresolvedImportant => Important,
            UnresolvedPlain => Plain,
            UnresolvedMissing => Missing,
            _ => self,
        }
    }
}
