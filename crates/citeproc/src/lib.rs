// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

#[macro_use]
extern crate serde_derive;
// #[macro_use]
// extern crate log;

pub(crate) mod db;
pub use self::db::update::{DocUpdate, UpdateSummary};
pub use self::db::Processor;

pub mod prelude {
    pub use crate::db::update::{DocUpdate, UpdateSummary};
    pub use crate::db::Processor;
    pub use citeproc_db::{
        CiteDatabase, CiteId, LocaleDatabase, LocaleFetchError, LocaleFetcher, StyleDatabase,
    };
    pub use citeproc_io::output::{html::Html, OutputFormat};
    pub use citeproc_io::{Cite, Cluster2, ClusterId, ClusterNumber, IntraNote, Reference};
    pub use citeproc_proc::db::IrDatabase;
    pub use csl::Atom;
}
