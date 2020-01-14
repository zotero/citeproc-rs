// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use crate::prelude::fnv_set_with_cap;
use csl::LocatorType;
use csl::{AnyVariable, NumberVariable, Variable};
use csl::{Cond, Position};
use csl::{CondSet, Match};
use fnv::FnvHashSet;

bitflags::bitflags! {
    /// A convenient enum of the only conds that can actually change between cites
    pub struct FreeCond: u64 {
        const YEAR_SUFFIX        = 1;
        const YEAR_SUFFIX_FALSE   = 1 << 1;

        const FIRST             = 1 << 2;
        const FIRST_FALSE        = 1 << 3;
        const IBID              = 1 << 4;
        const IBID_FALSE         = 1 << 5;
        const NEAR_NOTE          = 1 << 6;
        const NEAR_NOTE_FALSE     = 1 << 7;
        const FAR_NOTE           = 1 << 8;
        const FAR_NOTE_FALSE      = 1 << 9;
        const IBID_WITH_LOCATOR      = 1 << 10;
        const IBID_WITH_LOCATOR_FALSE = 1 << 11;
        const SUBSEQUENT        = 1 << 12; // Cool: FRNN = SUBSEQUENT.
        const SUBSEQUENT_FALSE   = 1 << 13;

        const LOCATOR           = 1 << 14;
        const LOCATOR_FALSE      = 1 << 15;

        const LT_BOOK               = 1 << 16;
        const LT_BOOK_FALSE         = 1 << 17;
        const LT_CHAPTER        = 1 << 18;
        const LT_CHAPTER_FALSE  = 1 << 19;
        const LT_COLUMN     = 1 << 20;
        const LT_COLUMN_FALSE = 1 << 21;
        const LT_FIGURE     = 1 << 22;
        const LT_FIGURE_FALSE = 1 << 23;
        const LT_FOLIO  = 1 << 24;
        const LT_FOLIO_FALSE = 1 << 25;
        const LT_ISSUE  = 1 << 26;
        const LT_ISSUE_FALSE = 1 << 27;
        const LT_LINE   = 1 << 28;
        const LT_LINE_FALSE = 1 << 29;
        const LT_NOTE   = 1 << 30;
        const LT_NOTE_FALSE = 1 << 31;
        const LT_OPUS   = 1 << 32;
        const LT_OPUS_FALSE = 1 << 33;
        const LT_PAGE   = 1 << 34;
        const LT_PAGE_FALSE = 1 << 35;
        const LT_PARAGRAPH  = 1 << 36;
        const LT_PARAGRAPH_FALSE = 1 << 37;
        const LT_PART   = 1 << 38;
        const LT_PART_FALSE = 1 << 39;
        const LT_SECTION     = 1 << 40;
        const LT_SECTION_FALSE = 1 << 41;
        const LT_SUBVERBO   = 1 << 41;
        const LT_SUBVERBO_FALSE = 1 << 43;
        const LT_VERSE  = 1 << 44;
        const LT_VERSE_FALSE = 1 << 45;
        const LT_VOLUME     = 1 << 46;
        const LT_VOLUME_FALSE = 1 << 47;

        const DISAMBIGUATE = 1 << 48;
        const DISAMBIGUATE_FALSE = 1 << 49;

        // TODO(CSL-M): enable these

        // const LT_ARTICLE    = 1 << 48;
        // const LT_ARTICLE_FALSE1 << 49;
        // const LT_SUBPARAGRAPH   = 1 << 50;
        // const LT_SUBPARAGRAPH_FALSE << 51;
        // const LT_RULE   = 1 << 52;
        // const LT_RULE_FALSE << 53;
        // const LT_SUBSECTION     = 1 << 54;
        // const LT_SUBSECTION_FALSE 1 << 55;
        // const LT_SCHEDULE   = 1 << 56;
        // const LT_SCHEDULE_FALSE << 57;
        // const LT_TITLE  = 1 << 58;
        // const LT_TITLE_FALSE<< 59;
        // const LT_SUPPLEMENT     = 1 << 60;
        // const LT_SUPPLEMENT_FALSE 1 << 61;

        // No disambiguate, because you can't use this to do any more disambiguation, so unhelpful.
    }
}

