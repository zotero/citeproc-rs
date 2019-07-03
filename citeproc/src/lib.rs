// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

pub(crate) mod db;
pub use self::db::{LocaleFetcher, LocaleFetchError};
pub use self::db::Processor;
pub use self::db::update::{DocUpdate, UpdateSummary};
pub mod input;
pub mod output;
mod utils;
pub use csl::error::StyleError;
mod proc;

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;

pub use csl::Atom;
