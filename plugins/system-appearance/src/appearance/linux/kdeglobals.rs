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

/// KDE's font settings, where `forceFontDPI` lives; it is not in `kdeglobals`.
pub const FONTS_FILE_NAME: &str = "kcmfonts";

/// Reads what the text of `kdeglobals` and, when the file exists, of `kcmfonts` say.
pub fn interpret(text: &str, fonts: Option<&str>) -> SourceReading {
    let globals = parse::kde_globals(text);
    let mut reading = SourceReading::new(AppearanceSource::KdeGlobals);
    reading.values.colour_scheme = globals.colour_scheme;
    reading.values.accent = globals.accent;
    reading.values.reduced_motion = globals.reduced_motion;
    reading.values.text_scale = fonts.and_then(parse::kde_text_scale);
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
            AppearanceFeature::ReducedMotion,
            reading.values.reduced_motion.is_some(),
            "kdeglobals has no AnimationDurationFactor",
        ),
        (
            AppearanceFeature::TextScale,
            reading.values.text_scale.is_some(),
            "kcmfonts does not set forceFontDPI",
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

/// The text of the user's `kcmfonts`, or `None` when it cannot be read.
fn read_fonts() -> Option<String> {
    std::fs::read_to_string(kwin::config_file(FONTS_FILE_NAME)?).ok()
}

/// Reads the user's `kdeglobals` and `kcmfonts`. A missing `kdeglobals` means KDE's defaults are in
/// effect, which this plugin cannot know, so it reports a miss for everything; a missing
/// `kcmfonts` only leaves the text scale unanswered.
pub fn read() -> SourceReading {
    let Some(path) = kwin::config_file(FILE_NAME) else {
        return SourceReading::failed(
            AppearanceSource::KdeGlobals,
            UnavailableReason::ReadFailed,
            "cannot locate the user config directory",
        );
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => interpret(&text, read_fonts().as_deref()),
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
            Some("[General]\nforceFontDPI=0\n"),
        );
        assert_eq!(reading.values.colour_scheme, Some(ColourScheme::Dark));
        assert_eq!(reading.values.accent.as_deref(), Some("#3daee9"));
        assert_eq!(reading.values.icon_theme.as_deref(), Some("breeze-dark"));
        // Without an AnimationDurationFactor the file does not say, which is a miss, not "off".
        assert_eq!(reading.values.reduced_motion, None);
        assert_eq!(reading.values.text_scale, Some(1.0));
        let missed: Vec<_> = reading.misses.iter().map(|m| m.feature).collect();
        assert_eq!(
            missed,
            vec![
                AppearanceFeature::ReducedMotion,
                AppearanceFeature::Contrast,
                AppearanceFeature::ReducedTransparency
            ]
        );
    }

    #[test]
    fn the_animation_duration_factor_decides_reduced_motion() {
        let reduced = interpret("[KDE]\nAnimationDurationFactor=0\n", None);
        assert_eq!(reduced.values.reduced_motion, Some(true));
        let normal = interpret("[KDE]\nAnimationDurationFactor=1\n", None);
        assert_eq!(normal.values.reduced_motion, Some(false));
        assert!(!normal
            .misses
            .iter()
            .any(|m| m.feature == AppearanceFeature::ReducedMotion));
    }

    #[test]
    fn the_text_scale_comes_from_kcmfonts_only() {
        let forced = interpret("", Some("[General]\nforceFontDPI=144\n"));
        assert_eq!(forced.values.text_scale, Some(1.5));
        // kdeglobals alone cannot see the setting, even with the key in the wrong file.
        let wrong_file = interpret("[General]\nforceFontDPI=144\n", None);
        assert_eq!(wrong_file.values.text_scale, None);
        assert!(wrong_file
            .misses
            .iter()
            .any(|m| m.feature == AppearanceFeature::TextScale
                && m.reason == UnavailableReason::SourceMissing));
    }

    #[test]
    fn an_empty_file_misses_what_it_does_not_say() {
        let reading = interpret("", None);
        assert_eq!(reading.values.colour_scheme, None);
        assert_eq!(reading.values.accent, None);
        assert!(reading
            .misses
            .iter()
            .any(|m| m.feature == AppearanceFeature::ColourScheme
                && m.reason == UnavailableReason::SourceMissing));
    }
}
