// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use self::InlineElement::*;
use super::links::Link;
use super::micro_html::MicroNode;
use super::{FormatCmd, LocalizedQuotes, OutputFormat};
use crate::utils::JoinMany;
use crate::IngestOptions;
use csl::{
    DisplayMode, FontStyle, FontVariant, FontWeight, Formatting, TextCase, TextDecoration,
    VerticalAlignment,
};
use url::Url;

mod rtf;
use self::rtf::RtfWriter;

mod html;
use self::html::HtmlWriter;

mod plain;
use self::plain::PlainWriter;

mod flip_flop;
use self::flip_flop::FlipFlopState;
mod move_punctuation;
use self::move_punctuation::move_punctuation;

pub use self::move_punctuation::is_punc;

use crate::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Markup {
    Html(FormatOptions),
    Rtf(FormatOptions),
    Plain(FormatOptions),
}

/// Controls how the output is formatted.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FormatOptions {
    /// See CSL 1.1, Appendix VI -- enable or disable making urls clickable. Default is enabled.
    pub link_anchors: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        FormatOptions { link_anchors: true }
    }
}

impl FormatOptions {
    pub fn test_suite() -> Self {
        FormatOptions {
            link_anchors: false,
        }
    }
}

/// TODO: serialize and deserialize using an HTML parser?
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum InlineElement {
    /// This is how we can flip-flop only user-supplied styling.
    /// Inside this is parsed micro html
    Micro(Vec<MicroNode>),

    Formatted(Vec<InlineElement>, Formatting),
    /// Bool is "is_inner"
    Quoted {
        is_inner: bool,
        localized: LocalizedQuotes,
        inlines: Vec<InlineElement>,
    },
    Text(String),
    Linked(Link),
    Div(DisplayMode, Vec<InlineElement>),
}

impl InlineElement {}

impl Markup {
    pub fn html() -> Self {
        Markup::Html(FormatOptions::default())
    }
    pub fn test_html() -> Self {
        Markup::Html(FormatOptions::test_suite())
    }
    pub fn rtf() -> Self {
        Markup::Rtf(FormatOptions::default())
    }
    pub fn plain() -> Self {
        Markup::Plain(FormatOptions::default())
    }
}

