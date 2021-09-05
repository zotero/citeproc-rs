// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

// #[macro_use]
// extern crate log;

pub(crate) mod api;
pub(crate) mod processor;

#[cfg(test)]
mod test;

pub use self::api::*;

pub use self::processor::{InitOptions, Processor};

pub mod prelude {
    pub use crate::api::*;
    pub use crate::processor::{InitOptions, Processor};
    pub use citeproc_db::PredefinedLocales;
    pub use citeproc_db::{
        CiteDatabase, CiteId, ClusterNumber, IntraNote, LocaleDatabase, LocaleFetchError,
        LocaleFetcher, StyleDatabase,
    };
    pub use citeproc_io::output::{markup::Markup, OutputFormat};
    pub use citeproc_io::{Cite, Reference, SmartString};
    pub use citeproc_proc::db::{ImplementationDetails, IrDatabase};
    pub use csl::Atom;

    pub use citeproc_db::ClusterId;
    pub use citeproc_io::{CiteMode, ClusterMode, Locator, Locators, NumberLike};
}

#[doc(no_inline)]
pub use citeproc_io as io;
#[doc(no_inline)]
pub use csl;

pub fn random_cluster_id() -> citeproc_io::SmartString {
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};
    let prefix = "cluster-";
    let mut string = citeproc_io::SmartString::from(prefix);
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(23 - prefix.len())
        .for_each(|ch| string.push(ch));
    string
}
