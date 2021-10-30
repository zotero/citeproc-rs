// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::{FormatOptions, InlineElement, MarkupWriter, MaybeTrimStart};
use crate::output::micro_html::MicroNode;
use crate::output::FormatCmd;
use crate::String;
use core::fmt::{self, Write};
use csl::Formatting;

#[derive(Debug)]
pub struct RtfWriter<'a> {
    dest: &'a mut String,
    options: FormatOptions,
}

impl<'a> RtfWriter<'a> {
    pub fn new(dest: &'a mut String, options: FormatOptions) -> Self {
        RtfWriter { dest, options }
    }
}

impl<'a> MarkupWriter for RtfWriter<'a> {
    fn buf(&mut self) -> &mut String {
        self.dest
    }

    fn write_escaped(&mut self, text: &str) {
        write!(self.dest, "{}", rtf_escape(text)).unwrap()
    }

    fn write_url(&mut self, url: &url::Url, trailing_slash: bool, in_attr: bool) {
        super::write_url(
            self.dest,
            url,
            trailing_slash,
            in_attr,
            |b, s| write!(b, "{}", rtf_escape_url_in_attr(s)),
            |b, s| write!(b, "{}", rtf_escape(s)),
        )
        .unwrap();
    }

    fn stack_preorder(&mut self, stack: &[FormatCmd]) {
        for cmd in stack.iter() {
            let tag = cmd.rtf_tag();
            self.dest.push('{');
            self.dest.push_str(tag);
        }
    }

    fn stack_postorder(&mut self, stack: &[FormatCmd]) {
        for cmd in stack.iter() {
            if *cmd == FormatCmd::DisplayRightInline {
                let tlen = self.dest.trim_end_matches(' ').len();
                self.dest.truncate(tlen);
            }
            self.dest.push('}');
        }
    }

    fn write_micro(&mut self, micro: &MicroNode, trim_start: bool) {
        use MicroNode::*;
        match micro {
            Text(text) => {
                self.write_escaped(text.trim_start_if(trim_start));
            }
            Quoted {
                is_inner,
                localized,
                children,
            } => {
                self.write_escaped(localized.opening(*is_inner).trim_start_if(trim_start));
                self.write_micros(children, false);
                self.write_escaped(localized.closing(*is_inner));
            }
            Formatted(nodes, cmd) => {
                let tag = cmd.rtf_tag();
                self.dest.push('{');
                self.dest.push_str(tag);
                self.write_micros(nodes, trim_start);
                self.dest.push('}');
            }
            NoCase(inners) => {
                self.write_micros(inners, trim_start);
            }
            NoDecor(inners) => {
                self.write_micros(inners, trim_start);
            }
        }
    }

    fn write_inline(&mut self, inline: &InlineElement, trim_start: bool) {
        use super::InlineElement::*;
        match inline {
            Text(text) => {
                let trimmed = text.trim_start_if(trim_start);
                write!(self.dest, "{}", rtf_escape(trimmed)).unwrap()
            }
            Div(display, inlines) => {
                self.stack_formats(inlines, Formatting::default(), Some(*display))
            }
            Micro(micros) => {
                self.write_micros(micros, trim_start);
            }
            Formatted(inlines, formatting) => {
                self.stack_formats(inlines, *formatting, None);
            }
            Quoted {
                is_inner,
                localized,
                inlines,
            } => {
                self.write_escaped(localized.opening(*is_inner).trim_start_if(trim_start));
                self.write_inlines(inlines, false);
                self.write_escaped(localized.closing(*is_inner));
            }
            Linked(link) => {
                self.write_link(
                    r#"{\field{\*\fldinst{HYPERLINK ""#,
                    link,
                    r#""}}{\fldrslt "#,
                    "}}",
                    self.options,
                );
            }
        }
    }
}

impl FormatCmd {
    fn rtf_tag(self) -> &'static str {
        use super::FormatCmd::*;
        match self {
            // TODO: RTF display commands
            DisplayBlock => "",
            DisplayIndent => "",
            DisplayLeftMargin => "",
            DisplayRightInline => "",

            FontStyleItalic => "\\i ",
            FontStyleOblique => "\\i ",
            FontStyleNormal => "\\i0 ",

            FontWeightBold => "\\b ",
            FontWeightNormal => "\\b0 ",

            // Not supported?
            FontWeightLight => "\\b0 ",

            FontVariantSmallCaps => "\\scaps ",
            FontVariantNormal => "\\scaps0 ",

            TextDecorationUnderline => "\\ul ",
            TextDecorationNone => "\\ul0 ",

            VerticalAlignmentSuperscript => "\\super ",
            VerticalAlignmentSubscript => "\\sub ",
            VerticalAlignmentBaseline => "\\nosupersub ",
        }
    }
}

use nom::{bytes::complete as nbc, character::complete::anychar, IResult, Parser};

