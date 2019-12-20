use super::InlineElement;
use crate::output::micro_html::MicroNode;

pub fn append_suffix(pre_and_content: &mut Vec<InlineElement>, mut suffix: Vec<MicroNode>) {
    if let Some(last) = pre_and_content.last_mut() {
        // Must be followed by some text
        if let Some(string) = suffix.first_mut().and_then(find_string_left_micro) {
            append_suffix_inner(last, string);
        }
    }
    pre_and_content.push(InlineElement::Micro(suffix));
}

pub fn append_suffix_inner(last: &mut InlineElement, suffix: &mut String) -> Option<()> {
    debug!("append_suffix_inner {:?} {:?}", last, suffix);
    if let Some(mut insertion_point) = find_right_quote(last, /*XXX*/ true) {
        // Last element burrowed down to a right quotation mark

        // That text must be is_punc
        if !suffix.chars().nth(0).map_or(false, is_punc) {
            return None;
        }

        // O(n), but n tends to be 2, like with ", " so this is ok
        let c = suffix.remove(0);

        // "Something?," is bad, so stop at removing it from the ", "
        if let Some(ch) = insertion_point.ends_with_punctuation() {
            return None;
        }

        // Will always be Some, as we established this with ends_with_punctuation()
        let insert = insertion_point.last_string_mut()?;
        insert.push(c);
    } else {
    }
    Some(())
}

fn is_punc(c: char) -> bool {
    c == '.' || c == ',' || c == '!' || c == '?' || c == ';' || c == ':'
}

fn is_punc_space(c: char) -> bool {
    is_punc(c) || c.is_whitespace()
}

fn find_string_left_micro(m: &mut MicroNode) -> Option<&mut String> {
    match m {
        MicroNode::Text(string) => Some(string),
        MicroNode::NoCase(nodes) | MicroNode::Formatted(nodes, _) => {
            nodes.first_mut().and_then(find_string_left_micro)
        }
        _ => None,
    }
}

fn find_string_left(next: &mut InlineElement) -> Option<&mut String> {
    match next {
        InlineElement::Text(ref mut string) => Some(string),
        InlineElement::Micro(ref mut micros) => micros.first_mut().and_then(find_string_left_micro),
        _ => None,
    }
}

// Basically, affixes go outside Quoted elements. So we can just look for text elements that come
// right after quoted ones.
pub fn move_punctuation(slice: &mut [InlineElement], punctuation_in_quote: Option<bool>) {
    if slice.len() >= 2 {
        // Basically windows(2)/peekable() iteration, but &mut.
        let len = slice.len();
        for i in 0..len - 1 {
            if let Some((this_last, rest)) = (&mut slice[i..]).split_first_mut() {
                let next = rest
                    .first_mut()
                    .expect("only iterated to len-1, so infallible");

                // Must be followed by some text
                let string = if let Some(x) = find_string_left(next) {
                    x
                } else {
                    continue;
                };

                // must start with punctuation, e.g. ", "
                if !string.chars().nth(0).map_or(false, is_punc) {
                    continue;
                }

                if let Some(piq) = punctuation_in_quote {
                    append_suffix_inner(this_last, string);
                }
            }
        }
    } else {
        // recurse manually over the 0 or 1 items in it, and their children
        for inl in slice.iter_mut() {
            match inl {
                InlineElement::Quoted { inlines, .. }
                | InlineElement::Div(_, inlines)
                | InlineElement::Formatted(inlines, _) => move_punctuation(inlines, punctuation_in_quote),
                _ => {}
            }
        }
    }
}

// The following functions search inwards, right-leaning, through formatting and as many quotes as possible.
//
// We're trying to find | in these, from $:
//
// "Quoted|"$
// <i>"Quoted|"</i>$
// <i>"'Quoted|'"</i>$
//
// Additionally, we are trying to avoid doubling up. If there's already punctuation right before |,
// don't actually insert anything.

fn find_right_quote<'a>(el: &'a mut InlineElement, punctuation_in_quote: bool) -> Option<RightQuoteInsertionPoint<'a>> {
    if punctuation_in_quote {
        find_right_quote_inside(el)
    } else {
        find_right_quote_outside(el)
    }
}

fn find_right_quote_inside<'a>(el: &'a mut InlineElement) -> Option<RightQuoteInsertionPoint<'a>> {
    match el {
        InlineElement::Quoted { inlines, .. } => {
            // prefer to dive deeper, and catch "'inner quotes,'" too.
            // See below re unsafe
            if !inlines.is_empty() {
                let len = inlines.len();
                let last_mut =
                    unsafe { &mut (*((inlines) as *mut Vec<InlineElement>))[len - 1] };
                let deeper = find_right_quote_inside(last_mut);
                if deeper.is_some() {
                    return deeper;
                }
            }
            Some(RightQuoteInsertionPoint::InsideInline(inlines))
        }
        InlineElement::Micro(micros) => micros.last_mut().and_then(find_right_quote_inside_micro),
        InlineElement::Div(_, inlines) | InlineElement::Formatted(inlines, _) => {
            inlines.last_mut().and_then(find_right_quote_inside)
        }
        _ => None,
    }
}

