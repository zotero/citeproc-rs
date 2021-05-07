use lexical_sort::{lexical_cmp, natural_lexical_cmp};
use std::cmp::Ordering;

#[derive(Debug)]
pub(crate) struct Lexical<S: AsRef<str>>(S);

impl<S: AsRef<str>> Lexical<S> {
    #[allow(dead_code)]
    pub(crate) fn new(inner: S) -> Self {
        Lexical(inner)
    }
}
impl<S: AsRef<str>> Eq for Lexical<S> {}
impl<S: AsRef<str>> PartialEq for Lexical<S> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<S: AsRef<str>> PartialOrd for Lexical<S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<S: AsRef<str>> Ord for Lexical<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        lexical_cmp(self.0.as_ref(), other.0.as_ref())
    }
}

#[test]
fn test_lexical_d_wander() {
    assert_eq!(
        Lexical("d'Wander").cmp(&Lexical("de'Wander")),
        Ordering::Less
    );
}

#[derive(Debug)]
pub(crate) struct Natural<S: AsRef<str>>(S);

impl<S: AsRef<str>> Natural<S> {
    pub(crate) fn new(inner: S) -> Self {
        Natural(inner)
    }
}
impl<S: AsRef<str>> Eq for Natural<S> {}
impl<S: AsRef<str>> PartialEq for Natural<S> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<S: AsRef<str>> PartialOrd for Natural<S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<S: AsRef<str>> Ord for Natural<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        natural_lexical_cmp(self.0.as_ref(), other.0.as_ref())
    }
}

#[test]
fn test_natural_numbers() {
    assert_eq!(
        Natural("Article 3").cmp(&Natural("Article 20")),
        Ordering::Less
    );
}
