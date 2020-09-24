use smartstring::alias::String;
use super::InlineElement;
use crate::output::micro_html::MicroNode;

#[test]
fn normalise() {
    let mut nodes = vec![
        InlineElement::Text("a".into()),
        InlineElement::Text("b".into()),
    ];
    normalise_text_elements(&mut nodes);
    assert_eq!(&nodes[..], &[InlineElement::Text("ab".into())][..]);
    let mut nodes = vec![
        InlineElement::Micro(MicroNode::parse("a", &Default::default())),
        InlineElement::Micro(MicroNode::parse("b", &Default::default())),
    ];
    normalise_text_elements(&mut nodes);
    assert_eq!(&nodes[..], &[InlineElement::Text("ab".into())][..]);
}

fn smash_string_push(base: &mut String, mut suff: &str) {
    trace!("smash_string_push {:?} <- {:?}", base, suff);
    let suff_trimmed = suff.trim_start();
    if base.trim_end().chars().rev().nth(0).map_or(false, is_punc)
        && suff_trimmed.chars().nth(0).map_or(false, is_punc)
    {
        suff = suff_trimmed;
        let trimmed = base.trim_end();
        if trimmed.chars().rev().nth(0).map_or(false, is_punc) {
            let trimmed_len = trimmed.len();
            base.truncate(trimmed_len);
        }
    }
    let base_len = base.len();
    let suff_len = suff.len();
    let last_width = base.chars().rev().nth(0).map_or(0, |x| x.len_utf8());
    let first_width = suff.chars().nth(0).map_or(0, |x| x.len_utf8());
    let width = last_width + first_width;
    base.push_str(suff);
    // Smash across the boundary
    if base_len > 0 && suff_len > 0 {
        let start = base_len - last_width;
        let range = start..start + width;
        if let Some(Some(replacement)) = FULL_MONTY_PLAIN.get(&base.as_bytes()[range.clone()]) {
            trace!(
                "smash_string_push REPLACING {:?} with {:?}",
                &base[range.clone()],
                *replacement
            );
            base.replace_range(range, *replacement);
        }
    }
}

fn smash_just_punc(base: &mut String, suff: &mut String) {
    trace!("smash_just_punc {:?} <- {:?}", base, suff);
    let mut suff_append: &str = suff.as_ref();
    let suff_trimmed = suff_append.trim_start();
    if base.trim_end().chars().rev().nth(0).map_or(false, is_punc)
        && suff_trimmed.chars().nth(0).map_or(false, is_punc)
    {
        suff_append = suff_trimmed;
        let trimmed = base.trim_end();
        if trimmed.chars().rev().nth(0).map_or(false, is_punc) {
            let trimmed_len = trimmed.len();
            base.truncate(trimmed_len)
        }
    }
    let base_len = base.len();
    let suff_len = suff.len();
    let last_width = base.chars().rev().nth(0).map_or(0, |x| x.len_utf8());
    let first_width = suff.chars().nth(0).map_or(0, |x| x.len_utf8());
    let width = last_width + first_width;
    base.push_str(&suff[..first_width]);
    // Smash across the boundary
    if base_len > 0 && suff_len > 0 {
        let start = base_len - last_width;
        let range = start..start + width;
        if let Some(Some(replacement)) = FULL_MONTY_PLAIN.get(&base.as_bytes()[range.clone()]) {
            trace!(
                "smash_just_punc REPLACING {:?} with {:?}",
                &base[range.clone()],
                *replacement
            );
            base.replace_range(range, *replacement);
            suff.replace_range(..first_width, "");
        } else {
            base.truncate(base_len);
        }
    }
}

impl InlineElement {
    // If returns Some, insert the string before the micro.
    fn normalise_micro_single_text(&mut self) -> Option<Vec<InlineElement>> {
        if let InlineElement::Micro(ref mut m) = self {
            let len = m.len();
            if len == 1 {
                if let Some(text) = m.first_mut().unwrap().take_text() {
                    drop(m);
                    *self = InlineElement::Text(text);
                    return Some(Vec::new());
                }
            } else if len > 1 {
                let mut vec = vec![];
                while let Some(text) = m.first_mut().unwrap().take_text() {
                    m.remove(0);
                    vec.push(InlineElement::Text(text));
                }
                if vec.len() > 0 {
                    return Some(vec);
                }
            }
        }
        None
    }
}

