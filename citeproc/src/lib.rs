// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

pub(crate) mod db;
pub use self::db::LocaleFetcher;
pub use self::db::Processor;
// mod driver;
// pub use self::driver::Driver;
pub mod input;
pub mod output;
mod utils;
pub use csl::error::StyleError;
mod proc;

#[macro_use]
extern crate serde_derive;

pub use csl::Atom;
