use crate::db::with_bib_context;
use crate::prelude::*;
use citeproc_io::output::plain::PlainText;
use citeproc_io::Reference;
use csl::Atom;
use fnv::FnvHashMap;
use std::borrow::Cow;
use std::sync::Arc;

use csl::style::*;
use csl::variables::*;

use csl::variables::*;
use std::cmp::Ordering;

fn plain_macro_element(macro_name: Atom) -> TextElement {
    use csl::style::{Element, TextCase, TextSource, VariableForm};
    use csl::variables::{StandardVariable, Variable};
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

pub fn sort_string_citation(db: &impl IrDatabase, ref_id: Atom, macro_name: Atom) -> Arc<String> {
    unimplemented!()
}

// Cached by the DB because typically the output needs to be compared more than once
pub fn sort_string_bibliography(
    db: &impl IrDatabase,
    ref_id: Atom,
    macro_name: Atom,
    key: SortKey,
) -> Option<Arc<String>> {
    with_bib_context(db, ref_id.clone(), None, Some(key), |bib, ctx| {
        let mut walker = SortingWalker::new(db, &ctx);
        let mut text = plain_macro_element(macro_name.clone());
        let (string, _gv) = walker.text_macro(&text, &macro_name);
        info!("{} macro {} produced: {}", ref_id, macro_name, string);
        Arc::new(string)
    })
}

pub fn sorted_refs(db: &impl IrDatabase) -> Arc<(Vec<Atom>, FnvHashMap<Atom, u32>)> {
    let style = db.style();
    let bib = match style.bibliography {
        None => None,
        Some(ref b) => b.sort.as_ref(),
    };

    let mut citation_numbers = FnvHashMap::default();

    // only the references that exist go in the bibliography
    // first, compute refs in the order that they are cited.
    // stable sorting will cause this to be the final tiebreaker.
    let all = db.all_keys();
    let all_cite_ids = db.all_cite_ids();
    let mut preordered = Vec::with_capacity(all.len());
    let mut i = 1;
    for &id in all_cite_ids.iter() {
        let ref_id = &id.lookup(db).ref_id;
        if all.contains(ref_id) && !citation_numbers.contains_key(ref_id) {
            preordered.push(ref_id.clone());
            citation_numbers.insert(ref_id.clone(), i as u32);
            i += 1;
        }
    }
    let refs = if let Some(ref sort) = bib {
        // dbg!(sort);
        preordered.sort_by(|a, b| {
            let ar = db.reference_input(a.clone());
            let br = db.reference_input(b.clone());
            bib_ordering(db, &ar, &br, sort, &style)
        });
        preordered
    } else {
        // In the absence of cs:sort, cites and bibliographic entries appear in the order in which
        // they are cited.
        preordered
    };
    for (i, ref_id) in refs.iter().enumerate() {
        citation_numbers.insert(ref_id.clone(), (i + 1) as u32);
    }
    Arc::new((refs, citation_numbers))
}

pub fn bib_number(db: &impl IrDatabase, id: CiteId) -> Option<u32> {
    let cite = id.lookup(db);
    let arc = db.sorted_refs();
    let (_, ref lookup_ref_ids) = &*arc;
    lookup_ref_ids.get(&cite.ref_id).cloned()
}

/// Creates a total ordering of References from a Sort element. (Not a query)
pub fn bib_ordering(
    db: &impl IrDatabase,
    a: &Reference,
    b: &Reference,
    sort: &Sort,
    _style: &Style,
) -> Ordering {
    enum Demoted {
        Left,
        Right,
    }
    fn compare_demoting_none<T: Ord>(
        aa: Option<&T>,
        bb: Option<&T>,
    ) -> (Ordering, Option<Demoted>) {
        match (aa, bb) {
            (None, None) => (Ordering::Equal, None),
            (None, Some(_)) => (Ordering::Greater, Some(Demoted::Left)),
            (Some(_), None) => (Ordering::Less, Some(Demoted::Right)),
            (Some(aaa), Some(bbb)) => (aaa.cmp(bbb), None),
        }
    }
    let mut ord = Ordering::Equal;
    for key in sort.keys.iter() {
        // If an ordering is found, you don't need to tie-break any further with more sort keys.
        if ord != Ordering::Equal {
            break;
        }
        let (o, demoted) = match key.sort_source {
            SortSource::Macro(ref macro_name) => {
                let a_string = db.sort_string_bibliography(a.id.clone(), macro_name.clone(), key.clone());
                let b_string = db.sort_string_bibliography(b.id.clone(), macro_name.clone(), key.clone());
                info!("cmp macro {}: {:?} <> {:?}", macro_name, a_string, b_string);
                (a_string.cmp(&b_string), None)
            }
            // For variables, we're not going to use the CiteContext wrappers, because if a
            // variable is not defined directly on the reference, it shouldn't be sortable-by, so
            // will just come back as None from reference.xxx.get() and produce Equal.
            SortSource::Variable(any) => match any {
                AnyVariable::Ordinary(v) => {
                    compare_demoting_none(a.ordinary.get(&v), b.ordinary.get(&v))
                }
                AnyVariable::Number(v) => compare_demoting_none(a.number.get(&v), b.number.get(&v)),
                AnyVariable::Name(v) => {
                    let a_strings = crate::names::sort_strings_for_names(db, a, v, key, CiteOrBib::Bibliography);
                    let b_strings = crate::names::sort_strings_for_names(db, b, v, key, CiteOrBib::Bibliography);
                    info!("{:?} <-> {:?}", a_strings, b_strings);
                    compare_demoting_none(a_strings.as_ref(), b_strings.as_ref())
                }
                AnyVariable::Date(v) => (Ordering::Equal, None),
            },
        };
        ord = match (key.direction.as_ref(), demoted) {
            // Wants to be reversed, but overridden by demotion
            (_, Some(Demoted::Left)) => Ordering::Greater,
            (_, Some(Demoted::Right)) => Ordering::Less,
            (Some(SortDirection::Descending), _) => o.reverse(),
            _ => o,
        };
    }
    ord
}

/// Currently only works where 
struct SortingWalker<'a, DB: IrDatabase, I: OutputFormat> {
    db: &'a DB,
    /// the cite is in its original format, but the formatter is PlainText
    ctx: CiteContext<'a, PlainText, I>,
    macro_stack: Vec<Atom>,
    plain_fmt: PlainText,
    state: IrState,
}

