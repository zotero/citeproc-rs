use crate::db::{with_bib_context, with_cite_context};
use crate::prelude::*;
use citeproc_db::{ClusterData, ClusterId, ClusterNumber};
use citeproc_io::{ClusterMode, DateOrRange};
use csl::{style::*, terms::*, variables::*, Atom};
use fnv::FnvHashMap;
use std::sync::Arc;

mod lexical;
pub mod natural_sort;
pub(crate) use lexical::Natural;
mod output_format;
pub(crate) use output_format::SortStringFormat;

fn plain_macro_element(macro_name: SmartString) -> TextElement {
    TextElement {
        source: TextSource::Macro(macro_name),
        formatting: None,
        affixes: Default::default(),
        quotes: false,
        strip_periods: false,
        text_case: TextCase::None,
        display: None,
    }
}

fn ctx_sort_string(
    db: &dyn IrDatabase,
    ctx: &CiteContext<Markup, Markup>,
    macro_name: SmartString,
) -> SmartString {
    let mut walker = SortingWalker::new(db, &ctx);
    let text = plain_macro_element(macro_name.clone());
    let (string, _gv) = walker.text_macro(&text, &macro_name);
    string
}

/// Distinguish between uncited and cited items for sorting the `citation-number` variable or
/// macro.
///
/// In sorting routines, we use BibNumber::cited_only() because we want (a) uncited items to be
/// mixed into a bibliography only if people literally specify a sort key that would do that, but
/// we also (b) want the 'demoting none' behaviour to apply to citation-number when it is used, and
/// for those uncited items to be very much last; finally (c) any time we actually *render* a
/// citation-number, it is still the position in the bibliography.
///
/// No sort keys at all                 =>  uncited items go last
/// key variable="title"                =>  uncited items mixed into bibliography
/// key variable="citation-number"      =>  uncited items go last
/// key variable="citation-number" desc =>  uncited items STILL go last
/// citation-number, title              =>  uncited items AT END are sorted by title among themselves
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BibNumber {
    Cited(u32),
    Uncited(u32),
}

impl BibNumber {
    pub fn get(&self) -> u32 {
        match *self {
            BibNumber::Cited(b) | BibNumber::Uncited(b) => b,
        }
    }
    pub fn cited_only(&self) -> Option<u32> {
        match *self {
            BibNumber::Cited(b) => Some(b),
            _ => None,
        }
    }
}

pub fn sorted_refs(db: &dyn IrDatabase) -> Arc<(Vec<Atom>, FnvHashMap<Atom, BibNumber>)> {
    let style = db.style();
    let bib = match style.bibliography {
        None => None,
        Some(ref b) => b.sort.as_ref(),
    };

    let mut citation_numbers = FnvHashMap::default();

    // Construct preordered, which will then be stably sorted. It contains:
    // - All refs from all cites, in the order they appear (excluding non-existent)
    // - Then, all of the uncited reference ids.
    //
    // first, compute refs in the order that they are cited.
    // stable sorting will cause this to be the final tiebreaker.
    let all = db.all_keys();
    let cited_keys = db.cited_keys();
    let disamb_participants = db.disamb_participants();
    let mut preordered = Vec::with_capacity(all.len());

    // Put all the cited refs in
    let mut i = 1;
    for id in cited_keys.iter() {
        if !citation_numbers.contains_key(id) {
            preordered.push(id.clone());
            citation_numbers.insert(id.clone(), BibNumber::Cited(i as u32));
            i += 1;
        }
    }
    // "The rest" ie the uncited items
    for id in disamb_participants.difference(&cited_keys) {
        if !citation_numbers.contains_key(id) {
            preordered.push(id.clone());
            citation_numbers.insert(id.clone(), BibNumber::Uncited(i as u32));
            i += 1;
        }
    }

    let max_cnum = preordered.len() as u32;
    let mut reverse = false;
    let now_sorted = if db.bibliography_no_sort() {
        preordered
    } else if let Some(ref sort) = bib {
        preordered.sort_by_cached_key(|a| {
            let a_cnum = citation_numbers
                .get(a)
                .expect("must have an citation_number entry for every bibliography item")
                .clone();
            let refr_arc = db.reference(a.clone());
            let demoting = with_bib_context(
                db,
                a.clone(),
                refr_arc.as_deref(),
                a_cnum.cited_only(),
                None,
                None,
                |_, mut a_ctx| {
                    Some(ctx_sort_items(
                        db,
                        CiteOrBib::Bibliography,
                        &mut a_ctx,
                        a_cnum,
                        sort,
                        max_cnum,
                    ))
                },
                |_, _, _| None,
            );
            log::debug!("(Bibliography) sort items for {:?}: {:?}", a_cnum, demoting);
            if let Some(Demoting {
                fake_cnum: Some(_), ..
            }) = &demoting
            {
                reverse = true;
            }
            demoting
        });
        preordered
    } else {
        // In the absence of cs:sort, cites and bibliographic entries appear in the order in which
        // they are cited. The uncited ones come last.
        preordered
    };
    for (i, ref_id) in now_sorted.iter().enumerate() {
        let mut i = i as u32 + 1u32;
        if reverse {
            i = max_cnum + 1 - i;
        }
        if let Some(bn) = citation_numbers.get_mut(&ref_id) {
            match bn {
                BibNumber::Cited(x) => *x = i,
                BibNumber::Uncited(x) => *x = i,
            }
        }
    }
    Arc::new((now_sorted, citation_numbers))
}

