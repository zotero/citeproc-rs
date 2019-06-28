use crate::input::{Date, DateOrRange, Name, NumericValue, Reference};
use crate::Atom;
use std::collections::HashSet;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum DisambToken {
    // Should this be an Atom, really? There will not typically be that much reuse going on. It
    // might inflate the cache too much. The size of the disambiguation index is reduced, though.
    Str(Atom),

    /// Significantly simplifies things compared to ultra-localized date output strings.
    /// Reference cannot predict what they'll look like.
    /// `Date` itself can encode the lack of day/month with those fields set to zero.
    Date(Date),

    Num(NumericValue),

    YearSuffix(Atom),
}

pub trait AddDisambTokens {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, indexing: bool);
    #[inline]
    fn add_tokens_index(&self, set: &mut HashSet<DisambToken>) {
        self.add_tokens_ctx(set, true);
    }
    #[inline]
    fn add_tokens(&self, set: &mut HashSet<DisambToken>) {
        self.add_tokens_ctx(set, false);
    }
}

impl AddDisambTokens for Reference {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, indexing: bool) {
        for val in self.ordinary.values() {
            set.insert(DisambToken::Str(val.as_str().into()));
        }
        for val in self.number.values() {
            set.insert(DisambToken::Num(val.clone()));
        }
        for val in self.name.values() {
            for name in val.iter() {
                name.add_tokens_ctx(set, indexing);
            }
        }
        for val in self.date.values() {
            val.add_tokens_ctx(set, indexing);
        }
    }
}

impl AddDisambTokens for Option<String> {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, _indexing: bool) {
        if let Some(ref x) = self {
            set.insert(DisambToken::Str(x.as_str().into()));
        }
    }
}

impl AddDisambTokens for Name {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, indexing: bool) {
        match self {
            Name::Person(ref pn) => {
                pn.family.add_tokens_ctx(set, indexing);
                pn.given.add_tokens_ctx(set, indexing);
                pn.non_dropping_particle.add_tokens_ctx(set, indexing);
                pn.dropping_particle.add_tokens_ctx(set, indexing);
                pn.suffix.add_tokens_ctx(set, indexing);
            }
            Name::Literal { ref literal } => {
                set.insert(DisambToken::Str(literal.as_str().into()));
            }
        }
    }
}

impl AddDisambTokens for DateOrRange {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, indexing: bool) {
        match self {
            DateOrRange::Single(ref single) => {
                single.add_tokens_ctx(set, indexing);
            }
            DateOrRange::Range(d1, d2) => {
                d1.add_tokens_ctx(set, indexing);
                d2.add_tokens_ctx(set, indexing);
            }
            DateOrRange::Literal(ref lit) => {
                set.insert(DisambToken::Str(lit.as_str().into()));
            }
        }
    }
}

impl AddDisambTokens for Date {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, indexing: bool) {
        // when processing a cite, only insert the segments you actually used
        set.insert(DisambToken::Date(*self));
        // for the index, add all possible variations
        if indexing {
            let just_ym = Date {
                year: self.year,
                month: self.month,
                day: 0,
            };
            let just_year = Date {
                year: self.year,
                month: 0,
                day: 0,
            };
            set.insert(DisambToken::Date(just_ym));
            set.insert(DisambToken::Date(just_year));
        }
    }
}
