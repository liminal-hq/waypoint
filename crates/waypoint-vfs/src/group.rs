// Grouping: which headed run of the sorted rows an entry belongs to, and the order of the runs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::cmp::Ordering;

use crate::icon::extension;
use crate::model::{
    GroupBy, GroupKey, GroupRun, IconGroup, ModifiedBucket, SizeBand, SortKey, SortSpec,
};
use crate::order::{cmp_extension, Sortable};

const DAY_MS: i64 = 86_400_000;

/// "Now" as the Modified groups need it: today's calendar day and year in the local time zone.
///
/// The index holds one for as long as its order stands, so every comparison between two
/// rebuilds agrees; it is read again whenever the index is rebuilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GroupClock {
    /// Days since 1970-01-01 of today, locally.
    today: i64,
    year: i32,
    /// The local time zone's offset from UTC, in milliseconds.
    offset_ms: i64,
}

impl GroupClock {
    pub fn new(now_ms: i64, offset_secs: i32) -> Self {
        let offset_ms = i64::from(offset_secs) * 1000;
        let today = (now_ms + offset_ms).div_euclid(DAY_MS);
        Self {
            today,
            year: civil_year(today),
            offset_ms,
        }
    }

    /// The clock of this moment in the system's local time zone.
    pub fn now() -> Self {
        let now = chrono::Local::now();
        Self::new(
            now.timestamp_millis(),
            chrono::Offset::fix(now.offset()).local_minus_utc(),
        )
    }

    /// The group of a modified time. The bands only ever get older as the time goes back, so the
    /// order of the bands is the order of the times.
    fn bucket(&self, modified_ms: Option<i64>) -> Band {
        let Some(ms) = modified_ms else {
            return Band::Modified(ModifiedBucket::Unknown);
        };
        let day = (ms.saturating_add(self.offset_ms)).div_euclid(DAY_MS);
        let ago = self.today - day;
        // 1970-01-01 was a Thursday, and the week starts on a Monday.
        let into_week = (self.today + 3).rem_euclid(7);
        Band::Modified(if ago <= 0 {
            ModifiedBucket::Today
        } else if ago == 1 {
            ModifiedBucket::Yesterday
        } else if ago <= into_week {
            ModifiedBucket::EarlierThisWeek
        } else if ago <= 7 {
            ModifiedBucket::Last7Days
        } else if ago <= 30 {
            ModifiedBucket::Last30Days
        } else {
            let year = civil_year(day);
            return if year >= self.year {
                Band::Modified(ModifiedBucket::ThisYear)
            } else {
                Band::Year(year)
            };
        })
    }
}

/// The calendar year of a day number (days since 1970-01-01), by Howard Hinnant's `civil_from_days`.
fn civil_year(days: i64) -> i32 {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    year as i32
}

/// A group before it is told apart by its value, in its natural order.
enum Band {
    Modified(ModifiedBucket),
    Year(i32),
}

fn size_band(size: Option<u64>) -> SizeBand {
    match size {
        None => SizeBand::Unspecified,
        Some(0) => SizeBand::Empty,
        Some(1..10_000) => SizeBand::Tiny,
        Some(10_000..100_000) => SizeBand::Small,
        Some(100_000..1_000_000) => SizeBand::Medium,
        Some(1_000_000..16_000_000) => SizeBand::Large,
        Some(16_000_000..128_000_000) => SizeBand::Huge,
        Some(_) => SizeBand::Gigantic,
    }
}

/// The upper-case first letter of a name, or `#` where it is not a letter.
fn initial(label: &std::ffi::OsStr) -> char {
    let text = label.to_string_lossy();
    match text.chars().next() {
        Some(c) if c.is_alphabetic() => c.to_uppercase().next().unwrap_or(c),
        _ => '#',
    }
}

/// How the order of the groups relates to the sort. A group column that is also the sort column
/// follows the sort's direction, and every other group column keeps its own natural order
/// (Modified reads newest first, then, so a sort by name over it is not turned upside down).
fn reversed(sort: SortSpec) -> bool {
    match (sort.group_by, sort.key) {
        (GroupBy::Modified, SortKey::Modified) => !sort.descending,
        (GroupBy::Name, SortKey::Name)
        | (GroupBy::Size, SortKey::Size)
        | (GroupBy::Kind | GroupBy::Type, SortKey::Kind) => sort.descending,
        _ => false,
    }
}

