// When a queued job may start (D157): at a time, or only inside a window of the day. Pure: it is
// given the time, and holds no clock of its own.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A window is minutes of the local day, so a job set to run "between 22:00 and 06:00" starts
// whenever the local time is inside that and waits otherwise, across midnight and from one day to
// the next. The local day is the UTC time moved by the offset recorded when the schedule was set,
// so a change of daylight saving time moves a window by an hour until it is set again.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

const MS_PER_MINUTE: i64 = 60_000;
const MINUTES_PER_DAY: i64 = 24 * 60;
const MS_PER_DAY: i64 = MINUTES_PER_DAY * MS_PER_MINUTE;

/// The largest offset from UTC a schedule accepts, in minutes (UTC+14 and UTC-12 are the extremes in use).
const MAX_OFFSET_MINUTES: i32 = 15 * 60;

/// When a queued job may start. A job that has started is not held by it again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum Schedule {
    /// Not before this time. A time that has passed means now.
    StartAt {
        #[ts(type = "number")]
        at_ms: i64,
    },
    /// Only while the local time of day is from `start_minute` up to, not including, `end_minute`
    /// (minutes after midnight). An end at or before the start runs across midnight.
    Window {
        start_minute: u16,
        end_minute: u16,
        /// Minutes the local time is ahead of UTC (negative west of it), as it was when the window was set.
        utc_offset_minutes: i32,
    },
}

impl Schedule {
    /// Whether the schedule is well formed: a window of at least a minute and less than a day,
    /// minutes inside the day and a real offset.
    pub fn validate(&self) -> Result<(), String> {
        match *self {
            Schedule::StartAt { .. } => Ok(()),
            Schedule::Window {
                start_minute,
                end_minute,
                utc_offset_minutes,
            } => {
                if i64::from(start_minute) >= MINUTES_PER_DAY
                    || i64::from(end_minute) >= MINUTES_PER_DAY
                {
                    Err("a window's times must be within a day".to_owned())
                } else if start_minute == end_minute {
                    Err("a window must start and end at different times".to_owned())
                } else if utc_offset_minutes.abs() > MAX_OFFSET_MINUTES {
                    Err("the time zone is out of range".to_owned())
                } else {
                    Ok(())
                }
            }
        }
    }

    /// The local minute of the day at `now_ms` for a window.
    fn minute_of_day(now_ms: i64, utc_offset_minutes: i32) -> i64 {
        let local = now_ms.saturating_add(i64::from(utc_offset_minutes) * MS_PER_MINUTE);
        local.rem_euclid(MS_PER_DAY) / MS_PER_MINUTE
    }

    /// Whether a job may start at `now_ms`.
    pub fn is_open(&self, now_ms: i64) -> bool {
        self.opens_in_ms(now_ms) == 0
    }

