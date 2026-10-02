// Combines the readings of several sources into one set of appearance values, with the source that won and each feature's availability
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::models::{
    AppearanceFeature, AppearanceFeatureStatus, AppearanceSource, AppearanceSources,
    AppearanceValues, ColourScheme, Contrast, UnavailableReason,
};

/// The values one source answered with; a field is `None` where it did not answer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Partial {
    pub colour_scheme: Option<ColourScheme>,
    pub accent: Option<String>,
    pub contrast: Option<Contrast>,
    pub reduced_motion: Option<bool>,
    pub reduced_transparency: Option<bool>,
    pub text_scale: Option<f64>,
    pub icon_theme: Option<String>,
}

impl Partial {
    /// Whether the source answered for `feature`.
    pub fn has(&self, feature: AppearanceFeature) -> bool {
        match feature {
            AppearanceFeature::ColourScheme => self.colour_scheme.is_some(),
            AppearanceFeature::Accent => self.accent.is_some(),
            AppearanceFeature::Contrast => self.contrast.is_some(),
            AppearanceFeature::ReducedMotion => self.reduced_motion.is_some(),
            AppearanceFeature::ReducedTransparency => self.reduced_transparency.is_some(),
            AppearanceFeature::TextScale => self.text_scale.is_some(),
            AppearanceFeature::IconTheme => self.icon_theme.is_some(),
        }
    }

    /// The features this reading did not answer.
    pub fn missing(&self) -> Vec<AppearanceFeature> {
        AppearanceFeature::ALL
            .into_iter()
            .filter(|feature| !self.has(*feature))
            .collect()
    }

    /// Fills the fields this reading lacks from `other`, keeping every value it already has.
    pub fn or(mut self, other: Partial) -> Partial {
        self.colour_scheme = self.colour_scheme.or(other.colour_scheme);
        self.accent = self.accent.or(other.accent);
        self.contrast = self.contrast.or(other.contrast);
        self.reduced_motion = self.reduced_motion.or(other.reduced_motion);
        self.reduced_transparency = self.reduced_transparency.or(other.reduced_transparency);
        self.text_scale = self.text_scale.or(other.text_scale);
        self.icon_theme = self.icon_theme.or(other.icon_theme);
        self
    }
}

/// A preference a source could not answer, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Miss {
    pub feature: AppearanceFeature,
    pub reason: UnavailableReason,
    pub detail: String,
}

/// One source's reading: what it answered and why it did not answer the rest.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceReading {
    pub source: AppearanceSource,
    pub values: Partial,
    pub misses: Vec<Miss>,
}

impl SourceReading {
    pub fn new(source: AppearanceSource) -> Self {
        Self {
            source,
            values: Partial::default(),
            misses: Vec::new(),
        }
    }

    /// Records that `feature` could not be answered.
    pub fn miss(
        &mut self,
        feature: AppearanceFeature,
        reason: UnavailableReason,
        detail: impl Into<String>,
    ) {
        self.misses.push(Miss {
            feature,
            reason,
            detail: detail.into(),
        });
    }

    /// A reading that answered nothing, because the source itself failed.
    pub fn failed(
        source: AppearanceSource,
        reason: UnavailableReason,
        detail: impl Into<String>,
    ) -> Self {
        let detail = detail.into();
        let mut reading = Self::new(source);
        for feature in AppearanceFeature::ALL {
            reading.miss(feature, reason, detail.clone());
        }
        reading
    }
}

/// The resolved preferences and the availability of each one.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolution {
    pub values: AppearanceValues,
    pub status: Vec<AppearanceFeatureStatus>,
}