#[allow(dead_code)]
const LT_MASK: FreeCond = FreeCond::from_bits_truncate(std::u64::MAX << 16);
const LT_MASK_TRUE: FreeCond = FreeCond::from_bits_truncate(0x5555_5555_5555_5555 << 16);
const LT_MASK_FALSE: FreeCond = FreeCond::from_bits_truncate(0xAAAA_AAAA_AAAA_AAAA << 16);

const FC_MASK_TRUE: FreeCond = FreeCond::from_bits_truncate(0x5555_5555_5555_5555);
const FC_MASK_FALSE: FreeCond = FreeCond::from_bits_truncate(0xAAAA_AAAA_AAAA_AAAA);

#[test]
fn test_lt_mask() {
    assert!(LT_MASK.contains(FreeCond::LT_VOLUME_FALSE));
    assert!(LT_MASK.contains(FreeCond::LT_VOLUME));
    assert!(LT_MASK.contains(FreeCond::LT_BOOK));
    assert!(!LT_MASK.contains(FreeCond::LOCATOR));
    assert!(!LT_MASK.contains(FreeCond::LOCATOR_FALSE));
    assert!(LT_MASK_TRUE.contains(FreeCond::LT_PAGE));
    assert!(LT_MASK_TRUE.contains(FreeCond::LT_VOLUME));
    assert!(!LT_MASK_TRUE.contains(FreeCond::LT_VOLUME_FALSE));
    assert!(LT_MASK_FALSE.contains(FreeCond::LT_VOLUME_FALSE));
    assert!(LT_MASK_FALSE.contains(FreeCond::LT_PAGE_FALSE));
    assert!(!LT_MASK_FALSE.contains(FreeCond::LT_PAGE));

    assert!(FC_MASK_TRUE.contains(FreeCond::LOCATOR));
    assert!(FC_MASK_FALSE.contains(FreeCond::LOCATOR_FALSE));
}

#[test]
fn test_invert() {
    assert!((FreeCond::LT_VOLUME & FreeCond::LT_VOLUME.invert()).is_empty());
    assert!(
        (FreeCond::LT_VOLUME | FreeCond::LT_PAGE).invert()
            == (FreeCond::LT_VOLUME_FALSE | FreeCond::LT_PAGE_FALSE)
    );
}

macro_rules! match_fc {
    (@internal match ($pos:expr) { $($c:ident => $lt:path,)+ }) => {
        $(
            if ($pos).contains(FreeCond::$c) {
                return Some($lt);
            }
        )*
    };
    (@locator_type $pos:expr) => {
        match_fc! { @internal
            match ($pos) {
                LT_BOOK       => LocatorType::Book,
                LT_CHAPTER    => LocatorType::Chapter,
                LT_COLUMN     => LocatorType::Chapter,
                LT_FIGURE     => LocatorType::Figure,
                LT_FOLIO      => LocatorType::Folio,
                LT_ISSUE      => LocatorType::Issue,
                LT_LINE       => LocatorType::Line,
                LT_NOTE       => LocatorType::Note,
                LT_OPUS       => LocatorType::Opus,
                LT_PAGE       => LocatorType::Page,
                LT_PARAGRAPH  => LocatorType::Paragraph,
                LT_PART       => LocatorType::Part,
                LT_SECTION    => LocatorType::Section,
                LT_SUBVERBO   => LocatorType::SubVerbo,
                LT_VERSE      => LocatorType::Verse,
                LT_VOLUME     => LocatorType::Volume,
            }
        }
    };
}

#[test]
fn test_free_to_loc_type() {
    let x = FreeCond::LT_PART;
    assert_eq!(x.to_loc_type(), Some(LocatorType::Part));
    let x = FreeCond::LT_PART_FALSE;
    assert_eq!(x.to_loc_type(), None);
    let x = FreeCond::LT_PART_FALSE | FreeCond::LT_VERSE;
    assert_eq!(x.to_loc_type(), Some(LocatorType::Verse));
    let x = FreeCond::LOCATOR;
    assert_eq!(x.to_loc_type(), Some(LocatorType::Page));
    let x = FreeCond::IBID | FreeCond::IBID_WITH_LOCATOR_FALSE;
    assert_eq!(x.to_loc_type(), None);
}

