use std::sync::Arc;
use citeproc_io::Reference;
use fnv::FnvHashMap;
use crate::db::with_bib_context;
use crate::prelude::*;
use csl::Atom;
use citeproc_io::output::plain::PlainText;
use std::borrow::Cow;

use csl::variables::*;
use csl::style::*;

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

pub fn sort_string_bibliography(db: &impl IrDatabase, ref_id: Atom, macro_name: Atom) -> Option<Arc<String>> {
    with_bib_context(db, ref_id.clone(), None, |bib, ctx| {
        let mut walker = SortingWalker::new(&ctx);
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
            crate::sort::bib_ordering(db, &ar, &br, sort, &style)
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
pub fn bib_ordering(db: &impl IrDatabase,  a: &Reference, b: &Reference, sort: &Sort, _style: &Style) -> Ordering {
    enum Demoted {
        Left,
        Right,
    }
    fn compare_demoting_none<T: Ord>(aa: Option<&T>, bb: Option<&T>) -> (Ordering, Option<Demoted>) {
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
                let a_string = db.sort_string_bibliography(a.id.clone(), macro_name.clone());
                let b_string = db.sort_string_bibliography(b.id.clone(), macro_name.clone());
                (a_string.cmp(&b_string), None)
            },
            // For variables, we're not going to use the CiteContext wrappers, because if a
            // variable is not defined directly on the reference, it shouldn't be sortable-by, so
            // will just come back as None from reference.xxx.get() and produce Equal.
            SortSource::Variable(any) => match any {
                AnyVariable::Ordinary(v) => {
                    compare_demoting_none(a.ordinary.get(&v), b.ordinary.get(&v))
                }
                AnyVariable::Number(v) => compare_demoting_none(a.number.get(&v), b.number.get(&v)),
                AnyVariable::Name(_) => (Ordering::Equal, None),
                AnyVariable::Date(_) => (Ordering::Equal, None),
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

struct SortingWalker<'a, O: OutputFormat> {
    ctx: &'a CiteContext<'a, O>,
    macro_stack: Vec<Atom>,
    plain_fmt: PlainText,
}

impl<'a, O: OutputFormat> SortingWalker<'a, O> {
    pub fn new(ctx: &'a CiteContext<'a, O>) -> Self {
        SortingWalker {
            ctx,
            macro_stack: Vec::new(),
            plain_fmt: PlainText,
        }
    }

    fn renderer(&'a self) -> Renderer<'a, O, PlainText> {
        Renderer::sorting(GenericContext::Cit(self.ctx), &self.plain_fmt)
    }
}

impl<'a, O: OutputFormat> StyleWalker for SortingWalker<'a, O> {
    type Output = (String, GroupVars);
    type Checker = GenericContext<'a, O>;

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
                None => {
                    Some(child)
                }
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
            StandardVariable::Number(nvar) => {
                self.ctx.get_number(nvar).map(|nval| {
                    renderer.text_variable(text, svar, nval.verbatim())
                })
            }
            StandardVariable::Ordinary(var) => {
                self.ctx.get_ordinary(var, form).map(|val| {
                    renderer.text_variable(text, svar, val)
                })
            }
        };
        let gv = GroupVars::rendered_if(res.is_some());
        (res.unwrap_or_default(), gv)
    }

    // TODO: reinstate variable suppression
    fn number(&mut self, number: &NumberElement) -> Self::Output {
        let renderer = self.renderer();
        let var = number.variable;
        let content = self.ctx.get_number(var)
            .map(|val| renderer.number_sort_string(var, number.form, &val, &number.affixes, number.text_case));
        let gv = GroupVars::rendered_if(content.is_some());
        (content.unwrap_or_default(), gv)
    }

    fn text_macro(&mut self, text: &TextElement, name: &Atom) -> Self::Output {
        // TODO: same todos as in Proc
        let style = self.ctx.style;
        let macro_unsafe = style.macros.get(name).expect("macro errors not implemented!");

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
