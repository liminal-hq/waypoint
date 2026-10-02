// Reads the appearance preferences from GSettings schemas with the gsettings tool, for desktops and keys the portal does not answer for
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    appearance::{
        models::{AppearanceFeature, AppearanceSource, Contrast, UnavailableReason},
        parse,
        resolve::SourceReading,
    },
    linux::cli,
};

/// A GSettings schema and key.
type Key = (&'static str, &'static str);

/// Where one desktop keeps each preference, in the order they are tried.
pub struct Keys {
    pub colour_scheme: &'static [Key],
    pub accent: &'static [Key],
    pub contrast: &'static [Key],
    /// `enable-animations`, whose `false` means reduced motion.
    pub animations: &'static [Key],
    pub text_scale: &'static [Key],
    pub icon_theme: &'static [Key],
    /// What to say when a preference has no key at all on this desktop.
    pub desktop: &'static str,
}

pub const GNOME: Keys = Keys {
    colour_scheme: &[("org.gnome.desktop.interface", "color-scheme")],
    accent: &[("org.gnome.desktop.interface", "accent-color")],
    contrast: &[("org.gnome.desktop.a11y.interface", "high-contrast")],
    animations: &[("org.gnome.desktop.interface", "enable-animations")],
    text_scale: &[("org.gnome.desktop.interface", "text-scaling-factor")],
    icon_theme: &[("org.gnome.desktop.interface", "icon-theme")],
    desktop: "GNOME",
};

pub const CINNAMON: Keys = Keys {
    // Cinnamon follows the XApp portal's setting, and the GNOME key on newer releases.
    colour_scheme: &[
        ("org.x.apps.portal", "color-scheme"),
        ("org.gnome.desktop.interface", "color-scheme"),
    ],
    accent: &[],
    contrast: &[("org.cinnamon.desktop.a11y.interface", "high-contrast")],
    animations: &[("org.cinnamon.desktop.interface", "enable-animations")],
    text_scale: &[("org.cinnamon.desktop.interface", "text-scaling-factor")],
    icon_theme: &[("org.cinnamon.desktop.interface", "icon-theme")],
    desktop: "Cinnamon",
};

/// The schemas whose changes mean an appearance preference may have changed on Cinnamon.
pub const CINNAMON_SCHEMAS: [&str; 3] = [
    "org.cinnamon.desktop.interface",
    "org.cinnamon.desktop.a11y.interface",
    "org.x.apps.portal",
];

