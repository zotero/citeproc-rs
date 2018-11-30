mod formatter;
pub use self::formatter::Format;

pub mod pandoc;

pub mod markdown;
pub mod plain;

#[cfg(test)]
mod test {

    use crate::style::element::Formatting;

    use super::Format;
    // #[cfg(feature = "pandoc")]
    // use super::pandoc::PandocFormat;
    // use super::markdown::MarkdownFormat;
    use super::plain::PlainTextFormat;

    // #[test]
    // fn markdown() {
    //     let f = MarkdownFormat::new();
    //     let o = f.text_node("hi", &Formatting::italic());
    //     let o2 = f.text_node("mom", &Formatting::bold());
    //     let o3 = f.group(&[o, o2], " ", &Formatting::italic());
    //     let serialized = serde_json::to_string(&o3).unwrap();
    //     assert_eq!(serialized, "\"_hi **mom**_\"");
    // }

    #[test]
    fn plain() {
        let f = PlainTextFormat::new();
        let o = f.text_node("hi", &Formatting::italic());
        let o2 = f.text_node("mom", &Formatting::default());
        let o3 = f.group(&[o, o2], " ", &Formatting::italic());
        let serialized = serde_json::to_string(&o3).unwrap();
        assert_eq!(serialized, "\"hi mom\"");
    }

}
