// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use csl::style::{Cond, Position};
use csl::variables::{AnyVariable, NumberVariable, Variable};
use fnv::FnvHashSet;

bitflags::bitflags! {
    /// A convenient enum of the only conds that can actually change between cites
    struct FreeCond: u32 {
        const YEAR_SUFFIX        = 0b0000000000000001;
        const YEAR_SUFFIX_FALSE   = 0b0000000000000010;
        const LOCATOR           = 0b0000000000000100;
        const LOCATOR_FALSE      = 0b0000000000001000;
        const IBID              = 0b0000000000010000;
        const IBID_FALSE         = 0b0000000000100000;
        const NEAR_NOTE          = 0b0000000001000000;
        const NEAR_NOTE_FALSE     = 0b0000000010000000;
        const FAR_NOTE           = 0b0000000100000000;
        const FAR_NOTE_FALSE      = 0b0000001000000000;
        const IBID_WITH_LOCATOR      = 0b0000010000000000;
        const IBID_WITH_LOCATOR_FALSE = 0b0000100000000000;
        const SUBSEQUENT        = 0b0001000000000000; // Cool: FRNN = SUBSEQUENT.
        const SUBSEQUENT_FALSE   = 0b0010000000000000;
        const FIRST             = 0b0100000000000000;
        const FIRST_FALSE        = 0b1000000000000000;

        // No disambiguate, because you can't use this to do any more disambiguation, so unhelpful.
    }
}

