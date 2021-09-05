use crate::{SmartCow, String};

fn next_char(mutable: &mut &str) -> Option<char> {
    let c = mutable.chars().next()?;
    *mutable = &mutable[c.len_utf8()..];
    Some(c)
}

pub(crate) fn lazy_lowercase_owned(s: String) -> String {
    lazy_char_transform_owned(s, |c| c.to_lowercase())
}

pub(crate) fn lazy_lowercase(s: &str) -> SmartCow {
    lazy_char_transform(s, |c| c.to_lowercase())
}

pub(crate) fn lazy_uppercase_owned(s: String) -> String {
    lazy_char_transform_owned(s, |c| c.to_uppercase())
}

pub fn lazy_char_transform_owned<I: Iterator<Item = char>>(
    s: String,
    f: impl Fn(char) -> I,
) -> String {
    let cow = lazy_char_transform(s.as_ref(), f);
    match cow {
        SmartCow::Borrowed(_) => s,
        SmartCow::Owned(new_s) => new_s,
    }
}

pub fn lazy_char_transform<I: Iterator<Item = char>>(s: &str, f: impl Fn(char) -> I) -> SmartCow {
    transform(s, |rest| {
        let next = next_char(rest).expect("only called when there is remaining input");
        let mut lower_iter = f(next).peekable();
        match lower_iter.next() {
            // It's identical to the original
            Some(c) if c == next => TransformedPart::Unchanged,
            Some(c) => {
                let mut transformed = String::new();
                transformed.push(c);
                transformed.extend(lower_iter);
                TransformedPart::Changed(transformed)
            }
            None => TransformedPart::Changed(String::new()),
        }
    })
}

pub fn lazy_replace_char_owned(orig: String, replace: char, with: &str) -> String {
    let cow = lazy_replace_char(&orig, replace, with);
    match cow {
        SmartCow::Borrowed(_) => orig,
        SmartCow::Owned(x) => x,
    }
}

pub fn lazy_replace_char<'a>(s: &'a str, replace: char, with: &str) -> SmartCow<'a> {
    transform(s, |rest| {
        let next = next_char(rest).expect("only called when there is remaining input");
        if next == replace {
            let with = String::from(with);
            TransformedPart::Changed(with)
        } else {
            TransformedPart::Unchanged
        }
    })
}

pub fn lazy_replace_char_if_owned(orig: String, pred: impl Fn(char) -> bool, with: &str) -> String {
    let cow = lazy_replace_char_if(&orig, pred, with);
    match cow {
        SmartCow::Borrowed(_) => orig,
        SmartCow::Owned(x) => x,
    }
}

pub fn lazy_replace_char_if<'a>(
    s: &'a str,
    pred: impl Fn(char) -> bool,
    with: &str,
) -> SmartCow<'a> {
    transform(s, |rest| {
        let next = next_char(rest).expect("only called when there is remaining input");
        if pred(next) {
            let with = String::from(with);
            TransformedPart::Changed(with)
        } else {
            TransformedPart::Unchanged
        }
    })
}

// Copied from lazy_transform_str
enum TransformedPart {
    Unchanged,
    Changed(String),
}
fn transform(
    slice: &str,
    mut transform_next: impl FnMut(&mut &str) -> TransformedPart,
) -> SmartCow {
    let mut rest = slice;
    let mut copied = loop {
        if rest.is_empty() {
            return SmartCow::Borrowed(slice);
        }
        let unchanged_rest = rest;
        if let TransformedPart::Changed(transformed) = transform_next(&mut rest) {
            let mut copied = String::from(&slice[..slice.len() - unchanged_rest.len()]);
            copied.push_str(&transformed);
            break copied;
        }
    };

    while !rest.is_empty() {
        let unchanged_rest = rest;
        match transform_next(&mut rest) {
            TransformedPart::Unchanged => {
                copied.push_str(&unchanged_rest[..unchanged_rest.len() - rest.len()]);
            }
            TransformedPart::Changed(changed) => copied.push_str(&changed),
        }
    }

    SmartCow::Owned(copied)
}
