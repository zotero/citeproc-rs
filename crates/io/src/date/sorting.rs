use std::cell::RefCell;
use std::cmp::Ordering;
use std::ops::DerefMut;
use std::{fmt, hash};

use super::{DateOrRange, LegacyDate};
use edtf::level_1::{Date, Edtf, Precision};

use chronology::Gregorian;
use chronology::{historical::Stampable, CalendarTo};

#[derive(Clone)]
pub struct OrderedDate<'a>(pub &'a DateOrRange);

impl DateOrRange {
    pub fn csl_sort(&self) -> OrderedDate {
        OrderedDate(self)
    }
}

impl fmt::Debug for OrderedDate<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl hash::Hash for OrderedDate<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        with_date_buf(&self.0, IncludeParts::ALL, |as_str| as_str.hash(state))
            .expect("hashing OrderedDate failed")
    }
}

impl PartialEq for OrderedDate<'_> {
    fn eq(&self, other: &Self) -> bool {
        eq_filtered(&self.0, &other.0, IncludeParts::ALL)
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
        cmp_filtered(&self.0, &other.0, IncludeParts::ALL)
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
        date_sort_stamp(d1, include, a.deref_mut()).ok()?;
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
    F: FnMut(&str, &str) -> T,
{
    DATE_CMP_A.with(|a_buf| {
        let mut a = a_buf.borrow_mut();
        a.clear();
        date_sort_stamp(d1, include, a.deref_mut()).ok()?;
        DATE_CMP_B.with(|b_buf| {
            let mut b = b_buf.borrow_mut();
            b.clear();
            date_sort_stamp(d2, include, b.deref_mut()).ok()?;
            Some(f(a.as_str(), b.as_str()))
        })
    })
}

fn partial_eq_filtered(d1: &DateOrRange, d2: &DateOrRange, include: IncludeParts) -> Option<bool> {
    with_date_bufs(d1, d2, include, |a, b| a.eq(b))
}

fn partial_cmp_filtered(
    d1: &DateOrRange,
    d2: &DateOrRange,
    include: IncludeParts,
) -> Option<Ordering> {
    with_date_bufs(d1, d2, include, |a, b| a.cmp(b))
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
struct IncludeParts {
    year: bool,
    month: bool,
    day: bool,
}

impl IncludeParts {
    const ALL: Self = IncludeParts {
        year: true,
        month: true,
        day: true,
    };
}

fn date_sort_stamp(
    date: &DateOrRange,
    include: IncludeParts,
    f: &mut impl fmt::Write,
) -> fmt::Result {
    match date_comparison_ymd(date).map_err(|_| fmt::Error)? {
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
struct YmdStamp(i32, u32, u32);

enum Stamp<'a> {
    Single(YmdStamp),
    Range(YmdStamp, YmdStamp),
    Literal(&'a str),
}

struct StampError;

fn date_comparison_ymd(date: &DateOrRange) -> Result<Stamp, StampError> {
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
            Edtf::YYear(_) => todo!(),
        },
        DateOrRange::Literal { literal, .. } => Ok(Stamp::Literal(literal.as_str())),
        DateOrRange::NoCal(date) => Ok(Stamp::Single(nocal_date_stamp(date)?)),
        DateOrRange::NoCalRange(from, to) => {
            let from = nocal_date_stamp(from)?;
            let to = nocal_date_stamp(to)?;
            Ok(Stamp::Range(from, to))
        }
    }
}

fn nocal_date_stamp(date: &LegacyDate) -> Result<YmdStamp, StampError> {
    let mut year = date.year;
    // we prevent year == 0 in CSL-JSON input.
    if year == 0 {
        return Err(StampError);
    }
    // we also make -1=1BC into an ISO-style year.
    if year < 0 {
        year += 1;
    }
    Ok(YmdStamp(year, date.month, date.day))
}

fn edtf_incomplete_sort_stamp(year: i32, month: u32) -> YmdStamp {
    YmdStamp(year, month, 0)
}

fn edtf_date_sort_stamp(date: &Date) -> Result<YmdStamp, StampError> {
    match date.precision() {
        Precision::Day(..) => {
            let iso = date.to_chrono().ok_or(StampError)?;
            let proleptic: Gregorian = iso.to_gregorian();
            let (y, ce, m, d) = proleptic.basic_stamp();
            let iso_year = chronology::year_ce_to_iso(y, ce);
            Ok(YmdStamp(iso_year, m, d))
        }
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
    fn new(iso_year: i32, month: u32, day: u32) -> YmdStamp {
        YmdStamp(iso_year, month, day)
    }
    /// This prints our own date format for sorting, it's mostly proleptic gregorian if you use
    /// proleptic gregorian dates.
    fn write_stamp(&self, include: IncludeParts, f: &mut impl fmt::Write) -> fmt::Result {
        if include.year {
            write!(f, "{:04}_", self.0)?;
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