fn find_right_quote_inside_micro<'b>(micro: &'b mut MicroNode) -> Option<RightQuoteInsertionPoint<'b>> {
    match micro {
        MicroNode::Quoted {
            localized,
            children,
            ..
        } => {
            // prefer to dive deeper, and catch "'inner quotes,'" too.
            // This is a limitation of NLL borrowck analysis at the moment, but will be
            // solved with Polonius: https://users.rust-lang.org/t/solved-borrow-doesnt-drop-returning-this-value-requires-that/24182
            //
            // The unsafe is casting a vec to itself; it's safe.
            //
            // let deeper = children.last_mut().and_then(find_right_quote_inside_micro);
            // if deeper.is_some() {
            //     return deeper;
            // }
            if !children.is_empty() {
                let len = children.len();
                let last_mut = unsafe { &mut (*((children) as *mut Vec<MicroNode>))[len - 1] };
                let deeper = find_right_quote_inside_micro(last_mut);
                if deeper.is_some() {
                    return deeper;
                }
            }
            Some(RightQuoteInsertionPoint::InsideMicro(children))
        }
        // Dive into formatted bits
        MicroNode::NoCase(nodes) | MicroNode::Formatted(nodes, _) => {
            nodes.last_mut().and_then(find_right_quote_inside_micro)
        }
        _ => None,
    }
}

fn find_right_quote_outside<'a>(el: &'a mut InlineElement) -> Option<RightQuoteInsertionPoint<'a>> {
    warn!("not implemented: find_right_quote_outside");
    None
}

/// "Insertion" == push to one of these vectors.
enum RightQuoteInsertionPoint<'a> {
    InsideInline(&'a mut Vec<InlineElement>),
    InsideMicro(&'a mut Vec<MicroNode>),
    OutsideInline {
        list: &'a mut Vec<InlineElement>,
        quoted_index: usize
    },
    OutsideMicro {
        list: &'a mut Vec<MicroNode>,
        quoted_index: usize
    },
}

impl RightQuoteInsertionPoint<'_> {
    fn ends_with_punctuation(&self) -> Option<char> {
        match self {
            RightQuoteInsertionPoint::InsideInline(inlines) => {
                inlines.last().and_then(ends_with_punctuation)
            }
            RightQuoteInsertionPoint::InsideMicro(micros) => {
                micros.last().and_then(ends_with_punctuation_micro)
            }
            RightQuoteInsertionPoint::OutsideInline { list, quoted_index } => {
                list.get(*quoted_index).and_then(ends_with_punctuation)
            }
            RightQuoteInsertionPoint::OutsideMicro { list, quoted_index } => {
                list.get(*quoted_index).and_then(ends_with_punctuation_micro)
            }
        }
    }
    fn last_string_mut(&mut self) -> Option<&mut String> {
        match self {
            // e.g. "quoted inlines;" => ';'
            RightQuoteInsertionPoint::InsideInline(inlines) => {
                last_string(inlines)
            }
            RightQuoteInsertionPoint::InsideMicro(micros) => {
                last_string_micro(micros)
            }
            // very similar; e.g. "quoted;" => ';'
            RightQuoteInsertionPoint::OutsideInline { list, quoted_index } => {
                last_string(&mut list[..*quoted_index])
            }
            RightQuoteInsertionPoint::OutsideMicro { list, quoted_index } => {
                last_string_micro(&mut list[..*quoted_index])
            }
        }
    }
}

fn last_string(is: &mut [InlineElement]) -> Option<&mut String> {
    is.last_mut().and_then(|i| match i {
        InlineElement::Micro(micros) => last_string_micro(micros),
        InlineElement::Quoted { inlines, .. }
        | InlineElement::Div(_, inlines)
        | InlineElement::Formatted(inlines, _) => {
            last_string(inlines)
        }
        InlineElement::Text(string) => Some(string),
        _ => None,
    })
}

fn last_string_micro(ms: &mut [MicroNode]) -> Option<&mut String> {
    ms.last_mut().and_then(|m| match m {
        MicroNode::Quoted { children, .. }
        | MicroNode::NoCase(children)
        | MicroNode::Formatted(children, _) => {
            last_string_micro(children)
        }
        MicroNode::Text(string) => Some(string),
    })
}

fn punc_some(c: char) -> Option<char> {
    if is_punc(c) {
        Some(c)
    } else {
        None
    }
}

fn ends_with_punctuation(i: &InlineElement) -> Option<char> {
    match i {
        InlineElement::Micro(micros) => micros.last().and_then(ends_with_punctuation_micro),
        InlineElement::Quoted { inlines, .. }
        | InlineElement::Div(_, inlines)
        | InlineElement::Formatted(inlines, _) => {
            inlines.last().and_then(ends_with_punctuation)
        }
        InlineElement::Text(string) => string.chars().last().and_then(punc_some),
        _ => None,
    }
}

fn ends_with_punctuation_micro(i: &MicroNode) -> Option<char> {
    match i {
        MicroNode::Quoted { children, .. }
        | MicroNode::NoCase(children)
        | MicroNode::Formatted(children, _) => {
            children.last().and_then(ends_with_punctuation_micro)
        }
        MicroNode::Text(string) => string.chars().last().and_then(punc_some)
    }
}
