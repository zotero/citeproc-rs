// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use csl::style::{Cond, Position};
use csl::variables::{AnyVariable, NumberVariable, StandardVariable, Variable};
use fnv::{FnvHashMap, FnvHashSet};

bitflags::bitflags! {
    /// A convenient enum of the only conds that can actually change between cites
    struct FreeCond: u32 {
        const YearSuffix        = 0b0000000000000001;
        const YearSuffixFalse   = 0b0000000000000010;
        const Locator           = 0b0000000000000100;
        const LocatorFalse      = 0b0000000000001000;
        const Ibid              = 0b0000000000010000;
        const IbidFalse         = 0b0000000000100000;
        const NearNote          = 0b0000000001000000;
        const NearNoteFalse     = 0b0000000010000000;
        const FarNote           = 0b0000000100000000;
        const FarNoteFalse      = 0b0000001000000000;
        const IbidWLocator      = 0b0000010000000000;
        const IbidWLocatorFalse = 0b0000100000000000;
        const Subsequent        = 0b0001000000000000; // Cool: FRNN = Subsequent.
        const SubsequentFalse   = 0b0010000000000000;
        const First             = 0b0100000000000000;
        const FirstFalse        = 0b1000000000000000;

        // No disambiguate, because you can't use this to do any more disambiguation, so unhelpful.
    }
}

impl FreeCond {
    fn is_incompatible(self) -> bool {
        lazy_static::lazy_static! {
            static ref INCOMPAT: Vec<FreeCond> = vec![
                FreeCond::Ibid | FreeCond::NearNote,
                FreeCond::Ibid | FreeCond::FarNote,
                FreeCond::IbidWLocator | FreeCond::NearNote,
                FreeCond::IbidWLocator | FreeCond::FarNote,
                FreeCond::IbidFalse | FreeCond::IbidWLocator,
                FreeCond::SubsequentFalse | FreeCond::Ibid,
                FreeCond::SubsequentFalse | FreeCond::FarNote,
                FreeCond::SubsequentFalse | FreeCond::NearNote,
                FreeCond::SubsequentFalse | FreeCond::IbidWLocator,
                FreeCond::SubsequentFalse | FreeCond::FirstFalse,
                FreeCond::First | FreeCond::Ibid,
                FreeCond::First | FreeCond::FarNote,
                FreeCond::First | FreeCond::NearNote,
                FreeCond::First | FreeCond::IbidWLocator,
                FreeCond::First | FreeCond::Subsequent,
                FreeCond::IbidWLocator | FreeCond::LocatorFalse,
                FreeCond::Ibid | FreeCond::IbidWLocatorFalse | FreeCond::Locator,
                FreeCond::FirstFalse
                    | FreeCond::IbidFalse
                    | FreeCond::SubsequentFalse
                    | FreeCond::FarNoteFalse
                    | FreeCond::NearNoteFalse
                    | FreeCond::IbidWLocatorFalse,
            ];
        }
        for &x in INCOMPAT.iter() {
            if self & x == x {
                return true;
            }
        }
        false
    }

    fn imply(mut self) -> Self {
        let mut prev = self;
        while self != prev {
            if self & FreeCond::IbidWLocator != FreeCond::empty() {
                self = self | FreeCond::Ibid;
            }
            if self & FreeCond::IbidWLocator != FreeCond::empty() {
                self = self | FreeCond::Ibid;
            }
            if self & FreeCond::Ibid != FreeCond::empty() {
                self = self | FreeCond::Subsequent;
            }
            if self & FreeCond::First != FreeCond::empty() {
                self = self
                    | FreeCond::IbidFalse
                    | FreeCond::IbidWLocatorFalse
                    | FreeCond::SubsequentFalse
                    | FreeCond::NearNoteFalse
                    | FreeCond::FarNoteFalse;
            }
            if self & FreeCond::First != FreeCond::empty() {
                self = self | FreeCond::SubsequentFalse;
            }
            // ugh what a pain
        }
        self
    }
}

