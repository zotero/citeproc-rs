// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::{FormatCmd, LocalizedQuotes, OutputFormat};
use crate::utils::JoinMany;
use crate::IngestOptions;
use csl::style::{
    FontStyle, FontVariant, FontWeight, Formatting, TextDecoration, VerticalAlignment,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Html {
    Html(HtmlOptions),
    Rtf(RtfOptions),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum QuoteType {
    SingleQuote,
    DoubleQuote,
}

/// TODO: serialize and deserialize using an HTML parser?
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InlineElement {
    /// This is how we can flip-flop only user-supplied styling.
    /// Inside this is parsed micro html
    Micro(Vec<MicroNode>),

    Formatted(Vec<InlineElement>, Formatting),
    Quoted(QuoteType, Vec<InlineElement>),
    Text(String),
    Anchor {
        title: String,
        url: String,
        content: Vec<InlineElement>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RtfOptions {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HtmlOptions {
    // TODO: is it enough to have one set of localized quotes for the entire style?
    // quotes: LocalizedQuotes,
    use_b_for_strong: bool,
}

impl Default for HtmlOptions {
    fn default() -> Self {
        HtmlOptions {
            use_b_for_strong: false,
        }
    }
}

impl HtmlOptions {
    pub fn test_suite() -> Self {
        HtmlOptions {
            use_b_for_strong: true,
        }
    }
}

impl MicroNode {
    fn to_html_inner(&self, s: &mut String, options: &HtmlOptions) {
        use MicroNode::*;
        match self {
            Text(text) => {
                use v_htmlescape::escape;
                s.push_str(&escape(text).to_string());
            }
            Formatted(nodes, cmd) => {
                let tag = cmd.html_tag(options);
                *s += "<";
                *s += tag.0;
                *s += tag.1;
                *s += ">";
                for node in nodes {
                    node.to_html_inner(s, options);
                }
                *s += "</";
                *s += tag.0;
                *s += ">";
            }
            NoCase(inners) => {
                for i in inners {
                    i.to_html_inner(s, options);
                }
            }
        }
    }
}

impl FormatCmd {
    fn html_tag(&self, options: &HtmlOptions) -> (&'static str, &'static str) {
        use FormatCmd::*;
        match self {
            FontStyleItalic => ("i", ""),
            FontStyleOblique => ("span", r#" style="font-style:oblique;""#),
            FontStyleNormal => ("span", r#" style="font-style:normal;""#),

            FontWeightBold => {
                if options.use_b_for_strong {
                    ("b", "")
                } else {
                    ("strong", "")
                }
            }
            FontWeightNormal => ("span", r#" style="font-weight:normal;""#),
            FontWeightLight => ("span", r#" style="font-weight:light;""#),

            FontVariantSmallCaps => ("span", r#" style="font-variant:small-caps;""#),
            FontVariantNormal => ("span", r#" style="font-variant:normal;""#),

            TextDecorationUnderline => ("span", r#" style="text-decoration:underline;""#),
            TextDecorationNone => ("span", r#" style="text-decoration:none;""#),

            VerticalAlignmentSuperscript => ("sup", ""),
            VerticalAlignmentSubscript => ("sub", ""),
            VerticalAlignmentBaseline => ("span", r#" style="vertical-alignment:baseline;"#),
        }
    }
}

fn stack_formats_html(
    s: &mut String,
    options: &HtmlOptions,
    inlines: &[InlineElement],
    formatting: Formatting,
) {
    use self::FormatCmd::*;

    let mut stack = Vec::new();

    match formatting.font_style {
        Some(FontStyle::Italic) => stack.push(FontStyleItalic),
        Some(FontStyle::Oblique) => stack.push(FontStyleOblique),
        Some(FontStyle::Normal) => stack.push(FontStyleNormal),
        _ => {}
    }
    match formatting.font_weight {
        Some(FontWeight::Bold) => stack.push(FontWeightBold),
        Some(FontWeight::Light) => stack.push(FontWeightLight),
        Some(FontWeight::Normal) => stack.push(FontWeightNormal),
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

    for cmd in stack.iter() {
        let tag = cmd.html_tag(options);
        s.push_str("<");
        s.push_str(tag.0);
        s.push_str(tag.1);
        s.push_str(">");
    }
    for inner in inlines {
        inner.to_html_inner(s, options);
    }
    for cmd in stack.iter().rev() {
        let tag = cmd.html_tag(options);
        s.push_str("</");
        s.push_str(tag.0);
        s.push_str(">");
    }
}
impl InlineElement {
    fn to_html(inlines: &[InlineElement], options: &HtmlOptions) -> String {
        let mut s = String::new();
        for i in inlines {
            i.to_html_inner(&mut s, options);
        }
        s
    }
    fn to_html_inner(&self, s: &mut String, options: &HtmlOptions) {
        match self {
            Text(text) => {
                use v_htmlescape::escape;
                s.push_str(&escape(text).to_string());
            }
            Micro(micros) => {
                for micro in micros {
                    micro.to_html_inner(s, options);
                }
            }
            Formatted(inlines, formatting) => {
                stack_formats_html(s, options, inlines, *formatting);
            }
            Quoted(_qt, inners) => {
                s.push_str(r#"<q>"#);
                for i in inners {
                    i.to_html_inner(s, options);
                }
                s.push_str("</q>");
            }
            Anchor {
                title: _,
                url,
                content,
            } => {
                s.push_str(r#"<a href=""#);
                // TODO: HTML-quoted-escape? the url?
                s.push_str(&url);
                s.push_str(r#"">"#);
                for i in content {
                    i.to_html_inner(s, options);
                }
                s.push_str("</a>");
            }
        }
    }
}

impl InlineElement {
    fn to_rtf(inlines: &[InlineElement], options: &RtfOptions) -> String {
        let mut s = String::new();
        for i in inlines {
            i.to_rtf_inner(&mut s, options);
        }
        s
    }
    fn to_rtf_inner(&self, s: &mut String, options: &RtfOptions) {
        unimplemented!()
    }
}

use self::InlineElement::*;

impl Default for Html {
    fn default() -> Self {
        Html::Html(HtmlOptions::default())
    }
}

impl Html {
    /// Wrap some nodes with formatting
    ///
    /// In pandoc, Emph, Strong and SmallCaps, Superscript and Subscript are all single-use styling
    /// elements. So formatting with two of those styles at once requires wrapping twice, in any
    /// order.

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
}

use super::micro_html::{MicroHtml, MicroNode};

impl OutputFormat for Html {
    type Input = String;
    type Build = Vec<InlineElement>;
    type Output = String;

    fn ingest(&self, input: &str, options: IngestOptions) -> Self::Build {
        vec![InlineElement::Micro(MicroNode::parse(input, options))]
    }

    #[inline]
    fn plain(&self, s: &str) -> Self::Build {
        self.text_node(s.to_owned(), None)
    }

    fn text_node(&self, text: String, f: Option<Formatting>) -> Vec<InlineElement> {
        let v = vec![Text(text)];
        self.fmt_vec(v, f)
    }

    #[inline]
    fn seq(&self, nodes: impl Iterator<Item = Self::Build>) -> Self::Build {
        itertools::concat(nodes)
    }

    fn join_delim(&self, a: Self::Build, delim: &str, b: Self::Build) -> Self::Build {
        [a, b].join_many(&self.plain(delim))
    }

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

    fn quoted(&self, b: Self::Build, quotes: &LocalizedQuotes) -> Self::Build {
        let qt = match quotes {
            LocalizedQuotes::Single(..) => QuoteType::SingleQuote,
            LocalizedQuotes::Double(..) => QuoteType::SingleQuote,
            // Would this be better? Only allow
            // LocalizedQuotes::Double(open, close) |
            // LocalizedQuotes::Single(open, close) => {
            //     return self.affixed(b, Affixes { prefix: open.clone(), suffix: close.clone() });
            // }
        };
        vec![InlineElement::Quoted(qt, b)]
    }

    fn hyperlinked(&self, a: Self::Build, target: Option<&str>) -> Self::Build {
        // TODO: allow internal linking using the Attr parameter (e.g.
        // first-reference-note-number)
        if let Some(target) = target {
            vec![InlineElement::Anchor {
                title: "".into(),
                url: target.into(),
                content: a,
            }]
        } else {
            a
        }
    }

    fn output(&self, inter: Vec<InlineElement>) -> Self::Output {
        let null = FlipFlopState::default();
        let flipped = flip_flop_inlines(&inter, &null);
        let string = match self {
            Html::Html(ref options) => InlineElement::to_html(&flipped, options),
            Html::Rtf(ref options) => InlineElement::to_rtf(&flipped, options),
        };
        string
    }
}

#[derive(Default, Debug, Clone)]
struct FlipFlopState {
    in_emph: bool,
    emph: FontStyle,
    in_strong: bool,
    in_small_caps: bool,
    in_outer_quotes: bool,
}

fn flip_flop_inlines(inlines: &[InlineElement], state: &FlipFlopState) -> Vec<InlineElement> {
    inlines
        .iter()
        .map(|inl| flip_flop(inl, state).unwrap_or_else(|| inl.clone()))
        .collect()
}

fn flip_flop_nodes(nodes: &[MicroNode], state: &FlipFlopState) -> Vec<MicroNode> {
    nodes
        .iter()
        .map(|nod| flip_flop_node(nod, state).unwrap_or_else(|| nod.clone()))
        .collect()
}

fn flip_flop_node(node: &MicroNode, state: &FlipFlopState) -> Option<MicroNode> {
    let fl = |nodes: &[MicroNode], st| flip_flop_nodes(nodes, st);
    match node {
        MicroNode::Formatted(ref nodes, cmd) => {
            let mut flop = state.clone();
            match cmd {
                FormatCmd::FontStyleItalic => {
                    flop.in_emph = !flop.in_emph;
                    let subs = fl(nodes, &flop);
                    if state.in_emph {
                        Some(MicroNode::Formatted(subs, FormatCmd::FontStyleNormal))
                    } else {
                        Some(MicroNode::Formatted(subs, *cmd))
                    }
                }
                FormatCmd::FontWeightBold => {
                    flop.in_strong = !flop.in_strong;
                    let subs = fl(nodes, &flop);
                    if state.in_strong {
                        Some(MicroNode::Formatted(subs, FormatCmd::FontWeightNormal))
                    } else {
                        Some(MicroNode::Formatted(subs, *cmd))
                    }
                }
                FormatCmd::FontVariantSmallCaps => {
                    flop.in_small_caps = !flop.in_small_caps;
                    let subs = fl(nodes, &flop);
                    if state.in_small_caps {
                        Some(MicroNode::Formatted(subs, FormatCmd::FontVariantNormal))
                    } else {
                        Some(MicroNode::Formatted(subs, *cmd))
                    }
                }
                // i.e. sup and sub
                _ => {
                    let subs = fl(nodes, state);
                    Some(MicroNode::Formatted(subs, *cmd))
                }
            }
        }
        MicroNode::Text(_) => None,
        MicroNode::NoCase(ref nodes) => {
            let subs = fl(nodes, state);
            Some(MicroNode::NoCase(subs))
        }
    }
}

fn flip_flop(inline: &InlineElement, state: &FlipFlopState) -> Option<InlineElement> {
    let fl = |ils: &[InlineElement], st| flip_flop_inlines(ils, st);
    match inline {
        Micro(nodes) => {
            let subs = flip_flop_nodes(nodes, state);
            Some(Micro(subs))
        }
        Formatted(ils, f) => {
            let mut flop = state.clone();
            let mut new_f = f.clone();
            if let Some(fs) = f.font_style {
                let samey = fs == state.emph;
                if samey {
                    new_f.font_style = None;
                }
                flop.in_emph = match fs {
                    FontStyle::Italic | FontStyle::Oblique => true,
                    _ => false,
                };
                flop.emph = fs;
            }
            if let Some(fw) = f.font_weight {
                let want = fw == FontWeight::Bold;
                if flop.in_strong == want && want == true {
                    new_f.font_weight = None;
                }
                flop.in_strong = want;
            }
            if let Some(fv) = f.font_variant {
                let want_small_caps = fv == FontVariant::SmallCaps;
                if flop.in_small_caps == want_small_caps {
                    new_f.font_variant = None;
                }
                flop.in_small_caps = want_small_caps;
            }
            let subs = fl(ils, &flop);
            Some(Formatted(subs, new_f))
        }

        Quoted(ref _q, ref ils) => {
            let mut flop = state.clone();
            flop.in_outer_quotes = !flop.in_outer_quotes;
            let subs = fl(ils, &flop);
            if !state.in_outer_quotes {
                Some(Quoted(QuoteType::SingleQuote, subs))
            } else {
                Some(Quoted(QuoteType::DoubleQuote, subs))
            }
        }

        Anchor {
            title,
            url,
            content,
        } => {
            let subs = fl(content, state);
            Some(Anchor {
                title: title.clone(),
                url: url.clone(),
                content: subs,
            })
        }

        _ => None,
    }

    // a => a
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_flip_emph() {
        let f = Html::default();
        let a = f.plain("normal");
        let b = f.text_node("emph".into(), Some(Formatting::italic()));
        let c = f.plain("normal");
        let group = f.group(vec![a, b, c], " ", Some(Formatting::italic()));
        let out = f.output(group.clone());

        let group_str = InlineElement::to_html(&group, &HtmlOptions::default());
        assert_ne!(group_str, out);
        assert_eq!(
            out,
            "<i>normal <span style=\"font-style: initial;\">emph</span> normal</i>"
        );
    }
}
