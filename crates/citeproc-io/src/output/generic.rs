// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::{LocalizedQuotes, OutputFormat};
use crate::utils::JoinMany;
use csl::style::Formatting;

// use std::sync::Arc;
// use std::any::Any;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Str(String),
    Fmt(Formatting, Vec<Node>),
    Link(String, Vec<Node>),
    Quoted(LocalizedQuotes, Vec<Node>),
    // Dynamic(Arc<dyn Any + Send + Sync>),
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct MicroHtml(pub String);

use Node::*;

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
pub struct GenericFormat;

impl OutputFormat for GenericFormat {
    type Input = MicroHtml;
    type Build = Vec<Node>;
    type Output = String;

    #[inline]
    fn ingest(&self, _micro_html: Self::Input) -> Self::Build {
        vec![]
    }

    #[inline]
    fn output(&self, _inter: Self::Build) -> Self::Output {
        "".to_owned()
    }

    #[inline]
    fn text_node(&self, text: String, f: Option<Formatting>) -> Self::Build {
        self.with_format(vec![Str(text)], f)
    }

    #[inline]
    fn plain(&self, text: &str) -> Self::Build {
        self.text_node(text.to_owned(), None)
    }

    #[inline]
    fn seq(&self, nodes: impl Iterator<Item = Self::Build>) -> Self::Build {
        itertools::concat(nodes)
    }

    #[inline]
    fn join_delim(&self, a: Self::Build, delim: &str, b: Self::Build) -> Self::Build {
        [a, b].join_many(&self.plain(delim))
    }

    fn group(
        &self,
        nodes: Vec<Self::Build>,
        delimiter: &str,
        formatting: Option<Formatting>,
    ) -> Self::Build {
        // TODO: see join_many
        if nodes.len() == 1 {
            if let Some(f) = formatting {
                vec![Fmt(f, nodes.into_iter().nth(0).unwrap())]
            } else {
                nodes.into_iter().nth(0).unwrap()
            }
        } else {
            let delim = self.plain(delimiter);
            let joined = nodes.join_many(&delim);
            if let Some(f) = formatting {
                vec![Fmt(f, joined)]
            } else {
                joined
            }
        }
    }

    #[inline]
    fn with_format(&self, a: Self::Build, f: Option<Formatting>) -> Self::Build {
        if let Some(f) = f {
            vec![Fmt(f, a)]
        } else {
            a
        }
    }

    #[inline]
    fn quoted(&self, b: Self::Build, quotes: &LocalizedQuotes) -> Self::Build {
        vec![Quoted(quotes.clone(), b)]
    }

    #[inline]
    fn hyperlinked(&self, a: Self::Build, target: Option<&str>) -> Self::Build {
        // TODO: allow internal linking using the Attr parameter (e.g.
        // first-reference-note-number)
        if let Some(target) = target {
            vec![Link(target.into(), a)]
        } else {
            a
        }
    }
}
