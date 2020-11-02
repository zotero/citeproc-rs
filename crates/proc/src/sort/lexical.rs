use lexical_sort::lexical_cmp;
use std::cmp::Ordering;

#[derive(Debug)]
pub(crate) struct Lexical<S: AsRef<str>>(S);

impl<S: AsRef<str>> Lexical<S> {
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
    assert_eq!(Lexical("d'Wander").cmp(&Lexical("de'Wander")), Ordering::Less);
}

