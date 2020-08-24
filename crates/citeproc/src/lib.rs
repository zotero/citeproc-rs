// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

#[macro_use]
extern crate serde_derive;
// #[macro_use]
// extern crate log;

pub(crate) mod processor;
pub(crate) mod api;

#[cfg(test)]
mod test;

pub use self::api::{DocUpdate, UpdateSummary, IncludeUncited, SupportedFormat};
pub use self::processor::{ErrorKind, Processor};

pub mod prelude {
    pub use crate::api::{DocUpdate, UpdateSummary, IncludeUncited, SupportedFormat};
    pub use crate::processor::Processor;
    pub use citeproc_db::{
        CiteDatabase, CiteId, LocaleDatabase, LocaleFetchError, LocaleFetcher, StyleDatabase,
    };
    pub use citeproc_io::output::{markup::Markup, OutputFormat};
    pub use citeproc_io::{Cite, Cluster, ClusterId, ClusterNumber, IntraNote, Reference, ClusterPosition};
    pub use citeproc_proc::db::{HasFormatter, IrDatabase};
    pub use csl::Atom;
}