fn cond_to_frees(c: &Cond) -> Option<(FreeCond, FreeCond)> {
    let x = match c {
        Cond::Locator(_) => (FreeCond::Locator, FreeCond::LocatorFalse),
        Cond::Position(p) => match p {
            Position::Ibid => (FreeCond::Ibid, FreeCond::IbidFalse),
            Position::IbidWithLocator => (FreeCond::IbidWLocator, FreeCond::IbidWLocatorFalse),
            Position::First => (FreeCond::First, FreeCond::FirstFalse),
            Position::Subsequent => (FreeCond::Subsequent, FreeCond::SubsequentFalse),
            Position::NearNote => (FreeCond::NearNote, FreeCond::NearNoteFalse),
            Position::FarNote => (FreeCond::FarNote, FreeCond::FarNoteFalse),
        },
        Cond::IsNumeric(AnyVariable::Number(nv)) | Cond::Variable(AnyVariable::Number(nv)) => {
            match nv {
                NumberVariable::Locator => (FreeCond::Locator, FreeCond::LocatorFalse),
                _ => return None,
            }
        }
        Cond::IsNumeric(AnyVariable::Ordinary(ov)) | Cond::Variable(AnyVariable::Ordinary(ov)) => {
            match ov {
                // Variable::LocatorExtra =>
                // Variable::Hereinafter =>
                // Variable::CitationLabel =>
                Variable::YearSuffix => (FreeCond::YearSuffix, FreeCond::YearSuffixFalse),
                _ => return None,
            }
        }
        _ => return None,
    };
    Some(x)
}

/// This is how we assemble the set of sets of conditions with which to do a linear style run on a
/// Reference.
///
///
/// Being made of bitflags, it is probably faster than constructing a whole knowledge database
/// again.
#[derive(Default, Debug, PartialEq)]
struct FreeSets(FnvHashSet<FreeCond>);

impl FreeSets {
    fn keep_compatible(&mut self) {
        self.0.retain(|&x| !x.is_incompatible());
    }
    fn cross_product(&mut self, other: Self) {
        let mut neu =
            FnvHashSet::with_capacity_and_hasher(self.0.len(), fnv::FnvBuildHasher::default());
        for &oth in other.0.iter() {
            for &set in self.0.iter() {
                let x = set | oth;
                if !x.is_incompatible() {
                    neu.insert(x);
                }
            }
        }
        self.0 = neu;
    }
    fn scalar_multiply(&mut self, k: Cond, assumed_k_to_be: bool) {
        if let Some((a, neg_a)) = cond_to_frees(&k) {
            let mut neu =
                FnvHashSet::with_capacity_and_hasher(self.0.len(), fnv::FnvBuildHasher::default());
            if assumed_k_to_be == true {
                neu.insert(neg_a);
            } else {
                neu.insert(a);
            }
            for set in self.0.iter() {
                if assumed_k_to_be == true {
                    let x = (*set | a).imply();
                    if !x.is_incompatible() {
                        neu.insert(x);
                    }
                }
                if assumed_k_to_be == false {
                    let y = (*set | neg_a).imply();
                    if !y.is_incompatible() {
                        neu.insert(y);
                    }
                }
            }
            self.0 = neu;
        }
    }
    pub fn all_that_comma_assuming(&mut self, k: (Cond, bool)) {
        self.scalar_multiply(k.0, k.1);
    }
}

#[test]
fn free_scalar_multiply() {
    let mut sets = FreeSets::default();
    sets.0.insert(FreeCond::Ibid);
    sets.0.insert(FreeCond::Locator);
    sets.0.insert(FreeCond::Subsequent);
    sets.all_that_comma_assuming((Cond::Position(Position::First), true));
    dbg!(&sets);
    let mut check = FreeSets::default();
    // 'branch not taken'
    check.0.insert(FreeCond::FirstFalse);
    // branch taken, most combos with First are incompatible
    check.0.insert(FreeCond::Locator | FreeCond::First);
    assert_eq!(sets, check);
}
#[test]
fn free_scalar_multiply_false() {
    let mut sets = FreeSets::default();
    sets.0.insert(FreeCond::Ibid);
    sets.0.insert(FreeCond::Locator);
    sets.0.insert(FreeCond::Subsequent);
    sets.all_that_comma_assuming((Cond::Position(Position::First), false));
    dbg!(&sets);
    let mut check = FreeSets::default();
    // 'branch not taken'
    check.0.insert(FreeCond::First);
    // branch taken, many combos with FirstFalse are compatible
    check.0.insert(FreeCond::Ibid | FreeCond::FirstFalse);
    check.0.insert(FreeCond::Locator | FreeCond::FirstFalse);
    check.0.insert(FreeCond::Subsequent | FreeCond::FirstFalse);
    assert_eq!(sets, check);
}

#[test]
fn free_cross_product() {
    let mut sets = FreeSets::default();
    sets.0.insert(FreeCond::Ibid);
    sets.0.insert(FreeCond::First);
    let mut othe = FreeSets::default();
    othe.0.insert(FreeCond::Locator);
    othe.0.insert(FreeCond::Subsequent);
    sets.cross_product(othe);
    dbg!(&sets);
    assert_eq!(sets.0.len(), 3);
}
