// kebab-case here is the same as Strum's "kebab_case",
// but with a more accurate name
#[derive(Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct PersonName {
    pub family: Option<String>,
    pub given: Option<String>,
    pub non_dropping_particle: Option<String>,
    pub dropping_particle: Option<String>,
    pub suffix: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Clone)]
#[serde(untagged, rename_all = "kebab-case")]
pub enum Name {
    Person(PersonName),
    /// In CSL-M, this will represent an institution
    Literal {
        literal: String,
    },
}
