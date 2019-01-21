use itertools::Itertools;
use std::borrow::Cow;

use super::cite_context::*;
use super::ir::*;
use super::Proc;
use crate::input::{Name, PersonName};
use crate::output::OutputFormat;
use crate::style::element::{
    DelimiterPrecedes, Name as NameEl, NameAsSortOrder, NameForm, Names, Position,
};
use crate::utils::Intercalate;

use crate::input::is_latin_cyrillic;

mod initials;
use self::initials::initialize;

impl PersonName {
    fn is_latin_cyrillic(&self) -> bool {
        self.family
            .as_ref()
            .map(|s| is_latin_cyrillic(s))
            .unwrap_or(true)
            && self
                .given
                .as_ref()
                .map(|s| is_latin_cyrillic(s))
                .unwrap_or(true)
            && self
                .suffix
                .as_ref()
                .map(|s| is_latin_cyrillic(s))
                .unwrap_or(true)
            && self
                .non_dropping_particle
                .as_ref()
                .map(|s| is_latin_cyrillic(s))
                .unwrap_or(true)
            && self
                .dropping_particle
                .as_ref()
                .map(|s| is_latin_cyrillic(s))
                .unwrap_or(true)
    }

    /// For a given display order, not all the name parts will have data in them at the end. So for
    /// this PersonName, reduce the DisplayOrdering to include only those parts that will end up
    /// with content.
    ///
    /// For example, for a last-name-only name like "Megalodon", `NamePartToken::Given` is removed,
    /// which for `&[Family, SortSeparator, Given]` would leave `&[Family, SortSeparator]` and
    /// render "Megalodon, ", so SortSeparator also has to be removed.
    pub fn filtered_parts<'a>(&self, order: DisplayOrdering) -> Vec<NamePartToken> {
        let parts: Vec<NamePartToken> = order
            .iter()
            .cloned()
            .filter(|npt| match npt {
                NamePartToken::Given => self.given.is_some(),
                NamePartToken::Family => self.family.is_some(),
                NamePartToken::NonDroppingParticle => self.non_dropping_particle.is_some(),
                NamePartToken::DroppingParticle => self.dropping_particle.is_some(),
                NamePartToken::Suffix => self.suffix.is_some(),
                NamePartToken::Space => true,
                NamePartToken::SortSeparator => true,
            })
            .collect();

        // don't include leading or trailing spaces or delimiters
        let len = parts.len();
        let take = if let Some(last) = parts.iter().rposition(|t| t.not_delim()) {
            last + 1
        } else {
            len
        };
        // We may have dropped some of the namey name parts, leaving some stylistic tokens that
        // are incorrect or redundant. So we need to drop stuff like 'two spaces in a row'.
        // It *could* be done without a new Vec, but this is easier.
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
                        // recall that separator includes a space
                        // "Doe , John" is wrong
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
}

impl DelimiterPrecedes {
    fn should_delimit_after(&self, name: &NameEl, count_before_spot: usize) -> bool {
        match self {
            DelimiterPrecedes::Contextual => count_before_spot >= 2,
            // anticipate whether name_as_sort_order would kick in for the
            // name just before the delimiter would go
            DelimiterPrecedes::AfterInvertedName => name.naso(count_before_spot > 0),
            DelimiterPrecedes::Always => true,
            DelimiterPrecedes::Never => false,
        }
    }
}

#[derive(Eq, PartialEq, Clone)]
enum NameToken<'a> {
    Name(&'a Name),
    EtAl,
    Ellipsis,
    Delimiter,
    And,
    Space,
}

impl NameEl {
    #[inline]
    fn naso(&self, seen_one: bool) -> bool {
        match self.name_as_sort_order {
            None => false,
            Some(NameAsSortOrder::First) => !seen_one,
            Some(NameAsSortOrder::All) => true,
        }
    }

    #[inline]
    fn ea_min(&self, pos: Position) -> usize {
        let first = self.et_al_min.unwrap_or(0);
        if pos == Position::First {
            first as usize
        } else {
            self.et_al_subsequent_min.unwrap_or(first) as usize
        }
    }

    #[inline]
    fn ea_use_first(&self, pos: Position) -> usize {
        let first = self.et_al_use_first.unwrap_or(1);
        if pos == Position::First {
            first as usize
        } else {
            self.et_al_subsequent_use_first.unwrap_or(first) as usize
        }
    }