/// Reads one preference through `get`, trying each key until one answers. Returns the parsed value,
/// or the reason and detail of the last failure.
fn try_keys<T>(
    keys: &[Key],
    get: &dyn Fn(&str, &str) -> Result<String, String>,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<T, (UnavailableReason, String)> {
    let mut failure = None;
    for (schema, key) in keys {
        match get(schema, key) {
            Ok(text) => match parse(&text) {
                Some(value) => return Ok(value),
                None => {
                    failure = Some((
                        UnavailableReason::ReadFailed,
                        format!("{schema} {key} holds {text:?}, which is not understood"),
                    ));
                }
            },
            Err(message) => {
                failure = Some((parse::classify_gsettings_error(&message), message));
            }
        }
    }
    Err(failure.unwrap_or((UnavailableReason::NoSource, String::new())))
}

/// Reads the `wanted` preferences with `get` (which runs `gsettings get schema key`).
pub fn read_with(
    keys: &Keys,
    wanted: &[AppearanceFeature],
    get: &dyn Fn(&str, &str) -> Result<String, String>,
) -> SourceReading {
    let mut reading = SourceReading::new(AppearanceSource::Gsettings);
    for &feature in wanted {
        let table: &[Key] = match feature {
            AppearanceFeature::ColourScheme => keys.colour_scheme,
            AppearanceFeature::Accent => keys.accent,
            AppearanceFeature::Contrast => keys.contrast,
            AppearanceFeature::ReducedMotion => keys.animations,
            AppearanceFeature::TextScale => keys.text_scale,
            AppearanceFeature::IconTheme => keys.icon_theme,
            AppearanceFeature::ReducedTransparency => &[],
        };
        if table.is_empty() {
            reading.miss(
                feature,
                UnavailableReason::NoSource,
                format!("{} has no setting for it", keys.desktop),
            );
            continue;
        }
        let outcome = match feature {
            AppearanceFeature::ColourScheme => try_keys(table, get, parse::gsettings_colour_scheme)
                .map(|v| {
                    reading.values.colour_scheme = Some(v);
                }),
            AppearanceFeature::Accent => try_keys(table, get, |text| {
                parse::gnome_accent_name(parse::gsettings_name(text)?.as_str())
            })
            .map(|v| reading.values.accent = Some(v)),
            AppearanceFeature::Contrast => {
                try_keys(table, get, parse::gsettings_bool).map(|high| {
                    reading.values.contrast = Some(if high {
                        Contrast::More
                    } else {
                        Contrast::Normal
                    });
                })
            }
            AppearanceFeature::ReducedMotion => {
                try_keys(table, get, parse::gsettings_bool).map(|enabled| {
                    reading.values.reduced_motion = Some(!enabled);
                })
            }
            AppearanceFeature::TextScale => try_keys(table, get, parse::gsettings_f64)
                .map(|v| reading.values.text_scale = Some(v)),
            AppearanceFeature::IconTheme => try_keys(table, get, parse::gsettings_name)
                .map(|v| reading.values.icon_theme = Some(v)),
            AppearanceFeature::ReducedTransparency => Ok(()),
        };
        if let Err((reason, detail)) = outcome {
            reading.miss(feature, reason, detail);
        }
    }
    reading
}

/// Reads the `wanted` preferences by running the `gsettings` tool.
pub fn read(keys: &Keys, wanted: &[AppearanceFeature]) -> SourceReading {
    read_with(keys, wanted, &|schema, key| {
        cli::run("gsettings", &["get", schema, key])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::models::ColourScheme;

    fn fake(
        answers: &'static [(&'static str, &'static str, &'static str)],
    ) -> impl Fn(&str, &str) -> Result<String, String> {
        move |schema, key| {
            answers
                .iter()
                .find(|(s, k, _)| *s == schema && *k == key)
                .map(|(_, _, value)| (*value).to_string())
                .ok_or_else(|| format!("gsettings failed: No such key “{key}”"))
        }
    }

    #[test]
    fn gnome_keys_are_read_and_mapped() {
        let get = fake(&[
            (
                "org.gnome.desktop.interface",
                "color-scheme",
                "'prefer-dark'",
            ),
            ("org.gnome.desktop.interface", "accent-color", "'teal'"),
            ("org.gnome.desktop.a11y.interface", "high-contrast", "true"),
            ("org.gnome.desktop.interface", "enable-animations", "false"),
            ("org.gnome.desktop.interface", "text-scaling-factor", "1.5"),
            ("org.gnome.desktop.interface", "icon-theme", "'Adwaita'"),
        ]);
        let reading = read_with(&GNOME, &AppearanceFeature::ALL, &get);
        let values = &reading.values;
        assert_eq!(values.colour_scheme, Some(ColourScheme::Dark));
        assert_eq!(values.accent.as_deref(), Some("#2190a4"));
        assert_eq!(values.contrast, Some(Contrast::More));
        assert_eq!(values.reduced_motion, Some(true));
        assert_eq!(values.text_scale, Some(1.5));
        assert_eq!(values.icon_theme.as_deref(), Some("Adwaita"));
        assert_eq!(reading.source, AppearanceSource::Gsettings);
        // Only reduced transparency has no key.
        assert_eq!(reading.misses.len(), 1);
        assert_eq!(
            reading.misses[0].feature,
            AppearanceFeature::ReducedTransparency
        );
    }

    #[test]
    fn only_the_wanted_features_are_read() {
        let get = fake(&[("org.gnome.desktop.interface", "icon-theme", "'Adwaita'")]);
        let reading = read_with(&GNOME, &[AppearanceFeature::IconTheme], &get);
        assert_eq!(reading.values.icon_theme.as_deref(), Some("Adwaita"));
        assert_eq!(reading.values.colour_scheme, None);
        assert!(reading.misses.is_empty());
    }

    #[test]
    fn cinnamon_tries_its_keys_in_order_and_has_no_accent() {
        let get = fake(&[(
            "org.gnome.desktop.interface",
            "color-scheme",
            "'prefer-light'",
        )]);
        let reading = read_with(
            &CINNAMON,
            &[AppearanceFeature::ColourScheme, AppearanceFeature::Accent],
            &get,
        );
        assert_eq!(reading.values.colour_scheme, Some(ColourScheme::Light));
        let accent = &reading.misses[0];
        assert_eq!(accent.feature, AppearanceFeature::Accent);
        assert_eq!(accent.reason, UnavailableReason::NoSource);
        assert!(accent.detail.contains("Cinnamon"));
    }

    #[test]
    fn failures_carry_their_classified_reason() {
        let missing_tool =
            |_: &str, _: &str| Err("gsettings: No such file or directory".to_string());
        let reading = read_with(&GNOME, &[AppearanceFeature::TextScale], &missing_tool);
        assert_eq!(reading.misses[0].reason, UnavailableReason::ToolMissing);

        let garbled = |_: &str, _: &str| Ok("'tall'".to_string());
        let reading = read_with(&GNOME, &[AppearanceFeature::TextScale], &garbled);
        assert_eq!(reading.misses[0].reason, UnavailableReason::ReadFailed);
        assert!(reading.misses[0].detail.contains("text-scaling-factor"));
    }
}