impl<'a, DB: IrDatabase, I: OutputFormat> SortingWalker<'a, DB, I> {
    pub fn new<O: OutputFormat>(db: &'a DB, ctx: &'a CiteContext<'a, O, I>) -> Self {
        let plain_ctx = ctx.change_format(PlainText);
        SortingWalker {
            db,
            ctx: plain_ctx,
            macro_stack: Vec::new(),
            plain_fmt: PlainText,
            state: Default::default(),
        }
    }

    fn renderer(&'a self) -> Renderer<'a, PlainText, I> {
        Renderer::sorting(GenericContext::Cit(&self.ctx))
    }
}

impl<'a, DB: IrDatabase, O: OutputFormat> StyleWalker for SortingWalker<'a, DB, O> {
    type Output = (String, GroupVars);
    type Checker = GenericContext<'a, PlainText>;

    fn fold(&mut self, elements: &[Element], _fold_type: WalkerFoldType) -> Self::Output {
        let mut iter = elements.iter();
        let mut output: Option<String> = None;
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
        (output.unwrap_or_default(), gv_acc)
    }

    fn text_value(&mut self, text: &TextElement, value: &Atom) -> Self::Output {
        let renderer = self.renderer();
        let val = renderer.text_value(text, &value);
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
            StandardVariable::Number(nvar) => self
                .ctx
                .get_number(nvar)
                .map(|nval| renderer.text_variable(text, svar, nval.verbatim())),
            StandardVariable::Ordinary(var) => self
                .ctx
                .get_ordinary(var, form)
                .map(|val| renderer.text_variable(text, svar, val)),
        };
        let gv = GroupVars::rendered_if(res.is_some());
        (res.unwrap_or_default(), gv)
    }

    // TODO: reinstate variable suppression
    fn number(&mut self, number: &NumberElement) -> Self::Output {
        let renderer = self.renderer();
        let var = number.variable;
        let content = self.ctx.get_number(var).map(|val| {
            renderer.number_sort_string(var, number.form, &val, &number.affixes, number.text_case)
        });
        let gv = GroupVars::rendered_if(content.is_some());
        (content.unwrap_or_default(), gv)
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
        let (ir, gv) = crate::names::intermediate(names, self.db, &mut self.state, &self.ctx);
        (ir.flatten(&self.ctx.format).unwrap_or_default(), gv)
    }

    fn text_macro(&mut self, text: &TextElement, name: &Atom) -> Self::Output {
        // TODO: same todos as in Proc
        let style = self.ctx.style;
        let macro_unsafe = style
            .macros
            .get(name)
            .expect("macro errors not implemented!");

        if self.macro_stack.contains(&name) {
            panic!(
                "foiled macro recursion: {} called from within itself; exiting",
                &name
            );
        }
        self.macro_stack.push(name.clone());
        let ret = self.fold(macro_unsafe, WalkerFoldType::Macro(text));
        self.macro_stack.pop();
        ret
    }
}
