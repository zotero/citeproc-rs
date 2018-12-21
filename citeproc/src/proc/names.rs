use itertools::Itertools;
use std::borrow::Cow;

use super::cite_context::*;
use super::ir::*;
use super::Proc;
use crate::input::{Name, PersonName};
use crate::output::OutputFormat;
use crate::style::element::{
    DemoteNonDroppingParticle, Name as NameEl, NameAsSortOrder, NameForm, Names, Position,
    DelimiterPrecedes
};
use crate::utils::Intercalate;

use crate::input::is_latin_cyrillic;

impl PersonName<'_> {
    fn is_latin_cyrillic(&self) -> bool {
        self.family.as_ref().map(|s| is_latin_cyrillic(s)).unwrap_or(true) &&
        self.given.as_ref().map(|s| is_latin_cyrillic(s)).unwrap_or(true) &&
        self.suffix.as_ref().map(|s| is_latin_cyrillic(s)).unwrap_or(true) &&
        self.non_dropping_particle.as_ref().map(|s| is_latin_cyrillic(s)).unwrap_or(true) &&
        self.dropping_particle.as_ref().map(|s| is_latin_cyrillic(s)).unwrap_or(true)
    }
}

#[derive(Eq, PartialEq, Clone)]
enum NameToken<'a, 'b: 'a> {
    Name(&'b Name<'a>),
    EtAl,
    Ellipsis,
    Delimiter,
    And,
    Space,
}

impl NameEl {

    fn naso(&self, seen_one: bool) -> bool {
        match self.name_as_sort_order {
            None => false,
            Some(NameAsSortOrder::First) => !seen_one,
            Some(NameAsSortOrder::All) => true,
        }
    }

    fn ea_min(&self, pos: Position) -> usize {
        let first = self.et_al_min.unwrap_or(0);
        if pos == Position::First {
            first as usize
        } else {
            self.et_al_subsequent_min.unwrap_or(first) as usize
        }
    }

    fn ea_use_first(&self, pos: Position) -> usize {
        let first = self.et_al_use_first.unwrap_or(1);
        if pos == Position::First {
            first as usize
        } else {
            self.et_al_subsequent_use_first.unwrap_or(first) as usize
        }
    }

