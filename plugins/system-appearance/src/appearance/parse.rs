// Parses desktop appearance values (portal variants, gsettings text, kdeglobals, Windows values) without doing any I/O
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::models::{ColourScheme, Contrast, UnavailableReason};
use crate::parse::{ini_value, unquote_gvariant_string};

/// The smallest and largest text scale believed; anything else is a misread value.
const TEXT_SCALE_RANGE: std::ops::RangeInclusive<f64> = 0.25..=8.0;

/// Maps the portal's `color-scheme` value: 1 prefers dark, 2 prefers light, anything else has no preference.
pub fn portal_colour_scheme(value: u32) -> ColourScheme {
    match value {
        1 => ColourScheme::Dark,
        2 => ColourScheme::Light,
        _ => ColourScheme::NoPreference,
    }
}

/// Maps the portal's `contrast` value: 1 asks for more contrast.
pub fn portal_contrast(value: u32) -> Contrast {
    if value == 1 {
        Contrast::More
    } else {
        Contrast::Normal
    }
}

/// Maps the portal's `reduced-motion` value: 1 asks for reduced motion.
pub fn portal_reduced_motion(value: u32) -> bool {
    value == 1
}

/// Formats an accent from the portal's red, green and blue, each in `0.0..=1.0`.
///
/// The portal reports a value outside that range when the user has no accent colour, which is
/// `None` here.
pub fn portal_accent(red: f64, green: f64, blue: f64) -> Option<String> {
    let channel = |value: f64| {
        (value.is_finite() && (0.0..=1.0).contains(&value)).then(|| (value * 255.0).round() as u8)
    };
    Some(hex(channel(red)?, channel(green)?, channel(blue)?))
}

/// Formats three 8-bit channels as lower-case `#rrggbb`.
pub fn hex(red: u8, green: u8, blue: u8) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}

/// Maps a GNOME accent colour name (`org.gnome.desktop.interface` `accent-color`) to the colour
/// libadwaita draws it in.
pub fn gnome_accent_name(name: &str) -> Option<String> {
    let colour = match name.trim() {
        "blue" => "#3584e4",
        "teal" => "#2190a4",
        "green" => "#3a944a",
        "yellow" => "#c88800",
        "orange" => "#ed5b00",
        "red" => "#e62d42",
        "pink" => "#d56199",
        "purple" => "#9141ac",
        "slate" => "#6f8396",
        _ => return None,
    };
    Some(colour.to_string())
}

/// Maps a `color-scheme` string: `prefer-dark`, `prefer-light` or `default` (no preference).
pub fn gsettings_colour_scheme(value: &str) -> Option<ColourScheme> {
    match unquote_gvariant_string(value) {
        "prefer-dark" => Some(ColourScheme::Dark),
        "prefer-light" => Some(ColourScheme::Light),
        "default" | "no-preference" => Some(ColourScheme::NoPreference),
        _ => None,
    }
}

/// Parses a boolean as `gsettings get` prints it.
pub fn gsettings_bool(value: &str) -> Option<bool> {
    match value.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Parses a double as `gsettings get` prints it (`1.25`, or `double 1.25` for a typed value).
pub fn gsettings_f64(value: &str) -> Option<f64> {
    let value = value.trim();
    let value = value.strip_prefix("double ").unwrap_or(value);
    text_scale(value.parse().ok()?)
}

/// Accepts a plausible text scale; `None` for NaN, zero, negative or absurd values.
pub fn text_scale(value: f64) -> Option<f64> {
    (value.is_finite() && TEXT_SCALE_RANGE.contains(&value)).then_some(value)
}

/// Maps GNOME's `org.gnome.desktop.a11y.interface` `reduced-motion` string.
pub fn named_reduced_motion(value: &str) -> Option<bool> {
    match unquote_gvariant_string(value) {
        "reduce" | "reduced" => Some(true),
        "no-preference" => Some(false),
        _ => None,
    }
}

/// The text of a `gsettings get` string value, or `None` when it is empty.
pub fn gsettings_name(value: &str) -> Option<String> {
    let name = unquote_gvariant_string(value);
    (!name.is_empty()).then(|| name.to_string())
}

/// What KDE's `kdeglobals` says; each field is `None` when the file does not say.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KdeGlobals {
    pub colour_scheme: Option<ColourScheme>,
    pub accent: Option<String>,
    pub reduced_motion: Option<bool>,
    pub text_scale: Option<f64>,
    pub icon_theme: Option<String>,
}

/// Parses an `r,g,b` triplet of 8-bit channels, as KDE writes colours.
pub fn kde_rgb(value: &str) -> Option<(u8, u8, u8)> {
    let mut channels = value.split(',').map(|part| part.trim().parse::<u8>());
    let red = channels.next()?.ok()?;
    let green = channels.next()?.ok()?;
    let blue = channels.next()?.ok()?;
    // KDE writes an alpha channel after the three in some files.
    channels
        .next()
        .is_none_or(|alpha| alpha.is_ok())
        .then_some((red, green, blue))
}