pub fn normalise_text_elements(slice: &mut Vec<InlineElement>) {
    let mut ix = 0;
    for inl in slice.iter_mut() {
        match inl {
            InlineElement::Micro(micros) => normalise_text_elements_micro(micros),
            _ => {}
        }
    }
    while ix < slice.len().saturating_sub(1) {
        let mut pop_tail = false;
        if let Some(head) = slice.get_mut(ix) {
            if let Some(before_head) = head.normalise_micro_single_text() {
                drop(head);
                drop(slice.splice(ix..ix, before_head.into_iter()));
                continue;
            }
        }
        if let Some(tail_head) = slice.get_mut(ix + 1) {
            if let Some(before) = tail_head.normalise_micro_single_text() {
                drop(tail_head);
                drop(slice.splice(ix + 1..ix + 1, before.into_iter()));
                continue;
            }
        }
        if let Some((head, tail)) = (&mut slice[ix..]).split_first_mut() {
            if let Some(head_2) = tail.first_mut() {
                match (head, head_2) {
                    (InlineElement::Text(ref mut s), InlineElement::Text(s2)) => {
                        smash_string_push(s, &s2);
                        pop_tail = true;
                    }
                    (InlineElement::Formatted(children, _), InlineElement::Text(s2)) => {
                        match children.last_mut().and_then(find_string_right_f) {
                            Some(s1) => smash_just_punc(s1, s2),
                            None => {}
                        }
                    }
                    (InlineElement::Formatted(children, _), InlineElement::Micro(ms2)) => {
                        trace!("formatted, micro");
                        match children.last_mut().and_then(find_string_right_f) {
                            Some(s1) => match ms2.first_mut().and_then(find_string_left_micro) {
                                Some(s2) => smash_just_punc(s1, s2),
                                None => {}
                            },
                            None => {}
                        }
                    }
                    (InlineElement::Micro(ref mut ms), InlineElement::Micro(ref mut ms2)) => {
                        // Only join if it doesn't end with a quoted
                        if ms.last().map_or(false, |x| match x {
                            MicroNode::Text(_) => true,
                            _ => false,
                        }) {
                            ms.extend(ms2.drain(..));
                            pop_tail = true;
                        }
                    }
                    _ => {}
                }
            }
        }
        if pop_tail {
            slice.remove(ix + 1);
            continue;
        }
        ix += 1;
    }
    for inl in slice.iter_mut() {
        match inl {
            InlineElement::Quoted { inlines, .. }
            | InlineElement::Div(_, inlines)
            | InlineElement::Anchor {
                content: inlines, ..
            }
            | InlineElement::Formatted(inlines, _) => normalise_text_elements(inlines),
            InlineElement::Micro(micros) => normalise_text_elements_micro(micros),
            _ => {}
        }
    }
}

pub fn normalise_text_elements_micro(slice: &mut Vec<MicroNode>) {
    let mut ix = 0;
    while ix < slice.len().saturating_sub(1) {
        let mut pop_tail = false;
        if let Some((head, tail)) = (&mut slice[ix..]).split_first_mut() {
            if let Some(head_2) = tail.first_mut() {
                match (head, head_2) {
                    (MicroNode::Text(ref mut s), MicroNode::Text(s2)) => {
                        smash_string_push(s, &s2);
                        pop_tail = true;
                    }
                    _ => {}
                }
            }
        }
        if pop_tail {
            slice.remove(ix + 1);
        } else {
            ix += 1;
        }
    }
    for inl in slice.iter_mut() {
        match inl {
            MicroNode::Quoted { children, .. }
            | MicroNode::NoDecor(children)
            | MicroNode::NoCase(children)
            | MicroNode::Formatted(children, _) => normalise_text_elements_micro(children),
            _ => {}
        }
    }
}

enum Motion {
    RemovedAndRetry(usize),
    RemovedNoChanges(usize),
}

