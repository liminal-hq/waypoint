// Pure classifiers that turn platform settings into a 12/24-hour answer, and the change tracker
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Each platform uses only some of these, but all of them are compiled and tested everywhere.
#![allow(dead_code)]

use crate::models::TimeFormat;

/// The desktop environments whose clock setting is read from a known place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    /// GNOME and the desktops that share its `org.gnome.desktop.interface` settings.
    Gnome,
    Cinnamon,
    /// KDE, Xfce, MATE, LXQt and anything unrecognised: the locale's convention is used.
    Other,
}

/// Picks the desktop from `XDG_CURRENT_DESKTOP`, a colon-separated list where the first
/// recognised entry wins.
pub fn desktop(xdg_current_desktop: &str) -> Desktop {
    for token in xdg_current_desktop.split(':') {
        let token = token.trim().to_ascii_uppercase();
        let found = match token.as_str() {
            "X-CINNAMON" | "CINNAMON" => Desktop::Cinnamon,
            "BUDGIE" | "PANTHEON" | "UNITY" | "UNITY7" => Desktop::Gnome,
            "KDE" | "MATE" | "XFCE" => Desktop::Other,
            t if t.starts_with("GNOME") => Desktop::Gnome,
            _ => continue,
        };
        return found;
    }
    Desktop::Other
}

