// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::Atom;
mod pandoc;
mod html;
mod plain;
use std::marker::{Send, Sync};

pub use self::pandoc::Pandoc;
pub use self::plain::PlainText;
pub use self::html::Html;
// pub use self::markdown::Markdown;

use csl::style::{Affixes, Formatting};
use serde::{de::DeserializeOwned, Serialize};

#[derive(Debug, Clone)]
pub enum LocalizedQuotes {
    Single(Atom, Atom),
    Double(Atom, Atom),
    // Would this be better?
    // /// When the locale supplied single quotes that were just unicode curly quotes, you can use
    // /// optimized HTML/Pandoc objects that do the flip-flopping for you. Otherwise, flip flopping
    // /// is not supported.
    // SystemSingle,
    // // See SystemSingle
    // SystemDouble,
}

pub trait OutputFormat: Send + Sync + Clone + Default + std::fmt::Debug {
    type Build: std::fmt::Debug + DeserializeOwned + Serialize + Default + Clone + Send + Sync + Eq;
    type Output: Clone + Send + Sync + Eq + Serialize + DeserializeOwned;

    /// Affixes are not included in the formatting on a text node. They are converted into text
    /// nodes themselves, with no formatting except whatever is applied by a parent group.
    ///
    /// [Spec](https://docs.citationstyles.org/en/stable/specification.html#affixes)

    // TODO: make formatting an Option<Formatting>
    fn text_node(&self, s: String, formatting: Option<Formatting>) -> Self::Build;

    /// Group some text nodes. You might want to optimise for the case where delimiter is empty.
    fn group(
        &self,
        nodes: Vec<Self::Build>,
        delimiter: &str,
        formatting: Option<Formatting>,
    ) -> Self::Build;

    fn seq(&self, nodes: impl Iterator<Item = Self::Build>) -> Self::Build;

    fn join_delim(&self, a: Self::Build, delim: &str, b: Self::Build) -> Self::Build;

    fn output(&self, intermediate: Self::Build) -> Self::Output;

    fn plain(&self, s: &str) -> Self::Build;

    fn affixed_text_quoted(
        &self,
        s: String,
        format_inner: Option<Formatting>,
        affixes: &Affixes,
        quotes: Option<&LocalizedQuotes>,
    ) -> Self::Build {
        self.affixed_quoted(self.text_node(s, format_inner), affixes, quotes)
    }

    fn affixed_text(
        &self,
        s: String,
        format_inner: Option<Formatting>,
        affixes: &Affixes,
    ) -> Self::Build {
        self.affixed(self.text_node(s, format_inner), affixes)
    }

    fn quoted(&self, b: Self::Build, quotes: &LocalizedQuotes) -> Self::Build;

    #[inline]
    fn affixed(&self, b: Self::Build, affixes: &Affixes) -> Self::Build {
        self.affixed_quoted(b, affixes, None)
    }

    fn affixed_quoted(
        &self,
        b: Self::Build,
        affixes: &Affixes,
        quotes: Option<&LocalizedQuotes>,
    ) -> Self::Build {
        use std::iter::once;
        let pre = affixes.prefix.is_empty();
        let suf = affixes.suffix.is_empty();
        let b = if let Some(lq) = quotes {
            self.quoted(b, lq)
        } else {
            b
        };
        match (pre, suf) {
            (true, true) => b,

            (false, true) => self.seq(once(self.plain(&affixes.prefix)).chain(once(b))),

            (true, false) => self.seq(once(b).chain(once(self.plain(&affixes.suffix)))),

            (false, false) => self.seq(
                once(self.plain(&affixes.prefix))
                    .chain(once(b))
                    .chain(once(self.plain(&affixes.suffix))),
            ),
        }
    }

    fn with_format(&self, a: Self::Build, f: Option<Formatting>) -> Self::Build;

    fn hyperlinked(&self, a: Self::Build, target: Option<&str>) -> Self::Build;
}