pub fn clusters_cites_sorted(db: &dyn IrDatabase) -> Arc<Vec<ClusterData>> {
    let cluster_ids = db.clusters_ordered();
    let mut clusters: Vec<_> = cluster_ids
        .iter()
        // No number? Not considered to be in document, position participant.
        // Although may be disamb participant.
        .filter_map(|&id| db.cluster_data_sorted(id))
        .collect();
    clusters.sort_by_key(|cluster| cluster.number);
    Arc::new(clusters)
}

pub fn cluster_data_sorted(db: &dyn IrDatabase, id: ClusterId) -> Option<ClusterData> {
    db.cluster_note_number(id).map(|mut number| {
        // mode = AuthorOnly means number should be ignored, cluster placed outside flow of
        // document.
        if let Some(ClusterMode::AuthorOnly) = db.cluster_mode(id) {
            number = ClusterNumber::OutsideFlow;
        }
        // Order of operations: bib gets sorted first, so cites can be sorted by
        // citation-number.
        let sorted_refs_arc = db.sorted_refs();
        let (_keys, citation_numbers_by_id) = &*sorted_refs_arc;
        let mut cites = db.cluster_cites(id);
        let style = db.style();
        let max_cnum = citation_numbers_by_id.len() as u32;
        if let Some(sort) = style.citation.sort.as_ref() {
            let mut neu = (*cites).clone();
            let getter = |cite_id: &CiteId| -> Option<BibNumber> {
                let cite = cite_id.lookup(db);
                let cnum = db.reference(cite.ref_id.clone()).map(|refr| {
                    citation_numbers_by_id
                        .get(&refr.id)
                        .expect("every cited reference should appear in sorted_refs")
                        .clone()
                });
                cnum
            };
            neu.sort_by_cached_key(|a| {
                getter(a).map(|a_cnum| {
                    let demoting = with_cite_context(
                        db,
                        a.clone(),
                        a_cnum.cited_only(),
                        // not set because this is per-sort-key, which we will set in
                        // ctx_sort_items
                        None,
                        true,
                        // Year suffix not available in sorting routines. Is that right?
                        None,
                        |mut a_ctx| {
                            ctx_sort_items(
                                db,
                                CiteOrBib::Citation,
                                &mut a_ctx,
                                a_cnum,
                                sort,
                                max_cnum,
                            )
                        },
                    );
                    log::debug!("sort items for {:?}: {:?}", a_cnum, demoting);
                    demoting
                })
            });
            cites = Arc::new(neu);
        }
        ClusterData { id, number, cites }
    })
}

/// May be None if the cite's reference does not exist.
pub fn bib_number(db: &dyn IrDatabase, id: CiteId) -> Option<BibNumber> {
    let cite = id.lookup(db);
    let arc = db.sorted_refs();
    let (_, ref lookup_ref_ids) = &*arc;
    lookup_ref_ids.get(&cite.ref_id).cloned()
}

#[derive(Debug, PartialEq, Eq)]
struct SortItem {
    direction: Option<SortDirection>,
    value: SortValue,
}
#[derive(Debug, PartialEq, Eq)]
enum SortValue {
    Macro(Option<NaturalCmp>),
    Cnum(Option<u32>),
    OrdinaryVariable(Option<Natural<SmartString>>),
    Number(Option<citeproc_io::NumericValueOwned>),
    Names(Option<Vec<Natural<SmartString>>>),
    Date(Option<DateOrRange>),
}

