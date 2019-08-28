use csl::style::Cond;
use fnv::{FnvHashMap, FnvHashSet};
use generational_arena::{Arena, Index as ArenaIndex};

type CondIndex = ArenaIndex;
type SomeOfIndex = ArenaIndex;
type GenerationIndex = usize;

#[derive(Debug, Copy, Clone)]
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

#[derive(Debug)]
struct SingleKnowledge(Vec<(GenerationIndex, Maybe)>);

impl SingleKnowledge {
    fn pop_larger_than(&mut self, ix: GenerationIndex) {
        let mut i = self.0.len() - 1;
        while i > 0 {
            let (g, _) = self.0[i];
            if g <= ix {
                break;
            }
            i -= 1;
        }
        self.0.truncate(i + 1);
    }

    fn determinism(&self) -> Option<Result<bool, Vec<SomeOfIndex>>> {
        use Maybe::*;
        use Possibility::*;
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

    /// returns true if pushed
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
    pub fn know_all_of(&mut self, things: impl Iterator<Item = (Cond, bool)> + Clone) {
        for (cond, truth) in things {
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
    }

    pub fn know_some_of(&mut self, iter: impl Iterator<Item = (Cond, bool)> + Clone) {
        if iter.clone().any(|c| self.is_determined(&c.0)) {
            // if any are already known, there is no new knowledge to be derived here.
            return;
        }
        let set = self.some_ofs.insert(FnvHashSet::default());
        let mut some_of_set = FnvHashSet::default();
        for (cond, truth) in iter {
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

    fn is_determined_inner(&self, cond: &Cond, truth: bool) -> Option<bool> {
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
                            return own.map(|o| o == truth);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn is_determined(&self, cond: &Cond) -> bool {
        self.is_determined_inner(cond, true).is_some()
    }
    pub fn demonstrates(&self, cond: &Cond, truth: bool) -> bool {
        self.is_determined_inner(cond, truth) == Some(true)
    }

    pub fn push(&mut self) {
        let gen = Default::default();
        self.gens.push(gen);
        self.current_gen = self.gens.len();
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
}

// TODO: transform conds by adding everything they imply

#[cfg(test)]
use csl::style::Position;
#[cfg(test)]
use csl::variables::{AnyVariable, NumberVariable, Variable};

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