impl FreeCond {
    fn is_incompatible(self) -> bool {
        lazy_static::lazy_static! {
            static ref INCOMPAT: Vec<FreeCond> = vec![
                FreeCond::IBID | FreeCond::NEAR_NOTE,
                FreeCond::IBID | FreeCond::FAR_NOTE,
                FreeCond::IBID_WITH_LOCATOR | FreeCond::NEAR_NOTE,
                FreeCond::IBID_WITH_LOCATOR | FreeCond::FAR_NOTE,
                FreeCond::IBID_FALSE | FreeCond::IBID_WITH_LOCATOR,
                FreeCond::SUBSEQUENT_FALSE | FreeCond::IBID,
                FreeCond::SUBSEQUENT_FALSE | FreeCond::FAR_NOTE,
                FreeCond::SUBSEQUENT_FALSE | FreeCond::NEAR_NOTE,
                FreeCond::SUBSEQUENT_FALSE | FreeCond::IBID_WITH_LOCATOR,
                FreeCond::SUBSEQUENT_FALSE | FreeCond::FIRST_FALSE,
                FreeCond::FIRST | FreeCond::IBID,
                FreeCond::FIRST | FreeCond::FAR_NOTE,
                FreeCond::FIRST | FreeCond::NEAR_NOTE,
                FreeCond::FIRST | FreeCond::IBID_WITH_LOCATOR,
                FreeCond::FIRST | FreeCond::SUBSEQUENT,
                FreeCond::IBID_WITH_LOCATOR | FreeCond::LOCATOR_FALSE,
                FreeCond::IBID | FreeCond::IBID_WITH_LOCATOR_FALSE | FreeCond::LOCATOR,
                FreeCond::FIRST_FALSE
                    | FreeCond::IBID_FALSE
                    | FreeCond::SUBSEQUENT_FALSE
                    | FreeCond::FAR_NOTE_FALSE
                    | FreeCond::NEAR_NOTE_FALSE
                    | FreeCond::IBID_WITH_LOCATOR_FALSE,
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
        let prev = self;
        while self != prev {
            if self & FreeCond::IBID_WITH_LOCATOR != FreeCond::empty() {
                self = self | FreeCond::IBID;
            }
            if self & FreeCond::IBID_WITH_LOCATOR != FreeCond::empty() {
                self = self | FreeCond::IBID;
            }
            if self & FreeCond::IBID != FreeCond::empty() {
                self = self | FreeCond::SUBSEQUENT;
            }
            if self & FreeCond::FIRST != FreeCond::empty() {
                self = self
                    | FreeCond::IBID_FALSE
                    | FreeCond::IBID_WITH_LOCATOR_FALSE
                    | FreeCond::SUBSEQUENT_FALSE
                    | FreeCond::NEAR_NOTE_FALSE
                    | FreeCond::FAR_NOTE_FALSE;
            }
            if self & FreeCond::FIRST != FreeCond::empty() {
                self = self | FreeCond::SUBSEQUENT_FALSE;
            }
            // ugh what a pain
        }
        self
    }
}

fn cond_to_frees(c: &Cond) -> Option<(FreeCond, FreeCond)> {
    let x = match c {
        Cond::Locator(_) => (FreeCond::LOCATOR, FreeCond::LOCATOR_FALSE),
        Cond::Position(p) => match p {
            Position::Ibid => (FreeCond::IBID, FreeCond::IBID_FALSE),
            Position::IbidWithLocator => (
                FreeCond::IBID_WITH_LOCATOR,
                FreeCond::IBID_WITH_LOCATOR_FALSE,
            ),
            Position::First => (FreeCond::FIRST, FreeCond::FIRST_FALSE),
            Position::Subsequent => (FreeCond::SUBSEQUENT, FreeCond::SUBSEQUENT_FALSE),
            Position::NearNote => (FreeCond::NEAR_NOTE, FreeCond::NEAR_NOTE_FALSE),
            Position::FarNote => (FreeCond::FAR_NOTE, FreeCond::FAR_NOTE_FALSE),
        },
        Cond::IsNumeric(AnyVariable::Number(nv)) | Cond::Variable(AnyVariable::Number(nv)) => {
            match nv {
                NumberVariable::Locator => (FreeCond::LOCATOR, FreeCond::LOCATOR_FALSE),
                _ => return None,
            }
        }
        Cond::IsNumeric(AnyVariable::Ordinary(ov)) | Cond::Variable(AnyVariable::Ordinary(ov)) => {
            match ov {
                // Variable::LocatorExtra =>
                // Variable::Hereinafter =>
                // Variable::CitationLabel => // CitationLabel
                Variable::YearSuffix => (FreeCond::YEAR_SUFFIX, FreeCond::YEAR_SUFFIX_FALSE),
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
    sets.0.insert(FreeCond::IBID);
    sets.0.insert(FreeCond::LOCATOR);
    sets.0.insert(FreeCond::SUBSEQUENT);
    sets.all_that_comma_assuming((Cond::Position(Position::First), true));
    dbg!(&sets);
    let mut check = FreeSets::default();
    // 'branch not taken'
    check.0.insert(FreeCond::FIRST_FALSE);
    // branch taken, most combos with First are incompatible
    check.0.insert(FreeCond::LOCATOR | FreeCond::FIRST);
    assert_eq!(sets, check);
}

#[test]
fn free_scalar_multiply_false() {
    let mut sets = FreeSets::default();
    sets.0.insert(FreeCond::IBID);
    sets.0.insert(FreeCond::LOCATOR);
    sets.0.insert(FreeCond::SUBSEQUENT);
    sets.all_that_comma_assuming((Cond::Position(Position::First), false));
    dbg!(&sets);
    let mut check = FreeSets::default();
    // 'branch not taken'
    check.0.insert(FreeCond::FIRST);
    // branch taken, many combos with FIRST_FALSE are compatible
    check.0.insert(FreeCond::IBID | FreeCond::FIRST_FALSE);
    check.0.insert(FreeCond::LOCATOR | FreeCond::FIRST_FALSE);
    check.0.insert(FreeCond::SUBSEQUENT | FreeCond::FIRST_FALSE);
    assert_eq!(sets, check);
}

#[test]
fn free_cross_product() {
    let mut sets = FreeSets::default();
    sets.0.insert(FreeCond::IBID);
    sets.0.insert(FreeCond::FIRST);
    let mut othe = FreeSets::default();
    othe.0.insert(FreeCond::LOCATOR);
    othe.0.insert(FreeCond::SUBSEQUENT);
    sets.cross_product(othe);
    dbg!(&sets);
    assert_eq!(sets.0.len(), 3);
}
