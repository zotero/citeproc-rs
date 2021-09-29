// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2021 Corporation for Digital Scholarship

//! This file looks like it does the same thing as `crates/io/src/date/sorting.rs`.
//! However, it is subtly different: sorting tries to put all dates on one timeline

use chronology::historical::{CalendarInUse, Canon, Stampable, StampedDate};
use chronology::{CalendarTo, Era, Gregorian, Iso};
use citeproc_io::edtf::Season;
use citeproc_io::{
    edtf::{self, Edtf, Precision},
    DateOrRange,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Stamp<'a> {
    // TODO: DateTime stamp. This should be a proper CSL feature too.
    Date(ExtendedStamp),
    Range {
        from: Option<ExtendedStamp>,
        to: Option<ExtendedStamp>,
    },
    Literal(&'a str),
}

/// Single variant for now, but can be extended to, e.g. `Decade(i32)`,
/// `Century(i32)`,
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExtendedStamp {
    Date {
        year: u32,
        era: Era,
        month: Option<u32>,
        day: Option<u32>,
        period: CalendarInUse,
    },
    Season {
        year: u32,
        era: Era,
        season: Season,
    },
}

impl From<CanonStamp> for ExtendedStamp {
    fn from(canon: CanonStamp) -> Self {
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
            month: super::nonzero(month),
            day: super::nonzero(day),
            period,
        }
    }
}

// We don't need more than one era (yet), because we're only doing [Canon], which is a
// single-era calendar. (chronology's era is separate from CE/BCE).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct AllTime;

type CanonStamp = StampedDate<CalendarInUse>;

/// In which we decide how to render all the exotic EDTF formats.
///
/// This is largely unspecified by CSL.
fn stamp_edtf_date(date: &edtf::Date) -> ExtendedStamp {
    match date.precision() {
        Precision::Day(..) => {
            let complete = date
                .complete()
                .expect("date.complete() should be available, given Precision::Day");
            let iso = complete.to_chrono();
            let historical = iso.to_historical(Canon);
            let stamp = historical.stamp();
            stamp.into()
        }
        Precision::DayOfMonth(y, m) | Precision::Month(y, m) => {
            // using day = 1, it's guaranteed not to fail by being e.g. february 31
            let iso = Iso::from_ymd(y, m, 1);
            let historical = iso.to_historical(Canon);
            let mut stamp = historical.stamp();
            // erase the day
            stamp.day = 0;
            stamp.into()
        }
        // Render Decade/Century as plain years (e.g. 1990, 1900) for now
        Precision::Decade(y)
        | Precision::Century(y)

        // Same for dayofyear
        | Precision::DayOfYear(y)
        | Precision::MonthOfYear(y)
        | Precision::Year(y) => {
            // using month=1,day=1, you just get day 1 of that year
            let iso = Iso::from_ymd(y, 1, 1);
            let historical = iso.to_historical(Canon);
            let mut stamp = historical.stamp();
            stamp.month = 0;
            stamp.day = 0;
            stamp.into()
        }
        Precision::Season(y, season) => {
            let (year, era) = chronology::iso_to_year_era(y);
            ExtendedStamp::Season { year, era, season }
        }
    }
}

impl Stamp<'_> {
    fn from_iso(iso: chronology::Iso) -> Self {
        let gregorian = Gregorian::from(iso);
        let stamp = gregorian.historical_date_stamp(AllTime);
        Stamp::Date(stamp)
    }
    fn from_edtf(edtf: &Edtf) -> Self {
        match edtf {
            Edtf::Date(date) => Stamp::from_edtf_date(date),
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
                Stamp::from_iso(naive_date)
            }
            Edtf::YYear(yy) => {
                let mut yy = *yy;
                // nobody is actually using this for citations... hopefully
                let yy64 = yy.year();
                let yy32 = if yy64 > i32::MAX as i64 {
                    i32::MAX
                } else if yy64 < i32::MIN as i64 {
                    i32::MIN
                } else {
                    yy64 as i32
                };
                let (era, year) = chronology::iso_to_year_era(yy32);
                let cal = match era {
                    // there are no yy years even close to Canon crossover
                    Era::CE => CalendarInUse::Gregorian,
                    Era::BCE => CalendarInUse::Julian,
                };
                // only year
                let stamp = CanonStamp::new(year, era, 0, 0, cal);
                Stamp::Date(stamp.into())
            }
            Edtf::Interval(from, to) => {
                let from = Some(stamp_edtf_date(from));
                let to = Some(stamp_edtf_date(to));
                Stamp::Range { from, to }
            }
            Edtf::IntervalFrom(from, _) => {
                let from = Some(stamp_edtf_date(from));
                Stamp::Range { from, to: None }
            }
            Edtf::IntervalTo(_, to) => {
                let to = Some(stamp_edtf_date(to));
                Stamp::Range { from: None, to }
            }
        }
    }
    pub(crate) fn from_date_or_range(dor: &DateOrRange) -> Self {
        match dor {
            DateOrRange::Edtf(e) => Stamp::from_edtf(e),
            DateOrRange::Literal { literal, circa: _ } => Stamp::Literal(literal),
        }
    }
}