impl Default for Markup {
    fn default() -> Self {
        Markup::Html(FormatOptions::default())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MarkupBibMeta {
    #[serde(rename = "markupPre")]
    markup_pre: String,
    #[serde(rename = "markupPost")]
    markup_post: String,
}

impl OutputFormat for Markup {
    type Input = String;
    type Build = Vec<InlineElement>;
    type Output = String;
    type BibMeta = MarkupBibMeta;

    fn meta(&self) -> Self::BibMeta {
        let (pre, post) = match self {
            Markup::Html(_) => ("<div class=\"csl-bib-body\">", "</div>"),
            Markup::Rtf(_) => ("", ""),
            Markup::Plain(_) => ("", ""),
        };
        MarkupBibMeta {
            markup_pre: pre.into(),
            markup_post: post.into(),
        }
    }

    #[inline]
    fn ingest(&self, input: &str, options: &IngestOptions) -> Self::Build {
        let mut nodes = MicroNode::parse(input, options);
        options.apply_text_case_micro(&mut nodes);
        if nodes.is_empty() {
            return Vec::new();
        }
        vec![InlineElement::Micro(nodes)]
    }

    #[inline]
    fn plain(&self, s: &str) -> Self::Build {
        self.text_node(s.into(), None)
    }

    #[inline]
    fn text_node(&self, text: String, f: Option<Formatting>) -> Vec<InlineElement> {
        if text.is_empty() {
            return vec![];
        }
        let v = vec![Text(text)];
        self.fmt_vec(v, f)
    }

    #[inline]
    fn seq(&self, nodes: impl IntoIterator<Item = Self::Build>) -> Self::Build {
        itertools::concat(nodes.into_iter())
    }

    #[inline]
    fn join_delim(&self, a: Self::Build, delim: &str, b: Self::Build) -> Self::Build {
        [a, b].join_many(&self.plain(delim))
    }

    #[inline]
    fn group(
        &self,
        nodes: Vec<Self::Build>,
        delimiter: &str,
        formatting: Option<Formatting>,
    ) -> Self::Build {
        if nodes.len() == 1 {
            self.fmt_vec(nodes.into_iter().nth(0).unwrap(), formatting)
        } else {
            let delim = self.plain(delimiter);
            self.fmt_vec(nodes.join_many(&delim), formatting)
        }
    }

    #[inline]
    fn with_format(&self, a: Self::Build, f: Option<Formatting>) -> Self::Build {
        self.fmt_vec(a, f)
    }

    #[inline]
    fn with_display(
        &self,
        a: Self::Build,
        display: Option<DisplayMode>,
        in_bib: bool,
    ) -> Self::Build {
        if in_bib {
            if let Some(d) = display {
                return vec![InlineElement::Div(d, a)];
            }
        }
        a
    }

    #[inline]
    fn quoted(&self, b: Self::Build, quotes: LocalizedQuotes) -> Self::Build {
        // Default not is_inner; figure out which ones are inner/outer when doing flip-flop later.
        vec![InlineElement::Quoted {
            is_inner: false,
            localized: quotes,
            inlines: b,
        }]
    }

    #[inline]
    fn link(&self, link: Link) -> Self::Build {
        vec![InlineElement::Linked(link)]
    }

    #[inline]
    fn is_empty(&self, a: &Self::Build) -> bool {
        a.is_empty()
    }

    #[inline]
    fn output(&self, intermediate: Self::Build, punctuation_in_quote: bool) -> Self::Output {
        let null = FlipFlopState::default();
        self.output_with_state(intermediate, null, Some(punctuation_in_quote))
    }

    #[inline]
    fn output_in_context(
        &self,
        intermediate: Self::Build,
        format_stacked: Formatting,
        punctuation_in_quote: Option<bool>,
    ) -> Self::Output {
        let state = FlipFlopState::from_formatting(format_stacked);
        self.output_with_state(intermediate, state, punctuation_in_quote)
    }

    #[inline]
    fn stack_preorder(&self, dest: &mut String, stack: &[FormatCmd]) {
        match *self {
            Markup::Html(options) => HtmlWriter::new(dest, options).stack_preorder(stack),
            Markup::Rtf(options) => PlainWriter::new(dest, options).stack_preorder(stack),
            Markup::Plain(options) => PlainWriter::new(dest, options).stack_preorder(stack),
        }
    }

    #[inline]
    fn stack_postorder(&self, dest: &mut String, stack: &[FormatCmd]) {
        match *self {
            Markup::Html(options) => HtmlWriter::new(dest, options).stack_postorder(stack),
            Markup::Rtf(options) => PlainWriter::new(dest, options).stack_postorder(stack),
            Markup::Plain(options) => PlainWriter::new(dest, options).stack_postorder(stack),
        }
    }

    #[inline]
    fn tag_stack(&self, formatting: Formatting, display: Option<DisplayMode>) -> Vec<FormatCmd> {
        tag_stack(formatting, display)
    }

    #[inline]
    fn append_suffix(&self, pre_and_content: &mut Self::Build, suffix: &str) {
        let suffix = MicroNode::parse(suffix, &IngestOptions::for_affixes());
        use self::move_punctuation::append_suffix;
        append_suffix(pre_and_content, suffix);
    }

    #[inline]
    fn ends_with_full_stop(&self, build: &Self::Build) -> bool {
        move_punctuation::ends_with_full_stop(build, true)
    }

    #[inline]
    fn apply_text_case(&self, build: &mut Self::Build, options: &IngestOptions) {
        if options.text_case == TextCase::None {
            return;
        }
        let is_uppercase = options.is_uppercase(build);
        options.apply_text_case_inner(build, false, is_uppercase);
    }
}

impl Markup {
    fn fmt_vec(
        &self,
        inlines: Vec<InlineElement>,
        formatting: Option<Formatting>,
    ) -> Vec<InlineElement> {
        if let Some(f) = formatting {
            vec![Formatted(inlines, f)]
        } else {
            inlines
        }
    }

    fn output_with_state(
        &self,
        intermediate: <Self as OutputFormat>::Build,
        initial_state: FlipFlopState,
        punctuation_in_quote: Option<bool>,
    ) -> <Self as OutputFormat>::Output {
        let mut flipped = initial_state.flip_flop_inlines(&intermediate);
        move_punctuation(&mut flipped, punctuation_in_quote);
        let mut dest = String::new();
        match *self {
            Markup::Html(options) => {
                HtmlWriter::new(&mut dest, options).write_inlines(&flipped, false)
            }
            Markup::Rtf(options) => {
                RtfWriter::new(&mut dest, options).write_inlines(&flipped, false)
            }
            Markup::Plain(options) => {
                PlainWriter::new(&mut dest, options).write_inlines(&flipped, false)
            }
        }
        dest
    }
}

pub trait MarkupWriter {
    fn write_escaped(&mut self, text: &str);
    /// Write a url; if outside an `href` attribute, modify the output slightly (remove trailing slash
    /// if not desired).
    fn write_url(&mut self, url: &Url, trailing_slash: bool, in_attr: bool);
    fn buf(&mut self) -> &mut String;
    fn write_raw(&mut self, s: &str) {
        self.buf().push_str(s)
    }
    fn write_link(
        &mut self,
        a_href: &str,
        link: &Link,
        href_close: &str,
        a_close: &str,
        options: FormatOptions,
    ) {
        match link {
            Link::Url {
                url,
                trailing_slash,
            } if allow_url_scheme(url.scheme()) => {
                if options.link_anchors {
                    self.write_raw(a_href);
                    self.write_url(url, *trailing_slash, true);
                    self.write_raw(href_close);
                    self.write_url(url, *trailing_slash, false);
                    self.write_raw(a_close);
                } else {
                    self.write_url(url, *trailing_slash, false);
                }
            }
            Link::Url {
                url,
                trailing_slash,
            } => {
                // This catches, e.g. `javascript:alert("hello")`
                warn!(
                    "refusing to render url anchor for scheme {} on url {}",
                    url.scheme(),
                    url
                );
                self.write_url(&url, *trailing_slash, false);
            }
            Link::Id { id, url } => {
                if options.link_anchors {
                    self.write_raw(a_href);
                    self.write_url(url, false, true);
                    self.write_raw(href_close);
                    self.write_url(url, false, true);
                    self.write_raw(a_close);
                } else {
                    self.write_escaped(id);
                }
            }
        }
    }
    fn stack_preorder(&mut self, stack: &[FormatCmd]);
    fn stack_postorder(&mut self, stack: &[FormatCmd]);

    /// Use this to write an InlineElement::Formatted
    fn stack_formats(
        &mut self,
        inlines: &[InlineElement],
        formatting: Formatting,
        display: Option<DisplayMode>,
    ) {
        let stack = tag_stack(formatting, display);
        self.stack_preorder(&stack);
        self.write_inlines(inlines, display == Some(DisplayMode::LeftMargin));
        self.stack_postorder(&stack);
    }

    fn write_micro(&mut self, micro: &MicroNode, trim_start: bool);
    /// Returned boolean = true if it used the peeked element to move some punctuation inside, and
    /// hence should skip it.
    fn write_micros(&mut self, micros: &[MicroNode], trim_start: bool) {
        let mut seen = false;
        for micro in micros {
            self.write_micro(micro, trim_start && !seen);
            seen = true;
        }
    }
    fn write_inline(&mut self, inline: &InlineElement, trim_start: bool);
    fn write_inlines(&mut self, inlines: &[InlineElement], trim_start: bool) {
        let mut seen = false;
        for inline in inlines {
            self.write_inline(inline, trim_start && !seen);
            seen = true;
        }
    }
}

fn tag_stack(formatting: Formatting, display: Option<DisplayMode>) -> Vec<FormatCmd> {
    use super::FormatCmd::*;
    let mut stack = Vec::new();
    match display {
        Some(DisplayMode::Block) => stack.push(DisplayBlock),
        Some(DisplayMode::Indent) => stack.push(DisplayIndent),
        Some(DisplayMode::LeftMargin) => stack.push(DisplayLeftMargin),
        Some(DisplayMode::RightInline) => stack.push(DisplayRightInline),
        _ => {}
    }
    match formatting.font_weight {
        Some(FontWeight::Bold) => stack.push(FontWeightBold),
        Some(FontWeight::Light) => stack.push(FontWeightLight),
        Some(FontWeight::Normal) => stack.push(FontWeightNormal),
        _ => {}
    }
    match formatting.font_style {
        Some(FontStyle::Italic) => stack.push(FontStyleItalic),
        Some(FontStyle::Oblique) => stack.push(FontStyleOblique),
        Some(FontStyle::Normal) => stack.push(FontStyleNormal),
        _ => {}
    }
    match formatting.font_variant {
        Some(FontVariant::SmallCaps) => stack.push(FontVariantSmallCaps),
        Some(FontVariant::Normal) => stack.push(FontVariantNormal),
        _ => {}
    };
    match formatting.text_decoration {
        Some(TextDecoration::Underline) => stack.push(TextDecorationUnderline),
        Some(TextDecoration::None) => stack.push(TextDecorationNone),
        _ => {}
    }
    match formatting.vertical_alignment {
        Some(VerticalAlignment::Superscript) => stack.push(VerticalAlignmentSuperscript),
        Some(VerticalAlignment::Subscript) => stack.push(VerticalAlignmentSubscript),
        Some(VerticalAlignment::Baseline) => stack.push(VerticalAlignmentBaseline),
        _ => {}
    }
    stack
}

trait MaybeTrimStart {
    fn trim_start_if<'a>(&'a self, trim_if: bool) -> &'a Self;
}
impl MaybeTrimStart for str {
    fn trim_start_if<'a>(&'a self, trim_if: bool) -> &'a Self {
        if trim_if {
            self.trim_start()
        } else {
            self
        }
    }
}

