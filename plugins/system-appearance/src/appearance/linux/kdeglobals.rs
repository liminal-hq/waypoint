// Reads KDE's kdeglobals file for the appearance preferences the portal does not answer for
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::ErrorKind;

use crate::{
    appearance::{
        models::{AppearanceFeature, AppearanceSource, UnavailableReason},
        parse,
        resolve::SourceReading,
    },
    linux::kwin,
};

/// The file whose changes mean an appearance preference may have changed on KDE.
pub const FILE_NAME: &str = "kdeglobals";

/// Reads what `kdeglobals` text says.
pub fn interpret(text: &str) -> SourceReading {
    let globals = parse::kde_globals(text);
    let mut reading = SourceReading::new(AppearanceSource::KdeGlobals);
    reading.values.colour_scheme = globals.colour_scheme;
    reading.values.accent = globals.accent;
    reading.values.reduced_motion = globals.reduced_motion.or(Some(false));
    reading.values.text_scale = globals.text_scale;
    reading.values.icon_theme = globals.icon_theme;
    for (feature, present, detail) in [
        (
            AppearanceFeature::ColourScheme,
            reading.values.colour_scheme.is_some(),
            "kdeglobals names no colour scheme",
        ),
        (
            AppearanceFeature::Accent,
            reading.values.accent.is_some(),
            "kdeglobals has no AccentColor",
        ),
        (
            AppearanceFeature::TextScale,
            reading.values.text_scale.is_some(),
            "kdeglobals holds a font DPI that is not understood",
        ),
        (
            AppearanceFeature::IconTheme,
            reading.values.icon_theme.is_some(),
            "kdeglobals names no icon theme",
        ),
    ] {
        if !present {
            reading.miss(feature, UnavailableReason::SourceMissing, detail);
        }
    }
    for feature in [
        AppearanceFeature::Contrast,
        AppearanceFeature::ReducedTransparency,
    ] {
        reading.miss(
            feature,
            UnavailableReason::NoSource,
            "KDE has no such setting",
        );
    }
    reading
}

/// Reads the user's `kdeglobals`. A missing file means KDE's defaults are in effect, which this
/// plugin cannot know, so it reports a miss for everything.
pub fn read() -> SourceReading {
    let Some(path) = kwin::config_file(FILE_NAME) else {
        return SourceReading::failed(
            AppearanceSource::KdeGlobals,
            UnavailableReason::ReadFailed,
            "cannot locate the user config directory",
        );
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => interpret(&text),
        Err(error) if error.kind() == ErrorKind::NotFound => SourceReading::failed(
            AppearanceSource::KdeGlobals,
            UnavailableReason::SourceMissing,
            format!("{} does not exist", path.display()),
        ),
        Err(error) => SourceReading::failed(
            AppearanceSource::KdeGlobals,
            UnavailableReason::ReadFailed,
            format!("{}: {error}", path.display()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::models::ColourScheme;

    #[test]
    fn a_typical_file_answers_most_features() {
        let reading = interpret(
            "[General]\nAccentColor=61,174,233\n[Colors:Window]\nBackgroundNormal=35,38,41\n[Icons]\nTheme=breeze-dark\n",
        );
        assert_eq!(reading.values.colour_scheme, Some(ColourScheme::Dark));
        assert_eq!(reading.values.accent.as_deref(), Some("#3daee9"));
        assert_eq!(reading.values.icon_theme.as_deref(), Some("breeze-dark"));
        // Animations at their default are not reduced.
        assert_eq!(reading.values.reduced_motion, Some(false));
        assert_eq!(reading.values.text_scale, Some(1.0));
        let missed: Vec<_> = reading.misses.iter().map(|m| m.feature).collect();
        assert_eq!(
            missed,
            vec![
                AppearanceFeature::Contrast,
                AppearanceFeature::ReducedTransparency
            ]
        );
    }

    #[test]
    fn an_empty_file_misses_what_it_does_not_say() {
        let reading = interpret("");
        assert_eq!(reading.values.colour_scheme, None);
        assert_eq!(reading.values.accent, None);
        assert!(reading
            .misses
            .iter()
            .any(|m| m.feature == AppearanceFeature::ColourScheme
                && m.reason == UnavailableReason::SourceMissing));
    }
}
