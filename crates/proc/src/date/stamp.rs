// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2021 Corporation for Digital Scholarship

//! This file looks like it does the same thing as `crates/io/src/date/sorting.rs`.
//! However, it is subtly different: sorting tries to put all dates on one timeline

use chrono::Datelike;
use chronology::historical::{CalendarInUse, Canon, Stampable, StampedDate};
use chronology::{CalendarTo, Era, Gregorian, Iso};
use citeproc_io::edtf::Season;
use citeproc_io::{
    edtf::{self, Edtf, Precision},
    DateOrRange,
};

use super::WhichDelim;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum RangeStamp<'a> {
    // TODO: DateTime stamp. This should be a proper CSL feature too.
    Date(Stamp),
    Range(Stamp, Stamp),
    Literal(&'a str),
}

/// Single variant for now, but can be extended to, e.g. `Decade(i32)`,
/// `Century(i32)`,
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Stamp {
    Date {
        year: u32,
        era: Era,
        iso_year: i32,
        month: Option<u32>,
        day: Option<u32>,
        period: CalendarInUse,
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
struct AllTime;

type CanonStamp = StampedDate<CalendarInUse>;

impl From<edtf::Date> for Stamp {
    fn from(date: edtf::Date) -> Self {
        Self::from_edtf_date(&date)
    }
}

impl Stamp {
    fn from_canon(canon: CanonStamp, iso_year: i32) -> Self {
        let CanonStamp {
            year,
            era,
            month,
            day,
            period,
        } = canon;
        Self::Date {
            year,
            era,
            iso_year,
            month: super::nonzero(month),
            day: super::nonzero(day),
            period,
        }
    }
    /// In which we decide how to render all the exotic EDTF formats.
    ///
    /// This is largely unspecified by CSL.
    pub(super) fn from_edtf_date(date: &edtf::Date) -> Self {
        match date.precision() {
            Precision::Day(..) => {
                let complete = date
                    .complete()
                    .expect("date.complete() should be available, given Precision::Day");
                let iso = complete.to_chrono();
                let historical = iso.to_historical(Canon);
                let stamp = historical.stamp();
                Self::from_canon(stamp, iso.year())
            }
            Precision::DayOfMonth(y, m) | Precision::Month(y, m) => {
                let (year, era) = chronology::iso_to_year_era(y);
                let crossover = Iso::from_ymd(1582, 10, 15);
                let jan1 = Iso::from_ymd(y, m, 1);
                let period = if jan1 >= crossover {
                    CalendarInUse::Gregorian
                } else {
                    CalendarInUse::Julian
                };
                Self::Date {
                    year,
                    era,
                    iso_year: y,
                    month: Some(m),
                    day: None,
                    period,
                }
            }
            // Render Decade/Century as plain years (e.g. 1990, 1900) for now
            // Same for dayofyear
            Precision::Decade(y)
            | Precision::Century(y)
            | Precision::DayOfYear(y)
            | Precision::MonthOfYear(y)
            | Precision::Year(y) => {
                let (year, era) = chronology::iso_to_year_era(y);
                let crossover = Iso::from_ymd(1582, 10, 15);
                let jan1 = Iso::from_ymd(y, 1, 1);
                let period = if jan1 >= crossover {
                    CalendarInUse::Gregorian
                } else {
                    CalendarInUse::Julian
                };
                Self::Date {
                    year,
                    era,
                    iso_year: y,
                    month: None,
                    day: None,
                    period,
                }
            }
            Precision::Season(y, season) => {
                let (year, era) = chronology::iso_to_year_era(y);
                Stamp::Season { year, iso_year: y, era, season }
            }
        }
    }
}

impl<'a> RangeStamp<'a> {
    fn from_iso(iso: chronology::Iso) -> Self {
        let gregorian = Gregorian::from(iso);
        let stamp = gregorian.historical_date_stamp(AllTime);
        let stamp = Stamp::Date {
            iso_year: iso.year(),
            year: stamp.year,
            month: Some(stamp.month),
            day: Some(stamp.day),
            era: stamp.era,
            period: CalendarInUse::Gregorian,
        };
        RangeStamp::Date(stamp)
    }
    fn from_edtf_date(date: &edtf::Date) -> Self {
        let stamp = Stamp::from_edtf_date(date);
        RangeStamp::Date(stamp)
    }
    fn from_edtf(edtf: &Edtf) -> Self {
        match edtf {
            Edtf::Date(date) => RangeStamp::from_edtf_date(date),
            Edtf::DateTime(edtf_dt) => {
                // this part determines which date will end up being rendered, when you have a
                // timestamp in a specific time zone attached.
                //
                // we choose: strip the time and tzoffset information. eventually we can render
                // dates AND times, and when we do, we should render them with the time zone offset
                // written on the EDTF. But for now, just grab the ISO date without modifying it.
                let chrono_dt = edtf_dt.to_chrono_naive();
                let naive_date = chrono_dt.date();
                // let naive_time = chrono_dt.time();
                // let offset = edtf_dt.time().offset();
                RangeStamp::from_iso(naive_date)
            }
            Edtf::YYear(yy) => {
                let yy = *yy;
                // nobody is actually using this for citations... hopefully
                let yy64 = yy.year();
                let yy32 = if yy64 > i32::MAX as i64 {
                    i32::MAX
                } else if yy64 < i32::MIN as i64 {
                    i32::MIN
                } else {
                    yy64 as i32
                };
                let (year, era) = chronology::iso_to_year_era(yy32);
                let cal = match era {
                    // there are no yy years even close to Canon crossover
                    Era::CE => CalendarInUse::Gregorian,
                    Era::BCE => CalendarInUse::Julian,
                };
                // only year
                let stamp = CanonStamp::new(year, era, 0, 0, cal);
                RangeStamp::Date(Stamp::from_canon(stamp, yy32))
            }
            Edtf::Interval(from, to) => {
                let from = Stamp::from_edtf_date(from);
                let to = Stamp::from_edtf_date(to);
                RangeStamp::Range(from, to)
            }
            Edtf::IntervalFrom(from, _) => {
                let from = Stamp::from_edtf_date(from);
                RangeStamp::Range(from, Stamp::RangeOpen)
            }
            Edtf::IntervalTo(_, to) => {
                let to = Stamp::from_edtf_date(to);
                RangeStamp::Range(Stamp::RangeOpen, to)
            }
        }
    }
    pub(crate) fn from_date_or_range(dor: &'a DateOrRange) -> Self {
        match dor {
            DateOrRange::Edtf(e) => RangeStamp::from_edtf(e),
            DateOrRange::Literal { literal, circa: _ } => RangeStamp::Literal(literal),
        }
    }
}