impl FreeCond {
    pub fn to_loc_type(self) -> Option<LocatorType> {
        // the run doesn't use or check variable="locator", doesn't check locator="XXX", or
        // relies on FORALL XXX match="none" locator="XXX" (i.e. 'no locator type so no
        // locator')
        if (!self.contains(FreeCond::LOCATOR) && !self.intersects(LT_MASK_TRUE))
            || self.contains(LT_MASK_FALSE)
        {
            return None;
        }
        match_fc!(@locator_type self);
        let deductive = self & LT_MASK_FALSE;
        // Could do this with xor, but can't be bothered
        if deductive.bits().count_zeros() >= 1 {
            // At least one possible locator type, but the run doesn't care which as long as its
            // not one of the ones it didn't want.
            let opposite = deductive.invert();
            match_fc!(@locator_type opposite);
        }
        if self.contains(FreeCond::LOCATOR) {
            return Some(LocatorType::Page);
        }
        None
    }

    pub fn invert(self) -> Self {
        FreeCond::from_bits_truncate(
            (self & FC_MASK_TRUE).bits() << 1 | ((self & FC_MASK_FALSE).bits() >> 1),
        )
    }
    pub fn is_incompatible(self) -> bool {
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
        if self.intersects(self.invert()) {
            return true;
        }
        if self.intersects(LT_MASK_TRUE) && self.contains(FreeCond::LOCATOR_FALSE) {
            return true;
        }
        if (self & LT_MASK_TRUE).bits().count_ones() > 1 {
            return true;
        }
        for &x in INCOMPAT.iter() {
            if self.contains(x) {
                return true;
            }
        }
        false
    }

    fn imply(mut self) -> Self {
        let prev = self;
        while self != prev {
            if self & FreeCond::IBID_WITH_LOCATOR != FreeCond::empty() {
                self |= FreeCond::IBID;
            }
            if self & FreeCond::NEAR_NOTE != FreeCond::empty() {
                self |= FreeCond::SUBSEQUENT;
            }
            if self & FreeCond::FAR_NOTE != FreeCond::empty() {
                self |= FreeCond::SUBSEQUENT;
            }
            if self & FreeCond::IBID != FreeCond::empty() {
                self |= FreeCond::SUBSEQUENT;
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
                self |= FreeCond::SUBSEQUENT_FALSE;
            }
            if self.intersects(LT_MASK_TRUE) {
                self |= FreeCond::LOCATOR;
            }
            // ugh what a pain
        }
        self
    }
}

