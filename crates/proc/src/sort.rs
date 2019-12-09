use crate::db::with_bib_context;
use crate::prelude::*;
use citeproc_io::output::plain::PlainText;
use citeproc_io::Reference;
use csl::*;
use fnv::FnvHashMap;
use std::sync::Arc;

use std::cmp::Ordering;

fn plain_macro_element(macro_name: Atom) -> TextElement {
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

pub fn sort_string_citation(
    _db: &impl IrDatabase,
    _ref_id: Atom,
    _macro_name: Atom,
) -> Arc<String> {
    unimplemented!()
}

// Cached by the DB because typically the output needs to be compared more than once
pub fn sort_string_bibliography(
    db: &impl IrDatabase,
    ref_id: Atom,
    macro_name: Atom,
    key: SortKey,
) -> Option<Arc<String>> {
    with_bib_context(db, ref_id.clone(), None, Some(key), |_bib, ctx| {
        let mut walker = SortingWalker::new(db, &ctx);
        let text = plain_macro_element(macro_name.clone());
        let (string, _gv) = walker.text_macro(&text, &macro_name);
        // info!("{} macro {} produced: {}", ref_id, macro_name, string);
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
            let a_cnum = citation_numbers.get(a).unwrap();
            let b_cnum = citation_numbers.get(b).unwrap();
            let ar = db.reference_input(a.clone());
            let br = db.reference_input(b.clone());
            bib_ordering(db, &ar, &br, *a_cnum, *b_cnum, sort, &style)
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
    a_cnum: u32,
    b_cnum: u32,
    sort: &Sort,
    _style: &Style,
) -> Ordering {
    #[derive(Debug)]
    enum Demoted {
        Left,
        Right,
    }
    use natural_sort::NaturalCmp;
    fn compare_demoting_none<T: Ord>(aa: Option<T>, bb: Option<T>) -> (Ordering, Option<Demoted>) {
        match (aa, bb) {
            (None, None) => (Ordering::Equal, None),
            (None, Some(_)) => (Ordering::Greater, Some(Demoted::Left)),
            (Some(_), None) => (Ordering::Less, Some(Demoted::Right)),
            (Some(aaa), Some(bbb)) => (aaa.cmp(&bbb), None),
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
                let a_string =
                    db.sort_string_bibliography(a.id.clone(), macro_name.clone(), key.clone());
                let b_string =
                    db.sort_string_bibliography(b.id.clone(), macro_name.clone(), key.clone());
                let a_nat = a_string.as_ref().and_then(|x| NaturalCmp::new(x));
                let b_nat = b_string.as_ref().and_then(|x| NaturalCmp::new(x));
                let x = compare_demoting_none(a_nat, b_nat);
                info!(
                    "cmp macro {}: {} {:?} {:?} {} {:?}",
                    macro_name, a.id, a_string, x.0, b.id, b_string
                );
                x
            }
            // For variables, we're not going to use the CiteContext wrappers, because if a
            // variable is not defined directly on the reference, it shouldn't be sortable-by, so
            // will just come back as None from reference.xxx.get() and produce Equal.
            SortSource::Variable(any) => match any {
                AnyVariable::Ordinary(v) => {
                    compare_demoting_none(a.ordinary.get(&v), b.ordinary.get(&v))
                }
                AnyVariable::Number(NumberVariable::CitationNumber) => {
                    compare_demoting_none(Some(a_cnum), Some(b_cnum))
                }
                AnyVariable::Number(v) => compare_demoting_none(a.number.get(&v), b.number.get(&v)),
                AnyVariable::Name(v) => {
                    let a_strings = crate::names::sort_strings_for_names(
                        db,
                        a,
                        v,
                        key,
                        CiteOrBib::Bibliography,
                    );
                    let b_strings = crate::names::sort_strings_for_names(
                        db,
                        b,
                        v,
                        key,
                        CiteOrBib::Bibliography,
                    );
                    compare_demoting_none(a_strings.as_ref(), b_strings.as_ref())
                }
                AnyVariable::Date(_v) => (Ordering::Equal, None),
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
    state: IrState,
}

impl<'a, DB: IrDatabase, I: OutputFormat> SortingWalker<'a, DB, I> {
    pub fn new<O: OutputFormat>(db: &'a DB, ctx: &'a CiteContext<'a, O, I>) -> Self {
        let plain_ctx = ctx.change_format(PlainText);
        SortingWalker {
            db,
            ctx: plain_ctx,
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
        let iter = elements.iter();
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
            renderer.number_sort_string(
                var,
                number.form,
                &val,
                number.affixes.as_ref(),
                number.text_case,
            )
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

    fn date(&mut self, date: &BodyDate) -> Self::Output {
        let (ir, gv) = date.intermediate(self.db, &mut self.state, &self.ctx);
        (ir.flatten(&self.ctx.format).unwrap_or_default(), gv)
    }

    fn text_macro(&mut self, text: &TextElement, name: &Atom) -> Self::Output {
        // TODO: same todos as in Proc
        let style = self.ctx.style;
        let macro_unsafe = style
            .macros
            .get(name)
            .expect("macro errors not implemented!");

        self.state.push_macro(name);
        let ret = self.fold(macro_unsafe, WalkerFoldType::Macro(text));
        self.state.pop_macro(name);
        ret
    }
}

// dates: Date variables called via the variable attribute are returned in the YYYYMMDD format,
// with zeros substituted for any missing date-parts (e.g. 20001200 for December 2000). As a
// result, less specific dates precede more specific dates in ascending sorts, e.g. “2000, May
// 2000, May 1st 2000”. Negative years are sorted inversely, e.g. “100BC, 50BC, 50AD, 100AD”.
// Seasons are ignored for sorting, as the chronological order of the seasons differs between the
// northern and southern hemispheres. In the case of date ranges, the start date is used for the
// primary sort, and the end date is used for a secondary sort, e.g. “2000–2001, 2000–2005,
// 2002–2003, 2002–2009”. Date ranges are placed after single dates when they share the same
// (start) date, e.g. “2000, 2000–2002”.

// Basically, everything would be very easy without the BC/AD sorting and the ranges coming later
// parts. But given these, we have to parse dates again.
pub mod natural_sort {

    // From the BMP(0) unicode private use area
    // Delimits a date so it can be parsed when doing a natural sort comparison
    pub const DATE_START: char = '\u{E000}';
    pub const DATE_START_STR: &str = "\u{E000}";
    pub const DATE_END: char = '\u{E001}';
    pub const DATE_END_STR: &str = "\u{E001}";

    // Delimits a number so it can be compared
    pub const NUM_START: char = '\u{E002}';
    pub const NUM_START_STR: &str = "\u{E002}";
    pub const NUM_END: char = '\u{E003}';
    pub const NUM_END_STR: &str = "\u{E003}";

    pub fn date_affixes() -> Affixes {
        Affixes {
            prefix: DATE_START_STR.into(),
            suffix: DATE_END_STR.into(),
        }
    }

    pub fn num_affixes() -> Affixes {
        Affixes {
            prefix: NUM_START_STR.into(),
            suffix: NUM_END_STR.into(),
        }
    }

    #[derive(PartialEq, Eq, Debug)]
    struct CmpDate<'a> {
        year: i32,
        rest: &'a str,
    }

    impl<'a> Ord for CmpDate<'a> {
        fn cmp(&self, other: &Self) -> Ordering {
            self.year
                .cmp(&other.year)
                .then_with(|| self.rest.cmp(other.rest))
        }
    }

    impl<'a> PartialOrd for CmpDate<'a> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(
                self.year
                    .cmp(&other.year)
                    .then_with(|| self.rest.cmp(other.rest)),
            )
        }
    }

    #[derive(PartialEq, Eq, Debug)]
    enum CmpRange<'a> {
        Single(CmpDate<'a>),
        Range(CmpDate<'a>, CmpDate<'a>),
    }

    impl<'a> Ord for CmpRange<'a> {
        fn cmp(&self, other: &Self) -> Ordering {
            match (self, other) {
                (CmpRange::Single(a), CmpRange::Single(b)) => a.cmp(b),
                (CmpRange::Single(a), CmpRange::Range(b, _c)) => a.cmp(b),
                (CmpRange::Range(a, _b), CmpRange::Single(c)) => a.cmp(c),
                (CmpRange::Range(a, b), CmpRange::Range(c, d)) => a.cmp(c).then_with(|| b.cmp(d)),
            }
        }
    }

    impl<'a> PartialOrd for CmpRange<'a> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    use csl::Affixes;
    use nom::{
        branch::alt,
        bytes::complete::{take_while, take_while1, take_while_m_n},
        character::complete::char,
        combinator::{map, opt},
        sequence::delimited,
        IResult,
    };
    use std::cmp::Ordering;
    use std::str::FromStr;

    fn to_u32(s: &str) -> u32 {
        FromStr::from_str(s.trim_start_matches('0')).unwrap()
    }

    fn to_i32(s: &str) -> i32 {
        FromStr::from_str(s).unwrap()
    }

    fn take_4_digits(inp: &str) -> IResult<&str, &str> {
        take_while_m_n(4, 4, |c: char| c.is_ascii_digit())(inp)
    }

    fn take_8_digits(inp: &str) -> IResult<&str, &str> {
        take_while_m_n(8, 8, |c: char| c.is_ascii_digit())(inp)
    }

    fn year_prefix(inp: &str) -> IResult<&str, char> {
        alt((char('+'), char('-')))(inp)
    }

    fn year(inp: &str) -> IResult<&str, i32> {
        let (rem1, pref) = opt(year_prefix)(inp)?;
        let (rem2, y) = take_4_digits(rem1)?;
        Ok((
            rem2,
            match pref {
                Some('-') => -to_i32(y),
                _ => to_i32(y),
            },
        ))
    }

    fn date(inp: &str) -> IResult<&str, CmpDate> {
        let (rem1, year) = year(inp)?;
        fn still_date(c: char) -> bool {
            c != DATE_END && c != '-'
        }
        let (rem2, rest) = take_while(still_date)(rem1)?;
        Ok((rem2, CmpDate { year, rest }))
    }

    fn range(inp: &str) -> IResult<&str, Token> {
        let (rem1, _) = char(DATE_START)(inp)?;
        let (rem2, first) = date(rem1)?;
        fn and_ymd(inp: &str) -> IResult<&str, CmpDate> {
            let (rem1, _) = char('-')(inp)?;
            Ok(date(rem1)?)
        }
        let (rem3, d2) = opt(and_ymd)(rem2)?;
        let (rem4, _) = char(DATE_END)(rem3)?;
        Ok((
            rem4,
            Token::Date(match d2 {
                None => CmpRange::Single(first),
                Some(d) => CmpRange::Range(first, d),
            }),
        ))
    }

    fn num(inp: &str) -> IResult<&str, Token> {
        delimited(
            char(NUM_START),
            map(take_8_digits, |x| Token::Num(to_u32(x))),
            char(NUM_END),
        )(inp)
    }

    fn str_token(inp: &str) -> IResult<&str, Token> {
        fn normal(c: char) -> bool {
            !(c == DATE_START || c == NUM_START)
        }
        map(take_while1(normal), Token::Str)(inp)
    }

    fn token(inp: &str) -> IResult<&str, Token> {
        alt((str_token, num, range))(inp)
    }

    struct TokenIterator<'a> {
        remain: &'a str,
    }

    #[derive(PartialEq, Debug)]
    enum Token<'a> {
        Str(&'a str),
        Num(u32),
        Date(CmpRange<'a>),
    }

    impl<'a> PartialOrd for Token<'a> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            match (self, other) {
                (Token::Str(a), Token::Str(b)) => a.partial_cmp(b),
                (Token::Date(a), Token::Date(b)) => a.partial_cmp(b),
                (Token::Num(a), Token::Num(b)) => a.partial_cmp(b),
                _ => None,
            }
        }
    }

    impl<'a> Iterator for TokenIterator<'a> {
        type Item = Token<'a>;
        fn next(&mut self) -> Option<Self::Item> {
            if self.remain.is_empty() {
                return None;
            }
            if let Ok((remainder, token)) = token(self.remain) {
                self.remain = remainder;
                Some(token)
            } else {
                None
            }
        }
    }

    #[derive(PartialEq, Eq)]
    pub struct NaturalCmp<'a>(&'a str);
    impl<'a> NaturalCmp<'a> {
        pub fn new(s: &'a str) -> Option<Self> {
            if s.is_empty() {
                None
            } else {
                Some(NaturalCmp(s))
            }
        }
    }
    impl<'a> PartialOrd for NaturalCmp<'a> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl<'a> Ord for NaturalCmp<'a> {
        fn cmp(&self, other: &Self) -> Ordering {
            natural_cmp(self.0, other.0)
        }
    }

    fn natural_cmp(a: &str, b: &str) -> Ordering {
        let a_i = TokenIterator { remain: a };
        let b_i = TokenIterator { remain: b };
        let mut iter = a_i.zip(b_i);
        let mut o = Ordering::Equal;
        while let Some((a_t, b_t)) = iter.next() {
            if o != Ordering::Equal {
                return o;
            }
            if let Some(c) = a_t.partial_cmp(&b_t) {
                o = c;
            }
        }
        o
    }

    #[test]
    fn natural_cmp_strings() {
        env_logger::init();
        assert_eq!(natural_cmp("a", "z"), Ordering::Less, "a - z");
        assert_eq!(natural_cmp("z", "a"), Ordering::Greater, "z - a");
        assert_eq!(
            natural_cmp("a\u{E000}20090407\u{E001}", "a\u{E000}20080407\u{E001}"),
            Ordering::Greater,
            "2009"
        );
        assert_eq!(
            natural_cmp("a\u{E000}20090507\u{E001}", "a\u{E000}20090407\u{E001}"),
            Ordering::Greater
        );
        assert_eq!(
            natural_cmp("a\u{E000}-0100\u{E001}", "a\u{E000}0100\u{E001}"),
            Ordering::Less,
            "100BC < 100AD"
        );

        // 2000, May 2000, May 1st 2000
        assert_eq!(
            natural_cmp("a\u{E000}2000\u{E001}", "a\u{E000}200004\u{E001}"),
            Ordering::Less,
            "2000 < May 2000"
        );
        assert_eq!(
            natural_cmp("a\u{E000}200004\u{E001}", "a\u{E000}20000401\u{E001}"),
            Ordering::Less,
            "May 2000 < May 1st 2000"
        );

        assert_eq!(
            natural_cmp("\u{E002}1000\u{E003}", "\u{E001}1000\u{E003}"),
            Ordering::Equal,
            "1000 == 1000"
        );
        assert_eq!(
            natural_cmp("\u{E002}1000\u{E003}", "\u{E001}2000\u{E003}"),
            Ordering::Less,
            "1000 < 2000"
        );
    }
}
