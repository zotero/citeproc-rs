pub(crate) mod db;
mod fetcher;
pub use self::fetcher::LocaleFetcher;

#[cfg(test)]
mod test;

#[cfg(test)]
pub use self::fetcher::Predefined;

