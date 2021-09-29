// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::String;
use edtf::level_1::{Certainty, Edtf};

pub(crate) use edtf::level_1::Date;
pub use sorting::{IncludeParts, OrderedDate};

mod calendar;
mod parts;
mod raw;
mod sorting;

/// The Eq implementation here is the default/Rust-derived one.
/// If you want one that matches the way dates are sorted in CSL output, use [OrderedDate].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateOrRange {
    Edtf(Edtf),
    Literal { literal: String, circa: bool },
}

/// This is a fairly primitive date type, which is not defined to be on any calendar in particular.
/// It must be rendered verbatim
#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct LegacyDate {
    /// think 10,000 BC; it's a signed int
    /// "not present" is expressed by not having a date in the first place
    pub year: i32,
    /// range 1 to 16 inclusive
    /// 0 is "not present"
    /// > 12 is a season identified by (month - 12)
    pub month: u32,
    /// range 1 to 31 inclusive
    /// 0 is "not present"
    pub day: u32,
    /// aka is_uncertain_date
    pub circa: bool,
}

#[test]
fn test_date_ord() {
    // years only
    assert!(Date::from_ymd(2000, 0, 0) < Date::from_ymd(2001, 0, 0));
    // Less specific comes first (2000 < May 2000 < 1 May 2000)
    assert!(Date::from_ymd(2000, 0, 0) < Date::from_ymd(2000, 5, 0));
    assert!(Date::from_ymd(2000, 5, 0) < Date::from_ymd(2000, 5, 1));

    assert!(Date::from_ymd(2000, 0, 0) < Date::from_ymd(2001, 0, 0));
}

impl LegacyDate {
    pub fn has_month(&self) -> bool {
        self.month != 0
    }
    pub fn has_day(&self) -> bool {
        self.day != 0
    }
    pub fn new_circa(y: i32, m: u32, d: u32) -> Self {
        let mut d = LegacyDate::new(y, m, d);
        d.circa = true;
        d
    }
    pub fn new(y: i32, m: u32, d: u32) -> Self {
        LegacyDate {
            year: y,
            month: m,
            day: d,
            circa: false,
        }
    }
    pub fn from_parts(parts: &[i32]) -> Option<Self> {
        let m = *parts.get(1).unwrap_or(&0);
        let d = *parts.get(2).unwrap_or(&0);
        Some(LegacyDate {
            year: *parts.get(0)?,
            month: if m >= 1 && m <= 16 { m as u32 } else { 0 },
            day: if d >= 1 && d <= 31 { d as u32 } else { 0 },
            circa: false,
        })
    }

    // it isn't possible to parse an invalid month, bad ones default to zero (see tests)
    // and cause the day not to parse as well.
    // so we only need to default days to zero when they are invalid.
    // Maybe just leave this as is. You would need to know leap years, hence a whole bunch
    // of date math that isn't all that valuable (real publications don't make those
    // mistakes anyway, and users who don't know when months end... it's on them)
    // plus there is no non-numeric day output for CSL. So it doesn't matter.
    // fn invalid_default() {
    // }
}

/// is-uncertain-date, aka circa or approximate
impl DateOrRange {
    pub fn with_circa(mut self, circa: bool) -> Self {
        self.set_circa(circa);
        self
    }
    pub fn set_circa(&mut self, circa: bool) {
        match self {
            DateOrRange::Edtf(edtf) => match edtf {
                Edtf::Date(d) => *d = d.and_certainty(Certainty::Uncertain),
                _ => {}
            },
            DateOrRange::Literal { circa: c, .. } => *c = circa,
        }
    }
    pub fn is_uncertain_date(&self) -> bool {
        match self {
            Self::Edtf(edtf) => match edtf {
                Edtf::Date(d) => d.certainty() != Certainty::Certain,
                Edtf::Interval(d1, d2) => {
                    d1.certainty() != Certainty::Certain && d2.certainty() != Certainty::Certain
                }
                Edtf::IntervalFrom(d1, _) => d1.certainty() != Certainty::Certain,
                Edtf::IntervalTo(_, d2) => d2.certainty() != Certainty::Certain,
                _ => false,
            },
            Self::Literal { circa, .. } => *circa,
        }
    }
}

/// Constructors, Accessors
impl DateOrRange {
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        Self::Edtf(Edtf::Date(Date::from_ymd(year, month, day)))
    }
    pub fn single(&self) -> Option<Date> {
        if let DateOrRange::Edtf(Edtf::Date(d)) = self {
            Some(*d)
        } else {
            None
        }
    }
    pub fn as_edtf(&self) -> Option<&Edtf> {
        match self {
            Self::Edtf(edtf) => Some(edtf),
            _ => None,
        }
    }
    pub fn single_or_first(&self) -> Option<Date> {
        match self.as_edtf()? {
            // These are a bit awkward -- it seems that more often than not, we want the actual
            // Date object and not the precision/certainty pair, because we can always get that
            // if it's needed.
            Edtf::Date(d)
            | Edtf::IntervalFrom(d, _)
            | Edtf::IntervalTo(_, d)
            | Edtf::Interval(d, _) => Some(*d),
            _ => None,
        }
    }
}

impl From<Edtf> for DateOrRange {
    fn from(d: Edtf) -> Self {
        Self::Edtf(d)
    }
}

impl From<Date> for DateOrRange {
    fn from(d: Date) -> Self {
        Self::Edtf(d.into())
    }
}

impl From<(Date, Date)> for DateOrRange {
    fn from(interval: (Date, Date)) -> Self {
        Self::Edtf(interval.into())
    }
}
