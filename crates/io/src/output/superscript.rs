use super::micro_html::MicroNode;
use super::FormatCmd;
use crate::String;

#[derive(Debug, Copy, Clone)]
enum SupSub {
    Super(&'static str),
    Sub(&'static str),
    Normal,
}

/// Takes a string slice with unicode superscript or subscript characters in it, and turns each subslice of them
/// into an superscript or subscript tag, with the contents being the decomposed characters without the sup/sub
/// property. So you can take input like '1ʳᵉ' and get `["1", "<sup>re</sup>"]` as a list of
/// `MicroNode`.
pub fn parse_sup_sub(slice: &str) -> Vec<MicroNode> {
    let mut stack = Vec::new();
    #[derive(Debug, Clone)]
    enum Current {
        // start, len
        Normal(usize, usize),
        // start, len
        Super(String),
        // start, len
        Sub(String),
    }

    let mut eject = |cur: Current| {
        let s = |start, len| slice[start..start + len].into();
        let node = match cur {
            Current::Normal(start, len) => MicroNode::Text(s(start, len)),
            Current::Super(sup) => MicroNode::Formatted(
                vec![MicroNode::Text(sup)],
                FormatCmd::VerticalAlignmentSuperscript,
            ),
            Current::Sub(sub) => MicroNode::Formatted(
                vec![MicroNode::Text(sub)],
                FormatCmd::VerticalAlignmentSubscript,
            ),
        };
        stack.push(node);
    };

    let mut current = None;
    for (ix, ch) in slice.char_indices() {
        current = match to_sup_sub(ch) {
            SupSub::Normal => match current {
                Some(Current::Normal(_, ref mut len)) => {
                    *len += ch.len_utf8();
                    continue;
                }
                Some(cur @ Current::Super(_)) | Some(cur @ Current::Sub(_)) => {
                    eject(cur);
                    Some(Current::Normal(ix, ch.len_utf8()))
                }
                None => Some(Current::Normal(ix, ch.len_utf8())),
            },
            SupSub::Super(c) => match current {
                Some(Current::Super(ref mut s)) => {
                    s.push_str(c);
                    continue;
                }
                Some(cur @ Current::Sub(_)) | Some(cur @ Current::Normal(..)) => {
                    eject(cur);
                    Some(Current::Super(c.into()))
                }
                None => Some(Current::Super(c.into())),
            },
            SupSub::Sub(c) => match current {
                Some(Current::Sub(ref mut s)) => {
                    s.push_str(c);
                    continue;
                }
                Some(cur @ Current::Super(_)) | Some(cur @ Current::Normal(..)) => {
                    eject(cur);
                    Some(Current::Sub(c.into()))
                }
                None => Some(Current::Sub(c.into())),
            },
        }
    }
    if let Some(last) = current {
        eject(last);
    }
    stack
}

#[test]
fn mixed() {
    let mixed = "normalʳᵉ₉";
    assert_eq!(
        parse_sup_sub(mixed),
        vec![
            MicroNode::Text("normal".into()),
            MicroNode::Formatted(
                vec![MicroNode::Text("re".into())],
                FormatCmd::VerticalAlignmentSuperscript
            ),
            MicroNode::Formatted(
                vec![MicroNode::Text("9".into())],
                FormatCmd::VerticalAlignmentSubscript
            ),
        ]
    );
}

#[test]
fn french() {
    let re = "ʳᵉ";
    assert_eq!(
        parse_sup_sub(re),
        vec![MicroNode::Formatted(
            vec![MicroNode::Text("re".into())],
            FormatCmd::VerticalAlignmentSuperscript
        )]
    );
}

fn to_sup_sub(c: char) -> SupSub {
    use crate::unicode::sup_sub::{
        lookup_decomposition, SUBSCRIPT_MEMBERSHIP, SUPERSCRIPT_MEMBERSHIP,
    };

    fn is_in_ranges(c: char, ranges: &[(char, char)]) -> bool {
        for (from, upto) in ranges {
            if c >= *from && c <= *upto {
                return true;
            }
        }
        false
    }

    if is_in_ranges(c, SUPERSCRIPT_MEMBERSHIP) {
        SupSub::Super(lookup_decomposition(c))
    } else if is_in_ranges(c, SUBSCRIPT_MEMBERSHIP) {
        SupSub::Sub(lookup_decomposition(c))
    } else {
        SupSub::Normal
    }
}
