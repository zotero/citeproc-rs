// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use self::initials::initialize;

use super::unicode::is_latin_cyrillic;
use crate::disamb::names::{
    DisambName, DisambNameData, DisambNameRatchet, NameIR, PersonDisambNameRatchet,
};
use crate::prelude::*;
use crate::NamesInheritance;
use citeproc_io::utils::Intercalate;
use citeproc_io::{Name, PersonName, Reference};
use csl::{
    Atom, DelimiterPrecedes, DemoteNonDroppingParticle, Name as NameEl, NameAnd, NameAsSortOrder,
    NameEtAl, NameForm, NameLabel, NamePart, NameVariable, Names, Position,
};
use std::sync::Arc;
use std::sync::Mutex;

mod initials;

impl DisambNameData {
    fn lookup_id(&mut self, db: &impl IrDatabase, advance_to_global: bool) -> DisambName {
        let id = db.disamb_name(self.clone());
        // test dismabiguate_AndreaEg2 decided that we shouldn't do this in RefIR mode.
        if advance_to_global {
            let globally_disambiguated = db.disambiguated_person_names();
            if let Some(my_data) = globally_disambiguated.get(&id) {
                *self = my_data.clone()
            }
        }
        id
    }
}

impl<B> DisambNameRatchet<B> {
    fn for_person(
        db: &impl IrDatabase,
        var: NameVariable,
        value: PersonName,
        ref_id: &Atom,
        name_el: &NameEl,
        primary: bool,
        global: bool,
    ) -> Self {
        let mut data = DisambNameData {
            var,
            value,
            ref_id: ref_id.clone(),
            el: name_el.clone(),
            primary,
        };
        let id = data.lookup_id(db, global);
        let ratchet = PersonDisambNameRatchet::new(&db.style(), id, data);
        DisambNameRatchet::Person(ratchet)
    }
}

/// One NameIR per variable
pub fn to_individual_name_irs<'a, O: OutputFormat>(
    names: &'a Names,
    names_inheritance: &'a NamesInheritance,
    db: &'a impl IrDatabase,
    state: &'a IrState,
    fmt: &'a O,
    refr: &'a Reference,
    should_start_with_global: bool,
) -> impl Iterator<Item = NameIR<O>> + 'a {
    names
        .variables
        .iter()
        .filter(move |var| !state.is_name_suppressed(**var))
        .filter_map(move |var| refr.name.get(var).map(|val| (*var, val.clone())))
        .map(move |(var, value)| {
            let ratchets = value
                .into_iter()
                .enumerate()
                .map(|(n, value)| {
                    // Each variable gets its own 'primary' name.
                    let primary = n == 0;
                    match value {
                        Name::Person(pn) => DisambNameRatchet::for_person(
                            db,
                            var,
                            pn,
                            &refr.id,
                            &names_inheritance.name,
                            primary,
                            should_start_with_global,
                        ),
                        Name::Literal { literal } => {
                            DisambNameRatchet::Literal(fmt.text_node(literal, None))
                        }
                    }
                })
                .collect();
            NameIR::new(
                names_inheritance.clone(),
                var,
                ratchets,
                Box::new(IR::Rendered(None)),
            )
        })
}

fn render_label<O: OutputFormat, I: OutputFormat>(
    ctx: &CiteContext<'_, O, I>,
    label: &NameLabel,
    var: NameVariable,
) -> IR<O> {
    let renderer = Renderer::cite(ctx);
    let o = renderer.name_label(label, var).map(CiteEdgeData::Output);
    IR::Rendered(o)
}

use crate::NameOverrider;
use csl::SortKey;

