use crate::output::markup::InlineElement;
use crate::output::micro_html::MicroNode;
use crate::output::LocalizedQuotes;
use crate::{lazy, IngestOptions, SmartCow, String};
use csl::{FontVariant, TextCase, VerticalAlignment};
use unic_segment::{GraphemeIndices, WordBoundIndices, Words};

// from the unic_segment example code
fn has_alphanumeric(s: &&str) -> bool {
    is_word(*s)
}
fn is_word(s: &str) -> bool {
    s.chars().any(|ch| ch.is_alphanumeric())
}

fn transform_first_char_of_word<F, I>(word: &str, f: F) -> SmartCow
where
    F: Fn(char) -> I,
    I: Iterator<Item = char> + Clone,
{
    // Naively capitalizes without a stopword filter
    let mut chars = word.chars();
    match chars.next() {
        None => SmartCow::Borrowed(word),
        Some(first) => {
            let tx = f(first);
            // Don't allocate for Already Capitalized Words
            if tx.clone().count() == 1 && tx.clone().nth(0) == Some(first) {
                return SmartCow::Borrowed(word);
            }
            let mut s = String::new();
            s.extend(tx);
            // Fast convert back from iterator which knows its own byte offset
            s.push_str(chars.as_str());
            SmartCow::Owned(s)
        }
    }
}

fn transform_uppercase_first(word: &str) -> SmartCow {
    transform_first_char_of_word(word, |c| c.to_uppercase())
}

// use phf::phf_set;
// static SPEC_STOPWORDS: phf::Set<&'static str> = phf_set! { "a", "an", "and", "as", "at", "but",
// "by", "down", "for", "from", "in", "into", "nor", "of", "on", "onto", "or", "over", "so", "the",
// "till", "to", "up", "via", "with", "yet", };

static CITEPROC_JS_STOPWORD_REGEX: once_cell::sync::OnceCell<regex::Regex> =
    once_cell::sync::OnceCell::new();
fn stopword_regex() -> &'static regex::Regex {
    let re = concat![
        // Match case insensitive (regex crate's simple case folding is fine)
        "(?i)",
        // Match the start only
        "^(?:",
        // Sort lines by length so that longer matches are preferred
        // vim: visual select, then, type !awk '{ print length(), $0 | "sort -n" }'
        "notwithstanding|",
        "regardless of|",
        "according to|",
        "rather than|",
        "pursuant to|",
        "vis-à-vis|",
        "underneath|",
        "throughout|",
        "outside of|",
        "instead of|",
        "except for|",
        "because of|",
        "aside from|",
        "as regards|",
        "apart from|",
        "inside of|",
        "forenenst|",
        "alongside|",
        "where as|",
        "prior to|",
        "out from|",
        "far from|",
        "close to|",
        "ahead of|",
        "without|",
        "towards|",
        "thruout|",
        "through|",
        "that of|",
        "such as|",
        "next to|",
        "near to|",
        "despite|",
        "between|",
        "besides|",
        "beneath|",
        "barring|",
        "back to|",
        "athwart|",
        "astride|",
        "apropos|",
        "amongst|",
        "against|",
        "within|",
        "versus|",
        "toward|",
        "out of|",
        "modulo|",
        "inside|",
        "except|",
        "during|",
        "due to|",
        "beyond|",
        "beside|",
        "behind|",
        "before|",
        "as per|",
        "as for|",
        "around|",
        "anenst|",
        "amidst|",
        "across|",
        "up to|",
        "until|",
        "under|",
        "since|",
        "on to|",
        "given|",
        "circa|",
        "below|",
        "aside|",
        "as of|",
        "among|",
        "along|",
        "after|",
        "afore|",
        "above|",
        "about|",
        "with|",
        "upon|",
        "unto|",
        "till|",
        "thru|",
        "than|",
        "sans|",
        "plus|",
        "over|",
        "onto|",
        "next|",
        "near|",
        "like|",
        "lest|",
        "into|",
        "from|",
        "down|",
        "atop|",
        "apud|",
        "amid|",
        "yet|",
        "vs.|",
        "von|",
        "via|",
        "the|",
        "qua|",
        "pro|",
        "per|",
        "out|",
        "off|",
        "nor|",
        "for|",
        "but|",
        "and|",
        "vs|",
        "van|",
        "v.|",
        "up|",
        "to|",
        "so|",
        "or|",
        "on|",
        "of|",
        "in|",
        "et|",
        "de|",
        "ca|",
        "by|",
        "at|",
        "as|",
        "an|",
        "al|",
        "v|",
        "c|",
        "a",
        // Skip the | on the last one
        ")(?:\\s|$)",
        // John d’Doe
        "|^(?-i)d\u{2019}",
        "|^(?-i)d'",
        "|^(?-i)l\u{2019}",
        "|^(?-i)l'",
        "|^(?-i)of-"
    ];

    CITEPROC_JS_STOPWORD_REGEX.get_or_init(|| regex::Regex::new(re).unwrap())
}

