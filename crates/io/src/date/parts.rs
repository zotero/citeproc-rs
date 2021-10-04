use edtf::level_1::Terminal;

use super::{calendar, Date, DateOrRange, Edtf};

impl DateOrRange {
    /// From a `"date-parts": [[1999, 12, 31]]`-style value in CSL-JSON. Converts to ISO using the
    /// [chronology::historical::Canon] timeline.
    pub fn from_csl_date_parts_arrays(parts: &[&[i32]]) -> Option<Self> {
        fn to_tuple(parts: &[i32]) -> Option<(i32, u32, u32)> {
            let m = *parts.get(1).unwrap_or(&0);
            let d = *parts.get(2).unwrap_or(&0);
            let year = *parts.get(0)?;
            if m >= 0 && d >= 0 {
                Some((year, m as u32, d as u32))
            } else {
                None
            }
        }
        Some(match parts {
            [single] => Self::from_csl_date_parts(to_tuple(single)?, None)?,
            // it's fine if people want to tack 2 million extra date-parts arrays on the end
            // go ahead, make my day
            [from, to] => Self::from_csl_date_parts(to_tuple(from)?, Some(to_tuple(to)?))?,
            _ => return None,
        })
    }

    pub(crate) fn from_csl_date_parts(
        from: (i32, u32, u32),
        to: Option<(i32, u32, u32)>,
    ) -> Option<Self> {
        Some(match to {
            None => Edtf::Date(date_from_csl_json_parts(from)?),
            Some((0, 0, 0)) => Edtf::IntervalFrom(date_from_csl_json_parts(from)?, Terminal::Open),
            Some(to) => match from {
                (0, 0, 0) => Edtf::IntervalTo(Terminal::Open, date_from_csl_json_parts(to)?),
                _ => Edtf::Interval(
                    date_from_csl_json_parts(from)?,
                    date_from_csl_json_parts(to)?,
                ),
            },
        })
        .map(DateOrRange::Edtf)
    }
}

fn date_from_csl_json_parts(parts: (i32, u32, u32)) -> Option<Date> {
    let year = parts.0;
    let m = parts.1;
    let d = parts.2;
    let month = if (1..=12).contains(&m) {
        m
    } else if (13..=16).contains(&m) {
        m + 8
    } else if (17..=20).contains(&m) {
        m + 4
    } else if (21..=24).contains(&m) {
        m
    } else {
        0
    };
    let day = if d >= 1 && d <= 31 { d as u32 } else { 0 };

    calendar::date_from_csl_json_parts_ymd(year, month, day)
}

#[cfg(test)]
#[test]
fn test_from_parts() {
    assert_eq!(
        DateOrRange::from_csl_date_parts_arrays(&[&[1998, 9, 21]]),
        Some(DateOrRange::new(1998, 09, 21))
    );
    assert_eq!(
        DateOrRange::from_csl_date_parts_arrays(&[&[1998, 9]]),
        Some(DateOrRange::new(1998, 9, 0))
    );
    assert_eq!(
        DateOrRange::from_csl_date_parts_arrays(&[&[1998]]),
        Some(DateOrRange::new(1998, 0, 0))
    );
    assert_eq!(
        DateOrRange::from_csl_date_parts_arrays(&[&[1998, 9, 21], &[2001, 8, 16]]),
        Some(DateOrRange::Edtf(Edtf::Interval(
            Date::from_ymd(1998, 9, 21),
            Date::from_ymd(2001, 8, 16)
        )))
    );
    assert_eq!(
        DateOrRange::from_csl_date_parts_arrays(&[&[1998, 9, 21], &[2001, 8]]),
        Some(DateOrRange::Edtf(Edtf::Interval(
            Date::from_ymd(1998, 9, 21),
            Date::from_ymd(2001, 8, 0)
        )))
    );
    assert_eq!(
        DateOrRange::from_csl_date_parts_arrays(&[&[1998, 9], &[2001, 8, 1]]),
        Some(DateOrRange::Edtf(Edtf::Interval(
            Date::from_ymd(1998, 9, 0),
            Date::from_ymd(2001, 8, 1)
        )))
    );
    assert_eq!(
        DateOrRange::from_csl_date_parts_arrays(&[&[1998], &[2001]]),
        Some(DateOrRange::Edtf(Edtf::Interval(
            Date::from_ymd(1998, 0, 0),
            Date::from_ymd(2001, 0, 0)
        )))
    );
}
