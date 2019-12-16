use crate::IngestOptions;
use crate::LocalizedQuotes;
use crate::output::micro_html::MicroNode;
use crate::output::FormatCmd;

pub fn parse_quotes(slice: &mut [MicroNode]) {
    let options = IngestOptions::default_with_quotes(LocalizedQuotes::simple());
    let matcher = QuoteMatcher {
        stack: vec![],
        options: &options,
    };
    // matcher.parse_quotes(slice);
}

struct QuoteMatcher<'a> {
    stack: Vec<Option<StackFrame>>,
    options: &'a IngestOptions,
}

enum Intermediate<'a> {
    Event(Event<'a>),
    Index(usize),
}


struct QuotedStack {
    dest: Vec<MicroNode>,
    stack: Vec<(SFQuoteKind, Vec<MicroNode>)>,
}

impl QuotedStack {
    fn with_capacity(n: usize) -> Self {
        QuotedStack {
            dest: Vec::with_capacity(n),
            stack: Vec::new(),
        }
    }
    fn mut_ref(&mut self) -> &mut Vec<MicroNode> {
        if let Some((_kind, top)) = self.stack.last_mut() {
            top
        } else {
            &mut self.dest
        }
    }
    fn push(&mut self, node: MicroNode) {
        self.mut_ref().push(node)
    }
    fn push_str(&mut self, txt: &str) {
        let dest = self.mut_ref();
        if let Some(MicroNode::Text(ref mut string)) = dest.last_mut() {
            string.push_str(txt);
        } else {
            dest.push(MicroNode::Text(txt.into()));
        }
    }
    fn collapse_hanging(mut self) -> Vec<MicroNode> {
        while let Some((kind, quoted)) = self.stack.pop() {
            self.push_str(kind.unmatched_str());
            self.mut_ref().extend(quoted.into_iter());
        }
        self.dest
    }
}

fn stamp<'a>(intermediates: impl ExactSizeIterator + Iterator<Item = Intermediate<'a>>, orig: &mut Vec<MicroNode>) -> Vec<MicroNode> {
    let mut stack = QuotedStack::with_capacity(intermediates.len());
    let mut drained = 0;
    let mut drain = |start: usize, end: usize, stack: &mut QuotedStack| {
        debug!("{}..{}", start - drained, end - drained);
        stack.mut_ref().extend(orig.drain(start - drained .. end - drained));
        drained += end - start;
    };
    let mut range_wip: Option<(usize, usize)> = None;
    for inter in intermediates {
        // NEXT: turn this into a struct so you can get mutable reference to dest without
        // having it hanging around in target, and don't need this if statement replicated
        // everywhere.
        match inter {
            Intermediate::Event(ev) => {
                if let Some(range) = range_wip {
                    drain(range.0, range.1, &mut stack);
                    range_wip = None;
                }
                match ev {
                    Event::Text(txt) => stack.push_str(txt),
                    Event::SmartMidwordInvertedComma => stack.push_str("\u{2019}"),
                    Event::SmartQuoteSingleOpen => {
                        stack.stack.push((SFQuoteKind::Single, Vec::new()));
                    }
                    Event::SmartQuoteDoubleOpen => {
                        stack.stack.push((SFQuoteKind::Double, Vec::new()));
                    }
                    Event::SmartQuoteSingleClose => {
                        if let Some((SFQuoteKind::Single, _)) = stack.stack.last() {
                            let (_, children) = stack.stack.pop().unwrap();
                            stack.push(MicroNode::Quoted {
                                is_inner: false,
                                localized: LocalizedQuotes::simple(),
                                children,
                            });
                        } else {
                            stack.push_str(SFQuoteKind::Single.unmatched_str());
                        }
                    }
                    Event::SmartQuoteDoubleClose => {
                        if let Some((SFQuoteKind::Double, _)) = stack.stack.last() {
                            let (_, children) = stack.stack.pop().unwrap();
                            stack.push(MicroNode::Quoted {
                                is_inner: false,
                                localized: LocalizedQuotes::simple(),
                                children,
                            });
                        } else {
                            stack.push_str(SFQuoteKind::Double.unmatched_str());
                        }
                    }
                    _ => unimplemented!(),
                }
            },
            // Move sequential index references out of the array together where possible
            Intermediate::Index(ix) => {
                if let Some(ref mut range) = range_wip {
                    if range.1 == ix {
                        range.1 = ix + 1;
                    } else {
                        drain(range.0, range.1, &mut stack);
                        range_wip = Some((ix, ix + 1));
                    }
                } else {
                    range_wip = Some((ix, ix + 1));
                }
            }
        }
    }
    stack.collapse_hanging()
}