    /// How long from `now_ms` until a job may start: zero when it may now. A malformed window
    /// (equal times) never opens and answers `i64::MAX`, which `validate` keeps out of the queue.
    pub fn opens_in_ms(&self, now_ms: i64) -> i64 {
        match *self {
            Schedule::StartAt { at_ms } => (at_ms - now_ms).max(0),
            Schedule::Window {
                start_minute,
                end_minute,
                utc_offset_minutes,
            } => {
                let (start, end) = (i64::from(start_minute), i64::from(end_minute));
                if start == end {
                    return i64::MAX;
                }
                let minute = Self::minute_of_day(now_ms, utc_offset_minutes);
                let inside = if start < end {
                    (start..end).contains(&minute)
                } else {
                    minute >= start || minute < end
                };
                if inside {
                    return 0;
                }
                // Whole minutes to the next start, less the part of the current minute that has gone.
                let into_minute = now_ms
                    .saturating_add(i64::from(utc_offset_minutes) * MS_PER_MINUTE)
                    .rem_euclid(MS_PER_MINUTE);
                let minutes = (start - minute).rem_euclid(MINUTES_PER_DAY);
                minutes * MS_PER_MINUTE - into_minute
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 60 * MS_PER_MINUTE;

    fn window(start: (u16, u16), end: (u16, u16), offset: i32) -> Schedule {
        Schedule::Window {
            start_minute: start.0 * 60 + start.1,
            end_minute: end.0 * 60 + end.1,
            utc_offset_minutes: offset,
        }
    }

    #[test]
    fn a_start_time_opens_at_it_and_a_past_one_is_now() {
        let s = Schedule::StartAt { at_ms: 10_000 };
        assert!(!s.is_open(9_999));
        assert_eq!(s.opens_in_ms(4_000), 6_000);
        assert!(s.is_open(10_000));
        assert!(s.is_open(99_999));
    }

    #[test]
    fn a_daytime_window_is_open_inside_and_waits_for_the_next_start_outside() {
        let w = window((9, 0), (17, 0), 0);
        assert!(!w.is_open(8 * HOUR));
        assert!(w.is_open(9 * HOUR));
        assert!(w.is_open(16 * HOUR + 59 * MS_PER_MINUTE));
        assert!(!w.is_open(17 * HOUR));
        assert_eq!(w.opens_in_ms(8 * HOUR), HOUR);
        // After it closes, the next opening is tomorrow morning.
        assert_eq!(w.opens_in_ms(18 * HOUR), 15 * HOUR);
    }

    #[test]
    fn a_window_across_midnight_is_open_on_both_sides_of_it() {
        let w = window((22, 0), (6, 0), 0);
        assert!(w.is_open(23 * HOUR));
        assert!(w.is_open(2 * HOUR));
        assert!(w.is_open(25 * HOUR));
        assert!(!w.is_open(6 * HOUR));
        assert!(!w.is_open(12 * HOUR));
        assert_eq!(w.opens_in_ms(12 * HOUR), 10 * HOUR);
    }

    #[test]
    fn the_window_is_in_local_time() {
        // 22:00 to 06:00 in UTC-5 is 03:00 to 11:00 UTC.
        let w = window((22, 0), (6, 0), -300);
        assert!(w.is_open(4 * HOUR));
        assert!(!w.is_open(12 * HOUR));
        assert_eq!(w.opens_in_ms(12 * HOUR), 15 * HOUR);
        // A time before the epoch still works.
        assert!(w.is_open(-20 * HOUR));
    }

    #[test]
    fn the_wait_counts_from_part_way_through_a_minute() {
        let w = window((9, 0), (10, 0), 0);
        assert_eq!(
            w.opens_in_ms(8 * HOUR + 59 * MS_PER_MINUTE + 30_000),
            30_000
        );
    }

    #[test]
    fn validation_refuses_what_cannot_run() {
        assert!(window((9, 0), (17, 0), 0).validate().is_ok());
        assert!(window((22, 0), (6, 0), 330).validate().is_ok());
        assert!(window((9, 0), (9, 0), 0).validate().is_err());
        let late = Schedule::Window {
            start_minute: 24 * 60,
            end_minute: 5,
            utc_offset_minutes: 0,
        };
        assert!(late.validate().is_err());
        assert!(window((1, 0), (2, 0), 16 * 60).validate().is_err());
        assert!(Schedule::StartAt { at_ms: -1 }.validate().is_ok());
    }

    #[test]
    fn a_schedule_serialises_plainly() {
        let json = serde_json::to_string(&Schedule::StartAt { at_ms: 5 }).unwrap();
        assert_eq!(json, r#"{"kind":"startAt","atMs":5}"#);
        let back: Schedule = serde_json::from_str(
            r#"{"kind":"window","startMinute":60,"endMinute":120,"utcOffsetMinutes":-300}"#,
        )
        .unwrap();
        assert_eq!(back, window((1, 0), (2, 0), -300));
    }
}
