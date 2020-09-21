// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

#[macro_use]
extern crate serde_derive;
// #[macro_use]
// extern crate log;

pub(crate) mod api;
pub(crate) mod processor;

#[cfg(test)]
mod test;

pub use self::api::{DocUpdate, FullRender, IncludeUncited, SupportedFormat, UpdateSummary};
pub use self::processor::{ErrorKind, PreviewPosition, Processor};

pub mod prelude {
    pub use crate::api::{DocUpdate, FullRender, IncludeUncited, SupportedFormat, UpdateSummary};
    pub use crate::processor::{PreviewPosition, Processor};
    pub use citeproc_db::{
        CiteDatabase, CiteId, LocaleDatabase, LocaleFetchError, LocaleFetcher, StyleDatabase,
    };
    pub use citeproc_io::output::{markup::Markup, OutputFormat};
    pub use citeproc_io::{
        Cite, Cluster, ClusterId, ClusterNumber, ClusterPosition, IntraNote, Reference,
    };
    pub use citeproc_proc::db::{HasFormatter, IrDatabase};
    pub use csl::Atom;
}