// Basically, affixes go outside Quoted elements. So we can just look for text elements that come
// right after quoted ones.
pub fn move_punctuation(slice: &mut Vec<InlineElement>, punctuation_in_quote: Option<bool>) {
    trace!("move_punctuation {:?} {:?}", slice, punctuation_in_quote);
    normalise_text_elements(slice);

    if slice.len() > 1 {
        // Basically windows(2)/peekable() iteration, but &mut.
        let mut len = slice.len();
        let mut ix = 0;
        while ix < len - 1 {
            // It's not that a style can have "no piq moving", it's just that we don't do it
            // until the very end, so we need to disable it until producing the final cluster
            // output
            let mut new_ix = ix + 1;
            if let Some(piq) = punctuation_in_quote {
                if let Some(motion) = move_around_quote(slice, ix, piq) {
                    match motion {
                        Motion::RemovedAndRetry(removed) => {
                            len -= removed;
                            new_ix = ix;
                        }
                        Motion::RemovedNoChanges(removed) => {
                            len -= removed;
                        }
                    }
                }
            }
            ix = new_ix;
        }
    }

    // recurse manually over the 0 or 1 items in it, and their children
    for inl in slice.iter_mut() {
        match inl {
            InlineElement::Quoted { inlines, .. }
            | InlineElement::Div(_, inlines)
            | InlineElement::Anchor {
                content: inlines, ..
            }
            | InlineElement::Formatted(inlines, _) => {
                move_punctuation(inlines, punctuation_in_quote)
            }
            _ => {}
        }
    }
}

fn can_move_in(ch: char) -> bool {
    is_punc(ch) && ch != ':' && ch != ';'
}

fn can_move_out(ch: char) -> bool {
    is_punc(ch) && ch != '.' && ch != '!' && ch != '?'
}

/// Return value = how many extra inlines were consumed by moving al the text out and then being
/// removed.
fn move_around_quote(els: &mut Vec<InlineElement>, ix: usize, piq: bool) -> Option<Motion> {
    debug!("move_around_quote {:?} {:?} {:?}", els.get(ix), ix, piq);
    if let Some(mut insertion_point) = find_right_quote(els, ix, piq) {
        // Last element burrowed down to a right quotation mark
        let mut has_two_puncs = None;
        let mut outside_char = {
            let suffix = insertion_point.next_string_mut()?;

            if let Some(first) = suffix.chars().nth(0) {
                if let Some(second) = suffix.chars().nth(1) {
                    if is_punc(first) && is_punc(second) {
                        has_two_puncs = Some(second)
                    }
                }
                first as char
            } else {
                // the string is empty! let's fix it just because;
                warn!("found empty string in move_punctuation");
                drop(suffix);
                return None;
            }
        };

        let mut inside_char = {
            // Will always be Some, as we established this with ends_with_punctuation()
            let insert = insertion_point.last_string_mut()?;
            insert.chars().rev().nth(0)?
        };

        let mut pop_count = 1;
        let mut out_remove_count = 1;
        let mut inside_authentic = true;

        if !is_punc(inside_char) && !is_punc(outside_char) {
            return None;
        } else if !is_punc(inside_char) {
            if let Some(second) = has_two_puncs {
                inside_authentic = false;
                pop_count = 0;
                out_remove_count = 2;
                inside_char = outside_char;
                outside_char = second;
            // Continue onwards
            } else if piq && can_move_in(outside_char) {
                {
                    let insert = insertion_point.last_string_mut()?;
                    insert.push(outside_char);
                }
                {
                    let outside = insertion_point.next_string_mut()?;
                    outside.remove(0);
                }
                return Some(Motion::RemovedAndRetry(remove_empty_left(els, ix + 1)));
            }
        } else if !is_punc(outside_char) {
            if !piq && can_move_out(inside_char) {
                {
                    let insert = insertion_point.last_string_mut()?;
                    insert.pop();
                }
                {
                    let outside = insertion_point.next_string_mut()?;
                    outside.insert(0, inside_char);
                }
                return Some(Motion::RemovedAndRetry(remove_empty_left(els, ix + 1)));
            }
        }

        // No panics here because all the punctuation characters are ASCII
        let bytes: [u8; 2] = [inside_char as u8, outside_char as u8];

        // XXX: this shouldn't examine characters from inside a quote (i.e. in the original field);
        // it should look at sequences of characters in a row that appear next to a quote.

        debug!("looking up [{:?}, {:?}]", inside_char, outside_char);

        let result = if piq {
            QUOTES_BOTH_PUNC_IN.get(&bytes[..])
        } else {
            QUOTES_BOTH_PUNC_OUT.get(&bytes[..])
        }?;

        match *result {
            Where::In(in_str) => {
                {
                    let insert = insertion_point.last_string_mut()?;
                    for _ in 0..pop_count {
                        insert.pop();
                    }
                    insert.push_str(in_str);
                }
                {
                    let outside = insertion_point.next_string_mut()?;
                    for _ in 0..out_remove_count {
                        outside.remove(0);
                    }
                }
                return Some(Motion::RemovedAndRetry(remove_empty_left(els, ix + 1)));
            }
            Where::Out(in_str) => {
                // magic_PunctuationInQuoteFalseSuppressExtra.txt -- the ? came from a field, i.e.
                // inside a parsed MicroNode, so . This is approximate, has a false positive for
                // question marks from style-provided text nodes inside InlineElement::Quoted. But
                // those are pretty rare, so don't bother for now. (Only appears in one style:
                // universite-du-quebec-a-montreal.csl)
                let moves_out = !inside_authentic || can_move_out(inside_char);
                if moves_out {
                    let insert = insertion_point.last_string_mut()?;
                    for _ in 0..pop_count {
                        insert.pop();
                    }
                }
                {
                    let outside = insertion_point.next_string_mut()?;
                    for _ in 0..out_remove_count {
                        outside.remove(0);
                    }
                    if moves_out {
                        outside.insert_str(0, in_str);
                    } else {
                        // don't include the first char
                        outside.insert_str(0, &in_str[1..]);
                    }
                }
                drop(insertion_point);
                return None;
            }
            Where::Split(inn, out) => {
                {
                    let insert = insertion_point.last_string_mut()?;
                    for _ in 0..pop_count {
                        insert.pop();
                    }
                    insert.push(inn);
                }
                {
                    let outside = insertion_point.next_string_mut()?;
                    for _ in 0..out_remove_count {
                        outside.remove(0);
                    }
                    outside.insert(0, out);
                }
                drop(insertion_point);
                return None;
            }
        }
    }
    None
}

