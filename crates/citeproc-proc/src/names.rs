// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::prelude::*;

use super::unicode::is_latin_cyrillic;
use citeproc_io::utils::Intercalate;
use citeproc_io::{Name, PersonName};
use csl::style::{
    DelimiterPrecedes, Name as NameEl, NameAnd, NameAsSortOrder, NameEtAl, NameForm, NamePart,
    Names, Position,
};

mod initials;
use self::initials::initialize;

fn pn_is_latin_cyrillic(pn: &PersonName) -> bool {
    pn.family
        .as_ref()
        .map(|s| is_latin_cyrillic(s))
        .unwrap_or(true)
        && pn
            .given
            .as_ref()
            .map(|s| is_latin_cyrillic(s))
            .unwrap_or(true)
        && pn
            .suffix
            .as_ref()
            .map(|s| is_latin_cyrillic(s))
            .unwrap_or(true)
        && pn
            .non_dropping_particle
            .as_ref()
            .map(|s| is_latin_cyrillic(s))
            .unwrap_or(true)
        && pn
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
pub fn pn_filtered_parts(pn: &PersonName, order: DisplayOrdering) -> Vec<NamePartToken> {
    let parts: Vec<NamePartToken> = order
        .iter()
        .cloned()
        .filter(|npt| match npt {
            NamePartToken::Given => pn.given.is_some(),
            NamePartToken::Family => pn.family.is_some(),
            NamePartToken::NonDroppingParticle => pn.non_dropping_particle.is_some(),
            NamePartToken::DroppingParticle => pn.dropping_particle.is_some(),
            NamePartToken::Suffix => pn.suffix.is_some(),
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
                (None, Space)
                | (None, SortSeparator)
                | (Some(Space), Space)
                | (Some(SortSeparator), SortSeparator)
                | (Some(SortSeparator), Space) => {
                    // do not add the token
                }
                (Some(Space), SortSeparator) => {
                    // recall that separator includes a space
                    // "Doe , John" is wrong
                    acc.pop();
                    acc.push(SortSeparator);
                }
                (_, t) => {
                    acc.push(t);
                }
            }
            acc
        })
}

