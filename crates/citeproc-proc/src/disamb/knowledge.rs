use csl::Cond;
use csl::{Context, CslType, Position};
use csl::{AnyVariable, NumberVariable};
use fnv::{FnvHashMap, FnvHashSet};
use generational_arena::{Arena, Index as ArenaIndex};

type CondIndex = ArenaIndex;
type SomeOfIndex = ArenaIndex;
type GenerationIndex = usize;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Maybe {
    True,
    False,
    // You should check some_ofs
    PartOf(SomeOfIndex),
}

#[derive(Copy, Clone)]
enum Possibility {
    Possible(Maybe),
    Unnecessary,
    Impossible,
    Base,
}

impl Possibility {
    fn base() -> Self {
        Possibility::Base
    }
    fn folder(self, m: &(GenerationIndex, Maybe)) -> Self {
        use Maybe::*;
        use Possibility::*;
        let (_, m) = m;
        match (self, *m) {
            (Base, x)
            | (Possible(PartOf(_)), x)
            | (Possible(True), x @ True)
            | (Possible(False), x @ False) => Possible(x),
            (Possible(True), PartOf(_)) | (Possible(False), PartOf(_)) => Unnecessary,
            (Unnecessary, _) => Unnecessary,
            (Possible(True), False) | (Possible(False), True) | (Impossible, _) => Impossible,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct SingleKnowledge(Vec<(GenerationIndex, Maybe)>);

#[test]
fn test_pop_larger_than() {
    let mut sk = SingleKnowledge(vec![(1, Maybe::True), (2, Maybe::True), (3, Maybe::True)]);
    sk.pop_larger_than(2);
    assert_eq!(
        sk,
        SingleKnowledge(vec![(1, Maybe::True), (2, Maybe::True)])
    );
    sk.pop_larger_than(2);
    assert_eq!(
        sk,
        SingleKnowledge(vec![(1, Maybe::True), (2, Maybe::True)])
    );
    sk.pop_larger_than(1);
    assert_eq!(sk, SingleKnowledge(vec![(1, Maybe::True)]));
    sk.pop_larger_than(0);
    assert_eq!(sk, SingleKnowledge(vec![]));
}

impl SingleKnowledge {
    fn pop_larger_than(&mut self, ix: GenerationIndex) {
        // Go backwards. Slightly faster.
        if let Some(i) = self.0.iter().rev().position(|(g, _)| *g <= ix) {
            self.0.truncate(self.0.len() - i);
        } else {
            self.0.clear();
        }
    }

    fn determinism(&self) -> Option<Result<bool, Vec<SomeOfIndex>>> {
        use Maybe::*;

        let mut indices = Vec::new();
        let mut folder = Err(());
        for &(_, mayb) in &self.0 {
            folder = match (folder, mayb) {
                (x, PartOf(i)) => {
                    indices.push(i);
                    x
                }
                (Err(()), True) => Ok(true),
                (Err(()), False) => Ok(false),
                (Ok(true), True) => Ok(true),
                (Ok(false), False) => Ok(false),
                (Ok(true), False) => return None,
                (Ok(false), True) => return None,
            };
        }
        Some(folder.map_err(|_| indices))
    }

    /// returns true if actually pushed
    fn push(&mut self, ix: GenerationIndex, m: Maybe) -> bool {
        self.pop_larger_than(ix);
        let poss = self.0.iter().fold(Possibility::Base, Possibility::folder);
        if let Possibility::Possible(_) = poss {
            self.0.push((ix, m));
            return true;
        }
        return false;
    }
}

type Generation = FnvHashSet<SomeOfIndex>;

pub struct Knowledge {
    known: FnvHashMap<Cond, SingleKnowledge>,
    some_ofs: Arena<FnvHashSet<(Cond, bool)>>,
    gens: Vec<Generation>,
    current_gen: GenerationIndex,
}

impl Knowledge {
    pub fn new() -> Self {
        Knowledge {
            known: Default::default(),
            some_ofs: Arena::new(),
            gens: vec![Default::default()],
            current_gen: 1,
        }
    }

    fn current_gen(&self) -> &Generation {
        &self.gens[self.current_gen - 1]
    }
    fn lookup_gen(&self, ix: GenerationIndex) -> Option<&Generation> {
        self.gens.get(ix - 1)
    }
    fn insert_know(&mut self, item: (Cond, bool)) {
        let (cond, truth) = item;
        if let Some(existing) = self.known.get_mut(&cond) {
            existing.pop_larger_than(self.current_gen);
            let b = if truth { Maybe::True } else { Maybe::False };
            existing.push(self.current_gen, b);
        } else {
            let b = if truth { Maybe::True } else { Maybe::False };
            let v = SingleKnowledge(vec![(self.current_gen, b)]);
            self.known.insert(cond.clone(), v);
        }
    }
    pub fn know_all_of(&mut self, things: impl Iterator<Item = (Cond, bool)> + Clone) {
        for pair in things {
            let inferences = cond_inferences(&pair);
            self.insert_know(pair);
            for inference in inferences {
                self.insert_know(inference);
            }
        }
    }

    pub fn know_some_of(&mut self, iter: impl Iterator<Item = (Cond, bool)> + Clone) {
        // let filtered = iter.filter(|(cond, truth)| {
        //     if self.demonstrates(&cond, !truth) {
        //         return false;
        //     }
        // })
        if iter.clone().any(|c| self.demonstrates(&c.0, c.1)) {
            // if any are already known, there is no new knowledge to be derived here.
            return;
        }
        let set = self.some_ofs.insert(FnvHashSet::default());
        let mut some_of_set = FnvHashSet::default();
        for (cond, truth) in iter {
            if self.demonstrates(&cond, !truth) {
                continue;
            }
            if let Some(existing) = self.known.get_mut(&cond) {
                if !existing.push(self.current_gen, Maybe::PartOf(set)) {
                    some_of_set.insert((cond.clone(), truth));
                }
            } else {
                let sk = SingleKnowledge(vec![(self.current_gen, Maybe::PartOf(set))]);
                self.known.insert(cond.clone(), sk);
                some_of_set.insert((cond.clone(), truth));
            }
        }
        self.some_ofs[set] = some_of_set;
        let gen = &mut self.gens[self.current_gen - 1];
        gen.insert(set);
    }

    fn is_determined_inner(&self, cond: &Cond) -> Option<bool> {
        if let Some(existing) = self.known.get(cond) {
            let det = existing.determinism();
            // dbg!(&det);
            if let Some(Ok(x)) = det {
                return Some(x);
            }
            if let Some(Err(sets_to_check)) = det {
                for s in sets_to_check {
                    if let Some(set) = &self.some_ofs.get(s) {
                        let mut own = None;
                        let all_others_disproven = set.iter().all(|(other_cond, other_truth)| {
                            if other_cond == cond {
                                own = Some(*other_truth);
                                return true;
                            }
                            // dbg!(&self.known.get(other_cond));
                            if let Some(other_existing) = self.known.get(other_cond) {
                                let other_det = other_existing.determinism();
                                // dbg!((&other_det, *other_truth));
                                if let Some(Ok(x)) = other_det {
                                    return x != *other_truth;
                                }
                                false
                            } else {
                                false
                            }
                        });
                        // dbg!((own, all_others_disproven));
                        if all_others_disproven {
                            return own;
                        }
                    }
                }
            }
        }
        None
    }

    pub fn is_determined(&self, cond: &Cond) -> bool {
        self.is_determined_inner(cond).is_some()
    }
    pub fn demonstrates(&self, cond: &Cond, truth: bool) -> bool {
        self.is_determined_inner(cond) == Some(truth)
    }

    pub fn push(&mut self) -> GenerationIndex {
        let before_gen = self.current_gen;
        let gen = Default::default();
        self.gens.push(gen);
        self.current_gen = self.gens.len();

        // Consolidate all the knowledge we have amassed, and make inferences from it
        let keys = self.known.keys().cloned().collect::<Vec<_>>();
        for cond in keys {
            if let Some(truth) = self.is_determined_inner(&cond) {
                self.insert_know((cond.clone(), truth));
                let inferences = cond_inferences(&(cond, truth));
                for inf in inferences {
                    self.insert_know(inf);
                }
            }
        }
        before_gen
    }
    pub fn pop(&mut self) {
        if let Some(gen) = self.gens.pop() {
            for set in gen {
                self.some_ofs.remove(set);
            }
            self.current_gen = self.gens.len();
        }
        for (_, v) in self.known.iter_mut() {
            v.pop_larger_than(self.current_gen);
        }
    }

    pub fn rollback(&mut self, to: GenerationIndex) {
        while self.current_gen > to {
            self.pop();
        }
    }
}

// XXX: turn this into an imperative routine that makes inferences (and can do from multiple vars
// at once) using a macro?
// Just so it doesn't have to iterate over every single cond in the database and mostly construct
// empty vec![]s
pub fn cond_inferences(pair: &(Cond, bool)) -> Vec<(Cond, bool)> {
    use Cond as C;
    let t = pair.1;
    match (&pair.0, t) {
        (C::Position(Position::NearNote), true) => {
            vec![(C::Position(Position::Subsequent), true),
            (C::Position(Position::FarNote), false)]
        }
        (C::Position(Position::FarNote), true) => {
            vec![(C::Position(Position::Subsequent), true),
            (C::Position(Position::FarNote), false)]
        }
        (C::Position(Position::Subsequent), true) => vec![],
        (C::Position(Position::Subsequent), false) => {
            vec![
                (C::Position(Position::Ibid), false),
                (C::Position(Position::IbidWithLocator), false),
                (C::Position(Position::FarNote), false),
                (C::Position(Position::NearNote), false),
            ]
        }
        (C::Position(Position::Ibid), true) => {
            vec![
                (C::Position(Position::Subsequent), true),
            ]
        }
        (C::Position(Position::IbidWithLocator), true) => {
            vec![
                (C::Position(Position::Ibid), true),
                (C::Variable(AnyVariable::Number(NumberVariable::Locator)), true),
                (C::Position(Position::Subsequent), true),
            ]
        }
        (C::Position(Position::IbidWithLocator), false) => {
            vec![
                (C::Position(Position::Ibid), false),
                (C::Position(Position::Subsequent), true),
            ]
        }
        (Cond::Context(Context::Citation), _) => vec![(Cond::Context(Context::Bibliography), !t)],
        (Cond::Disambiguate(d), _) => vec![(Cond::Disambiguate(!d), !t)],
        (Cond::Type(csl_type), true) => {
            <CslType as strum::IntoEnumIterator>::iter()
                .filter(|ty| *ty != *csl_type)
                .map(|ty| (Cond::Type(ty), false))
                .collect()
        }
        _ => vec![]
        // No need, we will add these manually anyway
        // (Cond::IsUncertainDate)
        // (Cond::HasDay(dv), _) => vec![(Cond::HasMonthOrSeason(dv), t), (Cond::HasYearOnly(dv), !t)],
        // (Cond::HasMonthOrSeason(dv), _) => vec![(Cond::HasYearOnly(dv), !t)],
    }
}

// TODO: transform conds by adding everything they imply

#[cfg(test)]
use csl::Variable;

#[test]
fn test_know() {
    let mut k = Knowledge::new();
    let title = &Cond::Variable(AnyVariable::Ordinary(Variable::Title));
    let ibid = &Cond::Position(Position::Ibid);
    let issue = &Cond::Variable(AnyVariable::Number(NumberVariable::Issue));

    k.know_all_of(vec![(title.clone(), true)].into_iter());
    assert!(k.demonstrates(title, true));

    k.push();

    k.know_some_of(vec![(ibid.clone(), true), (issue.clone(), true)].into_iter());

    // don't know yet
    assert!(!k.is_determined(ibid));

    k.push();

    // add some more knowledge
    k.know_all_of(vec![(issue.clone(), false)].into_iter());

    // now we know ibid
    assert!(k.is_determined(ibid));
    assert!(k.demonstrates(ibid, true));

    k.pop();

    // ibid is no longer known
    assert!(!k.is_determined(ibid));

    k.pop();
}

#[test]
fn inferences() {
    let mut k = Knowledge::new();
    let book = &Cond::Type(CslType::Book);
    let manuscript = &Cond::Type(CslType::Manuscript);
    let article_journal = &Cond::Type(CslType::ArticleJournal);
    let ibid = &Cond::Position(Position::Ibid);
    let subsequent = &Cond::Position(Position::Subsequent);
    // let page_first = &Cond::Variable(AnyVariable::Number(NumberVariable::PageFirst));

    let before_book = k.push();
    k.know_all_of(vec![(book.clone(), true)].into_iter());
    assert!(k.demonstrates(book, true));
    assert!(k.is_determined(manuscript));
    assert!(k.demonstrates(manuscript, false));
    assert!(k.demonstrates(article_journal, false));

    // now check if we made any secondary inferences
    k.know_some_of(vec![(manuscript.clone(), true), (ibid.clone(), true)].into_iter());
    assert!(k.demonstrates(ibid, true));
    assert!(!k.is_determined(subsequent));

    // consolidate inferences from the ibid, which needs anothe push
    k.push();
    assert!(k.demonstrates(subsequent, true));

    k.rollback(before_book);

    // types no longer known
    assert!(!k.is_determined(manuscript));
    assert!(!k.is_determined(article_journal));
    // inferences no longer known
    assert!(!k.is_determined(ibid));
    assert!(!k.is_determined(subsequent));
}
