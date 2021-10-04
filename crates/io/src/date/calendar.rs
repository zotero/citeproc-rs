// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

//! This is where we do the legacy conversion to EDTF.
//!
//! The `edtf::level_1::Edtf` is our internal representation, and it uses the ISO calendar, i.e.
//! proleptic gregorian but with a zero year, so 0000 = 1BC., -0001 = 2BC.
//!
//! However, old CSL-JSON raw and date-parts dates sit on some unspecified calendar. I believe the
//! logic goes that [[-0001, 1, 9]] is "Jan 9, 1BC", but nothing else about the date is
//! really known. That presents a dilemma, of which neither choice is good at all:
//! - If you read that example directly into fields on an EDTF date, and print it as a historical
//! date, it would be "Jan 9, 2BC".
//! - If you print the EDTF fields verbatim and just slap a BC on the end if negative, then actual
//! EDTFs would be incorrect.
//!
//! Therefore one or the other must be converted. We do not know what calendar dates are on, but we
//! can allow people to configure it if the default is not correct. So we could:
//!
//! - make the default proleptic gregorian, interpret unspecified CSL-JSON dates as being proleptic
//! gregorian using negative years.
//! - make the default 'chronology::historical::Canon', i.e. everything after the Gregorian
//! calendar's introduction is Gregorian, everything before that is Julian.
//!
//! The latter seems slightly better, but I have no evidence to go on.
//!
//! What's most important is that when you read in old CSL-JSON with very old dates, they are not
//! off-by-one-BC-year. Both options accomplish this.
//!
//! A lesser problem is being off-by-ten-days due to divergence between whatever calendar people
//! thought CSL-JSON ran on and the chosen behaviour. This is a fairly isolated problem because
//! most academic cited works are not more than a few hundred years old.
//!
//! There are two reasons why you MUST pick a calendar timeline for legacy CSL-JSON dates.
//!
//! - The values eventually need to be sortable. If you mix ISO and "as written" dates, you will
//! get incorrect sorting.
//! - You can either convert ISO to a chosen historical system at sort/display time, or convert
//! CSL-JSON dates to ISO at input time. Both are equivalent but ISO-from-the-start is easier to
//! manage.
//!
//! Tangentially, there are quite a few options controlling the display behaviour of ISO dates,
//! i.e. what calendar it gets rendered in:
//!
//! - Add InitOptions settings to citeproc-rs for a display calendar system, including a historical
//! 'Canon' one.
//! - Add (to CSL, with a new date part formatter) an indication for an old system date "(o.s.)" or
//! whatever people like to write.
//! - Support TC39 Temporal's postfix `2020-04-25[u-ca=hebrew]` notation for specifying a display
//! calendar system system on an individual date. Details:
//! https://tc39.es/proposal-temporal/docs/iso-string-ext.html
//!

use super::Date;
use chronology::{
    historical::{Canon, Historical},
    Era, Iso,
};
use std::convert::TryInto;

pub(crate) fn date_from_csl_json_parts_ymd(year: i32, month: u32, day: u32) -> Option<Date> {
    // citeproc-js spits the dummy with zero years, sometimes empty string, sometimes 1900. Just
    // treat these as invalid, 0 has no meaning when negative is BCE.
    if year == 0 {
        return None;
    }

    // -1 => (1, false)
    //  1 => (1, true)
    let year_abs = year.abs() as u32;
    let era = Era::from(year > 0);

    // This is where the magic happens. When day is specified, we interpret the whole thing in a
    // full-on calendar.
    if (1..=12).contains(&month) && day != 0 {
        // This could be configurable to `chronology::historical::GregorianAdoption` instead
        let calendar = Canon;
        let system = Historical::interpret(year_abs, era, month, day, calendar)?;
        let iso: Iso = system.try_into().ok()?;
        let date: Date = iso.try_into().ok()?;
        Some(date)
    } else {
        // Day not specified, so just fix the year to be ISO.
        let year_iso = chronology::year_era_to_iso_opt(year_abs, era)?;
        Date::from_ymd_opt(year_iso, month, 0)
    }
}
