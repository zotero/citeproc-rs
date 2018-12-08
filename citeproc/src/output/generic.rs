#[derive(Serialize, Deserialize, Debug)]
pub enum OutputNode {
    Fmt(FormatNode),
    Str(String),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FormatNode {
    formatting: Formatting,
    children: Vec<OutputNode>,
}


