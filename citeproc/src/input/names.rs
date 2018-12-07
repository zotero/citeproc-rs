// kebab-case here is the same as Strum's "kebab_case",
// but with a more accurate name
#[derive(Serialize, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "kebab-case")]
pub struct PersonName<'r> {
    pub family: Option<&'r str>,
    pub given: Option<&'r str>,
    pub non_dropping_particle: Option<&'r str>,
    pub dropping_particle: Option<&'r str>,
    pub suffix: Option<&'r str>,
}

#[derive(Eq, PartialEq, Hash)]
pub enum Name<'r> {
    Person(PersonName<'r>),
    // In CSL-M, this will represent an institution
    Literal(&'r str),
}
