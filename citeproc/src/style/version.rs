use lazy_static::lazy_static;
use semver::{Version, VersionReq};
use strum::EnumProperty;

lazy_static! {
    pub static ref COMPILED_VERSION: Version = { Version::parse("1.0.1").unwrap() };
    pub static ref COMPILED_VERSION_M: Version = { Version::parse("1.1.0").unwrap() };
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CslVersionReq(pub CslVariant, pub VersionReq);

#[derive(AsRefStr, EnumString, EnumProperty, Debug, PartialEq, Eq, Copy, Clone)]
pub enum CslVariant {
    // these strums are for reading from the <style> element
    #[strum(serialize = "csl")]
    Csl,
    #[strum(serialize = "csl-m")]
    CslM,
}

impl Default for CslVariant {
    fn default() -> Self {
        CslVariant::Csl
    }
}

impl CslVariant {
    pub fn filter_arg<T: EnumProperty>(&self, val: T) -> Option<T> {
        let version = match *self {
            CslVariant::Csl => "csl",
            CslVariant::CslM => "cslM",
        };
        if let Some("0") = val.get_str(version) {
            return None;
        }
        Some(val)
    }
}