use std::cmp::Ordering;
#[derive(Debug)]
enum Demoted {
    Left,
    Right,
}
use natural_sort::NaturalCmp;

/// This implements the part of the spec
#[derive(Debug, Eq)]
struct Demoting {
    fake_cnum: Option<u32>,
    items: Vec<SortItem>,
}

impl PartialEq for Demoting {
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items
    }
}

impl PartialOrd for Demoting {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Demoting {
    fn cmp(&self, other: &Self) -> Ordering {
        assert_eq!(self.items.len(), other.items.len());

        let mut ord = Ordering::Equal;
        for pair in self.items.iter().zip(other.items.iter()) {
            let (aa, bb) = pair;
            let dir = aa.direction;
            assert_eq!(dir, bb.direction);
            use SortValue::*;
            let (ordering, demoted) = match (&aa.value, &bb.value) {
                (Cnum(a), Cnum(b)) => compare_demoting_none(a.as_ref(), b.as_ref()),
                (Macro(a), Macro(b)) => compare_demoting_none(a.as_ref(), b.as_ref()),
                (OrdinaryVariable(a), OrdinaryVariable(b)) => compare_demoting_none(a.as_ref(), b.as_ref()),
                (Number(a), Number(b)) => compare_demoting_none(a.as_ref(), b.as_ref()),
                (Names(a), Names(b)) => compare_demoting_none(a.as_ref(), b.as_ref()),
                (Date(a), Date(b)) => compare_demoting_none(a.as_ref(), b.as_ref()),
                _ => unreachable!("SortItems should be constructed in the same order producing the exact same sequence"),
            };
            ord = match (dir, demoted) {
                // Wants to be reversed, but overridden by demotion
                (_, Some(Demoted::Left)) => Ordering::Greater,
                (_, Some(Demoted::Right)) => Ordering::Less,
                (Some(SortDirection::Descending), _) => ordering.reverse(),
                _ => ordering,
            };
            if ord != Ordering::Equal {
                break;
            }
        }
        ord
    }
}

fn compare_demoting_none<T: PartialOrd>(
    aa: Option<&T>,
    bb: Option<&T>,
) -> (Ordering, Option<Demoted>) {
    match (aa, bb) {
        (None, None) => (Ordering::Equal, None),
        (None, Some(_)) => (Ordering::Greater, Some(Demoted::Left)),
        (Some(_), None) => (Ordering::Less, Some(Demoted::Right)),
        (Some(aaa), Some(bbb)) => (aaa.partial_cmp(bbb).unwrap_or(Ordering::Equal), None),
    }
}

fn ctx_sort_items(
    db: &dyn IrDatabase,
    // Cached lookup from (id, macro name, sort key) -> a comparable string
    cite_or_bib: CiteOrBib,
    a_ctx: &mut CiteContext<'_, Markup, Markup>,
    a_cnum: BibNumber,
    sort: &Sort,
    max_cnum: u32,
) -> Demoting {
    let sort_string = |ctx: &mut CiteContext<Markup, Markup>,
                       macro_name: SmartString,
                       key: SortKey,
                       cnum: Option<u32>| {
        ctx.bib_number = cnum;
        if cite_or_bib == CiteOrBib::Bibliography {
            ctx.sort_key = Some(key);
            ctx_sort_string(db, ctx, macro_name)
        } else {
            ctx.sort_key = Some(key);
            ctx_sort_string(db, ctx, macro_name)
        }
    };

    use std::cell::Cell;
    let fake_cnum = Cell::new(None);
    let mut items = Vec::with_capacity(sort.keys.len());
    let mut push_item = |item: SortItem| {
        // Reverse direction after we have seen a descending / citation-number key
        if cite_or_bib == CiteOrBib::Bibliography {
            if let SortItem {
                direction: Some(SortDirection::Descending),
                value: SortValue::Cnum(Some(_)),
            } = item
            {
                fake_cnum.set(a_cnum.cited_only().map(|a| max_cnum + 1 - a));
            } else if let SortItem {
                direction: Some(SortDirection::Ascending),
                value: SortValue::Cnum(Some(_)),
            } = item
            {
                fake_cnum.set(None);
            }
        }
        items.push(item);
    };
    for key in sort.keys.iter() {
        let value = match key.sort_source {
            SortSource::Macro(ref macro_name) => {
                let a_string =
                    sort_string(a_ctx, macro_name.clone(), key.clone(), a_cnum.cited_only());
                if let Some(cnum) = natural_sort::extract_citation_number(&a_string) {
                    // We found a <text value="citation-number"/> (or number)
                    let cnum_item = SortItem {
                        direction: key.direction,
                        value: SortValue::Cnum(Some(cnum)),
                    };
                    push_item(cnum_item);
                }
                let a_nat = NaturalCmp::new(a_string);
                SortValue::Macro(a_nat)
            }
            // For variables, we're not going to use the CiteContext wrappers, because if a
            // variable is not defined directly on the reference, it shouldn't be sortable-by, so
            // will just come back as None from reference.xxx.get() and produce Equal.
            SortSource::Variable(any) => match any {
                AnyVariable::Ordinary(v) => {
                    use citeproc_io::micro_html_to_string;
                    fn strip_markup(s: impl AsRef<str>) -> SmartString {
                        micro_html_to_string(s.as_ref(), &Default::default())
                    }
                    let got = a_ctx
                        .get_ordinary(v, VariableForm::default())
                        .map(strip_markup)
                        .map(Natural::new);
                    SortValue::OrdinaryVariable(got)
                }
                AnyVariable::Number(NumberVariable::CitationNumber) => {
                    SortValue::Cnum(fake_cnum.get().or(a_cnum.cited_only()))
                }
                AnyVariable::Number(v) => SortValue::Number(a_ctx.get_number(v).map(Into::into)),
                AnyVariable::Name(v) => {
                    let a_strings = crate::names::sort_strings_for_names(
                        db,
                        &a_ctx.reference,
                        v,
                        key,
                        cite_or_bib,
                    );
                    SortValue::Names(a_strings)
                }
                // TODO: compare dates, using details from spec for ranges
                AnyVariable::Date(v) => {
                    let a_date = a_ctx.reference.date.get(&v);
                    SortValue::Date(a_date.cloned())
                }
            },
        };
        let item = SortItem {
            direction: key.direction,
            value,
        };
        push_item(item)
    }
    Demoting {
        items,
        fake_cnum: fake_cnum.get(),
    }
}

/// A walker for producing sort strings. These are encoded with `natural_sort` components, so the
/// output is destined for comparing with the `NaturalCmp` wrapper.
///
/// The cite context has to produce text of a different format to usual, SortStringFormat.
/// This does some extra normalisation / irrelevant character removal, and does not have formatting
/// at all.
struct SortingWalker<'a, I: OutputFormat> {
    db: &'a dyn IrDatabase,
    /// the cite is in its original format, but the formatter is PlainText
    ctx: CiteContext<'a, SortStringFormat, I>,
    state: IrState,
    /// Use this for generating names and dates, and not creating a new one each time
    arena: IrArena<SortStringFormat>,
}

