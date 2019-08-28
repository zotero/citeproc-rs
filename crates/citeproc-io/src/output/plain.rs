// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::{LocalizedQuotes, OutputFormat};
use crate::IngestOptions;

use csl::style::Formatting;

#[derive(Debug, Clone)]
pub struct PlainText {}

impl Default for PlainText {
    fn default() -> Self {
        PlainText {}
    }
}

impl OutputFormat for PlainText {
    type Input = String;
    type Build = String;
    type Output = String;

    #[inline]
    fn ingest(&self, s: &str, o: IngestOptions) -> Self::Build {
        if o.replace_hyphens {
            s.replace('-', "\u{2013}")
        } else {
            s.to_string()
        }
    }

    #[inline]
    fn plain(&self, s: &str) -> Self::Build {
        s.to_owned()
    }

    #[inline]
    fn text_node(&self, s: String, _: Option<Formatting>) -> Self::Build {
        s
    }

    fn join_delim(&self, mut a: Self::Build, delim: &str, b: Self::Build) -> Self::Build {
        a.push_str(&delim);
        a.push_str(&b);
        a
    }

    fn seq(&self, mut nodes: impl Iterator<Item = Self::Build>) -> Self::Build {
        if let Some(first) = nodes.next() {
            nodes.fold(first, |mut a, b| {
                a.push_str(&b);
                a
            })
        } else {
            String::new()
        }
    }

    fn group(
        &self,
        nodes: Vec<Self::Build>,
        delimiter: &str,
        _f: Option<Formatting>,
    ) -> Self::Build {
        nodes.join(delimiter)
    }

    fn quoted(&self, b: Self::Build, quotes: &LocalizedQuotes) -> Self::Build {
        match quotes {
            LocalizedQuotes::Single(ref open, ref close)
            | LocalizedQuotes::Double(ref open, ref close) => open.to_string() + &b + &close,
        }
    }

    #[inline]
    fn with_format(&self, a: Self::Build, _f: Option<Formatting>) -> Self::Build {
        a
    }

    #[inline]
    fn hyperlinked(&self, a: Self::Build, _target: Option<&str>) -> Self::Build {
        a
    }

    #[inline]
    fn output(&self, intermediate: Self::Build) -> Self::Output {
        intermediate
    }
}
