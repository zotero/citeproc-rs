// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

//! This module describes non-CSL inputs to the processor. Specifically, it aims to implement
//! [CSL-JSON](https://citeproc-js.readthedocs.io/en/latest/csl-json/markup.html)
//! ([schema](https://github.com/citation-style-language/schema/blob/master/csl-data.json)).
//!
//! These are constructed at deserialization time, i.e. when a [Reference][]
//! is read from CSL-JSON. Any data types that contain strings from the original JSON are typically
//! done with `&'r str` borrows under the [Reference][]'s lifetime `'r`, so the original JSON string
//! cannot be thrown away, but a large number of string allocations is saved.
//!
//! [Reference]: struct.Reference.html

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate log;

mod cite;
mod cluster;
mod csl_json;
mod date;
mod names;
pub use names::TrimInPlace;
mod numeric;
pub mod output;
mod reference;
pub mod unicode;
pub mod utils;

pub use csl_json::NumberLike;
pub use output::micro_html::micro_html_to_string;

#[doc(inline)]
pub use self::cite::*;
#[doc(inline)]
pub use self::cluster::*;
#[doc(inline)]
pub use self::date::*;
#[doc(inline)]
pub use self::names::*;
#[doc(inline)]
pub use self::numeric::*;
#[doc(inline)]
pub use self::reference::*;

use self::output::LocalizedQuotes;
use csl::TextCase;

// Export these, because proc is going to need them
// type Sixteen = smallstr::SmallString<[u8; 16]>;
// type TwentyFour = smallstr::SmallString<[u8; 24]>;

pub type SmartString = smartstring::alias::String;
pub(crate) type String = smartstring::alias::String;
pub type SmartCow<'a> = cervine::Cow<'a, String, str>;

#[derive(Debug, Clone, Default)]
pub struct IngestOptions {
    pub replace_hyphens: bool,
    pub text_case: TextCase,
    pub quotes: LocalizedQuotes,
    pub strip_periods: bool,
    pub is_english: bool,

    /// For `flipflop_LeadingMarkupWithApostrophe.txt`
    ///
    /// Set this for an affix on a whole cluster. You don't want to mess with people's quoting just
    /// because they put it in a cluster prefix. They know their style better than we do. For
    /// fields on a reference it's very different.
    pub is_external: bool,

    /// For names
    pub no_parse_quotes: bool,
    /// For affixes in a csl style, etc. No HTML parsing, but does parse super/subscript.
    pub is_attribute: bool,
}

impl IngestOptions {
    pub(crate) fn for_affixes() -> Self {
        IngestOptions {
            is_attribute: true,
            ..Default::default()
        }
    }
}

pub mod lazy;
mod text_case;