use phf::phf_map;

enum Where {
    // Leave no punctuation on the right of the quote, and replace the last char inside with this
    In(&'static str),
    // Leave no punctuation inside the quote, and replace the char on the right with this
    Out(&'static str),
    Split(char, char),
}

pub fn is_punc(c: char) -> bool {
    c == '.' || c == ',' || c == '!' || c == '?' || c == ';' || c == ':'
}

fn is_punc_space(c: char) -> bool {
    is_punc(c) || c.is_whitespace()
}

fn find_string_left_micro(m: &mut MicroNode) -> Option<&mut String> {
    match m {
        MicroNode::Text(string) => Some(string),
        MicroNode::NoDecor(nodes) | MicroNode::NoCase(nodes) | MicroNode::Formatted(nodes, _) => {
            nodes.first_mut().and_then(find_string_left_micro)
        }
        _ => None,
    }
}

fn find_string_left(next: &mut InlineElement) -> Option<&mut String> {
    match next {
        InlineElement::Text(ref mut string) => Some(string),
        InlineElement::Micro(ref mut micros) => micros.first_mut().and_then(find_string_left_micro),
        InlineElement::Quoted { .. } => None,
        _ => None,
    }
}

/// Allows finding formatting?
fn find_string_right_f(next: &mut InlineElement) -> Option<&mut String> {
    match next {
        InlineElement::Text(ref mut string) => Some(string),
        InlineElement::Micro(ref mut micros) => {
            micros.last_mut().and_then(find_string_right_f_micro)
        }
        InlineElement::Formatted(children, _) => children.last_mut().and_then(find_string_right_f),
        InlineElement::Quoted { .. } => None,
        _ => None,
    }
}

fn find_string_right_f_micro(m: &mut MicroNode) -> Option<&mut String> {
    match m {
        MicroNode::Text(string) => Some(string),
        MicroNode::NoDecor(nodes) | MicroNode::NoCase(nodes) | MicroNode::Formatted(nodes, _) => {
            nodes.last_mut().and_then(find_string_right_f_micro)
        }
        _ => None,
    }
}

fn remove_empty_left(els: &mut Vec<InlineElement>, ix: usize) -> usize {
    fn should_remove(node: &mut InlineElement) -> bool {
        match node {
            InlineElement::Text(s) => s.is_empty(),
            InlineElement::Micro(m) => {
                remove_empty_left_micro(m);
                m.is_empty()
            }
            _ => false,
        }
    }
    fn should_remove_micro(node: &mut MicroNode) -> bool {
        match node {
            MicroNode::Text(s) => s.is_empty(),
            _ => false,
        }
    }
    fn remove_empty_left_micro(els: &mut Vec<MicroNode>) {
        while !els.is_empty() {
            if should_remove_micro(&mut els[0]) {
                els.remove(0);
            } else {
                return;
            }
        }
    }
    let mut total = 0;
    while els.len() > ix {
        if should_remove(&mut els[ix]) {
            total += 1;
            els.remove(ix);
        } else {
            return total;
        }
    }
    total
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

fn find_right_quote<'a>(
    els: &'a mut Vec<InlineElement>,
    ix: usize,
    punctuation_in_quote: bool,
) -> Option<RightQuoteInsertionPoint<'a>> {
    // outside needs to return OutsideInline/OutsideMicro which need to append to the vector that
    // contains a Quoted, rather than the vector inside it. These variants still have access to the
    // quoted, through find_right_quote_inside(els.get_mut(ix)) later on
    // if punctuation_in_quote {
    (&mut els[ix..])
        .split_first_mut()
        .and_then(|(this_last, rest)| {
            let next = rest.first_mut()?;
            find_string_left(next)
                .and_then(move |suffix| find_right_quote_inside(this_last, suffix))
        })
    // } else {
    //     (&mut els[ix..])
    //         .split_first_mut()
    //         .and_then(|(this_last, rest)| {
    //             let next = rest.first_mut()?;
    //             find_string_left(next).and_then(move |suffix| find_right_quote_inside(this_last, suffix))
    //         })
    //     find_right_quote_outside(els, ix)
    // }
}

fn find_right_quote_inside<'a>(
    el: &'a mut InlineElement,
    next: &'a mut String,
) -> Option<RightQuoteInsertionPoint<'a>> {
    match el {
        InlineElement::Quoted { inlines, .. } => {
            // prefer to dive deeper, and catch "'inner quotes,'" too.
            // See below re unsafe
            if !inlines.is_empty() {
                let len = inlines.len();
                let next = unsafe { &mut *(next as *mut String) };
                let last_mut = unsafe { &mut (*((inlines) as *mut Vec<InlineElement>))[len - 1] };
                let deeper = find_right_quote_inside(last_mut, next);
                if deeper.is_some() {
                    return deeper;
                }
            }
            Some(RightQuoteInsertionPoint::InsideInline(inlines, next))
        }
        InlineElement::Micro(micros) => micros
            .last_mut()
            .and_then(move |x| find_right_quote_inside_micro(x, next)),
        InlineElement::Div(_, inlines) | InlineElement::Formatted(inlines, _) => inlines
            .last_mut()
            .and_then(move |x| find_right_quote_inside(x, next)),
        _ => None,
    }
}