#[test]
fn stopwords() {
    fn is_stopword(word_and_rest: &str) -> bool {
        stopword_regex().is_match(word_and_rest)
    }

    assert!(is_stopword("and "));
    assert!(!is_stopword("grandiloquent "));
    assert!(is_stopword("l’Anglais "));
    assert!(is_stopword("l’Égypte "));
    assert!(!is_stopword("this word followed by l’Égypte "));
}

/// Returns the length of the matched word
fn is_stopword(word_and_rest: &str) -> Option<usize> {
    stopword_regex().find(word_and_rest).map(|mat| mat.end())
}

fn upper_word_to_title(word: &str) -> Option<String> {
    let mut upper_gs = GraphemeIndices::new(word);
    if let Some((_, first_g)) = upper_gs.next() {
        let mut ret = String::new();
        ret.push_str(first_g);
        if let Some((rest_ix, _)) = upper_gs.next() {
            let rest = &word[rest_ix..];
            let rest_lower = lazy::lazy_lowercase(rest);
            ret.push_str(rest_lower.as_ref());
        }
        return Some(ret);
    }
    None
}

fn transform_sentence_case(s: String, seen_one: bool, is_last: bool, is_uppercase: bool) -> String {
    if is_uppercase {
        transform_each_word(
            &s,
            seen_one,
            is_last,
            |word, _word_and_rest, is_first, _no_stop| {
                if is_first {
                    if let Some(upper) = upper_word_to_title(word) {
                        return (SmartCow::Owned(upper), None);
                    }
                }
                (lazy::lazy_lowercase(word), None)
            },
        )
    } else {
        transform_first_word(s, transform_uppercase_first)
    }
}

fn title_case_word<'a>(
    word: &'a str,
    word_and_rest: &'a str,
    entire_is_uppercase: bool,
    no_stopword: bool,
) -> (SmartCow<'a>, Option<usize>) {
    if !no_stopword {
        if let Some(mut match_len) = is_stopword(word_and_rest) {
            // drop the trailing whitespace
            let matched = &word_and_rest[..match_len];
            let last_char = matched.chars().rev().nth(0).map_or(0, |c| {
                if c == '-' || c.is_whitespace() {
                    c.len_utf8()
                } else {
                    0
                }
            });
            match_len = match_len - last_char;
            let word_rest_upto_match = &word_and_rest[..match_len];
            let lowered = lazy::lazy_lowercase(word_rest_upto_match);
            return (lowered, Some(match_len));
        }
    }
    if !word.chars().any(|c| c.is_ascii_alphabetic() || c == '.') {
        // Entirely non-English
        // e.g. "β" in "β-Carotine"
        // Full stop is so A.D. doesn't become a.D.
        return (SmartCow::Borrowed(word), None);
    }
    if entire_is_uppercase {
        if let Some(ret) = upper_word_to_title(word) {
            return (SmartCow::Owned(ret), None);
        }
    }
    (
        transform_first_char_of_word(word, |c| c.to_uppercase()),
        None,
    )
}

fn transform_title_case(s: &str, seen_one: bool, is_last: bool) -> String {
    transform_each_word(
        &s,
        seen_one,
        is_last,
        |word, word_and_rest, _is_first, no_stop| {
            title_case_word(word, word_and_rest, false, no_stop)
        },
    )
}

