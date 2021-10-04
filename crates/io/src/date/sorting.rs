use std::cell::RefCell;
use std::cmp::Ordering;
use std::convert::TryInto;
use std::ops::DerefMut;
use std::{fmt, hash};

use super::DateOrRange;
use crate::String;
use edtf::level_1::{Date, Edtf, Precision};

use chronology::Gregorian;
use chronology::{historical::Stampable, CalendarTo};

impl DateOrRange {
    /// Gives a struct that sorts in CSL order
    pub fn csl_sort(&self) -> OrderedDate {
        OrderedDate(self)
    }

    /// Produces a string
    pub fn sort_string(&self, include: IncludeParts, into: &mut impl fmt::Write) {
        date_stamp_fmt(self, include, into).unwrap();
    }
}

#[derive(Copy, Clone)]
pub struct OrderedDate<'a>(&'a DateOrRange);

impl fmt::Debug for OrderedDate<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl PartialEq for OrderedDate<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for OrderedDate<'_> {}

impl PartialOrd for OrderedDate<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedDate<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        use super::DateOrRange as R;
        let (a, b) = (&self.0, &other.0);
        match (a, b) {
            (R::Literal { literal: a, .. }, R::Literal { literal: b, .. }) => a.cmp(b),
            (_, R::Literal { .. }) | (R::Literal { .. }, _) => {
                cmp_filtered(a, b, IncludeParts::ALL)
            }
            (R::Edtf(a), R::Edtf(b)) => cmp_edtfs(a, b),
        }
    }
}

fn cmp_edtfs(a: &Edtf, b: &Edtf) -> Ordering {
    edtf_start_date(a)
        .cmp(&edtf_start_date(b))
        .then_with(|| edtf_end_date(a).cmp(&edtf_end_date(b)))
}

fn edtf_start_date(edtf: &Edtf) -> Option<Date> {
    match edtf {
        Edtf::Date(d) => Some(*d),
        Edtf::Interval(d, _) => Some(*d),
        Edtf::IntervalFrom(d, _) => Some(*d),
        // this sorts first, which makes sense
        Edtf::IntervalTo(_, _) => None,
        Edtf::DateTime(d) => Some(Date::from_complete(d.date())),
        Edtf::YYear(y) => {
            // not super important
            let yi32: i32 = y.value().try_into().ok()?;
            Date::from_ymd_opt(yi32, 0, 0)
        }
    }
}

fn edtf_end_date(edtf: &Edtf) -> Option<Date> {
    match edtf {
        Edtf::Date(_) | Edtf::DateTime(_) | Edtf::YYear(_) => edtf_start_date(edtf),
        Edtf::Interval(_, d) => Some(*d),
        Edtf::IntervalFrom(_, _) => None,
        Edtf::IntervalTo(_, d) => Some(*d),
    }
}

// We're going to sort dates using strings, so let's just use two strings total instead of
// allocating two new ones every time.
//
thread_local! {
    static DATE_CMP_A: RefCell<String> = Default::default();
    static DATE_CMP_B: RefCell<String> = Default::default();
}

fn with_date_buf<F, T>(d1: &DateOrRange, include: IncludeParts, mut f: F) -> Option<T>
where
    F: FnMut(&str) -> T,
{
    DATE_CMP_A.with(|a_buf| {
        let mut a = a_buf.borrow_mut();
        a.clear();
        date_stamp_fmt(d1, include, a.deref_mut()).ok()?;
        Some(f(a.as_str()))
    })
}

fn with_date_bufs<F, T>(
    d1: &DateOrRange,
    d2: &DateOrRange,
    include: IncludeParts,
    mut f: F,
) -> Option<T>
where
    F: FnMut(Option<&str>, Option<&str>) -> T,
{
    DATE_CMP_A.with(|a_buf| {
        let mut a = a_buf.borrow_mut();
        a.clear();
        let a_ok = date_stamp_fmt(d1, include, a.deref_mut()).is_ok();
        DATE_CMP_B.with(|b_buf| {
            let mut b = b_buf.borrow_mut();
            b.clear();
            let b_ok = date_stamp_fmt(d2, include, b.deref_mut()).is_ok();
            let a = a_ok.then(|| a.as_str());
            let b = b_ok.then(|| b.as_str());
            log::debug!("comparing dates via OrderedDate: {:?} {:?}", a, b);
            Some(f(a, b))
        })
    })
}

fn partial_eq_filtered(d1: &DateOrRange, d2: &DateOrRange, include: IncludeParts) -> Option<bool> {
    with_date_bufs(d1, d2, include, |a, b| a.eq(&b))
}

fn partial_cmp_filtered(
    d1: &DateOrRange,
    d2: &DateOrRange,
    include: IncludeParts,
) -> Option<Ordering> {
    with_date_bufs(d1, d2, include, |a, b| a.cmp(&b))
}

fn cmp_filtered(d1: &DateOrRange, d2: &DateOrRange, include: IncludeParts) -> Ordering {
    partial_cmp_filtered(d1, d2, include).expect(
        "date comparison (cmp) failed, probably because of failure to convert ISO to display calendar",
    )
}