enum Encodable<'a> {
    Chunk(&'a str),
    Esc(&'static str),
    Unicode(char),
}

/// Try to gobble up as many non-escaping characters as possible.
fn scan_encodable<'a>(remain: &'a str) -> IResult<&'a str, Encodable<'a>> {
    nbc::take_till1(|x| match x {
        '\\' | '{' | '}' | '\n' | '\t' => true,
        '\x20'..='\x7e' => false,
        _ => true,
    })
    .map(Encodable::Chunk)
    .or(nbc::tag("\\").map(|_| Encodable::Esc("\\\\")))
    .or(nbc::tag("{").map(|_| Encodable::Esc("\\{")))
    .or(nbc::tag("}").map(|_| Encodable::Esc("\\}")))
    .or(nbc::tag("\t").map(|_| Encodable::Esc("\\tab ")))
    .or(nbc::tag("\n").map(|_| Encodable::Esc("\\line ")))
    .or(anychar.map(Encodable::Unicode))
    .parse(remain)
}

struct RtfEscaper<'a>(&'a str);

impl fmt::Display for RtfEscaper<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut utf16_buffer = [0; 2];
        let mut remain = self.0;
        while let Ok((rest, chunk)) = scan_encodable(remain) {
            remain = rest;
            match chunk {
                Encodable::Chunk(s) => f.write_str(s)?,
                Encodable::Esc(s) => f.write_str(s)?,
                Encodable::Unicode(c) => {
                    let slice = c.encode_utf16(&mut utf16_buffer);
                    for &u16c in slice.iter() {
                        // The spec says 'most control words' accept signed 16-bit, but Word and
                        // TextEdit both produce unsigned 16-bit, and even convert signed to unsigned
                        // when saving. So we'll do that here. (citeproc-js does this too.)
                        //
                        // Terminates the \uN keyword with a space, where citeproc-js uses \uN{}
                        write!(f, "\\uc0\\u{} ", u16c)?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn rtf_escape(s: &str) -> RtfEscaper {
    RtfEscaper(s)
}

use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};

/// The standard URL set is here: https://url.spec.whatwg.org/#fragment-percent-encode-set
/// But for RTF, you apparently have to be much more aggressive.
const PERCENT_ENCODABLE: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'\'')
    .add(b'%')
    .add(b'\\')
    .add(b'<')
    .add(b'>')
    .add(b'[')
    .add(b']')
    .add(b'\\')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');
const RE_ENCODE_URL: &AsciiSet = &PERCENT_ENCODABLE.remove(b'%');

struct RtfUrlEscaper<'a>(&'a str);

impl fmt::Display for RtfUrlEscaper<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        utf8_percent_encode(self.0, RE_ENCODE_URL).fmt(f)
    }
}

fn rtf_escape_url_in_attr(s: &str) -> RtfUrlEscaper {
    RtfUrlEscaper(s)
}

#[cfg(test)]
mod test {
    use super::*;

    fn rtf_escape(s: &str) -> String {
        let mut buf = String::new();
        write!(&mut buf, "{}", super::rtf_escape(s)).unwrap();
        buf
    }

    macro_rules! assert_eq {
        ($a:expr, $b:expr) => {
            ::pretty_assertions::assert_eq!(PrettyString($a), PrettyString($b))
        };
    }

    #[test]
    fn test_rtf_escape_unicode() {
        let tab = "Hello \t";
        assert_eq!(&rtf_escape(tab), r"Hello \tab ");

        let heart = "Hello \u{2764}";
        assert_eq!(&rtf_escape(heart), r"Hello \uc0\u10084 ");

        let poop = "Hello 💩";
        assert_eq!(&rtf_escape(poop), r"Hello \uc0\u55357 \uc0\u56489 ");
    }

    #[test]
    fn test_rtf_escape_url() {
        let crunchy_url_text = r"https://google.com/?”×{}\{\hello}";
        assert_eq!(
            &rtf_escape(crunchy_url_text),
            r"https://google.com/?\uc0\u8221 \uc0\u215 \{\}\\\{\\hello\}"
        );

        let fmt_url = |url_str: &str, in_attr: bool| {
            let mut dest = String::new();
            let url = url::Url::parse(url_str).unwrap();
            RtfWriter::new(&mut dest, Default::default()).write_url(
                &url,
                url_str.ends_with('/'),
                in_attr,
            );
            dest
        };

        let crunchy_valid_url = r#"https://google.com/"hi{\}"_?attr\=[]"#;
        // apparently backslashes are normalised to a forward slash in the url lirary if it's not in
        // the query string, so the first %5C can be replaced with a /
        // assert_eq!(&fmt_url(crunchy_valid_url, true), "https://google.com/%22hi%7B%5C%7D%22_?attr%5C=%5B%5D")
        assert_eq!(
            &fmt_url(crunchy_valid_url, true),
            "https://google.com/%22hi%7B/%7D%22_?attr%5C=%5B%5D"
        )
    }

    /// See the main citeproc/tests/suite.rs
    #[derive(PartialEq, Eq)]
    #[doc(hidden)]
    pub struct PrettyString<'a>(pub &'a str);

    /// Make diff to display string as multi-line string
    impl<'a> fmt::Debug for PrettyString<'a> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str(self.0)
        }
    }
}