fn transform_each_word<'a, F>(mut s: &'a str, seen_one: bool, is_last: bool, transform: F) -> String
where
    F: Fn(&'a str, &'a str, bool, bool) -> (SmartCow<'a>, Option<usize>),
{
    let mut acc = String::new();
    let mut is_first = !seen_one;
    let mut bounds = WordBoundIndices::new(s);
    while let Some((ix, substr)) = bounds.next() {
        let immediate_before = &s[..ix].chars().rev().nth(0);
        // 7.x; 7-x, do not title-case the x.
        let wordy = is_word(substr)
            && !(immediate_before.map_or(false, |b| b == '-' || b == '.') && substr.len() == 1)
            // textcase_NoSpaceBeforeApostrophe.txt
            // This is pretty iffy. Backticks are word boundaries. This is our big chance to force
            // people to kick bad typographic habits, and we're blowing it.
            && !(immediate_before.map_or(false, |b| b == '`') && {
                let immediate_before_that = s[..ix].chars().rev().nth(1);
                immediate_before_that
                    .map_or(false, |bt| unic_ucd_category::GeneralCategory::of(bt).is_letter())
               });
        if wordy {
            let before = &s[..ix].chars().rev().filter(|c| !c.is_whitespace()).nth(0);
            let follows_colon = *before == Some(':')
                || *before == Some('?')
                || *before == Some('!')
                || *before == Some('.');
            let rest = &s[ix + substr.len()..];
            let is_last = is_last && (rest.is_empty() || !is_word(rest));
            let no_stopword = is_first || is_last || follows_colon;
            let word = substr;
            let (tx, fast_forward) = transform(word, &s[ix..], is_first, no_stopword);
            acc.push_str(&tx);
            if let Some(ff) = fast_forward {
                s = &s[ix + ff..];
                trace!("fast_forward to {}", s);
                bounds = WordBoundIndices::new(s);
            }
        } else {
            acc.push_str(substr);
        }
        is_first = false;
    }
    acc
}

fn transform_first_word<'a>(s: String, transform: impl Fn(&str) -> SmartCow) -> String {
    let mut bounds = WordBoundIndices::new(&s);
    while let Some((ix, bound)) = bounds.next() {
        if is_word(bound) {
            let tx = transform(bound);
            if tx.as_ref() != bound {
                let mut ret = String::new();
                ret.push_str(&s[..ix]);
                ret.push_str(&tx);
                if let Some((rest_ix, _)) = bounds.next() {
                    ret.push_str(&s[rest_ix..]);
                }
                return ret;
            } else {
                return s;
            }
        }
    }
    s
}

fn string_contains_word(s: &str) -> bool {
    Words::new(s, has_alphanumeric).next().is_some()
}

impl IngestOptions {
    pub fn plain<'s>(&self, s: &'s str) -> SmartCow<'s> {
        if self.replace_hyphens && self.strip_periods {
            let cow = lazy::lazy_replace_char(s, '-', "\u{2013}");
            // For whatever reason you can't borrow from SmartCow with the original lifetime, only
            // a scoped one.
            SmartCow::Owned(lazy::lazy_replace_char(&cow, '.', "").into_owned())
        } else if self.replace_hyphens {
            lazy::lazy_replace_char(s, '-', "\u{2013}")
        } else if self.strip_periods {
            lazy::lazy_replace_char(s, '.', "")
        } else {
            SmartCow::Borrowed(s)
        }
    }
    pub fn apply_text_case_inner(
        &self,
        inlines: &mut [InlineElement],
        mut seen_one: bool,
        is_uppercase: bool,
    ) -> bool {
        let len = inlines.len();
        for (ix, inline) in inlines.iter_mut().enumerate() {
            if seen_one && self.text_case == TextCase::CapitalizeFirst {
                break;
            }
            let is_last = ix == len - 1;
            // order or short-circuits matters
            match inline {
                InlineElement::Text(txt) => {
                    let text = std::mem::take(txt);
                    *txt = self.transform_case(text, seen_one, is_last, is_uppercase);
                    seen_one = string_contains_word(txt.as_ref()) || seen_one;
                }
                InlineElement::Micro(micros) => {
                    seen_one =
                        self.apply_text_case_micro_inner(micros.as_mut(), seen_one, is_uppercase)
                            || seen_one;
                }
                InlineElement::Quoted {
                    inlines: content, ..
                }
                | InlineElement::Div(_, content) => {
                    seen_one = self.apply_text_case_inner(content.as_mut(), seen_one, is_uppercase)
                        || seen_one;
                }
                InlineElement::Formatted(content, formatting)
                    if formatting.font_variant != Some(FontVariant::SmallCaps)
                        && formatting.vertical_alignment
                            != Some(VerticalAlignment::Superscript)
                        && formatting.vertical_alignment != Some(VerticalAlignment::Subscript) =>
                {
                    seen_one = self.apply_text_case_inner(content.as_mut(), seen_one, is_uppercase)
                        || seen_one;
                }
                InlineElement::Formatted(content, _) => {
                    seen_one = seen_one || self.contains_word(content.as_ref());
                }
                InlineElement::Linked(_) => {}
            }
        }
        seen_one
    }
    pub fn apply_text_case_micro(&self, micros: &mut [MicroNode]) {
        if self.text_case == TextCase::None {
            return;
        }
        let is_uppercase = self.is_uppercase_micro(micros);
        self.apply_text_case_micro_inner(micros, false, is_uppercase);
    }
    pub fn apply_text_case_micro_inner(
        &self,
        micros: &mut [MicroNode],
        mut seen_one: bool,
        is_uppercase: bool,
    ) -> bool {
        let len = micros.len();
        for (ix, micro) in micros.iter_mut().enumerate() {
            if seen_one && self.text_case == TextCase::CapitalizeFirst {
                break;
            }
            let is_last = ix == len - 1;
            use crate::output::FormatCmd;
            // order or short-circuits matters
            match micro {
                MicroNode::Text(ref mut txt) => {
                    let text = std::mem::take(txt);
                    *txt = self.transform_case(text, seen_one, is_last, is_uppercase);
                    seen_one = string_contains_word(txt.as_ref()) || seen_one;
                }
                MicroNode::Formatted(children, FormatCmd::VerticalAlignmentSuperscript)
                | MicroNode::Formatted(children, FormatCmd::FontVariantSmallCaps)
                | MicroNode::Formatted(children, FormatCmd::VerticalAlignmentSubscript)
                | MicroNode::NoDecor(children)
                | MicroNode::NoCase(children) => {
                    seen_one = seen_one || self.contains_word_micro(children.as_ref());
                }
                MicroNode::Formatted(children, _) | MicroNode::Quoted { children, .. } => {
                    seen_one =
                        self.apply_text_case_micro_inner(children.as_mut(), seen_one, is_uppercase)
                            || seen_one;
                }
            }
        }
        seen_one
    }
    fn contains_word(&self, micros: &[InlineElement]) -> bool {
        any_inlines(string_contains_word, false, micros)
    }
    fn contains_word_micro(&self, micros: &[MicroNode]) -> bool {
        any_micros(string_contains_word, false, micros)
    }
    pub fn is_uppercase(&self, inlines: &[InlineElement]) -> bool {
        any_inlines(any_lowercase, true, inlines)
    }
    fn is_uppercase_micro(&self, micros: &[MicroNode]) -> bool {
        any_micros(any_lowercase, true, micros)
    }
    pub fn transform_case(
        &self,
        s: String,
        seen_one: bool,
        is_last: bool,
        entire_is_uppercase: bool,
    ) -> String {
        match self.text_case {
            TextCase::Lowercase => lazy::lazy_lowercase_owned(s),
            TextCase::Uppercase => lazy::lazy_uppercase_owned(s),
            TextCase::CapitalizeFirst => transform_first_word(s, transform_uppercase_first),
            TextCase::Sentence if !seen_one => {
                transform_sentence_case(s, seen_one, is_last, entire_is_uppercase)
            }
            // Fallback is nothing
            TextCase::Title if self.is_english => transform_title_case(&s, seen_one, is_last),
            TextCase::CapitalizeAll => {
                transform_each_word(&s, seen_one, is_last, |word, _, _, _| {
                    (transform_uppercase_first(word), None)
                })
            }
            TextCase::None | _ => s,
        }
    }
    pub fn default_with_quotes(quotes: LocalizedQuotes) -> Self {
        IngestOptions {
            quotes,
            ..Default::default()
        }
    }
}