#[test]
fn test_stamp() {
    env_logger::init();
    let mut orig = vec![MicroNode::Text("hi".into()), MicroNode::Text("ho".into())];
    let inters = vec![
        Intermediate::Event(Event::Text("prefix, ")),
        Intermediate::Event(Event::SmartQuoteSingleOpen),
        Intermediate::Index(0),
        Intermediate::Index(1),
        Intermediate::Event(Event::Text("suffix")),
    ];
    assert_eq!(
        &stamp(inters.into_iter(), &mut orig),
        &[
            MicroNode::Text("prefix, '".into()),
            MicroNode::Text("hi".into()),
            MicroNode::Text("hosuffix".into()),
        ]
    );
    let mut orig = vec![MicroNode::Text("hi".into()), MicroNode::Text("ho".into())];
    let inters = vec![
        Intermediate::Event(Event::Text("prefix, ")),
        Intermediate::Event(Event::SmartQuoteSingleOpen),
        Intermediate::Index(0),
        Intermediate::Index(1),
        Intermediate::Event(Event::SmartQuoteSingleClose),
        Intermediate::Event(Event::Text(", suffix")),
    ];
    assert_eq!(
        &stamp(inters.into_iter(), &mut orig),
        &[
            MicroNode::Text("prefix, ".into()),
            MicroNode::Quoted {
                localized: LocalizedQuotes::simple(),
                is_inner: false,
                children: vec![
                    MicroNode::Text("hi".into()),
                    MicroNode::Text("ho".into()),
                ]
            },
            MicroNode::Text(", suffix".into()),
        ]
    );
}

impl<'a> QuoteMatcher<'a> {
//     // Quotes are parsed after <i> and friends.
//     // So you cannot have quotes that surround an open-tag.
//     pub fn parse_quotes(&mut self, slice: Vec<MicroNode>) -> Vec<MicroNode> {
//         // Cow is so we can put either references or owned nodes in there.
//         // Not because they need to be mutated in particular.
//         let mut assembling_quote: Option<Vec<Cow<'_, MicroNode>>> = None;
//         let mut building: Vec<Intermediate<'a>> = Vec::with_capacity(slice.len());
//         let mut prev = None;
//         for (ix, node) in slice.iter_mut().enumerate() {
//             match node {
//                 MicroNode::Quoted { ref mut children, .. }
//                 | MicroNode::NoCase(ref mut children)
//                     | MicroNode::Formatted(ref mut children, _) => {
//                         building.push(children);
//                         parse_quotes(&mut *children);
//                     },
//                 MicroNode::Text(string) => {
//                     // Going to need to search for open & close quotes.
//                     // Any imbalance either goes onto the stack or is popped off.
//                     // Then handle case where there is no close quote, when ending a
//                     // stack frame.
//                     let splitter = QuoteSplitter {
//                         string: &string,
//                         previous_text_node: None, // TODO
//                         subsequent_text_node: None, // TODO
//                     };
//                     let mut seen_any = false;
//                     splitter.iterate_events(|event| {
//                         seen_any = true;
//                         building.push(event);
//                     });
//                 }
//             }
//         }
//     }


    fn new(options: &'a IngestOptions) -> Self {
        QuoteMatcher {
            options,
            stack: vec![]
        }
    }

    // fn pop_to_parent(&mut self) -> Option<()> {
    //     if self.stack.len() < 2 {
    //         return None;
    //     }
    //     let top = self.stack.pop()?;
    //     let last = self.stack.last_mut()?;
    //     last.children.push(top.package(&self.options.quotes));
    //     Some(())
    // }

}

