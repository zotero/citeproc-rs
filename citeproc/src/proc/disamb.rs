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
    fn add_disamb_tokens(&self, set: &mut HashSet<DisambToken>);
}

impl AddDisambTokens for Reference {
    fn add_disamb_tokens(&self, set: &mut HashSet<DisambToken>) {
        for val in self.ordinary.values() {
            set.insert(DisambToken::Str(val.as_str().into()));
        }
        for val in self.number.values() {
            set.insert(DisambToken::Num(val.clone()));
        }
        for val in self.name.values() {
            for name in val.iter() {
                name.add_disamb_tokens(set);
            }
        }
        for val in self.date.values() {
            val.add_disamb_tokens(set);
        }
    }
}

impl AddDisambTokens for Option<String> {
    fn add_disamb_tokens(&self, set: &mut HashSet<DisambToken>) {
        if let Some(ref x) = self {
            set.insert(DisambToken::Str(x.as_str().into()));
        }
    }
}

impl AddDisambTokens for Name {
    fn add_disamb_tokens(&self, set: &mut HashSet<DisambToken>) {
        match self {
            Name::Person(ref pn) => {
                pn.family.add_disamb_tokens(set);
                pn.given.add_disamb_tokens(set);
                pn.non_dropping_particle.add_disamb_tokens(set);
                pn.dropping_particle.add_disamb_tokens(set);
                pn.suffix.add_disamb_tokens(set);
            }
            Name::Literal { ref literal } => {
                set.insert(DisambToken::Str(literal.as_str().into()));
            }
        }
    }
}

impl AddDisambTokens for DateOrRange {
    fn add_disamb_tokens(&self, set: &mut HashSet<DisambToken>) {
        match self {
            DateOrRange::Single(ref single) => {
                single.add_disamb_tokens(set);
            }
            DateOrRange::Range(d1, d2) => {
                d1.add_disamb_tokens(set);
                d2.add_disamb_tokens(set);
            }
            DateOrRange::Literal(ref lit) => {
                set.insert(DisambToken::Str(lit.as_str().into()));
            }
        }
    }
}

impl AddDisambTokens for Date {
    fn add_disamb_tokens(&self, set: &mut HashSet<DisambToken>) {
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
        set.insert(DisambToken::Date(self.clone()));
        set.insert(DisambToken::Date(just_ym));
        set.insert(DisambToken::Date(just_year));
    }
}
