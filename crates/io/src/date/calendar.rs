// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use super::Date;
use chronology::{
    historical::{Canon, Historical},
    Iso,
};
use std::convert::TryInto;

/// This is where we do the legacy conversion to EDTF. See also `ymd_date` below.
///
/// The `edtf::level_1::Edtf` is our internal representation, and it uses the ISO calendar, i.e.
/// proleptic gregorian but with a zero year, so 0000 = 1BC., -0001 = 2BC.
///
/// However, old CSL-JSON raw and date-parts dates sit on some unspecified historical calendar. I
/// believe the logic goes that [[-0001, 1, 9]] is "Jan 9, 1BC", but nothing else about the date is
/// really known. That presents a dilemma, of which neither choice is good at all:
/// - If you read that example directly into fields on an EDTF date, and print it as a historical
/// date, it would be "Jan 9, 2BC".
/// - If you print the EDTF fields verbatim and just slap a BC on the end if negative, then actual
/// EDTFs would be incorrect.
///
/// What's most important is that when you read in old CSL-JSON with very old dates, they are not
/// off-by-one-BC-year or off-by-ten-days due to divergence between whatever calendar people
/// thought CSL-JSON ran on and EDTF's ISO calendar. This is a fairly isolated problem because most
/// academic cited works are not more than a few hundred years old. It is however caused entirely
/// by adopting EDTF as an internal format.
///
///
///
///
///
///
///
/// THEREFORE !!!! you could just not make EDTF the internal representation.
/// It should probably be that you can have verbatim dates and ranges, where the numbers
/// are trusted absolutely. (Even if February 31 is not a real thing.)
///
///
///
///
///
///
///
///
///
///
///
///
///
///
///
///
///
///
///
///
/// Tangentially, there is some potential for controlling the display behaviour of an EDTF, i.e.
/// what calendar it gets rendered in. Some options (not exclusive):
///
/// - Add InitOptions settings to citeproc-rs for a display calendar system, including a historical
/// one where it's proleptic Julian until the Gregorian adoption day (which would also be
/// configurable).
/// - Add (to CSL, with a new date part formatter) an indication for an old system date "(o.s.)" or
/// whatever people like to write.
/// - Support TC39 Temporal's postfix `2020-04-25[u-ca=hebrew]` notation for specifying a display
/// calendar system system on an individual date. Details:
/// https://tc39.es/proposal-temporal/docs/iso-string-ext.html
///
pub(crate) fn date_from_parts_ymd(year: i32, month: u32, day: u32) -> Option<Date> {
    // citeproc-js spits the dummy with zero years, sometimes empty string, sometimes 1900. Just
    // treat these as invalid, 0 has no meaning when negative is BCE.
    if year == 0 {
        return None;
    }

    // -1 => (1, false)
    //  1 => (1, true)
    let (year_abs, year_ce) = (year.abs() as u32, year.signum() == 1);

    // This is where the magic happens. When day is specified, we interpret the whole thing in a
    // full-on calendar.
    if day != 0 {
        // This could be configurable to `chronology::historical::GregorianAdoption` instead
        let system = Historical::from_ycmd_opt(year_abs, year_ce, month, day, Canon)?;
        let iso: Iso = system.try_into().ok()?;
        let date: Date = iso.try_into().ok()?;
        Some(date)
    } else {
        // Day not specified, so just fix the year to be ISO.
        let year_iso = chronology::year_ce_to_iso_opt(year_abs, year_ce)?;
        Date::from_ymd_opt(year_iso, month, 0)
    }
}