pub fn sort_strings_for_names(
    db: &impl IrDatabase,
    refr: &Reference,
    var: NameVariable,
    sort_key: &SortKey,
    loc: CiteOrBib,
) -> Option<Vec<String>> {
    let style = db.style();
    let fmt = db.get_formatter();
    let (delim, arc_name_el) = match loc {
        CiteOrBib::Citation => style.name_info_citation(),
        CiteOrBib::Bibliography => style.name_info_bibliography(),
    };
    let name_o = NameOverrider::default();
    // Not clear from the spec whether we need to preserve the contextual name options or not.
    // This code does preserve them, and then forces NASO and form as is definitely required.
    let names_inheritance = name_o.inherited_names_options_sort_key(&arc_name_el, &delim, sort_key);
    let runner = OneNameVar {
        name_el: &names_inheritance.name.merge(&NameEl {
            name_as_sort_order: Some(NameAsSortOrder::All),
            form: Some(NameForm::Long),
            ..Default::default()
        }),
        bump_name_count: 0,
        demote_non_dropping_particle: style.demote_non_dropping_particle,
        initialize_with_hyphen: style.initialize_with_hyphen,
        fmt: &fmt,
    };
    let mut out = Vec::new();
    if let Some(values) = refr.name.get(&var) {
        for value in values {
            match value {
                Name::Person(pn) => {
                    runner.person_name_sort_keys(pn, &mut out);
                }
                Name::Literal { literal } => {
                    if !literal.is_empty() {
                        out.push(literal.clone());
                    }
                }
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

pub fn intermediate<'c, O: OutputFormat, I: OutputFormat>(
    names: &Names,
    db: &impl IrDatabase,
    state: &mut IrState,
    ctx: &CiteContext<'c, O, I>,
) -> IrSum<O> {
    let fmt = &ctx.format;
    let mut names_inheritance = state.name_override.inherited_names_options(
        &ctx.name_citation,
        &ctx.names_delimiter,
        names,
    );

    if let Some(key) = &ctx.sort_key {
        names_inheritance = names_inheritance.override_with(
            &ctx.name_citation,
            &ctx.names_delimiter,
            NamesInheritance::from_sort_key(key),
        );
    }

    let name_irs: Vec<IR<O>> = to_individual_name_irs(
        names,
        &names_inheritance,
        db,
        state,
        fmt,
        &ctx.reference,
        true,
    )
    .map(|mut nir| {
        if let Some((ir, _gv)) = nir.intermediate_custom(db, ctx, ctx.disamb_pass) {
            *nir.ir = ir;
            IR::Name(Arc::new(Mutex::new(nir)))
        } else {
            // shouldn't happen; intermediate_custom should return Some the first time
            // round in any situation, and only retun None if it's impossible to crank any
            // further for a disamb pass
            error!("nir.intermediate_custom returned None the first time round");
            IR::Rendered(None)
        }
    })
    .collect();

    // Wait until iteration is done to collect
    state.maybe_suppress_name_vars(&names.variables);

    if name_irs.is_empty()
        || name_irs.iter().all(|ir| match ir {
            IR::Name(nir) => nir.lock().unwrap().disamb_names.is_empty(),
            _ => true,
        })
    {
        if let Some(subst) = names.substitute.as_ref() {
            for el in subst.0.iter() {
                // Need to clone the state so that any ultimately-non-rendering names blocks do not affect
                // substitution later on
                let mut new_state = state.clone();
                let old = new_state
                    .name_override
                    .replace_name_overrides_for_substitute(names_inheritance.clone());
                let (ir, gv) = el.intermediate(db, &mut new_state, ctx);
                if !ir.is_empty() {
                    new_state.name_override.restore_name_overrides(old);
                    *state = new_state;
                    return (ir, gv);
                }
            }
        }
        return (IR::Rendered(None), GroupVars::OnlyEmpty);
    }

    // TODO: &[editor, translator] => &[editor], and use editortranslator on
    // the label

    (
        IR::Seq(IrSeq {
            contents: name_irs,
            formatting: names_inheritance.formatting,
            affixes: names_inheritance.affixes.clone(),
            delimiter: names_inheritance
                .delimiter
                .unwrap_or_else(|| Atom::from("")),
            display: if ctx.in_bibliography {
                names.display
            } else {
                None
            },
        }),
        GroupVars::DidRender,
    )
}

impl<'c, O, I> Proc<'c, O, I> for Names
where
    O: OutputFormat,
    I: OutputFormat,
{
    fn intermediate(
        &self,
        db: &impl IrDatabase,
        state: &mut IrState,
        ctx: &CiteContext<'c, O, I>,
    ) -> IrSum<O> {
        intermediate(self, db, state, ctx)
    }
}

impl<'c, O: OutputFormat> NameIR<O> {
    pub fn intermediate_custom<I: OutputFormat>(
        &mut self,
        _db: &impl IrDatabase,
        ctx: &CiteContext<'c, O, I>,
        pass: Option<DisambPass>,
    ) -> Option<IrSum<O>> {
        let style = ctx.style;
        let locale = ctx.locale;
        let fmt = &ctx.format;
        let position = ctx.position.0;

        let NameIR {
            ref names_inheritance,
            variable,
            ..
        } = *self;

        let mut runner = OneNameVar {
            name_el: &names_inheritance.name,
            bump_name_count: self.name_counter.bump,
            demote_non_dropping_particle: style.demote_non_dropping_particle,
            initialize_with_hyphen: style.initialize_with_hyphen,
            fmt,
        };

        let ntbs = runner.names_to_builds(
            &self.disamb_names,
            position,
            locale,
            &names_inheritance.et_al,
            ctx.sort_key.is_some(),
        );

        // TODO: refactor into a method on NameCounter
        self.name_counter.current = ntb_len(&ntbs);
        if pass == Some(DisambPass::AddNames)
            && self.name_counter.current <= self.name_counter.max_recorded
        {
            return None;
        }
        self.name_counter.max_recorded = self.name_counter.current;

        let iter = ntbs
            .into_iter()
            .map(|ntb| match ntb {
                NameTokenBuilt::Built(b) => b,
                NameTokenBuilt::Ratchet(DisambNameRatchet::Literal(b)) => b.clone(),
                NameTokenBuilt::Ratchet(DisambNameRatchet::Person(pn)) => {
                    runner.name_el = &pn.data.el;
                    let ret = runner.render_person_name(&pn.data.value, !pn.data.primary);
                    runner.name_el = &names_inheritance.name;
                    ret
                }
            })
            .filter(|x| !fmt.is_empty(&x))
            .map(|x| IR::Rendered(Some(CiteEdgeData::Output(x))));
        let mut seq = IrSeq {
            contents: iter.collect(),
            formatting: runner.name_el.formatting,
            affixes: runner.name_el.affixes.clone(),
            delimiter: Atom::from(""),
            display: None,
        };
        if seq.contents.is_empty() {
            Some((IR::Rendered(None), GroupVars::OnlyEmpty))
        } else {
            if let Some(label) = names_inheritance.label.as_ref() {
                let label_ir = render_label(ctx, &label.concrete(), variable);
                if label.after_name {
                    seq.contents.push(label_ir);
                } else {
                    seq.contents.insert(0, label_ir);
                }
            }
            Some((IR::Seq(seq), GroupVars::DidRender))
        }
    }
}

pub fn ntb_len<B>(v: &[NameTokenBuilt<'_, B>]) -> u16 {
    v.iter()
        .filter(|x| match x {
            NameTokenBuilt::Ratchet(_) => true,
            _ => false,
        })
        .count() as u16
}

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
        .filter(|npt| pn_filter_part(pn, *npt))
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

fn pn_filter_part(pn: &PersonName, token: NamePartToken) -> bool {
    match token {
        NamePartToken::Given => pn.given.is_some(),
        NamePartToken::Family => pn.family.is_some(),
        NamePartToken::NonDroppingParticle => pn.non_dropping_particle.is_some(),
        NamePartToken::DroppingParticle => pn.dropping_particle.is_some(),
        NamePartToken::Suffix => pn.suffix.is_some(),
        NamePartToken::Space => true,
        NamePartToken::SortSeparator => true,
    }
}

fn should_delimit_after<O: OutputFormat>(
    prec: DelimiterPrecedes,
    name: &OneNameVar<'_, O>,
    count_before_spot: usize,
) -> bool {
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
enum NameToken<'a, B> {
    Name(&'a DisambNameRatchet<B>),
    EtAl,
    Ellipsis,
    Delimiter,
    And,
    Space,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct OneNameVar<'a, O: OutputFormat> {
    pub name_el: &'a NameEl,
    pub bump_name_count: u16,
    // From Style
    pub demote_non_dropping_particle: DemoteNonDroppingParticle,
    pub initialize_with_hyphen: bool,
    pub fmt: &'a O,
}

impl<'a, O: OutputFormat> OneNameVar<'a, O> {
    #[inline]
    fn naso(&self, seen_one: bool) -> bool {
        match self.name_el.name_as_sort_order {
            None => false,
            Some(NameAsSortOrder::First) => !seen_one,
            Some(NameAsSortOrder::All) => true,
        }
    }

    #[inline]
    fn ea_min(&self, pos: Position) -> usize {
        let first = self.name_el.et_al_min.unwrap_or(0);
        let min = if pos == Position::First {
            first as usize
        } else {
            self.name_el.et_al_subsequent_min.unwrap_or(first) as usize
        };
        let use_first = self.ea_use_first(pos);
        std::cmp::max(min, use_first + 1)
    }

    #[inline]
    fn ea_use_first(&self, pos: Position) -> usize {
        let first = self.name_el.et_al_use_first.unwrap_or(1);
        let use_first = if pos == Position::First {
            first as usize
        } else {
            self.name_el.et_al_subsequent_use_first.unwrap_or(first) as usize
        };
        use_first + self.bump_name_count as usize
    }

    fn name_tokens<'s>(
        &self,
        position: Position,
        names_slice: &'s [DisambNameRatchet<O::Build>],
        is_sort_key: bool,
    ) -> Vec<NameToken<'s, O::Build>> {
        let name_count = names_slice.len();
        let ea_min = self.ea_min(position);
        let ea_use_first = self.ea_use_first(position);
        if self.name_el.enable_et_al() && name_count >= ea_min {
            if self.name_el.et_al_use_last == Some(true) && ea_use_first + 2 <= name_count {
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
                if !is_sort_key {
                    let dpea = self
                        .name_el
                        .delimiter_precedes_et_al
                        .unwrap_or(DelimiterPrecedes::Contextual);
                    if should_delimit_after(dpea, self, ea_use_first) {
                        nms.push(NameToken::Delimiter);
                    } else {
                        nms.push(NameToken::Space);
                    }
                    nms.push(NameToken::EtAl);
                }
                nms
            }
        } else {
            let mut nms = names_slice
                .iter()
                .map(NameToken::Name)
                .intercalate(&NameToken::Delimiter);
            // "delimiter-precedes-last" would be better named as "delimiter-precedes-and",
            // because it only has any effect when "and" is set.
            if self.name_el.and.is_some() {
                if let Some(last_delim) = nms.iter().rposition(|t| *t == NameToken::Delimiter) {
                    let dpl = self
                        .name_el
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

    pub(crate) fn person_name_sort_keys(&self, pn: &PersonName, out: &mut Vec<String>) {
        let order = get_sort_order(
            pn_is_latin_cyrillic(pn),
            self.name_el.form == Some(NameForm::Long),
            self.demote_non_dropping_particle,
        );
        for sort_token in order {
            let mut s = String::new();
            for token in sort_token
                .iter()
                .cloned()
                .filter(|npt| pn_filter_part(pn, *npt))
            {
                match token {
                    NamePartToken::Given => {
                        if let Some(ref given) = pn.given {
                            // TODO: parametrize for disambiguation
                            let string = initialize(
                                &given,
                                self.name_el.initialize.unwrap_or(true),
                                self.name_el.initialize_with.as_ref().map(|s| s.as_ref()),
                                self.initialize_with_hyphen,
                            );
                            s.push_str(&string);
                        }
                    }
                    NamePartToken::Family => {
                        if let Some(ref family) = pn.family {
                            s.push_str(&family);
                        }
                    }
                    NamePartToken::NonDroppingParticle => {
                        s.push_str(&pn.non_dropping_particle.as_ref().unwrap());
                    }
                    NamePartToken::DroppingParticle => {
                        s.push_str(&pn.dropping_particle.as_ref().unwrap());
                    }
                    NamePartToken::Suffix => {
                        s.push_str(&pn.suffix.as_ref().unwrap());
                    }
                    NamePartToken::Space => {}
                    NamePartToken::SortSeparator => {}
                }
            }
            if !s.is_empty() {
                out.push(s);
            }
        }
    }

    fn format_with_part(&self, o_part: &Option<NamePart>, s: &str) -> O::Build {
        let fmt = self.fmt;
        match o_part {
            None => fmt.plain(s),
            Some(ref part) => {
                // TODO: text-case, IngestOptions
                fmt.affixed(fmt.text_node(s.to_string(), part.formatting), part.affixes.as_ref())
            }
        }
    }

    pub(crate) fn render_person_name(&self, pn: &PersonName, seen_one: bool) -> O::Build {
        let fmt = self.fmt;

        let order = get_display_order(
            pn_is_latin_cyrillic(pn),
            self.name_el.form == Some(NameForm::Long),
            self.naso(seen_one),
            self.demote_non_dropping_particle,
        );

        let mut build = vec![];
        for part in pn_filtered_parts(pn, order) {
            // We already tested is_some() for all these Some::unwrap() calls
            match part {
                NamePartToken::Given => {
                    if let Some(ref given) = pn.given {
                        let name_part = &self.name_el.name_part_given;
                        // TODO: parametrize for disambiguation
                        let string = initialize(
                            &given,
                            self.name_el.initialize.unwrap_or(true),
                            self.name_el.initialize_with.as_ref().map(|s| s.as_ref()),
                            self.initialize_with_hyphen,
                        );
                        build.push(self.format_with_part(name_part, &string));
                    }
                }
                NamePartToken::Family => {
                    let name_part = &self.name_el.name_part_family;
                    let string = pn.family.as_ref().unwrap();
                    build.push(self.format_with_part(name_part, &string));
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
                    build.push(if let Some(sep) = &self.name_el.sort_separator {
                        fmt.plain(&sep)
                    } else {
                        fmt.plain(", ")
                    });
                }
            }
        }

        fmt.seq(build.into_iter())
    }

    /// without the <name /> formatting and affixes applied
    pub(crate) fn names_to_builds<'b: 'a>(
        &self,
        names_slice: &'b [DisambNameRatchet<O::Build>],
        position: Position,
        locale: &csl::locale::Locale,
        et_al: &Option<NameEtAl>,
        is_sort_key: bool,
    ) -> Vec<NameTokenBuilt<'b, O::Build>> {
        let fmt = self.fmt.clone();
        let mut seen_one = false;
        let name_tokens = self.name_tokens(position, names_slice, is_sort_key);

        if self.name_el.form == Some(NameForm::Count) {
            let count: u32 = name_tokens.iter().fold(0, |acc, name| match name {
                NameToken::Name(_) => acc + 1,
                _ => acc,
            });
            if is_sort_key {
                let b = fmt.affixed_text(
                    format!("{:08}", count),
                    None,
                    Some(&crate::sort::natural_sort::num_affixes()),
                );
                return vec![NameTokenBuilt::Built(b)];
            } else {
                // This isn't sort-mode, you can render NameForm::Count as text.
                return vec![NameTokenBuilt::Built(
                    fmt.text_node(format!("{}", count), None),
                )];
            }
        }

        name_tokens
            .iter()
            .map(|n| match n {
                NameToken::Name(ratchet) => {
                    seen_one = true;
                    NameTokenBuilt::Ratchet(ratchet)
                }
                NameToken::Delimiter => {
                    NameTokenBuilt::Built(if let Some(delim) = &self.name_el.delimiter {
                        fmt.plain(&delim.0)
                    } else {
                        fmt.plain(", ")
                    })
                }
                NameToken::EtAl => {
                    use csl::*;
                    let mut term = MiscTerm::EtAl;
                    let mut formatting = None;
                    if let Some(ref etal_element) = et_al {
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
                    NameTokenBuilt::Built(fmt.text_node(text.to_string(), formatting))
                }
                NameToken::Ellipsis => NameTokenBuilt::Built(fmt.plain("…")),
                NameToken::Space => NameTokenBuilt::Built(fmt.plain(" ")),
                NameToken::And => {
                    use csl::terms::*;
                    let select = |form: TermFormExtended| {
                        TextTermSelector::Simple(SimpleTermSelector::Misc(MiscTerm::And, form))
                    };
                    // If an And token shows up, we already know self.name_el.and is Some.
                    let form = match self.name_el.and {
                        Some(NameAnd::Symbol) => "&",
                        // locale
                        //     .get_text_term(select(TermFormExtended::Symbol), false)
                        //     .unwrap_or("&"),
                        _ => locale
                            .get_text_term(select(TermFormExtended::Long), false)
                            .unwrap_or("and"),
                    };
                    NameTokenBuilt::Built(fmt.plain(form))
                }
            })
            .collect()
    }
}

pub enum NameTokenBuilt<'a, B> {
    Ratchet(&'a DisambNameRatchet<B>),
    Built(B),
}

use self::ord::{get_display_order, get_sort_order, DisplayOrdering, NamePartToken};

#[allow(dead_code)]
mod ord {
    //! Latin here means latin or cyrillic.
    //! TODO: use the regex crate with \\p{Cyrillic} and \\p{Latin}

    use csl::DemoteNonDroppingParticle as DNDP;

    pub type DisplayOrdering = &'static [NamePartToken];

    #[derive(Clone, Copy, PartialEq, Debug)]
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
    pub type SortToken = &'static [NamePartToken];

    use self::NamePartToken::*;

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
        &[NonDroppingParticle, Family],
        &[DroppingParticle],
        &[Given],
        &[Suffix],
    ];
    static LATIN_SORT: SortOrdering = &[
        &[Family],
        &[DroppingParticle, NonDroppingParticle],
        &[Given],
        &[Suffix],
    ];

    static NON_LATIN_LONG: DisplayOrdering = &[
        Family, // TODO: how do we determine if spaces are required?
        Given,
    ];
    static NON_LATIN_SHORT: DisplayOrdering = &[Family];
    static NON_LATIN_SORT_LONG: SortOrdering = &[&[Family], &[Given]];
    static NON_LATIN_SORT_SHORT: SortOrdering = &[&[Family]];
}