impl<'a, I: OutputFormat> SortingWalker<'a, I> {
    pub fn new<O: OutputFormat>(db: &'a dyn IrDatabase, ctx: &'a CiteContext<'a, O, I>) -> Self {
        let plain_ctx = ctx.change_format(SortStringFormat);
        SortingWalker {
            db,
            ctx: plain_ctx,
            state: Default::default(),
            arena: Default::default(),
        }
    }

    fn renderer(&'a self) -> Renderer<'a, SortStringFormat, I> {
        Renderer::gen(GenericContext::Cit(&self.ctx))
    }
}

impl<'a, O: OutputFormat> StyleWalker for SortingWalker<'a, O> {
    type Output = (SmartString, GroupVars);
    type Checker = CiteContext<'a, SortStringFormat, O>;

    fn default(&mut self) -> Self::Output {
        Default::default()
    }
    fn get_checker(&self) -> Option<&Self::Checker> {
        Some(&self.ctx)
    }

    fn fold(&mut self, elements: &[Element], fold_type: WalkerFoldType) -> Self::Output {
        let iter = elements.iter();
        let mut output: Option<SmartString> = None;
        // Avoid allocating one new string
        let mut gv_acc = GroupVars::new();
        for el in iter {
            let (child, gv) = self.element(el);
            gv_acc = gv_acc.neighbour(gv);
            output = match output {
                Some(ref mut s) => {
                    s.push_str(&child);
                    continue;
                }
                None => Some(child),
            }
        }
        let out = output.unwrap_or_default();
        let is_empty = out.is_empty();
        match fold_type {
            WalkerFoldType::Group(_g) => gv_acc.implicit_conditional(out, is_empty),
            _ => (out, gv_acc),
        }
    }

