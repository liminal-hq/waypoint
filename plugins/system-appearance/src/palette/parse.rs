// Maps GTK's named colours, KDE's colour sections and Windows colour values onto the palette without doing any I/O
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::models::{PaletteColour, PaletteColours, PaletteEntry, PaletteSource};
use crate::{
    appearance::{models::UnavailableReason, parse as appearance},
    parse::ini_value,
};

/// A colour as a theme reports it: red, green, blue and alpha, each from 0 to 1.
pub type Rgba = (f64, f64, f64, f64);

/// The named colours that can supply each palette colour on a GTK theme, most specific first.
///
/// The `theme_*` names exist in every GTK 3 theme; `popover_bg_color`, `accent_bg_color` and the
/// status colours come from libadwaita-style themes (and Adwaita's own), so a theme without them
/// is a miss rather than a guess.
pub const GTK_NAMES: [(PaletteColour, &[&str]); 14] = [
    (PaletteColour::WindowBackground, &["theme_bg_color"]),
    (PaletteColour::WindowForeground, &["theme_fg_color"]),
    (PaletteColour::ViewBackground, &["theme_base_color"]),
    (PaletteColour::ViewForeground, &["theme_text_color"]),
    (
        PaletteColour::SurfaceBackground,
        &["popover_bg_color", "card_bg_color"],
    ),
    (
        PaletteColour::SelectionBackground,
        &["theme_selected_bg_color"],
    ),
    (
        PaletteColour::SelectionForeground,
        &["theme_selected_fg_color"],
    ),
    (PaletteColour::Border, &["borders"]),
    (PaletteColour::Focus, &["accent_bg_color"]),
    (PaletteColour::Warning, &["warning_color"]),
    (PaletteColour::Error, &["error_color"]),
    (PaletteColour::Success, &["success_color"]),
    (PaletteColour::TitleBarBackground, &["headerbar_bg_color"]),
    // libadwaita-style themes shade the header bar with a translucent black laid over its
    // colour; it is flattened over the title bar colour (not the window's) to give the bottom.
    (
        PaletteColour::TitleBarBackgroundEnd,
        &["headerbar_shade_color", "headerbar_darker_shade_color"],
    ),
];