fn eq_filtered(d1: &DateOrRange, d2: &DateOrRange, include: IncludeParts) -> bool {
    partial_eq_filtered(d1, d2, include).expect(
        "date comparison (eq) failed, probably because of failure to convert ISO to display calendar",
    )
}

#[derive(Copy, Clone)]
pub struct IncludeParts {
    pub year: bool,
    pub month: bool,
    pub day: bool,
}

impl IncludeParts {
    pub const NONE: Self = IncludeParts {
        year: false,
        month: false,
        day: false,
    };
    pub const ALL: Self = IncludeParts {
        year: true,
        month: true,
        day: true,
    };
}

fn date_stamp_fmt(
    date: &DateOrRange,
    include: IncludeParts,
    f: &mut impl fmt::Write,
) -> fmt::Result {
    match date_stamp(date).map_err(|_| fmt::Error)? {
        Stamp::Literal(s) => f.write_str(s)?,
        Stamp::Range(from, to) => {
            from.write_stamp(include, f)?;
            write!(f, "/")?;
            to.write_stamp(include, f)?;
        }
        Stamp::Single(ymd) => ymd.write_stamp(include, f)?,
    }
    Ok(())
}

/// Verbatim dates have -1=1BC year to converted to ISO year, but everything else same
/// ISO dates must be converted to proleptic gregorian (or whatever calendar display people say
/// they want, that's TODO)
#[derive(Copy, Clone)]
struct YmdStamp(i32, u32, u32);

#[derive(Copy, Clone)]
enum Stamp<'a> {
    Single(YmdStamp),
    Range(YmdStamp, YmdStamp),
    Literal(&'a str),
}

pub struct StampError;

fn date_stamp(date: &DateOrRange) -> Result<Stamp, StampError> {
    match date {
        DateOrRange::Edtf(edtf) => match edtf {
            Edtf::Date(d) => Ok(Stamp::Single(edtf_date_sort_stamp(d)?)),
            Edtf::Interval(d1, d2) => {
                let from = edtf_date_sort_stamp(d1)?;
                let to = edtf_date_sort_stamp(d2)?;
                Ok(Stamp::Range(from, to))
            }
            // These... not sure.
            Edtf::IntervalFrom(d1, _terminal) => Ok(Stamp::Single(edtf_date_sort_stamp(d1)?)),
            Edtf::IntervalTo(_terminal, d2) => Ok(Stamp::Single(edtf_date_sort_stamp(d2)?)),
            Edtf::DateTime(dt) => {
                let date = Date::from_complete(dt.date());
                Ok(Stamp::Single(edtf_date_sort_stamp(&date)?))
            }
            Edtf::YYear(y) => {
                let as_i32: i32 = y.value().try_into().map_err(|_| StampError)?;
                Ok(Stamp::Single(YmdStamp(as_i32, 0, 0)))
            }
        },
        DateOrRange::Literal { literal, .. } => Ok(Stamp::Literal(literal.as_str())),
    }
}

// fn nocal_date_stamp(date: &LegacyDate) -> Result<YmdStamp, StampError> {
//     let mut year = date.year;
//     // we prevent year == 0 in CSL-JSON input.
//     if year == 0 {
//         return Err(StampError);
//     }
//     // we also make -1=1BC into an ISO-style year.
//     if year < 0 {
//         year += 1;
//     }
//     Ok(YmdStamp(year, date.month, date.day))
// }

fn edtf_incomplete_sort_stamp(year: i32, month: u32) -> YmdStamp {
    YmdStamp(year, month, 0)
}

fn edtf_date_sort_stamp(date: &Date) -> Result<YmdStamp, StampError> {
    match date.precision() {
        Precision::Day(y, m, d) => Ok(YmdStamp(y, m, d)),
        Precision::Season(y, s) => {
            // iffy month-like seasons here
            Ok(YmdStamp(y, s as u32, 0))
        }
        Precision::DayOfMonth(y, m) | Precision::Month(y, m) => Ok(YmdStamp(y, m, 0)),
        Precision::MonthOfYear(y) | Precision::DayOfYear(y) => Ok(YmdStamp(y, 0, 0)),
        Precision::Century(y) | Precision::Decade(y) | Precision::Year(y) => Ok(YmdStamp(y, 0, 0)),
    }
}

impl YmdStamp {
    /// This prints our own date format for sorting, it's mostly proleptic gregorian if you use
    /// proleptic gregorian dates.
    fn write_stamp(&self, include: IncludeParts, f: &mut impl fmt::Write) -> fmt::Result {
        if include.year {
            if self.0 < 0 {
                write!(f, "-")?;
            }
            write!(f, "{:04}_", self.0.abs())?;
        }
        if include.month {
            write!(f, "{:02}", self.1)?;
        }
        if include.day {
            write!(f, "{:02}", self.1)?;
        }
        Ok(())
    }
}