    fn text_value(&mut self, text: &TextElement, value: &SmartString) -> Self::Output {
        let renderer = self.renderer();
        let val = renderer.text_value(text, &value);
        (val.unwrap_or_default(), GroupVars::new())
    }

    fn text_term(
        &mut self,
        text: &TextElement,
        sel: TextTermSelector,
        plural: bool,
    ) -> Self::Output {
        let renderer = self.renderer();
        let val = renderer.text_term(text, sel, plural);
        (val.unwrap_or_default(), GroupVars::new())
    }

    // TODO: reinstate variable suppression
    fn text_variable(
        &mut self,
        text: &TextElement,
        svar: StandardVariable,
        form: VariableForm,
    ) -> Self::Output {
        let renderer = self.renderer();
        let res = match svar {
            StandardVariable::Number(nvar) => self.ctx.get_number(nvar).map(|nval| {
                if nvar == NumberVariable::CitationNumber {
                    renderer.number_sort_string(nvar, NumericForm::Numeric, &nval)
                } else {
                    renderer.text_variable(text, svar, nval.verbatim())
                }
            }),
            StandardVariable::Ordinary(var) => self
                .ctx
                .get_ordinary(var, form)
                .map(|val| renderer.text_variable(text, svar, &val)),
        };
        let gv = GroupVars::rendered_if(res.is_some());
        (res.unwrap_or_default(), gv)
    }

    // TODO: reinstate variable suppression
    fn number(&mut self, number: &NumberElement) -> Self::Output {
        let renderer = self.renderer();
        let var = number.variable;
        let content = self
            .ctx
            .get_number(var)
            .map(|val| renderer.number_sort_string(var, number.form, &val));
        let gv = GroupVars::rendered_if(content.is_some());
        (content.unwrap_or_default(), gv)
    }

    fn label(&mut self, label: &LabelElement) -> Self::Output {
        let renderer = self.renderer();
        let var = label.variable;
        let content = self
            .ctx
            .get_number(var)
            .and_then(|val| renderer.numeric_label(label, &val));
        (content.unwrap_or_default(), GroupVars::new())
    }

    // SPEC:
    // For name sorting, there are four advantages in using the same macro rendering
    // and sorting, instead of sorting directly on the name variable.
    //
    // 1.  First, substitution is available (e.g. the "editor" variable might
    //     substitute for an empty "author" variable).
    // 2.  Secondly, et-al abbreviation can be used (using either the
    //     et-al-min/et-al-subsequent-min, et-al-use-first/et-al-subsequent-use-first,
    //     and et-al-use-last options defined for the macro, or the overriding
    //     names-min, names-use-first and names-use-last attributes set on cs:key).
    //     When et-al abbreviation occurs, the "et-al" and "and others" terms are
    //     excluded from the sort key values.
    // 3.  Thirdly, names can be sorted by just the surname (using a macro for which
    //     the form attribute on cs:name is set to "short").
    // 4.  Finally, it is possible to sort by the number of names in a name list, by
    //     calling a macro for which the form attribute on cs:name is set to "count".
    //     As for names sorted via the variable attribute, names sorted via macro are
    //     returned with the cs:name attribute name-as-sort-order set to "all".
    //
    //     So
    //
    //     1. Override naso = all,
    //     2. Exclude et-al and & others terms,
    //     3. Return count as a {:08} padded number

    fn names(&mut self, names: &Names) -> Self::Output {
        let node =
            crate::names::intermediate(names, self.db, &mut self.state, &self.ctx, &mut self.arena);
        let tree = IrTreeRef::new(node, &self.arena);
        let gv = tree.get_node().unwrap().get().1;
        (tree.flatten(&self.ctx.format, None).unwrap_or_default(), gv)
    }