fn should_delimit_after(prec: DelimiterPrecedes, name: &OneName, count_before_spot: usize) -> bool {
    match prec {
        DelimiterPrecedes::Contextual => count_before_spot >= 2,
        // anticipate whether name_as_sort_order would kick in for the
        // name just before the delimiter would go
        DelimiterPrecedes::AfterInvertedName => name.naso(count_before_spot > 0),
        DelimiterPrecedes::Always => true,
        DelimiterPrecedes::Never => false,
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

struct OneName(NameEl);

impl OneName {
    #[inline]
    fn naso(&self, seen_one: bool) -> bool {
        match self.0.name_as_sort_order {
            None => false,
            Some(NameAsSortOrder::First) => !seen_one,
            Some(NameAsSortOrder::All) => true,
        }
    }

    #[inline]
    fn ea_min(&self, pos: Position) -> usize {
        let first = self.0.et_al_min.unwrap_or(0);
        if pos == Position::First {
            first as usize
        } else {
            self.0.et_al_subsequent_min.unwrap_or(first) as usize
        }
    }

    #[inline]
    fn ea_use_first(&self, pos: Position) -> usize {
        let first = self.0.et_al_use_first.unwrap_or(1);
        if pos == Position::First {
            first as usize
        } else {
            self.0.et_al_subsequent_use_first.unwrap_or(first) as usize
        }
    }

    fn name_tokens<'s>(&self, position: Position, names_slice: &'s [Name]) -> Vec<NameToken<'s>> {
        let name_count = names_slice.len();
        let ea_min = self.ea_min(position);
        let ea_use_first = self.ea_use_first(position);
        if self.0.enable_et_al() && name_count >= ea_min {
            if self.0.et_al_use_last == Some(true) && ea_use_first + 2 <= name_count {
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
                    .0
                    .delimiter_precedes_et_al
                    .unwrap_or(DelimiterPrecedes::Contextual);
                if should_delimit_after(dpea, self, ea_use_first) {
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
            if self.0.and.is_some() {
                if let Some(last_delim) = nms.iter().rposition(|t| *t == NameToken::Delimiter) {
                    let dpl = self
                        .0
                        .delimiter_precedes_last
                        .unwrap_or(DelimiterPrecedes::Contextual);
                    if should_delimit_after(dpl, self, name_count - 1) {
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
        db: &impl IrDatabase,
        _state: &mut IrState,
        ctx: &CiteContext<'c, O>,
        names_slice: &[Name],
        et_al: &Option<NameEtAl>,
    ) -> O::Build {
        let fmt = &ctx.format;

        let mut seen_one = false;
        let name_tokens = self.name_tokens(ctx.position, names_slice);
        let locale = db.locale_by_cite(ctx.cite.id);

        if self.0.form == Some(NameForm::Count) {
            let count: u32 = name_tokens.iter().fold(0, |acc, name| match name {
                NameToken::Name(_) => acc + 1,
                _ => acc,
            });
            // This isn't sort-mode, you can render NameForm::Count as text.
            return ctx.format.affixed_text(
                format!("{}", count),
                self.0.formatting,
                &self.0.affixes,
            );
        }

        let format_with_part = |o_part: &Option<NamePart>, s: &str| {
            match o_part {
                None => fmt.plain(s),
                Some(ref part) => {
                    // TODO: text-case
                    fmt.affixed(fmt.text_node(s.to_string(), part.formatting), &part.affixes)
                }
            }
        };

        let st = name_tokens.iter().map(|n| match n {
            NameToken::Name(Name::Person(ref pn)) => {
                let order = get_display_order(
                    pn_is_latin_cyrillic(pn),
                    self.0.form == Some(NameForm::Long),
                    self.naso(seen_one),
                    db.style().demote_non_dropping_particle,
                );
                seen_one = true;

                let mut build = vec![];
                for part in pn_filtered_parts(pn, order) {
                    // We already tested is_some() for all these Some::unwrap() calls
                    match part {
                        NamePartToken::Given => {
                            if let Some(ref given) = pn.given {
                                let name_part = &self.0.name_part_given;
                                // TODO: parametrize for disambiguation
                                let string = initialize(
                                    &given,
                                    self.0.initialize.unwrap_or(true),
                                    self.0.initialize_with.as_ref().map(|s| s.as_ref()),
                                    db.style().initialize_with_hyphen,
                                );
                                build.push(format_with_part(name_part, &string));
                            }
                        }
                        NamePartToken::Family => {
                            let name_part = &self.0.name_part_family;
                            let string = pn.family.as_ref().unwrap();
                            build.push(format_with_part(name_part, &string));
                        }
                        NamePartToken::NonDroppingParticle => {
                            build.push(fmt.plain(&pn.non_dropping_particle.as_ref().unwrap()));
                        }
                        NamePartToken::DroppingParticle => {
                            build.push(fmt.plain(pn.dropping_particle.as_ref().unwrap()));
                        }
                        NamePartToken::Suffix => {
                            build.push(fmt.plain(pn.suffix.as_ref().unwrap()));
                        }
                        NamePartToken::Space => {
                            build.push(fmt.plain(" "));
                        }
                        NamePartToken::SortSeparator => {
                            build.push(if let Some(sep) = &self.0.sort_separator {
                                fmt.plain(&sep)
                            } else {
                                fmt.plain(", ")
                            })
                        }
                    }
                }
                fmt.seq(build.into_iter())
            }
            NameToken::Name(Name::Literal { ref literal }) => {
                seen_one = true;
                fmt.plain(literal.as_str())
            }
            NameToken::Delimiter => {
                if let Some(delim) = &self.0.delimiter {
                    fmt.plain(&delim.0)
                } else {
                    fmt.plain(", ")
                }
            }
            NameToken::EtAl => {
                use csl::terms::*;
                let mut term = MiscTerm::EtAl;
                let mut formatting = None;
                if let Some(ref etal_element) = &et_al {
                    if etal_element.term == "and others" {
                        term = MiscTerm::AndOthers;
                    }
                    formatting = etal_element.formatting;
                }
                let text = locale
                    .get_text_term(
                        TextTermSelector::Simple(SimpleTermSelector::Misc(
                            term,
                            TermFormExtended::Long,
                        )),
                        false,
                    )
                    .unwrap_or("et al");
                fmt.text_node(text.to_string(), formatting)
            }
            NameToken::Ellipsis => fmt.plain("…"),
            NameToken::Space => fmt.plain(" "),
            NameToken::And => {
                use csl::terms::*;
                let select = |form: TermFormExtended| {
                    TextTermSelector::Simple(SimpleTermSelector::Misc(MiscTerm::And, form))
                };
                // If an And token shows up, we already know self.0.and is Some.
                let form = match self.0.and {
                    Some(NameAnd::Symbol) => locale
                        .get_text_term(select(TermFormExtended::Symbol), false)
                        .unwrap_or("&"),
                    _ => locale
                        .get_text_term(select(TermFormExtended::Long), false)
                        .unwrap_or("and"),
                };
                fmt.plain(form)
            }
        });

        fmt.affixed(
            fmt.with_format(fmt.seq(st), self.0.formatting),
            &self.0.affixes,
        )
    }
}

use self::ord::{get_display_order, DisplayOrdering, NamePartToken};

#[allow(dead_code)]
mod ord {
    //! Latin here means latin or cyrillic.
    //! TODO: use the regex crate with \\p{Cyrillic} and \\p{Latin}

    use csl::style::DemoteNonDroppingParticle as DNDP;

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
    fn intermediate(
        &self,
        db: &impl IrDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O>,
    ) -> IrSum<O>
    where
        O: OutputFormat,
    {
        let fmt = &ctx.format;
        let name_el = OneName(
            db.name_citation()
                .merge(self.name.as_ref().unwrap_or(&NameEl::default())),
        );
        let rendered: Vec<_> = self
            .variables
            .iter()
            // TODO: &[editor, translator] => &[editor], and use editortranslator on
            // the label
            .filter_map(|&var| ctx.get_name(var))
            .map(|val| name_el.render(db, state, ctx, val, &self.et_al))
            .collect();
        if rendered.is_empty() {
            return (IR::Rendered(None), GroupVars::new());
        }
        let delim = self.delimiter.as_ref().map(|d| d.0.as_ref()).unwrap_or("");
        let content = Some(fmt.affixed(fmt.group(rendered, delim, self.formatting), &self.affixes));
        let gv = GroupVars::rendered_if(content.is_some());
        (IR::Rendered(content), gv)
    }
}