/// Picks, for each preference, the value of the first reading that has one. `readings` run from
/// the most to the least authoritative source, so the first answer wins and its source is
/// recorded. A preference nobody answers keeps a neutral value and reports the first miss any
/// source recorded for it, or `NoSource` when none did.
pub fn resolve(readings: &[SourceReading]) -> Resolution {
    fn first<T: Clone>(
        readings: &[SourceReading],
        get: impl Fn(&Partial) -> Option<&T>,
    ) -> Option<(T, AppearanceSource)> {
        readings
            .iter()
            .find_map(|reading| get(&reading.values).map(|v| (v.clone(), reading.source)))
    }

    let colour_scheme = first(readings, |p| p.colour_scheme.as_ref());
    let accent = first(readings, |p| p.accent.as_ref());
    let contrast = first(readings, |p| p.contrast.as_ref());
    let reduced_motion = first(readings, |p| p.reduced_motion.as_ref());
    let reduced_transparency = first(readings, |p| p.reduced_transparency.as_ref());
    let text_scale = first(readings, |p| p.text_scale.as_ref());
    let icon_theme = first(readings, |p| p.icon_theme.as_ref());

    let sources = AppearanceSources {
        colour_scheme: colour_scheme.as_ref().map(|(_, s)| *s),
        accent: accent.as_ref().map(|(_, s)| *s),
        contrast: contrast.as_ref().map(|(_, s)| *s),
        reduced_motion: reduced_motion.as_ref().map(|(_, s)| *s),
        reduced_transparency: reduced_transparency.as_ref().map(|(_, s)| *s),
        text_scale: text_scale.as_ref().map(|(_, s)| *s),
        icon_theme: icon_theme.as_ref().map(|(_, s)| *s),
    };
    let values = AppearanceValues {
        colour_scheme: colour_scheme.map_or(ColourScheme::NoPreference, |(v, _)| v),
        accent: accent.map(|(v, _)| v),
        contrast: contrast.map_or(Contrast::Normal, |(v, _)| v),
        reduced_motion: reduced_motion.is_some_and(|(v, _)| v),
        reduced_transparency: reduced_transparency.is_some_and(|(v, _)| v),
        text_scale: text_scale.map_or(1.0, |(v, _)| v),
        icon_theme: icon_theme.map(|(v, _)| v),
        sources,
    };
    let status = AppearanceFeature::ALL
        .into_iter()
        .map(|feature| feature_status(feature, &values.sources, readings))
        .collect();
    Resolution { values, status }
}

fn feature_status(
    feature: AppearanceFeature,
    sources: &AppearanceSources,
    readings: &[SourceReading],
) -> AppearanceFeatureStatus {
    let source = match feature {
        AppearanceFeature::ColourScheme => sources.colour_scheme,
        AppearanceFeature::Accent => sources.accent,
        AppearanceFeature::Contrast => sources.contrast,
        AppearanceFeature::ReducedMotion => sources.reduced_motion,
        AppearanceFeature::ReducedTransparency => sources.reduced_transparency,
        AppearanceFeature::TextScale => sources.text_scale,
        AppearanceFeature::IconTheme => sources.icon_theme,
    };
    if source.is_some() {
        return AppearanceFeatureStatus {
            feature,
            available: true,
            source,
            reason: None,
            detail: None,
        };
    }
    let miss = readings
        .iter()
        .flat_map(|reading| &reading.misses)
        .find(|miss| miss.feature == feature);
    let (reason, detail) = match miss {
        Some(miss) => (miss.reason, miss.detail.clone()),
        None => (
            UnavailableReason::NoSource,
            "no source on this desktop offers it".to_string(),
        ),
    };
    AppearanceFeatureStatus {
        feature,
        available: false,
        source: None,
        reason: Some(reason),
        detail: Some(detail),
    }
}

