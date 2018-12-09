// cs:group implicitly acts as a conditional: cs:group and its child elements are suppressed if a)
// at least one rendering element in cs:group calls a variable (either directly or via a macro),
// and b) all variables that are called are empty. This accommodates descriptive cs:text elements.
//
// Make a new one of these per <group> subtree.

#[derive(Debug, Copy, Clone)]
pub enum GroupVars {
    /// A group has only seen stuff like `<text value=""/>` so far
    NoneSeen,
    /// Renderer encountered >= 1 variables, but did not render any of them
    OnlyEmpty,
    /// Renderer encountered >= 1 variables that it did render
    DidRender,
}

use self::GroupVars::*;

impl GroupVars {
    #[inline]
    pub fn new() -> Self {
        NoneSeen
    }

    #[inline]
    pub fn did_not_render(self) -> Self {
        match self {
            DidRender => DidRender,
            _ => OnlyEmpty
        }
    }

    #[inline]
    pub fn did_render(self) -> Self {
        DidRender
    }

    pub fn with_subtree(self, subtree: Self) -> Self {
        match subtree {
            NonSeen => self,
            OnlyEmpty => self.did_not_render(),
            DidRender => self.did_render(),
        }
    }

    #[inline]
    pub fn should_render_tree(&self) -> bool {
        *self != OnlyEmpty
    }
}