/// The natural place of an entry's group, as a number (the extension of a type group is compared
/// separately).
fn place(by: GroupBy, clock: &GroupClock, s: &Sortable) -> u64 {
    match by {
        GroupBy::None => 0,
        GroupBy::Kind => u64::from(kind_group(s)),
        GroupBy::Modified => match clock.bucket(s.modified_ms) {
            Band::Modified(ModifiedBucket::Unknown) => u64::MAX,
            Band::Modified(bucket) => bucket as u64,
            // Newer years first.
            Band::Year(year) => 100 + (1_000_000 - i64::from(year)).max(0) as u64,
        },
        GroupBy::Size => size_band(s.size) as u64,
        GroupBy::Name => match initial(s.label) {
            '#' => 0,
            letter => 1 + u64::from(u32::from(letter)),
        },
        GroupBy::Type => {
            if s.is_directory() {
                0
            } else if extension(s.label.as_encoded_bytes()).is_empty() {
                1
            } else {
                2
            }
        }
    }
}

/// Orders the groups of two entries. `Equal` means the same group.
pub(crate) fn compare_groups(
    sort: SortSpec,
    clock: &GroupClock,
    a: &Sortable,
    b: &Sortable,
) -> Ordering {
    let by = sort.group_by;
    if by == GroupBy::None {
        return Ordering::Equal;
    }
    // The folders stay at the top when they are asked to, whichever way the groups run.
    if sort.directories_first && matches!(by, GroupBy::Kind | GroupBy::Type) {
        match (a.is_directory(), b.is_directory()) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        }
    }
    let (pa, pb) = (place(by, clock, a), place(by, clock, b));
    let mut order = pa.cmp(&pb);
    if order == Ordering::Equal && by == GroupBy::Type && pa == 2 {
        order = cmp_extension(
            extension(a.label.as_encoded_bytes()),
            extension(b.label.as_encoded_bytes()),
        );
    }
    if reversed(sort) {
        order.reverse()
    } else {
        order
    }
}

/// The key of an entry's group.
pub(crate) fn group_key(by: GroupBy, clock: &GroupClock, s: &Sortable) -> GroupKey {
    match by {
        GroupBy::None | GroupBy::Kind => GroupKey::Kind {
            group: icon_group(kind_group(s)),
        },
        GroupBy::Modified => match clock.bucket(s.modified_ms) {
            Band::Modified(bucket) => GroupKey::Modified { bucket },
            Band::Year(year) => GroupKey::Year { year },
        },
        GroupBy::Size => GroupKey::Size {
            band: size_band(s.size),
        },
        GroupBy::Name => GroupKey::Name {
            initial: initial(s.label).to_string(),
        },
        GroupBy::Type => {
            if s.is_directory() {
                return GroupKey::Kind {
                    group: IconGroup::Folder,
                };
            }
            let raw = extension(s.label.as_encoded_bytes());
            GroupKey::Type {
                extension: String::from_utf8_lossy(raw).to_lowercase(),
            }
        }
    }
}

/// The icon group of an entry for grouping: a folder (or a link to one) is always in the Folders group.
fn kind_group(s: &Sortable) -> u8 {
    if s.is_directory() {
        IconGroup::Folder as u8
    } else {
        s.group
    }
}

fn icon_group(n: u8) -> IconGroup {
    match n {
        0 => IconGroup::Folder,
        1 => IconGroup::Image,
        2 => IconGroup::Audio,
        3 => IconGroup::Video,
        4 => IconGroup::Archive,
        5 => IconGroup::Code,
        6 => IconGroup::Document,
        _ => IconGroup::Other,
    }
}