/// Strips the GVariant text quoting that `gsettings get` prints around a string value.
pub fn unquote_gvariant(value: &str) -> &str {
    let value = value.trim();
    let value = value.strip_prefix("@s ").unwrap_or(value);
    for quote in ['\'', '"'] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

/// Maps GNOME's `clock-format` value (`24h` or `12h`, possibly quoted) to `is24Hour`.
pub fn gnome_clock_format(value: &str) -> Option<bool> {
    match unquote_gvariant(value).to_ascii_lowercase().as_str() {
        "24h" => Some(true),
        "12h" => Some(false),
        _ => None,
    }
}

/// Maps a `gsettings get` boolean such as Cinnamon's `clock-use-24h` to `is24Hour`.
pub fn gvariant_bool(value: &str) -> Option<bool> {
    match value.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Classifies a libc `strftime` time format such as `nl_langinfo(T_FMT)` returns.
///
/// `%I`, `%l`, `%r`, `%p` and `%P` mean 12-hour; `%H`, `%k`, `%T` and `%R` mean 24-hour. A
/// format with both (a 24-hour clock never prints a meridiem) counts as 12-hour, and one with
/// neither is unclassifiable.
pub fn libc_time_format(format: &str) -> Option<bool> {
    let mut chars = format.chars().peekable();
    let (mut twelve, mut twenty_four) = (false, false);
    while let Some(c) = chars.next() {
        if c != '%' {
            continue;
        }
        // Skip glibc's flags and the E/O alternative-representation modifiers.
        while matches!(chars.peek(), Some('-' | '_' | '0' | '^' | '#' | 'E' | 'O')) {
            chars.next();
        }
        match chars.next() {
            Some('I' | 'l' | 'r' | 'p' | 'P') => twelve = true,
            Some('H' | 'k' | 'T' | 'R') => twenty_four = true,
            // `%%` and every other conversion carry no hour information.
            _ => {}
        }
    }
    match (twelve, twenty_four) {
        (true, _) => Some(false),
        (false, true) => Some(true),
        (false, false) => None,
    }
}

/// Removes the quoted literal text from a Windows or ICU date-time pattern, where a pair of
/// single quotes delimits literals and `''` is an escaped quote.
fn without_quoted_literals(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut quoted = false;
    for c in pattern.chars() {
        if c == '\'' {
            quoted = !quoted;
        } else if !quoted {
            out.push(c);
        }
    }
    out
}

/// Classifies a Windows short-time pattern (`LOCALE_SSHORTTIME`): `H`/`HH` is 24-hour and
/// `h`/`hh` is 12-hour. A pattern with neither, or both, is unclassifiable.
pub fn windows_time_pattern(pattern: &str) -> Option<bool> {
    let pattern = without_quoted_literals(pattern);
    match (pattern.contains('h'), pattern.contains('H')) {
        (true, false) => Some(false),
        (false, true) => Some(true),
        _ => None,
    }
}

/// Decides from what Windows reports. The short-time pattern (`LOCALE_SSHORTTIME`) is what the
/// user's Region settings display, so it wins; the long-time pattern (`LOCALE_STIMEFORMAT`) is
/// consulted when the short one has no hour field, and `LOCALE_ITIME` (0 is 12-hour, 1 is
/// 24-hour) is the last resort. A user can customise the patterns without touching `ITIME`, so
/// when they disagree the pattern is the truth.
pub fn windows_is_24_hour(
    short_pattern: Option<&str>,
    long_pattern: Option<&str>,
    itime: Option<u32>,
) -> Option<bool> {
    short_pattern
        .and_then(windows_time_pattern)
        .or_else(|| long_pattern.and_then(windows_time_pattern))
        .or(match itime {
            Some(0) => Some(false),
            Some(1) => Some(true),
            _ => None,
        })
}

/// Classifies the pattern macOS produces for the template `j` (the locale's preferred hour
/// format): a day-period field (`a`) or `h`/`K` hours are 12-hour; `H`/`k` hours are 24-hour.
pub fn macos_hour_template(pattern: &str) -> Option<bool> {
    let pattern = without_quoted_literals(pattern);
    if pattern.contains('a') || pattern.contains('h') || pattern.contains('K') {
        Some(false)
    } else if pattern.contains('H') || pattern.contains('k') {
        Some(true)
    } else {
        None
    }
}

/// What an observation did to the tracked time format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    /// The first reading, which only sets the baseline.
    Baseline,
    /// The 12/24-hour answer is the same as before.
    Unchanged,
    /// The answer flipped since the previous reading.
    Changed,
}

/// Remembers the last reading so the change event fires exactly when the answer flips. A change
/// of source alone (for example the portal coming back after a locale fallback) is not a change,
/// because consumers only care about the hour cycle.
#[derive(Debug, Default)]
pub struct Tracker {
    last: Option<TimeFormat>,
}

impl Tracker {
    pub fn observe(&mut self, next: TimeFormat) -> Observation {
        let observation = match self.last {
            None => Observation::Baseline,
            Some(last) if last.is_24_hour == next.is_24_hour => Observation::Unchanged,
            Some(_) => Observation::Changed,
        };
        self.last = Some(next);
        observation
    }

    pub fn last(&self) -> Option<TimeFormat> {
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TimeFormatSource;

    #[test]
    fn desktop_table() {
        let cases = [
            ("GNOME", Desktop::Gnome),
            ("ubuntu:GNOME", Desktop::Gnome),
            ("GNOME-Flashback:GNOME", Desktop::Gnome),
            ("Budgie:GNOME", Desktop::Gnome),
            ("Pantheon", Desktop::Gnome),
            ("Unity:Unity7:ubuntu", Desktop::Gnome),
            ("X-Cinnamon", Desktop::Cinnamon),
            ("KDE", Desktop::Other),
            ("MATE", Desktop::Other),
            ("ubuntu:XFCE", Desktop::Other),
            ("LXQt", Desktop::Other),
            ("", Desktop::Other),
        ];
        for (input, expected) in cases {
            assert_eq!(desktop(input), expected, "{input:?}");
        }
    }

    #[test]
    fn gnome_clock_format_values() {
        assert_eq!(gnome_clock_format("24h"), Some(true));
        assert_eq!(gnome_clock_format("12h"), Some(false));
        assert_eq!(gnome_clock_format("'24h'"), Some(true));
        assert_eq!(gnome_clock_format("'12h'\n"), Some(false));
        assert_eq!(gnome_clock_format("@s '24h'"), Some(true));
        assert_eq!(gnome_clock_format("\"12H\""), Some(false));
        assert_eq!(gnome_clock_format(""), None);
        assert_eq!(gnome_clock_format("'metric'"), None);
    }

    #[test]
    fn gvariant_booleans() {
        assert_eq!(gvariant_bool("true\n"), Some(true));
        assert_eq!(gvariant_bool("false"), Some(false));
        assert_eq!(gvariant_bool("'true'"), None);
        assert_eq!(gvariant_bool(""), None);
    }

    #[test]
    fn libc_formats() {
        let cases = [
            ("%r", Some(false)),
            ("%I:%M:%S %p", Some(false)),
            ("%l:%M %P", Some(false)),
            ("%H:%M", Some(true)),
            ("%T", Some(true)),
            ("%R", Some(true)),
            ("%k.%M", Some(true)),
            ("%-H:%M", Some(true)),
            ("%EH:%OM", Some(true)),
            ("%H:%M %p", Some(false)),
            ("100%% %H", Some(true)),
            ("%%H", None),
            ("%M:%S", None),
            ("", None),
            ("%", None),
        ];
        for (input, expected) in cases {
            assert_eq!(libc_time_format(input), expected, "{input:?}");
        }
    }

    #[test]
    fn windows_patterns() {
        let cases = [
            ("HH:mm", Some(true)),
            ("H:mm", Some(true)),
            ("HH.mm", Some(true)),
            ("H:mm:ss", Some(true)),
            ("h:mm tt", Some(false)),
            ("hh:mm:ss tt", Some(false)),
            ("tt h:mm", Some(false)),
            ("HH'h'mm", Some(true)),
            ("H 'h' mm 'min'", Some(true)),
            ("'hh' HH:mm", Some(true)),
            ("HH:mm 'h'", Some(true)),
            ("mm:ss", None),
            ("tt", None),
            ("", None),
            ("h H", None),
        ];
        for (input, expected) in cases {
            assert_eq!(windows_time_pattern(input), expected, "{input:?}");
        }
    }

    #[test]
    fn windows_prefers_the_pattern_over_itime() {
        assert_eq!(windows_is_24_hour(Some("HH:mm"), None, Some(0)), Some(true));
        assert_eq!(
            windows_is_24_hour(Some("h:mm tt"), None, Some(1)),
            Some(false)
        );
        assert_eq!(windows_is_24_hour(Some("HH:mm"), None, None), Some(true));
        assert_eq!(windows_is_24_hour(None, None, Some(1)), Some(true));
        assert_eq!(windows_is_24_hour(None, None, Some(0)), Some(false));
        assert_eq!(windows_is_24_hour(Some(""), Some(""), Some(7)), None);
        assert_eq!(windows_is_24_hour(None, None, None), None);
    }

    #[test]
    fn windows_falls_back_from_the_short_pattern_to_the_long_one() {
        // A short pattern without an hour field defers to the long pattern, then to ITIME.
        assert_eq!(
            windows_is_24_hour(Some("mm:ss"), Some("h:mm:ss tt"), Some(1)),
            Some(false)
        );
        assert_eq!(
            windows_is_24_hour(Some("mm:ss"), Some("mm:ss"), Some(1)),
            Some(true)
        );
        // The short pattern outranks the long one when both have an hour field.
        assert_eq!(
            windows_is_24_hour(Some("HH:mm"), Some("h:mm:ss tt"), Some(0)),
            Some(true)
        );
    }

    #[test]
    fn macos_templates() {
        let cases = [
            ("h a", Some(false)),
            ("h\u{202f}a", Some(false)),
            ("a h", Some(false)),
            ("HH", Some(true)),
            ("H", Some(true)),
            ("k", Some(true)),
            ("K a", Some(false)),
            ("'h' HH", Some(true)),
            ("", None),
        ];
        for (input, expected) in cases {
            assert_eq!(macos_hour_template(input), expected, "{input:?}");
        }
    }

    fn format(is_24_hour: bool, source: TimeFormatSource) -> TimeFormat {
        TimeFormat { is_24_hour, source }
    }

    #[test]
    fn the_first_observation_is_a_baseline() {
        let mut tracker = Tracker::default();
        assert_eq!(
            tracker.observe(format(true, TimeFormatSource::GnomePortal)),
            Observation::Baseline
        );
        assert!(tracker.last().unwrap().is_24_hour);
    }

    #[test]
    fn only_a_flipped_answer_is_a_change() {
        let mut tracker = Tracker::default();
        tracker.observe(format(true, TimeFormatSource::GnomePortal));
        assert_eq!(
            tracker.observe(format(true, TimeFormatSource::GnomePortal)),
            Observation::Unchanged
        );
        assert_eq!(
            tracker.observe(format(false, TimeFormatSource::GnomePortal)),
            Observation::Changed
        );
        assert_eq!(
            tracker.observe(format(false, TimeFormatSource::GnomePortal)),
            Observation::Unchanged
        );
        assert_eq!(
            tracker.observe(format(true, TimeFormatSource::GnomePortal)),
            Observation::Changed
        );
    }

    #[test]
    fn a_new_source_with_the_same_answer_is_not_a_change_but_is_remembered() {
        let mut tracker = Tracker::default();
        tracker.observe(format(true, TimeFormatSource::Locale));
        assert_eq!(
            tracker.observe(format(true, TimeFormatSource::GnomePortal)),
            Observation::Unchanged
        );
        assert_eq!(
            tracker.last().unwrap().source,
            TimeFormatSource::GnomePortal
        );
    }

    #[test]
    fn the_wire_format_keeps_the_mobile_field_name() {
        let json = serde_json::to_value(format(true, TimeFormatSource::GnomePortal)).unwrap();
        assert_eq!(json["is24Hour"], true);
        assert_eq!(json["source"], "gnome-portal");
        let json =
            serde_json::to_value(format(false, TimeFormatSource::WindowsUserLocale)).unwrap();
        assert_eq!(json["source"], "windows-user-locale");
    }
}
