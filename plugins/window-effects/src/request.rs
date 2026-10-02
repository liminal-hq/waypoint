// Validates the requests (regions and insets) and maps an effect request to what each backend does with it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::error::{Result, WindowEffectsError};
use crate::models::{
    EffectKind, Effects, Insets, Rect, FEATURE_ACRYLIC, FEATURE_BLUR, FEATURE_MICA,
};

/// The feature an effect needs, or `None` for the request that clears.
pub fn feature_for(kind: EffectKind) -> Option<&'static str> {
    match kind {
        EffectKind::None => None,
        EffectKind::Blur => Some(FEATURE_BLUR),
        EffectKind::Mica => Some(FEATURE_MICA),
        EffectKind::Acrylic => Some(FEATURE_ACRYLIC),
    }
}

fn invalid_region(message: impl Into<String>) -> WindowEffectsError {
    WindowEffectsError::InvalidRegion {
        message: message.into(),
    }
}

/// Checks a region against the window's logical size: at least one rectangle, each with a positive size, inside the window.
pub fn validate_region(region: &[Rect], width: i64, height: i64) -> Result<()> {
    if region.is_empty() {
        return Err(invalid_region(
            "the region has no rectangles; leave it out to cover the whole window",
        ));
    }
    for (index, rect) in region.iter().enumerate() {
        if rect.width <= 0 || rect.height <= 0 {
            return Err(invalid_region(format!(
                "rectangle {index} has no size ({} by {})",
                rect.width, rect.height
            )));
        }
        let (x, y) = (i64::from(rect.x), i64::from(rect.y));
        if x < 0
            || y < 0
            || x + i64::from(rect.width) > width
            || y + i64::from(rect.height) > height
        {
            return Err(invalid_region(format!(
                "rectangle {index} reaches outside the {width} by {height} window"
            )));
        }
    }
    Ok(())
}

/// Checks insets against the window's logical size: none negative, and room left in the window.
pub fn validate_insets(insets: Insets, width: i64, height: i64) -> Result<()> {
    let invalid = |message: &str| WindowEffectsError::InvalidInsets {
        message: message.to_string(),
    };
    if insets.top < 0 || insets.right < 0 || insets.bottom < 0 || insets.left < 0 {
        return Err(invalid("an inset is negative"));
    }
    if i64::from(insets.left) + i64::from(insets.right) >= width
        || i64::from(insets.top) + i64::from(insets.bottom) >= height
    {
        return Err(invalid("the insets leave no room in the window"));
    }
    Ok(())
}

/// How strong the Windows materials' tint is: Tauri applies it to Blur and Acrylic on Windows 10 and ignores it on Windows 11.
const TINT_ALPHA: u8 = 125;

/// What a Windows backend asks DWM for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsEffect {
    MicaDark,
    MicaLight,
    Acrylic { tint: (u8, u8, u8, u8) },
    Blur { tint: (u8, u8, u8, u8) },
}

fn tint(dark: bool) -> (u8, u8, u8, u8) {
    if dark {
        (18, 18, 18, TINT_ALPHA)
    } else {
        (242, 242, 242, TINT_ALPHA)
    }
}

/// Maps a request to the material, `None` for a request that clears. The theme is the caller's (`dark`), so Mica never follows the system's own preference behind the page's back.
pub fn windows_effect(effects: &Effects) -> Option<WindowsEffect> {
    match effects.kind {
        EffectKind::None => None,
        EffectKind::Mica if effects.dark => Some(WindowsEffect::MicaDark),
        EffectKind::Mica => Some(WindowsEffect::MicaLight),
        EffectKind::Acrylic => Some(WindowsEffect::Acrylic {
            tint: tint(effects.dark),
        }),
        EffectKind::Blur => Some(WindowsEffect::Blur {
            tint: tint(effects.dark),
        }),
    }
}