fn find_right_quote_inside_micro<'b>(
    micro: &'b mut MicroNode,
    next: &'b mut String,
) -> Option<RightQuoteInsertionPoint<'b>> {
    match micro {
        MicroNode::Quoted {
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
                let next = unsafe { &mut *(next as *mut String) };
                let last_mut = unsafe { &mut (*((children) as *mut Vec<MicroNode>))[len - 1] };
                let deeper = find_right_quote_inside_micro(last_mut, next);
                if deeper.is_some() {
                    return deeper;
                }
            }
            Some(RightQuoteInsertionPoint::InsideMicro(children, next))
        }
        // Dive into formatted bits
        MicroNode::NoDecor(nodes) | MicroNode::NoCase(nodes) | MicroNode::Formatted(nodes, _) => {
            nodes
                .last_mut()
                .and_then(move |x| find_right_quote_inside_micro(x, next))
        }
        _ => None,
    }
}

/// "Insertion" == push to one of these vectors.
#[derive(Debug)]
enum RightQuoteInsertionPoint<'a> {
    InsideInline(&'a mut Vec<InlineElement>, &'a mut String),
    InsideMicro(&'a mut Vec<MicroNode>, &'a mut String),
    OutsideInline {
        list: &'a mut Vec<InlineElement>,
        quoted_index: usize,
    },
    OutsideMicro {
        list: &'a mut Vec<MicroNode>,
        quoted_index: usize,
    },
}