/// The runs of an ordered view: one for each stretch of rows `compare_groups` calls the same
/// group. `rows` yields the entries in view order.
pub(crate) fn runs_of<'a>(
    sort: SortSpec,
    clock: &GroupClock,
    rows: impl Iterator<Item = Sortable<'a>>,
) -> Vec<GroupRun> {
    if sort.group_by == GroupBy::None {
        return Vec::new();
    }
    let mut out: Vec<GroupRun> = Vec::new();
    let mut first: Option<Sortable<'a>> = None;
    for (at, row) in rows.enumerate() {
        let same = first
            .as_ref()
            .is_some_and(|f| compare_groups(sort, clock, f, &row) == Ordering::Equal);
        if same {
            if let Some(run) = out.last_mut() {
                run.count += 1;
            }
        } else {
            out.push(GroupRun {
                key: group_key(sort.group_by, clock, &row),
                start: at as u32,
                count: 1,
            });
            first = Some(row);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EntryKind;

    // Wednesday 2026-10-07 at noon UTC, in UTC.
    const NOW: i64 = 1_791_374_400_000;

    fn clock() -> GroupClock {
        GroupClock::new(NOW, 0)
    }

    fn days_ago(days: i64) -> Option<i64> {
        Some(NOW - days * DAY_MS)
    }

    fn bucket(ms: Option<i64>) -> GroupKey {
        let clock = clock();
        let sortable = Sortable {
            name: std::ffi::OsStr::new("x"),
            label: std::ffi::OsStr::new("x"),
            key: b"x",
            kind: EntryKind::File,
            link_target: None,
            group: 7,
            size: Some(1),
            modified_ms: ms,
            deleted_ms: None,
        };
        group_key(GroupBy::Modified, &clock, &sortable)
    }

    fn modified(bucket: ModifiedBucket) -> GroupKey {
        GroupKey::Modified { bucket }
    }

    #[test]
    fn civil_years_follow_the_calendar() {
        assert_eq!(civil_year(0), 1970);
        assert_eq!(civil_year(-1), 1969);
        assert_eq!(civil_year(19_723), 2024); // 2024-01-01
        assert_eq!(civil_year(19_722), 2023);
    }

    #[test]
    fn the_clock_reads_the_local_day() {
        // 00:30 UTC on the 8th is still the evening of the 7th at UTC-4.
        let west = GroupClock::new(NOW + 12 * 3_600_000 + 1_800_000, -4 * 3600);
        assert_eq!(west.today, clock().today);
        let utc = GroupClock::new(NOW + 12 * 3_600_000 + 1_800_000, 0);
        assert_eq!(utc.today, clock().today + 1);
    }

    #[test]
    fn icon_groups_survive_the_trip_through_their_number() {
        for group in [
            IconGroup::Folder,
            IconGroup::Image,
            IconGroup::Audio,
            IconGroup::Video,
            IconGroup::Archive,
            IconGroup::Code,
            IconGroup::Document,
            IconGroup::Other,
        ] {
            assert_eq!(icon_group(group as u8), group);
        }
    }

    #[test]
    fn modified_bands_run_from_today_back_through_the_years() {
        // NOW is a Wednesday, so Monday and Tuesday are "earlier this week" once yesterday is set aside.
        assert_eq!(bucket(days_ago(0)), modified(ModifiedBucket::Today));
        assert_eq!(
            bucket(Some(NOW + 3 * DAY_MS)),
            modified(ModifiedBucket::Today)
        );
        assert_eq!(bucket(days_ago(1)), modified(ModifiedBucket::Yesterday));
        assert_eq!(
            bucket(days_ago(2)),
            modified(ModifiedBucket::EarlierThisWeek)
        );
        assert_eq!(bucket(days_ago(3)), modified(ModifiedBucket::Last7Days));
        assert_eq!(bucket(days_ago(7)), modified(ModifiedBucket::Last7Days));
        assert_eq!(bucket(days_ago(8)), modified(ModifiedBucket::Last30Days));
        assert_eq!(bucket(days_ago(30)), modified(ModifiedBucket::Last30Days));
        assert_eq!(bucket(days_ago(40)), modified(ModifiedBucket::ThisYear));
        assert_eq!(bucket(days_ago(300)), GroupKey::Year { year: 2025 });
        assert_eq!(bucket(days_ago(2000)), GroupKey::Year { year: 2021 });
        assert_eq!(bucket(None), modified(ModifiedBucket::Unknown));
    }

    #[test]
    fn size_bands_use_decimal_thresholds() {
        assert_eq!(size_band(None), SizeBand::Unspecified);
        assert_eq!(size_band(Some(0)), SizeBand::Empty);
        assert_eq!(size_band(Some(9_999)), SizeBand::Tiny);
        assert_eq!(size_band(Some(10_000)), SizeBand::Small);
        assert_eq!(size_band(Some(999_999)), SizeBand::Medium);
        assert_eq!(size_band(Some(1_000_000)), SizeBand::Large);
        assert_eq!(size_band(Some(16_000_000)), SizeBand::Huge);
        assert_eq!(size_band(Some(128_000_000)), SizeBand::Gigantic);
    }

    #[test]
    fn initials_are_upper_case_letters_or_a_hash() {
        use std::ffi::OsStr;
        assert_eq!(initial(OsStr::new("apple")), 'A');
        assert_eq!(initial(OsStr::new("élan")), 'É');
        assert_eq!(initial(OsStr::new("9lives")), '#');
        assert_eq!(initial(OsStr::new(".hidden")), '#');
        assert_eq!(initial(OsStr::new("")), '#');
    }
}
