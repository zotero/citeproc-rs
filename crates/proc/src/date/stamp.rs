// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2021 Corporation for Digital Scholarship

//! This file looks like it does the same thing as `crates/io/src/date/sorting.rs`.
//! However, it is subtly different: sorting tries to put all dates on one timeline

use std::convert::TryInto;

use chrono::Datelike;
use chronology::historical::{CalendarInUse, DateInterpreter, Stampable, StampedDate};
use chronology::{CalendarTo, CommonEra, Era, Gregorian, Iso};
use citeproc_io::edtf::Season;
use citeproc_io::{
    edtf::{self, Edtf, Precision},
    DateOrRange,
};

use super::WhichDelim;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum RangeStamp<'a, Period = AlwaysGregorian> {
    // TODO: DateTime stamp. This should be a proper CSL feature too.
    Date(Stamp<Period>),
    Range(Stamp<Period>, Stamp<Period>),
    Literal(&'a str),
}

/// Single variant for now, but can be extended to, e.g. `Decade(i32)`,
/// `Century(i32)`,
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Stamp<Period = AlwaysGregorian> {
    Date {
        year: u32,
        era: Era,
        /// XXX: remove
        #[deprecated]
        iso_year: i32,
        month: Option<u32>,
        day: Option<u32>,
        period: Period,
    },
    Season {
        year: u32,
        era: Era,
        season: Season,
        iso_year: i32,
    },
    // if you wanted to handle `1999/` differently from `1999/..`, add `RangeUnknown`,
    RangeOpen,
}

impl Stamp {
    /// XXX: this should just be done on EDTF, not their stamp.
    /// Using iso_year is a big ol hack, not good because
    /// month and day could be stamped weirdly too
    #[deprecated = "this should be done on EDTF"]
    pub(super) fn get_component(&self, part: WhichDelim) -> Option<i32> {
        match *self {
            Self::Date {
                year: _,
                era: _,
                iso_year,
                month,
                day,
                period: _,
            } => match part {
                WhichDelim::Day => day.map(|x| x as i32),
                WhichDelim::Month => month.map(|x| x as i32),
                WhichDelim::Year => Some(iso_year),
                WhichDelim::None => None,
            },
            Self::Season {
                year: _,
                era: _,
                iso_year,
                season,
            } => match part {
                WhichDelim::Month => Some(season as u32 as i32),
                WhichDelim::Year => Some(iso_year),
                _ => None,
            },
            Self::RangeOpen => match part {
                _ => None,
            },
        }
    }
}

// We don't need more than one era (yet), because we're only doing [Canon], which is a
// single-era calendar. (chronology's era is separate from CE/BCE).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct AlwaysGregorian;

impl DateInterpreter for AlwaysGregorian {
    type Interpretation = Option<Iso>;

    type Period = Self;

    fn interpret(&self, year: u32, era: Era, month: u32, day: u32) -> Self::Interpretation {
        Gregorian::from_ce_date_opt((), year, era, month, day).map(|x| x.to_iso())
    }

    fn interpret_as(
        &self,
        year: u32,
        era: Era,
        month: u32,
        day: u32,
        _period: Self::Period,
    ) -> Option<Iso> {
        self.interpret(year, era, month, day)
    }

    fn if_unambiguous(&self, interpretation: Self::Interpretation) -> Option<Iso> {
        interpretation
    }

    fn calendar_in_use(&self, _iso: Iso) -> Self::Period {
        AlwaysGregorian
    }

    fn stamp(&self, iso: Iso, kind: Self::Period) -> StampedDate<Self::Period> {
        iso.to_gregorian().historical_date_stamp(kind, ())
    }
}

trait EdtfStamper: DateInterpreter {
    /// XXX: remove iso_year param
    fn stamp_historical(
        &self,
        stamp: StampedDate<Self::Period>,
        iso_year: i32,
    ) -> Stamp<Self::Period> {
        let StampedDate {
            year,
            era,
            month,
            day,
            period: _,
        } = stamp;
        Stamp::Date {
            year,
            era,
            iso_year,
            month: super::nonzero(month),
            day: super::nonzero(day),
            period: stamp.period,
        }
    }

    fn stamp_ymd(&self, iso: Iso) -> Stamp<Self::Period> {
        let stamp = self.stamp_for(iso);
        self.stamp_historical(stamp, iso.year())
    }

    fn stamp_ym(&self, year: i32, month: u32) -> Stamp<Self::Period>;
    fn stamp_y(&self, year: i32) -> Stamp<Self::Period>;
    fn stamp_yy(&self, year: i64) -> Stamp<Self::Period>;

