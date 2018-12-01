use strum::EnumProperty;

#[derive(AsRefStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all = "snake_case")]
pub enum CslVersion {
    // these strums are for reading from the
    // <style> element
    #[strum(serialize = "1.0")]
    Csl101,
    #[strum(serialize = "1.1mlz1")]
    CslM,
}

impl CslVersion {
    pub fn filter_arg<T: EnumProperty>(&self, val: T) -> Option<T> {
        let version = match *self {
            CslVersion::Csl101 => "csl101",
            CslVersion::CslM => "cslM"
        };
        if let Some("0") = val.get_str(version) {
            return None;
        }
        Some(val)
    }
}