/// Whether an sRGB colour is dark, by its relative luminance (WCAG).
pub fn is_dark(red: u8, green: u8, blue: u8) -> bool {
    let linear = |channel: u8| {
        let value = f64::from(channel) / 255.0;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue) < 0.18
}

/// Reads the appearance out of `kdeglobals` text.
///
/// The colour scheme comes from the window background colour, so a custom scheme counts as dark or
/// light by what it paints; the scheme's name is only a fallback. Animations count as reduced when
/// the duration factor is zero. A text scale is the forced font DPI over 96, and an absent or zero
/// DPI means the default size.
pub fn kde_globals(text: &str) -> KdeGlobals {
    let colour_scheme = ini_value(text, "Colors:Window", "BackgroundNormal")
        .and_then(|value| kde_rgb(&value))
        .map(|(red, green, blue)| {
            if is_dark(red, green, blue) {
                ColourScheme::Dark
            } else {
                ColourScheme::Light
            }
        })
        .or_else(|| {
            ini_value(text, "General", "ColorScheme").map(|name| {
                if name.to_ascii_lowercase().contains("dark") {
                    ColourScheme::Dark
                } else {
                    ColourScheme::Light
                }
            })
        });
    let accent = ini_value(text, "General", "AccentColor")
        .and_then(|value| kde_rgb(&value))
        .map(|(red, green, blue)| hex(red, green, blue));
    let reduced_motion = ini_value(text, "KDE", "AnimationDurationFactor")
        .and_then(|value| value.parse::<f64>().ok())
        .map(|factor| factor <= 0.0);
    let dpi = ini_value(text, "General", "forceFontDPI")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    let text_scale = if dpi > 0.0 {
        text_scale(dpi / 96.0)
    } else {
        Some(1.0)
    };
    let icon_theme = ini_value(text, "Icons", "Theme").filter(|name| !name.is_empty());
    KdeGlobals {
        colour_scheme,
        accent,
        reduced_motion,
        text_scale,
        icon_theme,
    }
}

/// Maps the Windows `AppsUseLightTheme` registry value: 0 is the dark mode and 1 the light one.
pub fn windows_colour_scheme(apps_use_light_theme: u32) -> ColourScheme {
    if apps_use_light_theme == 0 {
        ColourScheme::Dark
    } else {
        ColourScheme::Light
    }
}

/// Formats a Windows `UISettings` accent colour (its alpha is ignored).
pub fn windows_accent(red: u8, green: u8, blue: u8) -> String {
    hex(red, green, blue)
}