    /// In which we decide how to render all the exotic EDTF formats.
    ///
    /// This is largely unspecified by CSL.
    fn stamp_edtf_date(&self, date: &edtf::Date) -> Stamp<Self::Period> {
        match date.precision() {
            Precision::Day(..) => {
                let complete = date
                    .complete()
                    .expect("date.complete() should be available, given Precision::Day");
                let iso = complete.to_chrono();
                let gregorian = iso.to_gregorian();
                self.stamp_ymd(iso)
            }
            Precision::DayOfMonth(y, m) | Precision::Month(y, m) => self.stamp_ym(y, m),
            // Render Decade/Century as plain years (e.g. 1990, 1900) for now
            // Same for dayofyear
            Precision::Decade(y)
            | Precision::Century(y)
            | Precision::DayOfYear(y)
            | Precision::MonthOfYear(y)
            | Precision::Year(y) => self.stamp_y(y),
            Precision::Season(y, season) => {
                let (year, era) = chronology::iso_to_year_era(y);
                Stamp::Season {
                    year,
                    iso_year: y,
                    era,
                    season,
                }
            }
        }
    }
    /// Ties it all together
    fn stamp_edtf(&self, edtf: &Edtf) -> RangeStamp<Self::Period> {
        match edtf {
            Edtf::Date(date) => RangeStamp::Date(self.stamp_edtf_date(date)),
            Edtf::DateTime(edtf_dt) => {
                // this part determines which date will end up being rendered, when you have a
                // timestamp in a specific time zone attached.
                //
                // we choose: strip the time and tzoffset information. eventually we can render
                // dates AND times, and when we do, we should render them with the time zone offset
                // written on the EDTF. But for now, just grab the ISO date without modifying it.
                let chrono_dt = edtf_dt.to_chrono_naive();
                let iso = chrono_dt.date();
                // let naive_time = chrono_dt.time();
                // let offset = edtf_dt.time().offset();
                RangeStamp::Date(self.stamp_ymd(iso))
            }
            Edtf::YYear(yy) => {
                // nobody is actually using this for citations... hopefully
                RangeStamp::Date(self.stamp_yy(yy.value()))
            }
            Edtf::Interval(from, to) => {
                let from = self.stamp_edtf_date(from);
                let to = self.stamp_edtf_date(to);
                RangeStamp::Range(from, to)
            }
            Edtf::IntervalFrom(from, _) => {
                let from = self.stamp_edtf_date(from);
                RangeStamp::Range(from, Stamp::RangeOpen)
            }
            Edtf::IntervalTo(_, to) => {
                let to = self.stamp_edtf_date(to);
                RangeStamp::Range(Stamp::RangeOpen, to)
            }
        }
    }
}

impl EdtfStamper for AlwaysGregorian {
    fn stamp_ym(&self, iso_year: i32, month: u32) -> Stamp<Self::Period> {
        let (year, era) = chronology::iso_to_year_era(iso_year);
        Stamp::Date {
            year,
            era,
            iso_year,
            month: super::nonzero(month),
            day: None,
            period: AlwaysGregorian,
        }
    }

    fn stamp_y(&self, iso_year: i32) -> Stamp<Self::Period> {
        let (year, era) = chronology::iso_to_year_era(iso_year);
        Stamp::Date {
            year,
            era,
            iso_year,
            month: None,
            day: None,
            period: AlwaysGregorian,
        }
    }

    fn stamp_yy(&self, year: i64) -> Stamp<Self::Period> {
        // overflow becomes 1BC. Better than panicking.
        let iso_year: i32 = year.try_into().unwrap_or(0);
        let (year, era) = chronology::iso_to_year_era(iso_year);
        Stamp::Date {
            year,
            era,
            iso_year,
            month: None,
            day: None,
            period: AlwaysGregorian,
        }
    }
}

impl Stamp {
    fn from_canon(canon: StampedDate<AlwaysGregorian>, iso_year: i32) -> Self {
        let StampedDate {
            year,
            era,
            month,
            day,
            period: _,
        } = canon;
        Self::Date {
            year,
            era,
            iso_year,
            month: super::nonzero(month),
            day: super::nonzero(day),
            period: AlwaysGregorian,
        }
    }
    fn from_edtf_date(date: &edtf::Date) -> Self {
        AlwaysGregorian.stamp_edtf_date(date)
    }
}

impl<'a> RangeStamp<'a> {
    pub(crate) fn from_date_or_range(dor: &'a DateOrRange) -> Self {
        match dor {
            DateOrRange::Edtf(e) => AlwaysGregorian.stamp_edtf(e),
            DateOrRange::Literal { literal, circa: _ } => RangeStamp::Literal(literal),
        }
    }
}