    fn filtered_parts<'a>(pn: &PersonName, order: &'static [NamePartToken]) -> Vec<NamePartToken> {
        let mut parts: Vec<NamePartToken> = order
            .iter()
            .cloned()
            .filter_map(|nt| match nt {
                NamePartToken::Given => pn.given.as_ref().map(|_| nt),
                NamePartToken::Family => pn.family.as_ref().map(|_| nt),
                NamePartToken::NonDroppingParticle => pn.non_dropping_particle.as_ref().map(|_| nt),
                NamePartToken::DroppingParticle => pn.dropping_particle.as_ref().map(|_| nt),
                NamePartToken::Suffix => pn.suffix.as_ref().map(|_| nt),
                NamePartToken::Space => Some(nt),
                NamePartToken::SortSeparator => Some(nt),
            })
            // remove doubled up spaces or delimiters
            .dedup()
            .collect();

        // don't include leading or trailing spaces or delimiters
        let len = parts.len();
        let take = if let Some(last) = parts.iter().rposition(|t| t.not_delim()) {
            last + 1
        } else {
            len
        };
        parts
            .into_iter()
            .take(take)
            .fold(Vec::with_capacity(len), |mut acc, token| {
                use self::ord::NamePartToken::*;
                match (acc.last(), token) {
                    (None, Space) => {}
                    (None, SortSeparator) => {}
                    (Some(Space), Space) => {}
                    (Some(Space), SortSeparator) => {
                        acc.pop();
                        acc.push(SortSeparator);
                    }
                    (Some(SortSeparator), Space) => {}
                    (Some(SortSeparator), SortSeparator) => {}
                    (_, t) => {
                        acc.push(t);
                    }
                }
                acc
            })
    }

    fn render<'c, 'r, 'ci, O: OutputFormat>(
        &self,
        ctx: &CiteContext<'c, 'r, 'ci, O>,
        names_slice: &[Name],
    ) -> O::Build {
        // TODO: NameForm::Count
        let mut seen_one = false;
        let name_count = names_slice.len();
        let ea_min = self.ea_min(ctx.position);
        let ea_use_first = self.ea_use_first(ctx.position);
        let names = if name_count >= ea_min {
            if self.et_al_use_last == Some(true) && ea_use_first + 2 <= name_count {
                let last = names_slice.iter().last().unwrap();
                let mut nms = names_slice
                    .iter()
                    .map(NameToken::Name)
                    .take(ea_use_first)
                    .intercalate(&NameToken::Delimiter);
                nms.push(NameToken::Delimiter);
                nms.push(NameToken::Ellipsis);
                nms.push(NameToken::Space);
                nms.push(NameToken::Name(last));
                nms
            } else {
                // TODO: et-al
                let mut nms = names_slice
                    .iter()
                    .map(NameToken::Name)
                    .take(ea_use_first)
                    .intercalate(&NameToken::Delimiter);
                nms.push(NameToken::Delimiter);
                nms.push(NameToken::EtAl);
                nms
            }
        } else {
            let mut nms = names_slice
                .iter()
                .map(NameToken::Name)
                .intercalate(&NameToken::Delimiter);
            // "delimiter-precedes-last" would be better named as "delimiter-precedes-and",
            // because it only has any effect when "and" is set.
            if let Some(ref and) = self.and {
                if let Some(last_delim) = nms.iter().rposition(|t| *t == NameToken::Delimiter) {
                    let dpl = self.delimiter_precedes_last.unwrap_or(DelimiterPrecedes::Contextual);
                    let insert = match dpl {
                        DelimiterPrecedes::Contextual => name_count >= 3,
                        // anticipate whether name_as_sort_order would kick in for the
                        // (n-1)th name
                        DelimiterPrecedes::AfterInvertedName => self.naso(name_count > 1),
                        DelimiterPrecedes::Always => true,
                        DelimiterPrecedes::Never => false,
                    };
                    if insert {
                        nms.insert(last_delim + 1, NameToken::And);
                        nms.insert(last_delim + 2, NameToken::Space);
                    } else {
                        nms[last_delim] = NameToken::Space;
                        nms.insert(last_delim + 1, NameToken::And);
                        nms.insert(last_delim + 2, NameToken::Space);
                    }
                }
            }
            nms
        };

        let st = names
            .iter()
            .map(|n| match n {
                NameToken::Name(Name::Person(ref pn)) => {
                    let naso = self.naso(seen_one);
                    let order = get_display_order(
                        pn.is_latin_cyrillic(),
                        self.form == Some(NameForm::Long),
                        naso,
                        // TODO: dynamic
                        DemoteNonDroppingParticle::DisplayAndSort,
                    );
                    seen_one = true;
                    let parts = Self::filtered_parts(pn, order);
                    let out = parts
                        .iter()
                        .filter_map(|nt| match nt {
                            NamePartToken::Given => pn.given.as_ref(),
                            NamePartToken::Family => pn.family.as_ref(),
                            NamePartToken::NonDroppingParticle => pn.non_dropping_particle.as_ref(),
                            NamePartToken::DroppingParticle => pn.dropping_particle.as_ref(),
                            NamePartToken::Suffix => pn.suffix.as_ref(),
                            NamePartToken::Space => Some(Cow::Borrowed(" ")).as_ref(),
                            NamePartToken::SortSeparator => Some(Cow::Borrowed(", ")).as_ref(),
                        })
                        .join("");
                    Cow::Owned(out.trim().to_string())
                }
                NameToken::Name(Name::Literal { ref literal }) => {
                    seen_one = true;
                    literal.clone()
                }
                NameToken::Delimiter => Cow::Borrowed(", "),
                NameToken::EtAl => Cow::Borrowed("et al"),
                NameToken::Ellipsis => Cow::Borrowed("…"),
                NameToken::Space => Cow::Borrowed(" "),
                NameToken::And => Cow::Borrowed("and"),
            })
            // TODO: and, et-al, et cetera
            .join("");

        ctx.format
            .affixed_text(st, self.formatting.as_ref(), &self.affixes)
    }
}

use self::ord::{get_display_order, get_sort_order, NameOrdering, NamePartToken};

mod ord {
    //! Latin here means latin or cyrillic.
    //! TODO: use the regex crate with \\p{Cyrillic} and \\p{Latin}

    use crate::style::element::DemoteNonDroppingParticle as DNDP;

    pub type NameOrdering = &'static [NamePartToken];
    pub type SortOrdering = &'static [SortToken];

