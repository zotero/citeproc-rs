mod driver;
mod db;
mod db_impl;
mod utils;
pub use self::driver::Driver;
pub mod input;
pub mod output;
pub mod style;
pub mod locale;
pub use self::style::error::StyleError;
pub mod proc;

#[macro_use]
extern crate strum_macros;
#[macro_use]
extern crate serde_derive;
// #[macro_use]
// extern crate failure;

pub(crate) use string_cache::DefaultAtom as Atom;