/// Classifies why a `gsettings` call failed, from the error `cli::run` produced.
pub fn classify_gsettings_error(message: &str) -> UnavailableReason {
    let lower = message.to_ascii_lowercase();
    if lower.contains("no such schema") || lower.contains("no such key") {
        UnavailableReason::SourceMissing
    } else if lower.starts_with("gsettings:") && !lower.contains("failed") {
        // The tool itself could not be started.
        UnavailableReason::ToolMissing
    } else {
        UnavailableReason::ReadFailed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_values_map_to_the_models() {
        assert_eq!(portal_colour_scheme(0), ColourScheme::NoPreference);
        assert_eq!(portal_colour_scheme(1), ColourScheme::Dark);
        assert_eq!(portal_colour_scheme(2), ColourScheme::Light);
        assert_eq!(portal_colour_scheme(9), ColourScheme::NoPreference);
        assert_eq!(portal_contrast(0), Contrast::Normal);
        assert_eq!(portal_contrast(1), Contrast::More);
        assert!(!portal_reduced_motion(0));
        assert!(portal_reduced_motion(1));
    }

    #[test]
    fn portal_accent_table() {
        let cases = [
            ((0.207_843_14, 0.517_647_1, 0.894_117_65), Some("#3584e4")),
            ((0.0, 0.0, 0.0), Some("#000000")),
            ((1.0, 1.0, 1.0), Some("#ffffff")),
            // The portal's way of saying "no accent".
            ((-1.0, -1.0, -1.0), None),
            ((1.5, 0.2, 0.2), None),
            ((f64::NAN, 0.2, 0.2), None),
        ];
        for ((red, green, blue), expected) in cases {
            assert_eq!(
                portal_accent(red, green, blue).as_deref(),
                expected,
                "{red} {green} {blue}"
            );
        }
    }

    #[test]
    fn gnome_accent_names() {
        assert_eq!(gnome_accent_name("blue").as_deref(), Some("#3584e4"));
        assert_eq!(gnome_accent_name("slate").as_deref(), Some("#6f8396"));
        assert_eq!(gnome_accent_name("chartreuse"), None);
    }

    #[test]
    fn gsettings_scheme_table() {
        let cases = [
            ("'prefer-dark'", Some(ColourScheme::Dark)),
            ("'prefer-light'", Some(ColourScheme::Light)),
            ("'default'", Some(ColourScheme::NoPreference)),
            ("prefer-dark", Some(ColourScheme::Dark)),
            ("'sepia'", None),
            ("", None),
        ];
        for (text, expected) in cases {
            assert_eq!(gsettings_colour_scheme(text), expected, "{text}");
        }
    }

    #[test]
    fn gsettings_scalars() {
        assert_eq!(gsettings_bool("true\n"), Some(true));
        assert_eq!(gsettings_bool("false"), Some(false));
        assert_eq!(gsettings_bool("'yes'"), None);
        assert_eq!(gsettings_f64("1.25"), Some(1.25));
        assert_eq!(gsettings_f64("double 1.5"), Some(1.5));
        assert_eq!(gsettings_f64("0.0"), None);
        assert_eq!(gsettings_f64("abc"), None);
        assert_eq!(gsettings_f64("100"), None);
        assert_eq!(named_reduced_motion("'reduce'"), Some(true));
        assert_eq!(named_reduced_motion("'no-preference'"), Some(false));
        assert_eq!(named_reduced_motion("'maybe'"), None);
        assert_eq!(gsettings_name("'Adwaita'").as_deref(), Some("Adwaita"));
        assert_eq!(gsettings_name("''"), None);
    }

    #[test]
    fn kde_colours() {
        assert_eq!(kde_rgb("61,174,233"), Some((61, 174, 233)));
        assert_eq!(kde_rgb(" 1, 2, 3 "), Some((1, 2, 3)));
        assert_eq!(kde_rgb("1,2,3,255"), Some((1, 2, 3)));
        assert_eq!(kde_rgb("1,2"), None);
        assert_eq!(kde_rgb("1,2,300"), None);
        assert!(is_dark(35, 38, 41));
        assert!(!is_dark(239, 240, 241));
    }

    #[test]
    fn kdeglobals_dark_with_everything() {
        let text = "\
[General]
AccentColor=61,174,233
ColorScheme=BreezeDark
forceFontDPI=120

[Colors:Window]
BackgroundNormal=35,38,41

[Icons]
Theme=breeze-dark

[KDE]
AnimationDurationFactor=0
";
        assert_eq!(
            kde_globals(text),
            KdeGlobals {
                colour_scheme: Some(ColourScheme::Dark),
                accent: Some("#3daee9".to_string()),
                reduced_motion: Some(true),
                text_scale: Some(1.25),
                icon_theme: Some("breeze-dark".to_string()),
            }
        );
    }

    #[test]
    fn kdeglobals_light_by_background_and_defaults() {
        let text =
            "[Colors:Window]\nBackgroundNormal=239,240,241\n[KDE]\nAnimationDurationFactor=1\n";
        let parsed = kde_globals(text);
        assert_eq!(parsed.colour_scheme, Some(ColourScheme::Light));
        assert_eq!(parsed.reduced_motion, Some(false));
        assert_eq!(parsed.text_scale, Some(1.0));
        assert_eq!(parsed.accent, None);
        assert_eq!(parsed.icon_theme, None);
    }

    #[test]
    fn kdeglobals_scheme_name_is_the_fallback() {
        let dark = kde_globals("[General]\nColorScheme=BreezeDark\n");
        assert_eq!(dark.colour_scheme, Some(ColourScheme::Dark));
        let light = kde_globals("[General]\nColorScheme=BreezeLight\n");
        assert_eq!(light.colour_scheme, Some(ColourScheme::Light));
        assert_eq!(kde_globals("").colour_scheme, None);
    }

    #[test]
    fn windows_values() {
        assert_eq!(windows_colour_scheme(0), ColourScheme::Dark);
        assert_eq!(windows_colour_scheme(1), ColourScheme::Light);
        assert_eq!(windows_accent(0, 120, 212), "#0078d4");
    }

    #[test]
    fn gsettings_errors_are_classified() {
        let cases = [
            (
                "gsettings failed: No such schema “org.gnome.desktop.interface”",
                UnavailableReason::SourceMissing,
            ),
            (
                "gsettings failed: No such key “accent-color”",
                UnavailableReason::SourceMissing,
            ),
            (
                "gsettings: No such file or directory (os error 2)",
                UnavailableReason::ToolMissing,
            ),
            ("gsettings failed: boom", UnavailableReason::ReadFailed),
        ];
        for (message, expected) in cases {
            assert_eq!(classify_gsettings_error(message), expected, "{message}");
        }
    }
}