/// The value of `_KDE_NET_WM_BLUR_BEHIND_REGION`: `x, y, width, height` for each rectangle, in device pixels, and nothing at all for the whole window.
pub fn x11_blur_property(region: Option<&[Rect]>, scale: u32) -> Vec<u32> {
    let scale = i64::from(scale.max(1));
    region
        .unwrap_or_default()
        .iter()
        .flat_map(|rect| {
            [rect.x, rect.y, rect.width, rect.height]
                .map(|value| (i64::from(value) * scale).clamp(0, i64::from(u32::MAX)) as u32)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    fn effects(kind: EffectKind, dark: bool) -> Effects {
        Effects {
            kind,
            dark,
            region: None,
        }
    }

    #[test]
    fn a_region_inside_the_window_is_accepted() {
        assert!(validate_region(&[rect(0, 0, 800, 600), rect(10, 10, 5, 5)], 800, 600).is_ok());
    }

    #[test]
    fn a_region_with_a_bad_rectangle_is_refused_with_its_index() {
        for (region, needle) in [
            (vec![], "no rectangles"),
            (vec![rect(0, 0, 0, 10)], "rectangle 0 has no size"),
            (vec![rect(0, 0, 10, -1)], "rectangle 0 has no size"),
            (
                vec![rect(0, 0, 5, 5), rect(-1, 0, 5, 5)],
                "rectangle 1 reaches",
            ),
            (vec![rect(0, -2, 5, 5)], "rectangle 0 reaches"),
            (vec![rect(790, 0, 11, 5)], "rectangle 0 reaches"),
            (vec![rect(0, 595, 5, 6)], "rectangle 0 reaches"),
        ] {
            let error = validate_region(&region, 800, 600).unwrap_err();
            let WindowEffectsError::InvalidRegion { message } = error else {
                panic!("not an invalid region: {error:?}");
            };
            assert!(
                message.contains(needle),
                "{message} should contain {needle}"
            );
        }
    }

    #[test]
    fn a_huge_rectangle_cannot_overflow_the_check() {
        assert!(validate_region(&[rect(i32::MAX, 0, i32::MAX, 1)], 800, 600).is_err());
    }

    #[test]
    fn insets_must_be_positive_and_leave_room() {
        let ok = Insets {
            top: 8,
            right: 8,
            bottom: 8,
            left: 8,
        };
        assert!(validate_insets(ok, 800, 600).is_ok());
        assert!(validate_insets(
            Insets {
                top: 0,
                right: 0,
                bottom: 0,
                left: 0
            },
            800,
            600
        )
        .is_ok());
        assert!(validate_insets(Insets { left: -1, ..ok }, 800, 600).is_err());
        assert!(validate_insets(
            Insets {
                left: 400,
                right: 400,
                ..ok
            },
            800,
            600
        )
        .is_err());
        assert!(validate_insets(
            Insets {
                top: 300,
                bottom: 300,
                ..ok
            },
            800,
            600
        )
        .is_err());
    }

    #[test]
    fn each_kind_needs_its_own_feature() {
        assert_eq!(feature_for(EffectKind::None), None);
        assert_eq!(feature_for(EffectKind::Blur), Some("blur"));
        assert_eq!(feature_for(EffectKind::Mica), Some("mica"));
        assert_eq!(feature_for(EffectKind::Acrylic), Some("acrylic"));
    }

    #[test]
    fn mica_follows_the_requested_theme() {
        assert_eq!(
            windows_effect(&effects(EffectKind::Mica, true)),
            Some(WindowsEffect::MicaDark)
        );
        assert_eq!(
            windows_effect(&effects(EffectKind::Mica, false)),
            Some(WindowsEffect::MicaLight)
        );
    }

    #[test]
    fn acrylic_and_blur_are_tinted_to_the_theme_and_none_clears() {
        assert_eq!(
            windows_effect(&effects(EffectKind::Acrylic, true)),
            Some(WindowsEffect::Acrylic {
                tint: (18, 18, 18, TINT_ALPHA)
            })
        );
        assert_eq!(
            windows_effect(&effects(EffectKind::Blur, false)),
            Some(WindowsEffect::Blur {
                tint: (242, 242, 242, TINT_ALPHA)
            })
        );
        assert_eq!(windows_effect(&effects(EffectKind::None, true)), None);
    }

    #[test]
    fn the_x11_property_is_four_numbers_a_rectangle_in_device_pixels() {
        assert_eq!(x11_blur_property(None, 2), Vec::<u32>::new());
        assert_eq!(
            x11_blur_property(Some(&[rect(1, 2, 30, 40), rect(0, 0, 5, 5)]), 2),
            vec![2, 4, 60, 80, 0, 0, 10, 10]
        );
        assert_eq!(
            x11_blur_property(Some(&[rect(1, 2, 3, 4)]), 0),
            vec![1, 2, 3, 4]
        );
    }

    #[test]
    fn effects_deserialise_with_their_defaults() {
        let parsed: Effects = serde_json::from_str(r#"{"kind":"mica"}"#).unwrap();
        assert_eq!(parsed, effects(EffectKind::Mica, false));
        let parsed: Effects = serde_json::from_str(
            r#"{"kind":"blur","dark":true,"region":[{"x":0,"y":0,"width":1,"height":2}]}"#,
        )
        .unwrap();
        assert_eq!(parsed.region, Some(vec![rect(0, 0, 1, 2)]));
    }
}