    #[derive(Clone, Copy, PartialEq)]
    pub enum NamePartToken {
        Given,
        Family,
        NonDroppingParticle,
        DroppingParticle,
        Suffix,
        SortSeparator,
        Space,
    }

    impl NamePartToken {
        pub fn not_delim(self) -> bool {
            match self {
                SortSeparator => false,
                Space => false,
                _ => true,
            }
        }
    }

    #[derive(PartialEq)]
    pub enum SortToken {
        One(NamePartToken),
        Two(NamePartToken, NamePartToken),
    }

    use self::NamePartToken::*;
    use self::SortToken::*;

    pub fn get_display_order(latin: bool, long: bool, naso: bool, demote: DNDP) -> NameOrdering {
        match (latin, long, naso, demote) {
            (false, long, ..) => {
                if long {
                    NON_LATIN_LONG
                } else {
                    NON_LATIN_SHORT
                }
            }
            (true, false, ..) => LATIN_SHORT,
            (true, true, false, _) => LATIN_LONG,
            (true, true, true, demote) => {
                if demote == DNDP::DisplayAndSort {
                    LATIN_LONG_NASO_DEMOTED
                } else {
                    LATIN_LONG_NASO
                }
            }
        }
    }

    pub fn get_sort_order(latin: bool, long: bool, demote: DNDP) -> SortOrdering {
        match (latin, long, demote) {
            (false, long, _) => {
                if long {
                    NON_LATIN_SORT_LONG
                } else {
                    NON_LATIN_SORT_SHORT
                }
            }
            (true, _, demote) => {
                if demote == DNDP::Never {
                    LATIN_SORT_NEVER
                } else {
                    LATIN_SORT
                }
            }
        }
    }

    static LATIN_LONG: NameOrdering = &[
        Given,
        Space,
        DroppingParticle,
        Space,
        NonDroppingParticle,
        Space,
        Family,
        Space,
        Suffix,
    ];
    static LATIN_LONG_NASO: NameOrdering = &[
        NonDroppingParticle,
        Space,
        Family,
        SortSeparator,
        Given,
        Space,
        DroppingParticle,
        SortSeparator,
        Suffix,
    ];
    static LATIN_LONG_NASO_DEMOTED: NameOrdering = &[
        Family,
        SortSeparator,
        Given,
        Space,
        DroppingParticle,
        Space,
        NonDroppingParticle,
        SortSeparator,
        Suffix,
    ];
    static LATIN_SHORT: NameOrdering = &[NonDroppingParticle, Space, Family];

    static LATIN_SORT_NEVER: SortOrdering = &[
        Two(NonDroppingParticle, Family),
        One(DroppingParticle),
        One(Given),
        One(Suffix),
    ];

    static LATIN_SORT: SortOrdering = &[
        One(Family),
        Two(DroppingParticle, NonDroppingParticle),
        One(Given),
        One(Suffix),
    ];

    static NON_LATIN_LONG: NameOrdering = &[
        Family, // TODO: how do we determine if spaces are required?
        Given,
    ];
    static NON_LATIN_SHORT: NameOrdering = &[Family];
    static NON_LATIN_SORT_LONG: SortOrdering = &[One(Family), One(Given)];
    static NON_LATIN_SORT_SHORT: SortOrdering = &[One(Family)];
}

impl<'c, 'r: 'c, 'ci: 'c, O> Proc<'c, 'r, 'ci, O> for Names
where
    O: OutputFormat,
{
    fn intermediate<'s: 'c>(&'s self, ctx: &CiteContext<'c, 'r, 'ci, O>) -> IR<'c, O>
    where
        O: OutputFormat,
    {
        let fmt = ctx.format;
        let name_el =
            NameEl::root_default().merge(self.name.as_ref().unwrap_or(&NameEl::default()));
        let rendered: Vec<_> = self
            .variables
            .iter()
            // TODO: &[editor, translator] => &[editor], and use editortranslator on
            // the label
            .filter_map(|var| ctx.get_name(var))
            .map(|val| name_el.render(ctx, val))
            .collect();
        let delim = self.delimiter.as_ref().map(|d| d.0.as_ref()).unwrap_or("");
        let content = Some(fmt.affixed(
            fmt.group(rendered, delim, self.formatting.as_ref()),
            &self.affixes,
        ));
        IR::Rendered(content)
    }
}