impl RightQuoteInsertionPoint<'_> {
    fn insert_smushed(&mut self, smushed: &str) {
        match self {
            // "quoted" => "quoted,"
            RightQuoteInsertionPoint::InsideInline(..)
            | RightQuoteInsertionPoint::InsideMicro(..) => {
                if let Some(last_string) = self.last_string_mut() {
                    last_string.push_str(smushed);
                }
            }
            // "quoted" => "quoted",
            RightQuoteInsertionPoint::OutsideInline { list, quoted_index } => {
                // if let Some(next) = self.next_string_mut() {
                //     next.insert_str(0, smushed)
                // }
                list.insert(*quoted_index + 1, InlineElement::Text(smushed.into()));
            }
            RightQuoteInsertionPoint::OutsideMicro { list, quoted_index } => {
                list.insert(*quoted_index + 1, MicroNode::Text(smushed.into()));
            }
        }
    }
    fn last_string_mut(&mut self) -> Option<&mut String> {
        match self {
            // e.g. "quoted inlines;" => ';'
            RightQuoteInsertionPoint::InsideInline(inlines, _) => last_string(inlines),
            RightQuoteInsertionPoint::InsideMicro(micros, _) => last_string_micro(micros),
            // very similar; e.g. "quoted;" => ';'
            RightQuoteInsertionPoint::OutsideInline { list, quoted_index } => {
                last_string(&mut list[..*quoted_index])
            }
            RightQuoteInsertionPoint::OutsideMicro { list, quoted_index } => {
                last_string_micro(&mut list[..*quoted_index])
            }
        }
    }
    fn next_string_mut(&mut self) -> Option<&mut String> {
        match self {
            // e.g. "quoted inlines;" => ';'
            RightQuoteInsertionPoint::InsideMicro(_, string)
            | RightQuoteInsertionPoint::InsideInline(_, string) => Some(string),
            // very similar; e.g. "quoted;" => ';'
            RightQuoteInsertionPoint::OutsideInline { list, quoted_index } => {
                list.get_mut(*quoted_index).and_then(find_string_left)
            }
            RightQuoteInsertionPoint::OutsideMicro { list, quoted_index } => {
                list.get_mut(*quoted_index).and_then(find_string_left_micro)
            }
        }
    }
}

fn last_string(is: &mut [InlineElement]) -> Option<&mut String> {
    is.last_mut().and_then(|i| match i {
        InlineElement::Micro(micros) => last_string_micro(micros),
        InlineElement::Quoted { inlines, .. }
        | InlineElement::Div(_, inlines)
        | InlineElement::Formatted(inlines, _) => last_string(inlines),
        InlineElement::Text(string) => Some(string),
        _ => None,
    })
}

fn last_string_micro(ms: &mut [MicroNode]) -> Option<&mut String> {
    ms.last_mut().and_then(|m| match m {
        MicroNode::Quoted { children, .. }
        | MicroNode::NoDecor(children)
        | MicroNode::NoCase(children)
        | MicroNode::Formatted(children, _) => last_string_micro(children),
        MicroNode::Text(string) => Some(string),
    })
}

pub fn append_suffix(pre_and_content: &mut Vec<InlineElement>, suffix: Vec<MicroNode>) {
    // if let Some(last) = pre_and_content.last_mut() {
    //     // Must be followed by some text
    //     if let Some(string) = suffix.first_mut().and_then(find_string_left_micro) {
    //         move_around_quote(last, string);
    //     }
    // }
    // Do punctuation moves later; simply avoid doing anything while the inlines could still be
    // turned into disamb tokens
    pre_and_content.push(InlineElement::Micro(suffix));
}

