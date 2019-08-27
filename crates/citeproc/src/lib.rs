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
    pub use csl::Atom;
    pub use citeproc_db::{CiteDatabase, StyleDatabase, LocaleFetcher, LocaleFetchError, LocaleDatabase};
    pub use citeproc_proc::IrDatabase;
    pub use citeproc_io::{Reference, Cite, Cluster, CiteId, ClusterId};
    pub use crate::db::Processor;
    pub use crate::db::update::{DocUpdate, UpdateSummary};
    pub use citeproc_io::output::{OutputFormat, html::Html};
}