fn cond_to_frees(c: &Cond) -> Option<(FreeCond, FreeCond)> {
    let x = match c {
        Cond::Disambiguate(_b) => (FreeCond::DISAMBIGUATE, FreeCond::DISAMBIGUATE_FALSE),
        Cond::Position(p) => match p {
            Position::Ibid => (FreeCond::IBID, FreeCond::IBID_FALSE),
            Position::IbidNear => (
                FreeCond::IBID | FreeCond::NEAR_NOTE,
                FreeCond::IBID_FALSE | FreeCond::NEAR_NOTE_FALSE,
            ),
            Position::IbidWithLocatorNear => (
                FreeCond::IBID_WITH_LOCATOR | FreeCond::NEAR_NOTE,
                FreeCond::IBID_WITH_LOCATOR_FALSE | FreeCond::NEAR_NOTE_FALSE,
            ),
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
        Cond::Locator(lt) => match lt {
            LocatorType::Book => (FreeCond::LT_BOOK, FreeCond::LT_BOOK_FALSE),
            LocatorType::Chapter => (FreeCond::LT_CHAPTER, FreeCond::LT_CHAPTER_FALSE),
            LocatorType::Column => (FreeCond::LT_COLUMN, FreeCond::LT_COLUMN_FALSE),
            LocatorType::Figure => (FreeCond::LT_FIGURE, FreeCond::LT_FIGURE_FALSE),
            LocatorType::Folio => (FreeCond::LT_FOLIO, FreeCond::LT_FOLIO_FALSE),
            LocatorType::Issue => (FreeCond::LT_ISSUE, FreeCond::LT_ISSUE_FALSE),
            LocatorType::Line => (FreeCond::LT_LINE, FreeCond::LT_LINE_FALSE),
            LocatorType::Note => (FreeCond::LT_NOTE, FreeCond::LT_NOTE_FALSE),
            LocatorType::Opus => (FreeCond::LT_OPUS, FreeCond::LT_OPUS_FALSE),
            LocatorType::Page => (FreeCond::LT_PAGE, FreeCond::LT_PAGE_FALSE),
            LocatorType::Paragraph => (FreeCond::LT_PARAGRAPH, FreeCond::LT_PARAGRAPH_FALSE),
            LocatorType::Part => (FreeCond::LT_PART, FreeCond::LT_PART_FALSE),
            LocatorType::Section => (FreeCond::LT_SECTION, FreeCond::LT_SECTION_FALSE),
            LocatorType::SubVerbo => (FreeCond::LT_SUBVERBO, FreeCond::LT_SUBVERBO_FALSE),
            LocatorType::Verse => (FreeCond::LT_VERSE, FreeCond::LT_VERSE_FALSE),
            LocatorType::Volume => (FreeCond::LT_VOLUME, FreeCond::LT_VOLUME_FALSE),
            _ => unimplemented!("CSL-M locator types")

            // TODO(CSL-M) enable
            // Article => (FreeCond::LT_LINE, FreeCond::LT_LINE_FALSE),
            // Subparagraph => (FreeCond::LT_LINE, FreeCond::LT_LINE_FALSE),
            // Rule => (FreeCond::LT_LINE, FreeCond::LT_LINE_FALSE),
            // Subsection => (FreeCond::LT_LINE, FreeCond::LT_LINE_FALSE),
            // Schedule => (FreeCond::LT_LINE, FreeCond::LT_LINE_FALSE),
            // Title => (FreeCond::LT_LINE, FreeCond::LT_LINE_FALSE),
            // Supplement => (FreeCond::LT_LINE, FreeCond::LT_LINE_FALSE),
        },
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreeCondSets(pub FnvHashSet<FreeCond>);

impl Default for FreeCondSets {
    fn default() -> Self {
        FreeCondSets::mult_identity()
    }
}

impl FreeCondSets {
    /// Like the number 1, but for multiplying FreeCondSets using cross_products.
    ///
    /// The cross product of any set X and mult_identity() is X.
    pub fn mult_identity() -> FreeCondSets {
        let mut f = FreeCondSets(FnvHashSet::default());
        // the multiplicative identity
        f.0.insert(FreeCond::empty());
        f
    }
}

impl FreeCondSets {
    fn at_least_1_ref(&mut self) {
        if self.0.is_empty() {
            self.0.insert(FreeCond::empty());
        }
    }
    fn at_least_1(mut self) -> Self {
        self.at_least_1_ref();
        self
    }
    pub fn empty() -> Self {
        FreeCondSets(FnvHashSet::default())
    }
    pub fn keep_compatible(&mut self) {
        self.0.retain(|&x| !x.is_incompatible());
    }
    pub fn cross_product(&mut self, other: Self) {
        let mut neu = fnv_set_with_cap(self.0.len());
        for &oth in other.0.iter() {
            for &set in self.0.iter() {
                let x = (set | oth).imply();
                if !x.is_incompatible() {
                    neu.insert(x);
                }
            }
        }
        self.0 = neu;
        self.at_least_1_ref();
    }
    fn add_k_alone(neu: &mut FnvHashSet<FreeCond>, k: Cond, assumed_k_to_be: bool) {
        if let Some((a, neg_a)) = cond_to_frees(&k) {
            if assumed_k_to_be {
                neu.insert(neg_a);
            } else {
                neu.insert(a);
            }
        }
    }
    pub fn scalar_multiply(&self, a: FreeCond) -> Self {
        let mut neu = fnv_set_with_cap(self.0.len());
        for set in self.0.iter() {
            let x = (*set | a).imply();
            if !x.is_incompatible() {
                neu.insert(x);
            }
        }
        FreeCondSets(neu).at_least_1()
    }
    pub fn scalar_multiply_cond(&mut self, k: Cond, assumed_k_to_be: bool) {
        if let Some((a, neg_a)) = cond_to_frees(&k) {
            let mut neu = fnv_set_with_cap(self.0.len());
            FreeCondSets::add_k_alone(&mut neu, k, assumed_k_to_be);
            for set in self.0.iter() {
                if assumed_k_to_be {
                    let x = (*set | a).imply();
                    if !x.is_incompatible() {
                        neu.insert(x);
                    }
                }
                if !assumed_k_to_be {
                    let y = (*set | neg_a).imply();
                    if !y.is_incompatible() {
                        neu.insert(y);
                    }
                }
            }
            self.0 = neu;
            self.at_least_1_ref();
        }
    }
    pub fn all_that_assuming(&mut self, k: (Cond, bool)) {
        self.scalar_multiply_cond(k.0, k.1);
    }
    pub fn all_branches<'a>(
        cond_results: impl Iterator<Item = (&'a CondSet, Self)>,
        else_result: Option<Self>,
    ) -> Self {
        let mut all = FreeCondSets::empty();
        let mut accumulator = FreeCondSets::default();
        for (cond_set, inner) in cond_results {
            let (mut outer, negation) = condset_to_frees(cond_set, inner);
            // outer.0.extend(accumulator.0.clone().drain());
            for x in &accumulator.0 {
                outer = outer.scalar_multiply(*x);
            }
            all.0.extend(outer.0.drain());
            for x in negation.0 {
                accumulator = accumulator.scalar_multiply(x);
            }
        }
        if let Some(mut outer) = else_result {
            for x in &accumulator.0 {
                outer = outer.scalar_multiply(*x);
            }
            all.0.extend(outer.0.drain());
        }
        all.0.extend(accumulator.0.drain());
        all.at_least_1()
    }
    pub fn insert_validated(&mut self, a: FreeCond) {
        if !a.is_incompatible() {
            self.0.insert(a);
        }
    }
}

/// The second one is the negation
fn condset_to_frees(c: &CondSet, inner: FreeCondSets) -> (FreeCondSets, FreeCondSets) {
    let conds = &c.conds;
    match c.match_type {
        Match::None => {
            // Any of these being true would kill the None matcher and cause no output from
            // this branch. Each must be false.
            // That means we multiply the inner result by ALL of them at once
            let all_false: FreeCond = conds
                .iter()
                .filter_map(cond_to_frees)
                .map(|(_a, neg_a)| neg_a)
                .collect();
            let none = inner.scalar_multiply(all_false);
            let mut any = FreeCondSets::empty();
            get_any_outside(conds, &mut any.0);
            (none, any)
        }
        Match::All => {
            // Similarly, collect all these frees in one and multiply by the lot
            let all: FreeCond = conds
                .iter()
                .filter_map(cond_to_frees)
                .map(|(a, _neg_a)| a)
                .collect();
            let _all_false: FreeCond = conds
                .iter()
                .filter_map(cond_to_frees)
                .map(|(_a, neg_a)| neg_a)
                .collect();
            let all = inner.scalar_multiply(all);
            let mut nand = FreeCondSets::empty();
            get_nand_outside(conds, &mut nand.0);
            (all, nand)
        }
        // Any is the default
        // XXX: Not sure of the correct impl here
        Match::Any => {
            let any = get_any_multiplied(conds, &inner);
            // for when none of these frees were true, but some other cond was true
            let all_false: FreeCond = conds
                .iter()
                .filter_map(cond_to_frees)
                .map(|(_a, neg_a)| neg_a)
                .collect();
            let any = FreeCondSets(any);
            let mut none = FreeCondSets::empty();
            none.insert_validated(all_false);
            (any, none.at_least_1())
        }
        Match::Nand => unimplemented!(),
        // // Completely Untested
        // Match::Nand => {
        //     // _exactly one_ of them is true
        //     // So negate the rest, for each
        //     let vec: Vec<_> = conds
        //         .iter()
        //         .filter_map(cond_to_frees)
        //         .map(|(a, _neg_a)| a)
        //         .collect();
        //     let mut all = fnv_set_with_cap(inner.0.len());
        //     for each in vec {
        //         let rest: FreeCond = conds
        //             .iter()
        //             .filter_map(cond_to_frees)
        //             .filter_map(|(a, neg_a)| if a != each { Some(neg_a) } else { None })
        //             .collect();
        //         all.extend(inner.scalar_multiply(rest | each).0.drain());
        //     }
        //     FreeCondSets(all)
        // }
    }
}

fn get_any_outside(conds: &FnvHashSet<Cond>, outside: &mut FnvHashSet<FreeCond>) {
    use itertools::Itertools;
    let vec: Vec<_> = conds
        .iter()
        .filter_map(cond_to_frees)
        .map(|(a, _neg_a)| a)
        .collect();
    for i in 1..=vec.len() {
        let iter = vec
            .iter()
            .cloned()
            .combinations(i)
            .map(|fc_vec| fc_vec.into_iter().collect::<FreeCond>())
            .filter_map(|mut fc| {
                for (a, neg_a) in conds.iter().filter_map(cond_to_frees) {
                    if !fc.contains(a) {
                        fc = (fc | neg_a).imply();
                    }
                }
                if !fc.is_incompatible() {
                    Some(fc)
                } else {
                    None
                }
            });
        for x in iter {
            outside.insert(x);
        }
    }
}

fn get_any_multiplied(conds: &FnvHashSet<Cond>, inner: &FreeCondSets) -> FnvHashSet<FreeCond> {
    use itertools::Itertools;
    let vec: Vec<_> = conds
        .iter()
        .filter_map(cond_to_frees)
        .map(|(a, _neg_a)| a)
        .collect();
    if vec.is_empty() {
        inner.0.clone()
    } else {
        let mut any = fnv_set_with_cap(inner.0.len());
        for i in 1..=vec.len() {
            vec.iter()
                .cloned()
                .combinations(i)
                .map(|fc_vec| fc_vec.into_iter().collect::<FreeCond>())
                .filter_map(|mut fc| {
                    for (a, neg_a) in conds.iter().filter_map(cond_to_frees) {
                        if !fc.contains(a) {
                            fc = (fc | neg_a).imply();
                        }
                    }
                    if !fc.is_incompatible() {
                        Some(inner.scalar_multiply(fc))
                    } else {
                        None
                    }
                })
                .fold(&mut any, |acc, mut x| {
                    acc.extend(x.0.drain());
                    acc
                });
        }
        any
    }
}

fn get_nand_outside(conds: &FnvHashSet<Cond>, outside: &mut FnvHashSet<FreeCond>) {
    use itertools::Itertools;
    let vec: Vec<_> = conds
        .iter()
        .filter_map(cond_to_frees)
        .map(|(_a, neg_a)| neg_a)
        .collect();
    for i in 1..=vec.len() {
        let iter = vec
            .iter()
            .cloned()
            .combinations(i)
            .map(|fc_vec| fc_vec.into_iter().collect::<FreeCond>())
            .filter_map(|mut fc| {
                for (a, neg_a) in conds.iter().filter_map(cond_to_frees) {
                    if !fc.contains(neg_a) {
                        fc = (fc | a).imply();
                    }
                }
                if !fc.is_incompatible() {
                    Some(fc)
                } else {
                    None
                }
            });
        for x in iter {
            outside.insert(x);
        }
    }
}

#[test]
fn free_scalar_multiply() {
    let mut sets = FreeCondSets::empty();
    sets.0.insert(FreeCond::IBID);
    sets.0.insert(FreeCond::LOCATOR);
    sets.0.insert(FreeCond::SUBSEQUENT);
    sets.all_that_assuming((Cond::Position(Position::First), true));
    dbg!(&sets);
    let mut check = FreeCondSets::empty();
    // 'branch not taken'
    check.0.insert(FreeCond::FIRST_FALSE);
    // branch taken, most combos with First are incompatible
    check.0.insert(FreeCond::LOCATOR | FreeCond::FIRST);
    assert_eq!(sets, check);
}

#[test]
fn free_scalar_multiply_false() {
    let mut sets = FreeCondSets::empty();
    sets.0.insert(FreeCond::IBID);
    sets.0.insert(FreeCond::LOCATOR);
    sets.0.insert(FreeCond::SUBSEQUENT);
    sets.all_that_assuming((Cond::Position(Position::First), false));
    dbg!(&sets);
    let mut check = FreeCondSets::empty();
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
    let mut sets = FreeCondSets::empty();
    sets.0.insert(FreeCond::IBID);
    sets.0.insert(FreeCond::FIRST);
    let mut othe = FreeCondSets::empty();
    othe.0.insert(FreeCond::LOCATOR);
    othe.0.insert(FreeCond::SUBSEQUENT);
    sets.cross_product(othe);
    dbg!(&sets);
    assert_eq!(sets.0.len(), 3);
}

#[test]
fn free_all_branches_match_all() {
    use csl::Position;
    let ibid = Cond::Position(Position::Ibid);
    let mut if_inner = FreeCondSets::empty();
    if_inner.scalar_multiply_cond(ibid, true);
    let mut if_branch_conds = FnvHashSet::default();
    if_branch_conds.insert(Cond::Position(Position::First));
    let if_branch = CondSet {
        match_type: Match::All,
        // should not end up in the output
        conds: if_branch_conds,
    };
    let cs = vec![(&if_branch, if_inner)];
    let all = FreeCondSets::all_branches(cs.into_iter(), None);
    let mut result = FnvHashSet::default();
    // the one where the branch is taken excludes IBID (true) because it is incompatible with FIRST
    result.insert(FreeCond::IBID_FALSE | FreeCond::FIRST);
    // and the one where the branch is not taken has a FIRST_FALSE
    result.insert(FreeCond::FIRST_FALSE);
    assert_eq!(all.0, result);
}

#[test]
fn free_all_branches_match_none() {
    use csl::Position;
    let ibid = Cond::Position(Position::Ibid);
    let mut if_inner = FreeCondSets::empty();
    if_inner.scalar_multiply_cond(ibid, true);
    let mut if_branch_conds = FnvHashSet::default();
    if_branch_conds.insert(Cond::Variable(AnyVariable::Number(NumberVariable::Locator)));
    let if_branch = CondSet {
        match_type: Match::None,
        // should not end up in the output
        conds: if_branch_conds,
    };
    let cs = vec![(&if_branch, if_inner)];
    let all = FreeCondSets::all_branches(cs.into_iter(), None);
    let mut result = FnvHashSet::default();
    // the one where the branch is taken excludes IBID (true) because it is incompatible with FIRST
    result.insert(FreeCond::LOCATOR_FALSE | FreeCond::IBID_FALSE);
    // and the one where the branch is not taken has a FIRST_FALSE
    result.insert(FreeCond::LOCATOR);
    assert_eq!(all.0, result);
}

#[test]
fn free_all_branches_match_any() {
    // inner = {IBID, IBID_FALSE};
    // conds = any (LOCATOR)
    use csl::Position;
    let ibid = Cond::Position(Position::Ibid);
    let mut if_inner = FreeCondSets::empty();
    if_inner.scalar_multiply_cond(ibid, true);

    let mut if_branch_conds = FnvHashSet::default();
    if_branch_conds.insert(Cond::Variable(AnyVariable::Number(NumberVariable::Locator)));
    if_branch_conds.insert(Cond::Locator(LocatorType::Page));

    let if_branch = CondSet {
        match_type: Match::Any,
        // should not end up in the output
        conds: if_branch_conds,
    };
    let cs = vec![(&if_branch, if_inner)];
    let all = FreeCondSets::all_branches(cs.into_iter(), None);
    let mut result = FnvHashSet::default();
    // the one where the branch is taken excludes IBID (true) because it is incompatible with FIRST
    result.insert(FreeCond::LOCATOR | FreeCond::LT_PAGE | FreeCond::IBID_FALSE);
    result.insert(FreeCond::LOCATOR | FreeCond::LT_PAGE_FALSE | FreeCond::IBID_FALSE);
    // LOCATOR_FALSE | LT_PAGE_TRUE is not possible, so also excluded
    // and the one where the branch is not taken:
    result.insert(FreeCond::LOCATOR_FALSE | FreeCond::LT_PAGE_FALSE);
    assert_eq!(all.0, result);
}