use core::fmt::{self, Write};

thread_local! {
    static ESC_BUF: std::cell::RefCell<String> = std::cell::RefCell::new(String::new());
}

/// Write a url; if outside an `href` attribute, modify the output slightly (remove trailing slash
/// if not desired).
fn write_url(
    f: &mut String,
    url: &Url,
    trailing_slash: bool,
    in_attr: bool,
    escape_in_attribute: for<'tls> fn(&mut String, &'tls str) -> fmt::Result,
    escape: for<'tls> fn(&mut String, &'tls str) -> fmt::Result,
) -> fmt::Result {
    ESC_BUF.with(|tls_buf| {
        let mut tls_buf = tls_buf.borrow_mut();
        tls_buf.clear();
        write!(tls_buf, "{}", url)?;
        if in_attr {
            escape_in_attribute(f, &tls_buf)?;
        } else {
            // outside the href, be faithful to the user's intention re any
            // trailing slash or absence thereof.
            // normally, Url will write a trailing slash for "special" https://
            // etc schemes, in line with WHATWG URL.
            if url.has_host() && matches!(url.scheme(), "https" | "http") {
                if !trailing_slash && tls_buf.ends_with('/') {
                    tls_buf.pop();
                }
            }
            escape(f, &tls_buf)?;
        }
        Ok(())
    })
}

fn allow_url_scheme(scheme: &str) -> bool {
    // see https://security.stackexchange.com/questions/148428/which-url-schemes-are-dangerous-xss-exploitable
    // list from wordpress https://developer.wordpress.org/reference/functions/wp_allowed_protocols/
    [
        "https", "http", "ftp", "ftps", "mailto", "news", "irc", "irc6", "ircs", "gopher", "nntp",
        "feed", "telnet", "mms", "rtsp", "sms", "svn", "tel", "fax", "xmpp", "webcal", "urn",
    ]
    .contains(&scheme)
}