    fn name_tokens<'s>(&self, position: Position, names_slice: &'s [Name]) -> Vec<NameToken<'s>> {
        let name_count = names_slice.len();
        let ea_min = self.ea_min(position);
        let ea_use_first = self.ea_use_first(position);
        if name_count >= ea_min {
            if self.et_al_use_last == Some(true) && ea_use_first + 2 <= name_count {
                let last = &names_slice[name_count - 1];
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
                let mut nms = names_slice
                    .iter()
                    .map(NameToken::Name)
                    .take(ea_use_first)
                    .intercalate(&NameToken::Delimiter);
                let dpea = self
                    .delimiter_precedes_et_al
                    .unwrap_or(DelimiterPrecedes::Contextual);
                if dpea.should_delimit_after(self, ea_use_first) {
                    nms.push(NameToken::Delimiter);
                } else {
                    nms.push(NameToken::Space);
                }
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
            if let Some(ref _and) = self.and {
                if let Some(last_delim) = nms.iter().rposition(|t| *t == NameToken::Delimiter) {
                    let dpl = self
                        .delimiter_precedes_last
                        .unwrap_or(DelimiterPrecedes::Contextual);
                    if dpl.should_delimit_after(self, name_count - 1) {
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
        }
    }

    fn render<'c, O: OutputFormat>(
        &self,
        ctx: &CiteContext<'c, O>,
        names_slice: &[Name],
    ) -> O::Build {
        let mut seen_one = false;
        let name_tokens = self.name_tokens(ctx.position, names_slice);

        if self.form == Some(NameForm::Count) {
            let count: u32 = name_tokens.iter().fold(0, |acc, name| match name {
                NameToken::Name(_) => acc + 1,
                _ => acc,
            });
            // This isn't sort-mode, you can render NameForm::Count as text.
            return ctx.format.affixed_text(
                format!("{}", count),
                self.formatting.as_ref(),
                &self.affixes,
            );
        }

        let st = name_tokens
            .iter()
            .map(|n| match n {
                NameToken::Name(Name::Person(ref pn)) => {
                    let order = get_display_order(
                        pn.is_latin_cyrillic(),
                        self.form == Some(NameForm::Long),
                        self.naso(seen_one),
                        ctx.style.demote_non_dropping_particle,
                    );
                    seen_one = true;
                    let mut build = String::new();
                    for part in pn.filtered_parts(order) {
                        // We already tested is_some() for all these Some::unwrap() calls
                        match part {
                            NamePartToken::Given => {
                                if let Some(ref given) = pn.given {
                                    // TODO: parametrize for disambiguation
                                    build.push_str(&initialize(
                                        &given,
                                        self.initialize.unwrap_or(true),
                                        self.initialize_with
                                            .as_ref()
                                            .map(|s| s.as_str())
                                            .unwrap_or(""),
                                        ctx.style.initialize_with_hyphen,
                                    ))
                                }
                            }
                            NamePartToken::Family => build.push_str(&pn.family.as_ref().unwrap()),
                            NamePartToken::NonDroppingParticle => {
                                build.push_str(&pn.non_dropping_particle.as_ref().unwrap())
                            }
                            NamePartToken::DroppingParticle => {
                                build.push_str(&pn.dropping_particle.as_ref().unwrap())
                            }
                            NamePartToken::Suffix => build.push_str(&pn.suffix.as_ref().unwrap()),
                            NamePartToken::Space => build.push_str(" "),
                            NamePartToken::SortSeparator => build.push_str(", "),
                        }
                    }
                    Cow::Owned(build.trim().to_string())
                }
                NameToken::Name(Name::Literal { ref literal }) => {
                    seen_one = true;
                    Cow::Borrowed(literal.as_str())
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

use self::ord::{get_display_order, DisplayOrdering, NamePartToken};

#[allow(dead_code)]
mod ord {
    //! Latin here means latin or cyrillic.
    //! TODO: use the regex crate with \\p{Cyrillic} and \\p{Latin}

    use crate::style::element::DemoteNonDroppingParticle as DNDP;

    pub type DisplayOrdering = &'static [NamePartToken];

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

    pub type SortOrdering = &'static [SortToken];

    #[derive(PartialEq)]
    pub enum SortToken {
        One(NamePartToken),
        Two(NamePartToken, NamePartToken),
    }

    use self::NamePartToken::*;
    use self::SortToken::*;

    pub fn get_display_order(latin: bool, long: bool, naso: bool, demote: DNDP) -> DisplayOrdering {
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

    static LATIN_LONG: DisplayOrdering = &[
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
    static LATIN_LONG_NASO: DisplayOrdering = &[
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
    static LATIN_LONG_NASO_DEMOTED: DisplayOrdering = &[
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
    static LATIN_SHORT: DisplayOrdering = &[NonDroppingParticle, Space, Family];

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

    static NON_LATIN_LONG: DisplayOrdering = &[
        Family, // TODO: how do we determine if spaces are required?
        Given,
    ];
    static NON_LATIN_SHORT: DisplayOrdering = &[Family];
    static NON_LATIN_SORT_LONG: SortOrdering = &[One(Family), One(Given)];
    static NON_LATIN_SORT_SHORT: SortOrdering = &[One(Family)];

}

impl<'c, O> Proc<'c, O> for Names
where
    O: OutputFormat,
{
    fn intermediate<'s: 'c>(&'s self, ctx: &CiteContext<'c, O>) -> IR<'c, O>
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
