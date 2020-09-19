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
    Unresolved,

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
            (Unresolved, Missing)
            | (Missing, Unresolved)
            | (UnresolvedMissing, Missing)
            | (Missing, UnresolvedMissing)
            | (UnresolvedMissing, Unresolved)
            | (Unresolved, UnresolvedMissing)
            | (Plain, UnresolvedMissing)
            | (UnresolvedMissing, Plain)
            | (UnresolvedMissing, UnresolvedMissing)
            | (UnresolvedMissing, UnresolvedPlain)
            | (UnresolvedPlain, UnresolvedMissing)
            | (UnresolvedPlain, Missing)
            | (Missing, UnresolvedPlain) => UnresolvedMissing,

            (UnresolvedPlain, UnresolvedPlain)
            | (UnresolvedPlain, Unresolved)
            | (Unresolved, UnresolvedPlain)
            | (UnresolvedPlain, Plain)
            | (Plain, UnresolvedPlain)
            | (Unresolved, Plain)
            | (Plain, Unresolved) => UnresolvedPlain,

            // promote Missing over Plain; the style tried and failed to render a variable,
            // so we must take note of this.
            (Missing, Missing) | (Missing, Plain) | (Plain, Missing) => Missing,

            (Plain, Plain) => Plain,

            (Unresolved, Unresolved) => Unresolved,
        }
    }

    #[inline]
    pub fn should_render_tree(self) -> bool {
        self != Missing && self != UnresolvedMissing && self != Unresolved
    }

    #[inline]
    pub fn implicit_conditional<T: Default + PartialEq + std::fmt::Debug>(
        self,
        ir: T,
    ) -> (T, Self) {
        let default = T::default();
        if self == Missing {
            (default, GroupVars::Missing)
        } else if ir == default {
            (default, GroupVars::Plain)
        } else {
            // "reset" the group vars so that G(Plain, G(Missing)) will
            // render the Plain part. Groups shouldn't look inside inner
            // groups.
            //
            // https://discourse.citationstyles.org/t/groups-variables-and-missing-dates/1529/18
            (ir, if self == Plain { Important } else { self })
        }
    }
}