fn any_lowercase(s: &str) -> bool {
    s.chars().any(|c| c.is_lowercase())
}

fn any_inlines<F: Fn(&str) -> bool + Copy>(f: F, invert: bool, inlines: &[InlineElement]) -> bool {
    inlines.iter().any(|i| match i {
        InlineElement::Text(txt) => f(txt.as_ref()),
        InlineElement::Micro(micros) => any_micros(f, invert, micros.as_ref()),
        InlineElement::Quoted { inlines, .. }
        | InlineElement::Div(_, inlines)
        | InlineElement::Formatted(inlines, _) => any_inlines(f, invert, inlines.as_ref()) ^ invert,
        InlineElement::Linked(_) => false,
    }) ^ invert
}

fn any_micros<F: Fn(&str) -> bool + Copy>(f: F, invert: bool, micros: &[MicroNode]) -> bool {
    micros.iter().any(|m| match m {
        MicroNode::Text(txt) => f(txt.as_ref()),
        MicroNode::Formatted(children, _)
        | MicroNode::Quoted { children, .. }
        | MicroNode::NoDecor(children)
        | MicroNode::NoCase(children) => any_micros(f, invert, children) ^ invert,
    }) ^ invert
}

#[test]
fn test_any_micros() {
    fn parse(x: &str) -> Vec<MicroNode> {
        MicroNode::parse(x, &Default::default())
    }
    fn upper(x: &str) -> bool {
        any_micros(any_lowercase, true, &parse(x))
    }
    assert_eq!(upper("Hello, <sup>superscript</sup>"), false);
    assert_eq!(upper("HELLOSUPERSCRIPT"), true);
    assert_eq!(upper("HELLO, <sup>SUPERSCRIPT</sup>"), true);
}