struct QuoteSplitter<'a> {
    string: &'a str,
    previous_text_node: Option<&'a str>,
    subsequent_text_node: Option<&'a str>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Event<'a> {
    Text(&'a str),
    SmartMidwordInvertedComma, // => return self.append_text("\u{2019}"),
    SmartQuoteSingleOpen, // => self.open_tag(MicroTag::SingleQuote),
    SmartQuoteDoubleOpen, // => self.open_tag(MicroTag::DoubleQuote),
    SmartQuoteSingleClose, // => return self.end_tag(Some(MicroTag::SingleQuote)),
    SmartQuoteDoubleClose, // => return self.end_tag(Some(MicroTag::DoubleQuote)),
}

impl<'a> QuoteSplitter<'a> {
    fn iterate_events(&self, mut callback: impl FnMut(Event<'a>)) {
        let mut text_start = 0;
        for (ix, quote_char) in self.iterate_possibles() {
            let mut prefix = &self.string[..ix];
            let mut suffix = &self.string[ix+1..];
            if prefix.is_empty() {
                if let Some(prev) = self.previous_text_node {
                    prefix = prev;
                }
            }
            if suffix.is_empty() {
                if let Some(next) = self.subsequent_text_node {
                    suffix = next;
                }
            }
            if let Some(kind) = quote_kind(quote_char as u8, prefix, suffix) {
                callback(Event::Text(&self.string[text_start..ix]));
                let quote_event = match (kind, quote_char) {
                    (SmartQuoteKind::Open, '\'') => Event::SmartQuoteSingleOpen,
                    (SmartQuoteKind::Close, '\'') => Event::SmartQuoteSingleClose,
                    (SmartQuoteKind::Open, '"') => Event::SmartQuoteDoubleOpen,
                    (SmartQuoteKind::Close, '"') => Event::SmartQuoteDoubleClose,
                    (SmartQuoteKind::Midword, '\'') => Event::SmartMidwordInvertedComma,
                    // Don't parse this as a quote at all
                    _ => continue,
                };
                callback(quote_event);
                text_start = ix + 1;
            }
        }
        if text_start > 0 {
            callback(Event::Text(&self.string[text_start..]));
        }
    }
    // TODO: should this find guillements or localized quote terms? Maybe no.
    fn iterate_possibles(&self) -> impl Iterator<Item = (usize, char)> + 'a {
        self.string
            .char_indices()
            .filter(|(ix, ch)| *ch == '\'' || *ch == '"')
    }
}

#[test]
fn test_quote_splitter_simple() {
    let string = "hello, I'm a man with a plan, \"Canal Panama\".";
    let splitter = QuoteSplitter {
        string,
        previous_text_node: None,
        subsequent_text_node: None,
    };
    let mut events = Vec::new();
    splitter.iterate_events(|event| {
        events.push(event);
    });
    assert_eq!(events, vec![
        Event::Text("hello, I"),
        Event::SmartMidwordInvertedComma,
        Event::Text("m a man with a plan, "),
        Event::SmartQuoteDoubleOpen,
        Event::Text("Canal Panama"),
        Event::SmartQuoteDoubleClose,
        Event::Text("."),
    ]);
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum SmartQuoteKind {
    Open,
    Close,
    Midword,
}

use super::puncttable::is_punctuation;

/// Determines what kind a smart quote should open at this point
fn quote_kind(character: u8, prefix: &str, suffix: &str) -> Option<SmartQuoteKind> {
    let not_italic_ish = |c: &char| { *c != '*' && *c != '~' && *c != '_' && *c != '\'' && *c != '"' };

    // Beginning and end of line == whitespace.
    let next_char = suffix.chars().filter(not_italic_ish).nth(0).unwrap_or(' ');
    let prev_char = prefix.chars().rev().filter(not_italic_ish).nth(0).unwrap_or(' ');

    let next_white = next_char.is_whitespace();
    let prev_white = prev_char.is_whitespace();
    // i.e. braces and the like
    let not_term_punc = |c: char| is_punctuation(c) && c != '.' && c != ',';
    let wordy = |c: char| !is_punctuation(c) && !c.is_whitespace() && !c.is_control();

    if prev_white && next_white {
        None
    } else if prev_white && next_char.is_numeric() && character == b'\'' {
        // '09 -- force a close quote
        Some(SmartQuoteKind::Midword)
    } else if !prev_white && next_white {
        Some(SmartQuoteKind::Close)
    } else if prev_white && !next_white {
        Some(SmartQuoteKind::Open)
    } else if next_white && (prev_char == '.' || prev_char == ',' || prev_char == '!') {
        Some(SmartQuoteKind::Close)
    } else if is_punctuation(prev_char) && not_term_punc(next_char) {
        Some(SmartQuoteKind::Close)
    } else if not_term_punc(prev_char) && is_punctuation(next_char) {
        Some(SmartQuoteKind::Open)
    } else if wordy(prev_char) && wordy(next_char) && character == b'\'' {
        Some(SmartQuoteKind::Midword)
    } else if is_punctuation(prev_char) && wordy(next_char) {
        Some(SmartQuoteKind::Open)
    } else if wordy(prev_char) && is_punctuation(next_char) {
        Some(SmartQuoteKind::Close)
    } else {
        None
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum MicroTag {
    None,
    Italic,
    Bold,
    SmallCaps,
    Sup,
    Sub,
    Underline,
    SingleQuote,
    DoubleQuote,
    NoCase,
}

impl MicroTag {
    fn is_quote(self) -> bool {
        match self {
            MicroTag::SingleQuote | MicroTag::DoubleQuote => true,
            _ => false,
        }
    }

}

#[derive(Debug)]
enum SFQuoteKind {
    Single,
    Double,
    // no midword
}

impl SFQuoteKind {
    fn unmatched_str(&self) -> &'static str {
        match self {
            SFQuoteKind::Single => "'",
            SFQuoteKind::Double => "\"",
        }
    }
}

struct StackFrame {
    kind: SFQuoteKind,
    children: Vec<MicroNode>,
}
