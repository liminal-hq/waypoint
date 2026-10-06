// Zip times: a DOS date and time is a wall clock in the local zone, an extended timestamp is a Unix instant.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The convention (D185): the archive provider's `modified_ms` is always a UTC instant. A zip
//! entry that has an extended timestamp (the `UT` extra field Info-ZIP writes) is read from it
//! and written with it. The older MS-DOS date and time has no zone, and every zip tool reads and
//! writes it as the wall clock of the machine it runs on, so it is converted with this machine's
//! zone on the way in and out. Reading a DOS time as UTC (the first version) shifted every such
//! file by the zone's offset.

use chrono::{DateTime, Datelike, Local, LocalResult, NaiveDate, TimeZone, Timelike};

/// The UTC instant (in milliseconds) of the local wall clock `year-month-day hour:minute:second`.
/// A clock time that falls in a daylight saving gap, or is read twice in its overlap, takes the
/// earlier instant, as `mktime` does for an unset `tm_isdst`.
pub(crate) fn local_wall_to_utc_ms(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> Option<i64> {
    let naive = NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(hour, minute, second)?;
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(time) => Some(time.timestamp_millis()),
        LocalResult::Ambiguous(earlier, _) => Some(earlier.timestamp_millis()),
        // In a gap the wall clock never showed: use the instant it would have had before the jump.
        LocalResult::None => {
            let before = Local
                .from_local_datetime(&(naive - chrono::Duration::hours(3)))
                .earliest()?;
            Some(before.timestamp_millis() + 3 * 3_600_000)
        }
    }
}

/// The local wall clock of the UTC instant `modified_ms`: year, month, day, hour, minute, second.
pub(crate) fn utc_ms_to_local_wall(modified_ms: i64) -> Option<(i32, u32, u32, u32, u32, u32)> {
    let time: DateTime<Local> = Local.timestamp_millis_opt(modified_ms).single()?;
    Some((
        time.year(),
        time.month(),
        time.day(),
        time.hour(),
        time.minute(),
        time.second(),
    ))
}