/// From `punctuation_FullMontyQuotesIn.txt`
static QUOTES_BOTH_PUNC_IN: phf::Map<&'static [u8], Where> = phf_map! {
    // Colon
    b"::" => Where::Out(":"),
    b".:" => Where::Split('.', ':'),
    b";:" => Where::Out(";"),
    b"!:" => Where::In("!"),
    b"?:" => Where::In("?"),
    b",:" => Where::Split(',', ':'),
    // Period
    b":." => Where::Out(":"),
    b".." => Where::In("."),
    b";." => Where::Out(";"),
    b"!." => Where::In("!"),
    b"?." => Where::In("?"),
    b",." => Where::In(",."),
    // Semicolon
    b":;" => Where::Out(":;"),
    b".;" => Where::Split('.', ';'),
    b";;" => Where::Out(";"),
    b"!;" => Where::Split('!', ';'),
    b"?;" => Where::Split('?', ';'),
    b",;" => Where::Split(',', ';'),
    // Exclamation
    b":!" => Where::In("!"),
    b".!" => Where::In(".!"),
    b";!" => Where::In("!"),
    b"!!" => Where::In("!"),
    b"?!" => Where::In("?!"),
    b",!" => Where::In(",!"),
    // Question
    b":?" => Where::In("?"),
    b".?" => Where::In(".?"),
    b";?" => Where::In("?"),
    b"!?" => Where::In("!?"),
    b"??" => Where::In("?"),
    b",?" => Where::In(",?"),
    // Comma
    b":," => Where::Out(":,"),
    b".," => Where::In(".,"),
    b";," => Where::Out(";,"),
    b"!," => Where::In("!,"),
    b"?," => Where::In("?,"),
    b",," => Where::In(","),
};

/// From `punctuation_FullMontyQuotesIn.txt`
static QUOTES_BOTH_PUNC_OUT: phf::Map<&'static [u8], Where> = phf_map! {
    // Colon
    b"::" => Where::Out(":"),
    b".:" => Where::Out(".:"),
    b";:" => Where::Out(";"),
    b"!:" => Where::Out("!"),
    b"?:" => Where::Out("?"),
    b",:" => Where::Out(",:"),
    // Period
    b":." => Where::Out(":"),
    b".." => Where::Out("."),
    b";." => Where::Out(";"),
    b"!." => Where::Out("!"),
    b"?." => Where::Out("?"),
    b",." => Where::Out(",."),
    // Semicolon
    b":;" => Where::Out(":;"),
    b".;" => Where::Out(".;"),
    b";;" => Where::Out(";"),
    b"!;" => Where::Out("!;"),
    b"?;" => Where::Out("?;"),
    b",;" => Where::Out(",;"),
    // Exclamation
    b":!" => Where::Out("!"),
    b".!" => Where::Out(".!"),
    b";!" => Where::Out("!"),
    b"!!" => Where::Out("!"),
    b"?!" => Where::Out("?!"),
    b",!" => Where::Out(",!"),
    // Question
    b":?" => Where::Out("?"),
    b".?" => Where::Out(".?"),
    b";?" => Where::Out("?"),
    b"!?" => Where::Out("!?"),
    b"??" => Where::Out("?"),
    b",?" => Where::Out(",?"),
    // Comma
    b":," => Where::Out(":,"),
    b".," => Where::Out(".,"),
    b";," => Where::Out(";,"),
    b"!," => Where::Out("!,"),
    b"?," => Where::Out("?,"),
    b",," => Where::Out(","),
};

/// From `punctuation_FullMontyPlain.txt` and `punctuation_FullMontyField.txt`,
/// which have identical output. If None, do nothing.
static FULL_MONTY_PLAIN: phf::Map<&'static [u8], Option<&'static str>> = phf_map! {
    // Spaces (a0 is nbsp)
    b"  " => Some(" "),
    b"\xC2\xA0 " => Some("\u{a0}"),
    b" \xC2\xA0" => Some("\u{a0}"),

    // Colon
    b"::" => Some(":"),
    b".:" => None,
    b";:" => Some(";"),
    b"!:" => Some("!"),
    b"?:" => Some("?"),
    b",:" => None,
    // Period
    b":." => Some(":"),
    b".." => Some("."),
    b";." => Some(";"),
    b"!." => Some("!"),
    b"?." => Some("?"),
    b",." => Some(",."),
    // Semicolon
    b":;" => None,
    b".;" => None,
    b";;" => Some(";"),
    b"!;" => None,
    b"?;" => None,
    b",;" => None,
    // Exclamation
    b":!" => Some("!"),
    b".!" => None,
    b";!" => Some("!"),
    b"!!" => Some("!"),
    b"?!" => None,
    b",!" => None,
    // Question
    b":?" => Some("?"),
    b".?" => None,
    b";?" => Some("?"),
    b"!?" => None,
    b"??" => Some("?"),
    b",?" => Some(",?"),
    // Comma
    b":," => None,
    b".," => None,
    b";," => None,
    b"!," => None,
    b"?," => None,
    b",," => Some(","),
};