/// The resolution for a platform that offers no appearance preferences at all.
pub fn unsupported(detail: &str) -> Resolution {
    resolve(&[SourceReading::failed(
        // The source is never recorded: nothing is answered.
        AppearanceSource::Portal,
        UnavailableReason::PlatformUnsupported,
        detail,
    )])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(source: AppearanceSource, values: Partial) -> SourceReading {
        SourceReading {
            source,
            values,
            misses: Vec::new(),
        }
    }

    #[test]
    fn the_first_source_to_answer_wins_and_is_recorded() {
        let portal = reading(
            AppearanceSource::Portal,
            Partial {
                colour_scheme: Some(ColourScheme::Dark),
                accent: Some("#3584e4".to_string()),
                ..Partial::default()
            },
        );
        let kde = reading(
            AppearanceSource::KdeGlobals,
            Partial {
                colour_scheme: Some(ColourScheme::Light),
                icon_theme: Some("breeze".to_string()),
                text_scale: Some(1.25),
                ..Partial::default()
            },
        );
        let resolution = resolve(&[portal, kde]);
        let values = resolution.values;
        assert_eq!(values.colour_scheme, ColourScheme::Dark);
        assert_eq!(values.sources.colour_scheme, Some(AppearanceSource::Portal));
        assert_eq!(values.accent.as_deref(), Some("#3584e4"));
        assert_eq!(values.icon_theme.as_deref(), Some("breeze"));
        assert_eq!(
            values.sources.icon_theme,
            Some(AppearanceSource::KdeGlobals)
        );
        assert_eq!(values.text_scale, 1.25);
        assert_eq!(
            values.sources.text_scale,
            Some(AppearanceSource::KdeGlobals)
        );
    }

    #[test]
    fn an_explicit_false_from_the_first_source_is_not_overridden() {
        let portal = reading(
            AppearanceSource::Portal,
            Partial {
                reduced_motion: Some(false),
                ..Partial::default()
            },
        );
        let kde = reading(
            AppearanceSource::KdeGlobals,
            Partial {
                reduced_motion: Some(true),
                ..Partial::default()
            },
        );
        let values = resolve(&[portal, kde]).values;
        assert!(!values.reduced_motion);
        assert_eq!(
            values.sources.reduced_motion,
            Some(AppearanceSource::Portal)
        );
    }

    #[test]
    fn an_unanswered_preference_is_neutral_and_says_why() {
        let mut portal = SourceReading::new(AppearanceSource::Portal);
        portal.values.colour_scheme = Some(ColourScheme::Light);
        portal.miss(
            AppearanceFeature::Contrast,
            UnavailableReason::SourceMissing,
            "the portal does not expose contrast",
        );
        let resolution = resolve(&[portal]);
        assert_eq!(resolution.values.contrast, Contrast::Normal);
        assert_eq!(resolution.values.sources.contrast, None);
        let status = |feature| {
            resolution
                .status
                .iter()
                .find(|s| s.feature == feature)
                .cloned()
                .unwrap()
        };
        let contrast = status(AppearanceFeature::Contrast);
        assert!(!contrast.available);
        assert_eq!(contrast.reason, Some(UnavailableReason::SourceMissing));
        assert_eq!(
            contrast.detail.as_deref(),
            Some("the portal does not expose contrast")
        );
        let scheme = status(AppearanceFeature::ColourScheme);
        assert!(scheme.available);
        assert_eq!(scheme.source, Some(AppearanceSource::Portal));
        assert_eq!(scheme.reason, None);
        // Nobody recorded a miss for the icon theme.
        let icons = status(AppearanceFeature::IconTheme);
        assert_eq!(icons.reason, Some(UnavailableReason::NoSource));
    }

    #[test]
    fn the_status_lists_every_feature_in_order() {
        let status = resolve(&[]).status;
        let features: Vec<_> = status.iter().map(|s| s.feature).collect();
        assert_eq!(features, AppearanceFeature::ALL.to_vec());
        assert!(status.iter().all(|s| !s.available));
    }

    #[test]
    fn a_later_source_fills_in_what_an_earlier_one_could_not() {
        let mut portal = SourceReading::failed(
            AppearanceSource::Portal,
            UnavailableReason::PortalUnavailable,
            "no portal",
        );
        portal.values.colour_scheme = None;
        let gsettings = reading(
            AppearanceSource::Gsettings,
            Partial {
                colour_scheme: Some(ColourScheme::Dark),
                ..Partial::default()
            },
        );
        let resolution = resolve(&[portal, gsettings]);
        assert_eq!(
            resolution.values.sources.colour_scheme,
            Some(AppearanceSource::Gsettings)
        );
        // The accent stays unavailable, with the portal's reason.
        let accent = &resolution.status[1];
        assert_eq!(accent.reason, Some(UnavailableReason::PortalUnavailable));
    }

    #[test]
    fn partial_or_keeps_existing_values() {
        let first = Partial {
            text_scale: Some(1.0),
            ..Partial::default()
        };
        let second = Partial {
            text_scale: Some(2.0),
            icon_theme: Some("x".to_string()),
            ..Partial::default()
        };
        let merged = first.or(second);
        assert_eq!(merged.text_scale, Some(1.0));
        assert_eq!(merged.icon_theme.as_deref(), Some("x"));
        assert!(merged.has(AppearanceFeature::IconTheme));
        assert!(!merged.has(AppearanceFeature::Accent));
    }

    #[test]
    fn an_unsupported_platform_reports_every_feature_unavailable_with_a_reason() {
        let resolution = unsupported("appearance is not read on this platform");
        assert_eq!(resolution.values, AppearanceValues::default());
        assert_eq!(resolution.status.len(), 7);
        for status in &resolution.status {
            assert!(!status.available);
            assert_eq!(status.source, None);
            assert_eq!(status.reason, Some(UnavailableReason::PlatformUnsupported));
            assert_eq!(
                status.detail.as_deref(),
                Some("appearance is not read on this platform")
            );
        }
    }
}
