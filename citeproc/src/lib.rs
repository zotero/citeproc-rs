#![feature(test)]

// #[macro_use]
extern crate failure;

mod utils;
mod driver;
pub use self::driver::Driver;
pub mod input;
pub mod output;
pub mod style;
pub use self::style::error::StyleError;
pub mod proc;

#[macro_use]
extern crate strum_macros;
#[macro_use]
extern crate serde_derive;
#[cfg(test)]
extern crate test;