fn channel(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Lays a colour over `backdrop` by its alpha, so a translucent border becomes the opaque colour
/// it looks like on the window. The result is lower-case `#rrggbb`.
pub fn flatten(colour: Rgba, backdrop: Option<(u8, u8, u8)>) -> String {
    let (red, green, blue, alpha) = colour;
    let alpha = alpha.clamp(0.0, 1.0);
    let (red, green, blue) = match backdrop {
        Some((r, g, b)) if alpha < 1.0 => (
            red * alpha + f64::from(r) / 255.0 * (1.0 - alpha),
            green * alpha + f64::from(g) / 255.0 * (1.0 - alpha),
            blue * alpha + f64::from(b) / 255.0 * (1.0 - alpha),
        ),
        _ => (red, green, blue),
    };
    appearance::hex(channel(red), channel(green), channel(blue))
}

/// Parses a `#rrggbb` string into its channels.
pub fn parse_hex(value: &str) -> Option<(u8, u8, u8)> {
    let digits = value.strip_prefix('#')?;
    if digits.len() != 6 || !digits.is_ascii() {
        return None;
    }
    let part = |range: std::ops::Range<usize>| u8::from_str_radix(&digits[range], 16).ok();
    Some((part(0..2)?, part(2..4)?, part(4..6)?))
}

/// Builds the palette from the names a GTK theme defines. `lookup` answers a name with its
/// colour, or `None` when the theme has no such name. Translucent colours are flattened over the
/// window background (which is taken as opaque).
pub fn gtk_palette(lookup: impl Fn(&str) -> Option<Rgba>) -> PaletteColours {
    let mut colours = PaletteColours::unavailable(UnavailableReason::SourceMissing, "");
    let window = GTK_NAMES[0]
        .1
        .iter()
        .find_map(|name| lookup(name))
        .map(|(red, green, blue, _)| (channel(red), channel(green), channel(blue)));
    for (colour, names) in GTK_NAMES {
        let found = names.iter().find_map(|name| lookup(name));
        // The title bar's bottom lies on its top, so it needs that colour to be flattened.
        let backdrop = if colour == PaletteColour::TitleBarBackgroundEnd {
            colours
                .title_bar_background
                .colour
                .as_deref()
                .and_then(parse_hex)
        } else {
            window
        };
        let found = if colour == PaletteColour::TitleBarBackgroundEnd && backdrop.is_none() {
            None
        } else {
            found
        };
        *colours.entry_mut(colour) = match found {
            Some(rgba) => PaletteEntry::found(flatten(rgba, backdrop), PaletteSource::GtkTheme),
            None => PaletteEntry::missing(
                UnavailableReason::SourceMissing,
                format!("the GTK theme defines none of {}", names.join(", ")),
            ),
        };
    }
    colours
}

/// The `kdeglobals` section and key that supply each palette colour. `None` is a colour the file
/// has no counterpart for.
const KDE_KEYS: [(PaletteColour, Option<(&str, &str)>); 14] = [
    (
        PaletteColour::WindowBackground,
        Some(("Colors:Window", "BackgroundNormal")),
    ),
    (
        PaletteColour::WindowForeground,
        Some(("Colors:Window", "ForegroundNormal")),
    ),
    (
        PaletteColour::ViewBackground,
        Some(("Colors:View", "BackgroundNormal")),
    ),
    (
        PaletteColour::ViewForeground,
        Some(("Colors:View", "ForegroundNormal")),
    ),
    (
        PaletteColour::SurfaceBackground,
        Some(("Colors:Button", "BackgroundNormal")),
    ),
    (
        PaletteColour::SelectionBackground,
        Some(("Colors:Selection", "BackgroundNormal")),
    ),
    (
        PaletteColour::SelectionForeground,
        Some(("Colors:Selection", "ForegroundNormal")),
    ),
    (PaletteColour::Border, None),
    (
        PaletteColour::Focus,
        Some(("Colors:Window", "DecorationFocus")),
    ),
    (
        PaletteColour::Warning,
        Some(("Colors:View", "ForegroundNeutral")),
    ),
    (
        PaletteColour::Error,
        Some(("Colors:View", "ForegroundNegative")),
    ),
    (
        PaletteColour::Success,
        Some(("Colors:View", "ForegroundPositive")),
    ),
    (
        PaletteColour::TitleBarBackground,
        Some(("Colors:Header", "BackgroundNormal")),
    ),
    (
        PaletteColour::TitleBarBackgroundEnd,
        Some(("Colors:Header", "BackgroundAlternate")),
    ),
];

/// Builds the palette from the text of `kdeglobals`. A section or key the file lacks is a miss.
pub fn kde_palette(text: &str) -> PaletteColours {
    let mut colours = PaletteColours::unavailable(UnavailableReason::SourceMissing, "");
    for (colour, location) in KDE_KEYS {
        *colours.entry_mut(colour) = match location {
            None => PaletteEntry::missing(
                UnavailableReason::NoSource,
                "kdeglobals has no border colour",
            ),
            Some((section, key)) => {
                match ini_value(text, section, key).and_then(|value| appearance::kde_rgb(&value)) {
                    Some((red, green, blue)) => PaletteEntry::found(
                        appearance::hex(red, green, blue),
                        PaletteSource::KdeGlobals,
                    ),
                    None => PaletteEntry::missing(
                        UnavailableReason::SourceMissing,
                        format!("kdeglobals has no {key} in [{section}]"),
                    ),
                }
            }
        };
    }
    colours
}

/// The colour of a Windows `COLORREF` (`0x00bbggrr`) as `#rrggbb`.
pub fn windows_colorref(value: u32) -> String {
    appearance::hex(
        (value & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        ((value >> 16) & 0xff) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn table(pairs: &[(&str, Rgba)]) -> impl Fn(&str) -> Option<Rgba> {
        let map: HashMap<String, Rgba> = pairs
            .iter()
            .map(|(name, colour)| (name.to_string(), *colour))
            .collect();
        move |name| map.get(name).copied()
    }

    /// Adwaita dark as GTK 3.24 reports its named colours.
    fn adwaita_dark() -> impl Fn(&str) -> Option<Rgba> {
        let c = |r: u8, g: u8, b: u8| {
            (
                f64::from(r) / 255.0,
                f64::from(g) / 255.0,
                f64::from(b) / 255.0,
                1.0,
            )
        };
        table(&[
            ("theme_bg_color", c(0x24, 0x24, 0x24)),
            ("theme_fg_color", c(0xff, 0xff, 0xff)),
            ("theme_base_color", c(0x1e, 0x1e, 0x1e)),
            ("theme_text_color", c(0xff, 0xff, 0xff)),
            ("theme_selected_bg_color", c(0x35, 0x84, 0xe4)),
            ("theme_selected_fg_color", c(0xff, 0xff, 0xff)),
            ("borders", (0.0, 0.0, 0.0, 0.5)),
            ("warning_color", c(0xcd, 0x93, 0x09)),
            ("error_color", c(0xc0, 0x1c, 0x28)),
            ("success_color", c(0x26, 0xa2, 0x69)),
        ])
    }

    #[test]
    fn a_full_gtk_theme_maps_every_name_it_has() {
        let colours = gtk_palette(adwaita_dark());
        assert_eq!(colours.window_background.colour.as_deref(), Some("#242424"));
        assert_eq!(colours.window_foreground.colour.as_deref(), Some("#ffffff"));
        assert_eq!(colours.view_background.colour.as_deref(), Some("#1e1e1e"));
        assert_eq!(
            colours.selection_background.colour.as_deref(),
            Some("#3584e4")
        );
        assert_eq!(colours.error.colour.as_deref(), Some("#c01c28"));
        assert_eq!(
            colours.window_background.source,
            Some(PaletteSource::GtkTheme)
        );
        assert!(colours.is_usable());
    }

    #[test]
    fn a_theme_with_a_header_bar_colour_and_shade_gives_two_tones() {
        let lookup = table(&[
            ("theme_bg_color", (1.0, 1.0, 1.0, 1.0)),
            ("theme_fg_color", (0.0, 0.0, 0.0, 1.0)),
            ("headerbar_bg_color", (1.0, 1.0, 1.0, 1.0)),
            ("headerbar_shade_color", (0.0, 0.0, 0.0, 0.5)),
        ]);
        let colours = gtk_palette(lookup);
        assert_eq!(
            colours.title_bar_background.colour.as_deref(),
            Some("#ffffff")
        );
        // Half black over white, not over the window colour, is mid grey.
        assert_eq!(
            colours.title_bar_background_end.colour.as_deref(),
            Some("#808080")
        );
    }

    #[test]
    fn a_theme_without_header_bar_names_reports_a_miss_for_both() {
        let colours = gtk_palette(adwaita_dark());
        assert_eq!(colours.title_bar_background.colour, None);
        assert_eq!(
            colours.title_bar_background.reason,
            Some(UnavailableReason::SourceMissing)
        );
        assert_eq!(colours.title_bar_background_end.colour, None);
    }

    #[test]
    fn a_shade_without_a_header_bar_colour_is_not_drawn() {
        let lookup = table(&[
            ("theme_bg_color", (1.0, 1.0, 1.0, 1.0)),
            ("headerbar_shade_color", (0.0, 0.0, 0.0, 0.5)),
        ]);
        assert_eq!(gtk_palette(lookup).title_bar_background_end.colour, None);
    }

    #[test]
    fn a_translucent_border_is_flattened_over_the_window_background() {
        let colours = gtk_palette(adwaita_dark());
        // Half black over #242424 is #121212.
        assert_eq!(colours.border.colour.as_deref(), Some("#121212"));
    }

    #[test]
    fn a_name_the_theme_lacks_is_a_typed_miss_that_lists_the_names_tried() {
        let colours = gtk_palette(adwaita_dark());
        let surface = &colours.surface_background;
        assert_eq!(surface.colour, None);
        assert_eq!(surface.reason, Some(UnavailableReason::SourceMissing));
        assert_eq!(
            surface.detail.as_deref(),
            Some("the GTK theme defines none of popover_bg_color, card_bg_color")
        );
        assert_eq!(colours.focus.reason, Some(UnavailableReason::SourceMissing));
    }

    #[test]
    fn libadwaita_names_fill_the_surface_and_the_focus() {
        let lookup = table(&[
            ("theme_bg_color", (1.0, 1.0, 1.0, 1.0)),
            ("theme_fg_color", (0.0, 0.0, 0.0, 1.0)),
            ("popover_bg_color", (0.5, 0.5, 0.5, 1.0)),
            ("accent_bg_color", (0.0, 0.0, 1.0, 1.0)),
        ]);
        let colours = gtk_palette(lookup);
        assert_eq!(
            colours.surface_background.colour.as_deref(),
            Some("#808080")
        );
        assert_eq!(colours.focus.colour.as_deref(), Some("#0000ff"));
    }

    #[test]
    fn a_theme_without_the_window_colours_is_not_usable() {
        let colours = gtk_palette(table(&[("theme_base_color", (1.0, 1.0, 1.0, 1.0))]));
        assert!(!colours.is_usable());
        let status = colours.status();
        assert!(!status.available);
        assert_eq!(status.reason, Some(UnavailableReason::SourceMissing));
        assert!(status.detail.unwrap().contains("theme_bg_color"));
    }

    const BREEZE_DARK: &str = "\
[Colors:Button]
BackgroundNormal=49,54,59
ForegroundNormal=252,252,252

[Colors:Selection]
BackgroundNormal=61,174,233
ForegroundNormal=252,252,252

[Colors:View]
BackgroundNormal=27,30,32
ForegroundNormal=252,252,252
ForegroundNegative=218,68,83
ForegroundNeutral=246,116,0
ForegroundPositive=39,174,96

[Colors:Window]
BackgroundNormal=42,46,50
DecorationFocus=61,174,233
ForegroundNormal=252,252,252
";

    #[test]
    fn a_kdeglobals_file_maps_each_section() {
        let colours = kde_palette(BREEZE_DARK);
        assert_eq!(colours.window_background.colour.as_deref(), Some("#2a2e32"));
        assert_eq!(colours.window_foreground.colour.as_deref(), Some("#fcfcfc"));
        assert_eq!(colours.view_background.colour.as_deref(), Some("#1b1e20"));
        assert_eq!(
            colours.surface_background.colour.as_deref(),
            Some("#31363b")
        );
        assert_eq!(
            colours.selection_background.colour.as_deref(),
            Some("#3daee9")
        );
        assert_eq!(colours.focus.colour.as_deref(), Some("#3daee9"));
        assert_eq!(colours.warning.colour.as_deref(), Some("#f67400"));
        assert_eq!(colours.error.colour.as_deref(), Some("#da4453"));
        assert_eq!(colours.success.colour.as_deref(), Some("#27ae60"));
        assert_eq!(
            colours.window_background.source,
            Some(PaletteSource::KdeGlobals)
        );
    }

    #[test]
    fn kdeglobals_header_section_gives_the_title_bar_tones() {
        let text = format!(
            "{BREEZE_DARK}\n[Colors:Header]\nBackgroundNormal=32,35,38\nBackgroundAlternate=27,30,32\n"
        );
        let colours = kde_palette(&text);
        assert_eq!(
            colours.title_bar_background.colour.as_deref(),
            Some("#202326")
        );
        assert_eq!(
            colours.title_bar_background_end.colour.as_deref(),
            Some("#1b1e20")
        );
        assert_eq!(
            colours.title_bar_background.source,
            Some(PaletteSource::KdeGlobals)
        );
    }

    #[test]
    fn a_kdeglobals_without_a_header_section_has_no_title_bar_colour() {
        let colours = kde_palette(BREEZE_DARK);
        assert_eq!(colours.title_bar_background.colour, None);
        let detail = colours.title_bar_background.detail.unwrap();
        assert!(detail.contains("[Colors:Header]"), "{detail}");
    }

    #[test]
    fn kdeglobals_has_no_border_colour() {
        let colours = kde_palette(BREEZE_DARK);
        assert_eq!(colours.border.colour, None);
        assert_eq!(colours.border.reason, Some(UnavailableReason::NoSource));
    }

    #[test]
    fn a_kdeglobals_without_colour_sections_is_not_usable() {
        let colours = kde_palette("[General]\nColorScheme=BreezeDark\n");
        assert!(!colours.is_usable());
        let detail = colours.status().detail.unwrap();
        assert!(detail.contains("[Colors:Window]"), "{detail}");
    }

    #[test]
    fn a_kde_colour_with_an_alpha_channel_keeps_its_three_channels() {
        let colours = kde_palette("[Colors:Window]\nBackgroundNormal=10,20,30,255\n");
        assert_eq!(colours.window_background.colour.as_deref(), Some("#0a141e"));
    }

    #[test]
    fn fill_from_keeps_what_was_found_and_takes_the_rest() {
        let mut first = kde_palette("[Colors:Window]\nBackgroundNormal=1,2,3\n");
        let mut second = PaletteColours::unavailable(UnavailableReason::NoSource, "x");
        second.set(
            PaletteColour::Focus,
            "#112233".to_string(),
            PaletteSource::Portal,
        );
        second.set(
            PaletteColour::WindowBackground,
            "#ffffff".to_string(),
            PaletteSource::Portal,
        );
        first.fill_from(&second);
        assert_eq!(first.window_background.colour.as_deref(), Some("#010203"));
        assert_eq!(first.focus.colour.as_deref(), Some("#112233"));
        assert_eq!(first.focus.source, Some(PaletteSource::Portal));
    }

    #[test]
    fn a_windows_colorref_is_blue_green_red() {
        assert_eq!(windows_colorref(0x00ff_8000), "#0080ff");
        assert_eq!(windows_colorref(0x0000_00ff), "#ff0000");
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(parse_hex("#0a141e"), Some((10, 20, 30)));
        assert_eq!(parse_hex("0a141e"), None);
        assert_eq!(parse_hex("#0a14"), None);
    }

    #[test]
    fn the_wire_format_names_each_colour_with_its_source_and_reason() {
        let json = serde_json::to_value(kde_palette(BREEZE_DARK)).unwrap();
        assert_eq!(json["windowBackground"]["colour"], "#2a2e32");
        assert_eq!(json["windowBackground"]["source"], "kdeGlobals");
        assert_eq!(json["border"]["colour"], serde_json::Value::Null);
        assert_eq!(json["border"]["reason"], "noSource");
    }
}