    // The spec is not functional. Specificlly, negative/BCE years won't work. So the year must be
    // interpreted as a number, and the rest can still be a string. Hence CmpDate below.
    //
    fn date(&mut self, date: &BodyDate) -> Self::Output {
        let node = date.intermediate(self.db, &mut self.state, &self.ctx, &mut self.arena);
        let tree = IrTreeRef::new(node, &self.arena);
        let gv = tree.get_node().unwrap().get().1;
        (tree.flatten(&self.ctx.format, None).unwrap_or_default(), gv)
    }

    fn text_macro(&mut self, text: &TextElement, name: &SmartString) -> Self::Output {
        // TODO: same todos as in Proc
        let style = self.ctx.style;
        let macro_elements = style
            .macros
            .get(name)
            .expect("undefined macro should not be valid CSL");

        self.state.push_macro(name);
        let ret = self.fold(macro_elements, WalkerFoldType::Macro(text));
        self.state.pop_macro(name);
        ret
    }
}

#[cfg(test)]
fn sort_string_bibliography(
    db: &dyn IrDatabase,
    ref_id: Atom,
    macro_name: SmartString,
    key: SortKey,
) -> Option<Arc<SmartString>> {
    let refr_arc = db.reference(ref_id.clone());
    with_bib_context(
        db,
        ref_id,
        refr_arc.as_deref(),
        None,
        Some(key),
        None,
        |_bib, ctx| Some(Arc::new(ctx_sort_string(db, &ctx, macro_name))),
        |_, _, _| None,
    )
}

#[test]
fn test_date_as_macro_strip_delims() {
    use crate::test::MockProcessor;
    let mut db = MockProcessor::new();
    let mut refr = citeproc_io::Reference::empty("ref_id".into(), CslType::Book);
    use citeproc_io::{Date, DateOrRange};
    refr.ordinary.insert(Variable::Title, String::from("title"));
    refr.date.insert(
        DateVariable::Issued,
        DateOrRange::Single(Date::new(2000, 1, 1)),
    );
    db.insert_references(vec![refr]);
    db.set_style_text(r#"<?xml version="1.0" encoding="utf-8"?>
        <style version="1.0" class="note">
           <macro name="year-date">
               <date variable="issued">
                 <date-part name="year" />
               </date>
           </macro>
           <macro name="year-date-choose">
             <choose>
                 <if variable="issued">
                    <date variable="issued">
                       <date-part name="year"/>
                    </date>
                 </if>
                 <else>
                    <text term="no date" form="short"/>
                 </else>
              </choose>
           </macro>
           <macro name="local">
               <date variable="issued" date-parts="year" form="numeric"/>
           </macro>
           <macro name="term">
             <text term="anonymous"/>
           </macro>
           <macro name="indep">
             <text variable="title" />
             <date variable="issued">
               <date-part name="year" form="short" prefix="PREFIX" suffix="SUFFIX" />
               <date-part name="month" form="long" vertical-align="sup" prefix="PREFIX" suffix="SUFFIX" />
             </date>
           </macro>
           <citation><layout></layout></citation>
           <bibliography>
             <sort>
               <key macro="indep" />
             </sort>
             <layout>
             </layout>
           </bibliography>
        </style>
    "#);

    assert_eq!(
        sort_string_bibliography(
            &db,
            "ref_id".into(),
            "indep".into(),
            SortKey::macro_named("indep")
        ),
        Some(Arc::new("title\u{e000}2000_01/0000_00\u{e001}".into()))
    );

    assert_eq!(
        sort_string_bibliography(
            &db,
            "ref_id".into(),
            "local".into(),
            SortKey::macro_named("local")
        ),
        Some(Arc::new("\u{e000}2000_/0000_\u{e001}".into()))
    );

    assert_eq!(
        sort_string_bibliography(
            &db,
            "ref_id".into(),
            "year-date".into(),
            SortKey::macro_named("year-date")
        ),
        Some(Arc::new("\u{e000}2000_/0000_\u{e001}".into()))
    );

    assert_eq!(
        sort_string_bibliography(
            &db,
            "ref_id".into(),
            "year-date-choose".into(),
            SortKey::macro_named("year-date-choose")
        ),
        Some(Arc::new("\u{e000}2000_/0000_\u{e001}".into()))
    );

    assert_eq!(
        sort_string_bibliography(
            &db,
            "ref_id".into(),
            "term".into(),
            SortKey::macro_named("term")
        ),
        Some(Arc::new("anonymous".into()))
    );
}
