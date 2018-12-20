use std::borrow::Cow;

// kebab-case here is the same as Strum's "kebab_case",
// but with a more accurate name
#[derive(Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PersonName<'r> {
    pub family: Option<Cow<'r, str>>,
    pub given: Option<Cow<'r, str>>,
    pub non_dropping_particle: Option<Cow<'r, str>>,
    pub dropping_particle: Option<Cow<'r, str>>,
    pub suffix: Option<Cow<'r, str>>,
}

#[derive(Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(untagged, rename_all = "kebab-case")]
pub enum Name<'r> {
    Person(PersonName<'r>),
    // In CSL-M, this will represent an institution
    Literal { literal: Cow<'r, str> },
}
