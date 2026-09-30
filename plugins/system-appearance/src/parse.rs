// Parses desktop preference strings into titlebar models without doing any I/O
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::models::{
    ButtonLayout, DesktopEnvironment, LayoutSource, TitlebarAction, TitlebarActions,
    TitlebarPreferences, WindowButton,
};

/// Appends `button` unless it is already present anywhere in `layout`, so the first occurrence wins.
fn push_unique(layout: &mut ButtonLayout, side_is_start: bool, button: WindowButton) {
    if layout.start.contains(&button) || layout.end.contains(&button) {
        return;
    }
    if side_is_start {
        layout.start.push(button);
    } else {
        layout.end.push(button);
    }
}

/// Maps `XDG_CURRENT_DESKTOP` (colon-separated) to a desktop environment.
///
/// The first token that names a known desktop wins. Budgie, Pantheon, Unity and GNOME Flashback
/// belong to the GNOME family because their settings are exposed the same way.
pub fn desktop_environment(xdg_current_desktop: &str) -> DesktopEnvironment {
    for token in xdg_current_desktop.split(':') {
        let token = token.trim().to_ascii_uppercase();
        let found = match token.as_str() {
            "KDE" => DesktopEnvironment::Kde,
            "X-CINNAMON" | "CINNAMON" => DesktopEnvironment::Cinnamon,
            "MATE" => DesktopEnvironment::Mate,
            "XFCE" => DesktopEnvironment::Xfce,
            "BUDGIE" | "PANTHEON" | "UNITY" | "UNITY7" => DesktopEnvironment::Gnome,
            t if t.starts_with("GNOME") => DesktopEnvironment::Gnome,
            _ => continue,
        };
        return found;
    }
    DesktopEnvironment::Unknown
}

/// Strips the GVariant text quoting that `gsettings get` prints around a string value.
pub fn unquote_gvariant_string(value: &str) -> &str {
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

/// Parses a GNOME-style `button-layout` such as `appmenu:minimize,maximize,close`.
///
/// Tokens before the first colon go to the start side and tokens after it to the end side. A value
/// without a colon puts everything at the start. Unknown tokens and `spacer` are ignored.
pub fn gnome_button_layout(value: &str) -> ButtonLayout {
    let (left, right) = value.trim().split_once(':').unwrap_or((value.trim(), ""));
    let mut layout = ButtonLayout::default();
    for (side_is_start, tokens) in [(true, left), (false, right)] {
        for token in tokens.split(',') {
            let button = match token.trim() {
                "appmenu" => WindowButton::AppMenu,
                "minimize" => WindowButton::Minimise,
                "maximize" => WindowButton::Maximise,
                "close" => WindowButton::Close,
                // GNOME writes the window menu as `icon`; Metacity-compatible desktops such as MATE write `menu`.
                "icon" | "menu" => WindowButton::WindowMenu,
                _ => continue,
            };
            push_unique(&mut layout, side_is_start, button);
        }
    }
    layout
}

/// Parses a GNOME-family titlebar action name such as `toggle-maximize`; unknown names mean none.
pub fn gnome_action(value: &str) -> TitlebarAction {
    match value.trim() {
        "toggle-maximize" => TitlebarAction::ToggleMaximise,
        "toggle-maximize-horizontally" => TitlebarAction::ToggleMaximiseHorizontally,
        "toggle-maximize-vertically" => TitlebarAction::ToggleMaximiseVertically,
        "toggle-shade" | "shade" => TitlebarAction::ToggleShade,
        "minimize" => TitlebarAction::Minimise,
        "lower" => TitlebarAction::Lower,
        "menu" => TitlebarAction::Menu,
        _ => TitlebarAction::None,
    }
}

/// Builds actions from optional GNOME-family action values, using the defaults for absent ones.
pub fn gnome_actions(
    double_click: Option<&str>,
    middle_click: Option<&str>,
    right_click: Option<&str>,
) -> TitlebarActions {
    let default = TitlebarActions::DEFAULT;
    TitlebarActions {
        double_click: double_click.map_or(default.double_click, gnome_action),
        middle_click: middle_click.map_or(default.middle_click, gnome_action),
        right_click: right_click.map_or(default.right_click, gnome_action),
    }
}

/// Default KWin decoration buttons for the start side.
pub const KDE_DEFAULT_LEFT: &str = "MS";
/// Default KWin decoration buttons for the end side.
pub const KDE_DEFAULT_RIGHT: &str = "HIAX";

/// Parses KWin `ButtonsOnLeft` / `ButtonsOnRight` letters, ignoring `_` spacers and unknown letters.
fn kde_buttons(letters: &str) -> Vec<WindowButton> {
    letters
        .trim()
        .chars()
        .filter_map(|letter| match letter {
            'M' => Some(WindowButton::WindowMenu),
            'N' => Some(WindowButton::AppMenu),
            'S' => Some(WindowButton::Stick),
            'H' => Some(WindowButton::Help),
            'I' => Some(WindowButton::Minimise),
            'A' => Some(WindowButton::Maximise),
            'X' => Some(WindowButton::Close),
            'F' => Some(WindowButton::KeepAbove),
            'B' => Some(WindowButton::KeepBelow),
            'L' => Some(WindowButton::Shade),
            _ => None,
        })
        .collect()
}

/// Builds a KWin button layout from the optional configured strings, using KWin's defaults.
pub fn kde_button_layout(left: Option<&str>, right: Option<&str>) -> ButtonLayout {
    let mut layout = ButtonLayout::default();
    for (side_is_start, letters) in [
        (true, left.unwrap_or(KDE_DEFAULT_LEFT)),
        (false, right.unwrap_or(KDE_DEFAULT_RIGHT)),
    ] {
        for button in kde_buttons(letters) {
            push_unique(&mut layout, side_is_start, button);
        }
    }
    layout
}

/// The KWin command names that mean the same thing for a double-click and for a titlebar button.
fn kde_common_command(value: &str) -> Option<TitlebarAction> {
    match value {
        "Maximize" => Some(TitlebarAction::ToggleMaximise),
        "Maximize (horizontal only)" => Some(TitlebarAction::ToggleMaximiseHorizontally),
        "Maximize (vertical only)" => Some(TitlebarAction::ToggleMaximiseVertically),
        "Minimize" => Some(TitlebarAction::Minimise),
        "Shade" => Some(TitlebarAction::ToggleShade),
        "Close" => Some(TitlebarAction::Close),
        _ => None,
    }
}

/// Parses KWin's `TitlebarDoubleClickCommand` value.
pub fn kde_double_click(value: &str) -> TitlebarAction {
    let value = value.trim();
    kde_common_command(value).unwrap_or(match value {
        "Lower" => TitlebarAction::Lower,
        _ => TitlebarAction::None,
    })
}

/// Parses KWin's `CommandActiveTitlebar2` / `CommandActiveTitlebar3` values.
pub fn kde_titlebar_command(value: &str) -> TitlebarAction {
    let value = value.trim();
    kde_common_command(value).unwrap_or(match value {
        "Lower" => TitlebarAction::Lower,
        "Toggle raise and lower" => TitlebarAction::ToggleRaiseLower,
        "Operations menu" => TitlebarAction::Menu,
        _ => TitlebarAction::None,
    })
}

/// Builds actions from the text of `kwinrc`, using the defaults for absent keys.
pub fn kde_actions(kwinrc: &str) -> TitlebarActions {
    let default = TitlebarActions::DEFAULT;
    let windows = |key| ini_value(kwinrc, "Windows", key);
    TitlebarActions {
        double_click: windows("TitlebarDoubleClickCommand")
            .map_or(default.double_click, |v| kde_double_click(&v)),
        middle_click: windows("CommandActiveTitlebar2")
            .map_or(default.middle_click, |v| kde_titlebar_command(&v)),
        right_click: windows("CommandActiveTitlebar3")
            .map_or(default.right_click, |v| kde_titlebar_command(&v)),
    }
}

/// Builds the layout from the text of `kwinrc`.
pub fn kde_layout_from_kwinrc(kwinrc: &str) -> ButtonLayout {
    let decoration = |key| ini_value(kwinrc, "org.kde.kdecoration2", key);
    kde_button_layout(
        decoration("ButtonsOnLeft").as_deref(),
        decoration("ButtonsOnRight").as_deref(),
    )
}

/// Returns the first value of `key` inside `[section]` of INI text, trimmed.
pub fn ini_value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_section = name == section;
        } else if in_section {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

/// Parses xfwm4's `button_layout` such as `O|HMC`; the `|` marks the title position.
///
/// Letters before the bar go to the start side and letters after it to the end side. A value
/// without a bar puts everything at the start. `_` and unknown letters are ignored.
pub fn xfce_button_layout(value: &str) -> ButtonLayout {
    let (left, right) = value.trim().split_once('|').unwrap_or((value.trim(), ""));
    let mut layout = ButtonLayout::default();
    for (side_is_start, letters) in [(true, left), (false, right)] {
        for letter in letters.chars() {
            let button = match letter {
                'O' => WindowButton::WindowMenu,
                'T' => WindowButton::Stick,
                'S' => WindowButton::Shade,
                'H' => WindowButton::Minimise,
                'M' => WindowButton::Maximise,
                'C' => WindowButton::Close,
                _ => continue,
            };
            push_unique(&mut layout, side_is_start, button);
        }
    }
    layout
}

/// Parses xfwm4's `double_click_action`; only maximise, shade and hide map to actions.
pub fn xfce_double_click(value: &str) -> TitlebarAction {
    match value.trim() {
        "maximize" => TitlebarAction::ToggleMaximise,
        "shade" => TitlebarAction::ToggleShade,
        "hide" => TitlebarAction::Minimise,
        _ => TitlebarAction::None,
    }
}

/// Parses macOS `AppleActionOnDoubleClick`; a missing or unrecognised value means maximise.
pub fn macos_double_click(value: Option<&str>) -> TitlebarAction {
    match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
        Some("minimize") => TitlebarAction::Minimise,
        Some("none") => TitlebarAction::None,
        _ => TitlebarAction::ToggleMaximise,
    }
}

/// The fixed Windows titlebar: buttons at the end, conventional actions.
pub fn windows_preferences() -> TitlebarPreferences {
    TitlebarPreferences {
        button_layout: ButtonLayout {
            start: Vec::new(),
            end: vec![
                WindowButton::Minimise,
                WindowButton::Maximise,
                WindowButton::Close,
            ],
        },
        actions: TitlebarActions::DEFAULT,
        desktop_environment: DesktopEnvironment::Windows,
        source: LayoutSource::Platform,
    }
}

/// The fixed macOS titlebar: traffic lights at the start, with the configured double-click action.
pub fn macos_preferences(double_click: TitlebarAction) -> TitlebarPreferences {
    TitlebarPreferences {
        button_layout: ButtonLayout {
            start: vec![
                WindowButton::Close,
                WindowButton::Minimise,
                WindowButton::Maximise,
            ],
            end: Vec::new(),
        },
        actions: TitlebarActions {
            double_click,
            ..TitlebarActions::DEFAULT
        },
        desktop_environment: DesktopEnvironment::Macos,
        source: LayoutSource::Platform,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TitlebarAction as A;
    use WindowButton as B;

    fn layout(start: &[WindowButton], end: &[WindowButton]) -> ButtonLayout {
        ButtonLayout {
            start: start.to_vec(),
            end: end.to_vec(),
        }
    }

    #[test]
    fn desktop_environment_table() {
        use DesktopEnvironment as D;
        let cases = [
            ("GNOME", D::Gnome),
            ("ubuntu:GNOME", D::Gnome),
            ("GNOME-Flashback:GNOME", D::Gnome),
            ("Budgie:GNOME", D::Gnome),
            ("Budgie", D::Gnome),
            ("Pantheon", D::Gnome),
            ("Unity", D::Gnome),
            ("Unity:Unity7:ubuntu", D::Gnome),
            ("KDE", D::Kde),
            ("X-Cinnamon", D::Cinnamon),
            ("MATE", D::Mate),
            ("XFCE", D::Xfce),
            ("ubuntu:XFCE", D::Xfce),
            ("LXQt", D::Unknown),
            ("ubuntu", D::Unknown),
            ("", D::Unknown),
        ];
        for (input, expected) in cases {
            assert_eq!(desktop_environment(input), expected, "{input:?}");
        }
    }

    #[test]
    fn unquote_table() {
        let cases = [
            ("'appmenu:close'", "appmenu:close"),
            ("  'a:b'\n", "a:b"),
            ("\"a:b\"", "a:b"),
            ("@s 'a:b'", "a:b"),
            ("''", ""),
            ("bare", "bare"),
            ("'", "'"),
        ];
        for (input, expected) in cases {
            assert_eq!(unquote_gvariant_string(input), expected, "{input:?}");
        }
    }

    #[test]
    fn gnome_layout_table() {
        let cases = [
            (
                "appmenu:minimize,maximize,close",
                layout(&[B::AppMenu], &[B::Minimise, B::Maximise, B::Close]),
            ),
            (
                "close,minimize,maximize:",
                layout(&[B::Close, B::Minimise, B::Maximise], &[]),
            ),
            ("close:appmenu", layout(&[B::Close], &[B::AppMenu])),
            ("icon:close", layout(&[B::WindowMenu], &[B::Close])),
            (
                "menu:minimize,maximize,close",
                layout(&[B::WindowMenu], &[B::Minimise, B::Maximise, B::Close]),
            ),
            (":", layout(&[], &[])),
            ("", layout(&[], &[])),
            // No colon puts everything at the start.
            ("minimize,close", layout(&[B::Minimise, B::Close], &[])),
            // Unknown tokens and spacers are ignored.
            (
                "spacer,close:bogus,maximize,spacer",
                layout(&[B::Close], &[B::Maximise]),
            ),
            // Duplicates keep the first occurrence, across both sides.
            (
                "close,close:close,minimize,minimize",
                layout(&[B::Close], &[B::Minimise]),
            ),
            // Whitespace is tolerated.
            (
                "  appmenu , close : minimize ,maximize  ",
                layout(&[B::AppMenu, B::Close], &[B::Minimise, B::Maximise]),
            ),
            // Extra colons leave the tail as an unknown token.
            ("close:minimize:maximize", layout(&[B::Close], &[])),
        ];
        for (input, expected) in cases {
            assert_eq!(gnome_button_layout(input), expected, "{input:?}");
        }
    }

    #[test]
    fn gnome_action_table() {
        let cases = [
            ("toggle-maximize", A::ToggleMaximise),
            (
                "toggle-maximize-horizontally",
                A::ToggleMaximiseHorizontally,
            ),
            ("toggle-maximize-vertically", A::ToggleMaximiseVertically),
            ("toggle-shade", A::ToggleShade),
            ("shade", A::ToggleShade),
            ("minimize", A::Minimise),
            ("lower", A::Lower),
            ("menu", A::Menu),
            ("none", A::None),
            ("", A::None),
            ("something-new", A::None),
            (" menu ", A::Menu),
        ];
        for (input, expected) in cases {
            assert_eq!(gnome_action(input), expected, "{input:?}");
        }
    }

    #[test]
    fn gnome_actions_fall_back_per_field() {
        assert_eq!(gnome_actions(None, None, None), TitlebarActions::DEFAULT);
        assert_eq!(
            gnome_actions(Some("lower"), Some("menu"), None),
            TitlebarActions {
                double_click: A::Lower,
                middle_click: A::Menu,
                right_click: A::Menu,
            }
        );
    }

    #[test]
    fn kde_layout_table() {
        let cases = [
            (
                None,
                None,
                layout(
                    &[B::WindowMenu, B::Stick],
                    &[B::Help, B::Minimise, B::Maximise, B::Close],
                ),
            ),
            (
                Some("MS"),
                Some("HIAX"),
                layout(
                    &[B::WindowMenu, B::Stick],
                    &[B::Help, B::Minimise, B::Maximise, B::Close],
                ),
            ),
            (
                Some(""),
                Some("IAX"),
                layout(&[], &[B::Minimise, B::Maximise, B::Close]),
            ),
            (
                Some("XIA"),
                Some(""),
                layout(&[B::Close, B::Minimise, B::Maximise], &[]),
            ),
            (Some(""), Some(""), layout(&[], &[])),
            (
                Some("N_M"),
                Some("FBL_X"),
                layout(
                    &[B::AppMenu, B::WindowMenu],
                    &[B::KeepAbove, B::KeepBelow, B::Shade, B::Close],
                ),
            ),
            (
                Some("Z?M"),
                Some("X"),
                layout(&[B::WindowMenu], &[B::Close]),
            ),
            (
                Some(" M "),
                Some("XX"),
                layout(&[B::WindowMenu], &[B::Close]),
            ),
            // Duplicates keep the first occurrence, across both sides.
            (
                Some("XM"),
                Some("MX"),
                layout(&[B::Close, B::WindowMenu], &[]),
            ),
        ];
        for (left, right, expected) in cases {
            assert_eq!(
                kde_button_layout(left, right),
                expected,
                "{left:?} {right:?}"
            );
        }
    }

    #[test]
    fn kde_double_click_table() {
        let cases = [
            ("Maximize", A::ToggleMaximise),
            ("Maximize (horizontal only)", A::ToggleMaximiseHorizontally),
            ("Maximize (vertical only)", A::ToggleMaximiseVertically),
            ("Shade", A::ToggleShade),
            ("Lower", A::Lower),
            ("Minimize", A::Minimise),
            ("Close", A::Close),
            ("", A::None),
        ];
        for (input, expected) in cases {
            assert_eq!(kde_double_click(input), expected, "{input:?}");
        }
    }

    #[test]
    fn kde_titlebar_command_table() {
        let cases = [
            ("Nothing", A::None),
            ("Lower", A::Lower),
            ("Toggle raise and lower", A::ToggleRaiseLower),
            ("Minimize", A::Minimise),
            ("Shade", A::ToggleShade),
            ("Operations menu", A::Menu),
            ("Maximize", A::ToggleMaximise),
            ("Maximize (horizontal only)", A::ToggleMaximiseHorizontally),
            ("Maximize (vertical only)", A::ToggleMaximiseVertically),
            ("Close", A::Close),
            ("", A::None),
        ];
        for (input, expected) in cases {
            assert_eq!(kde_titlebar_command(input), expected, "{input:?}");
        }
    }

    const KWINRC: &str = "\
[$Version]
update_info=kwin.upd:1

[Windows]
TitlebarDoubleClickCommand=Shade
CommandActiveTitlebar2=Lower
; a comment
CommandActiveTitlebar3 = Nothing

[org.kde.kdecoration2]
ButtonsOnLeft=NM
ButtonsOnRight = IAX
";

    #[test]
    fn kwinrc_is_parsed_end_to_end() {
        assert_eq!(
            kde_layout_from_kwinrc(KWINRC),
            layout(
                &[B::AppMenu, B::WindowMenu],
                &[B::Minimise, B::Maximise, B::Close]
            )
        );
        assert_eq!(
            kde_actions(KWINRC),
            TitlebarActions {
                double_click: A::ToggleShade,
                middle_click: A::Lower,
                right_click: A::None,
            }
        );
    }

    #[test]
    fn empty_kwinrc_uses_kde_defaults() {
        assert_eq!(
            kde_layout_from_kwinrc(""),
            layout(
                &[B::WindowMenu, B::Stick],
                &[B::Help, B::Minimise, B::Maximise, B::Close]
            )
        );
        assert_eq!(kde_actions(""), TitlebarActions::DEFAULT);
    }

    #[test]
    fn ini_value_respects_sections() {
        let text = "[A]\nk=1\n[B]\nk=2\nother=3\n";
        assert_eq!(ini_value(text, "A", "k").as_deref(), Some("1"));
        assert_eq!(ini_value(text, "B", "k").as_deref(), Some("2"));
        assert_eq!(ini_value(text, "B", "other").as_deref(), Some("3"));
        assert_eq!(ini_value(text, "A", "other"), None);
        assert_eq!(ini_value(text, "C", "k"), None);
        assert_eq!(ini_value("k=1", "A", "k"), None);
        assert_eq!(ini_value("[A]\nk=\n", "A", "k").as_deref(), Some(""));
        assert_eq!(ini_value("[A]\nk=a=b\n", "A", "k").as_deref(), Some("a=b"));
    }

    #[test]
    fn xfce_layout_table() {
        let cases = [
            (
                "O|HMC",
                layout(&[B::WindowMenu], &[B::Minimise, B::Maximise, B::Close]),
            ),
            ("|HMC", layout(&[], &[B::Minimise, B::Maximise, B::Close])),
            ("CHM|", layout(&[B::Close, B::Minimise, B::Maximise], &[])),
            ("|", layout(&[], &[])),
            ("", layout(&[], &[])),
            // No bar puts everything at the start.
            (
                "OTSHMC",
                layout(
                    &[
                        B::WindowMenu,
                        B::Stick,
                        B::Shade,
                        B::Minimise,
                        B::Maximise,
                        B::Close,
                    ],
                    &[],
                ),
            ),
            (
                "O_T|_HC",
                layout(&[B::WindowMenu, B::Stick], &[B::Minimise, B::Close]),
            ),
            ("Z?O|X", layout(&[B::WindowMenu], &[])),
            ("OO|OC", layout(&[B::WindowMenu], &[B::Close])),
            (
                " O | HMC \n",
                layout(&[B::WindowMenu], &[B::Minimise, B::Maximise, B::Close]),
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(xfce_button_layout(input), expected, "{input:?}");
        }
    }

    #[test]
    fn xfce_double_click_table() {
        let cases = [
            ("maximize", A::ToggleMaximise),
            ("shade", A::ToggleShade),
            ("hide", A::Minimise),
            ("above", A::None),
            ("below", A::None),
            ("fullscreen", A::None),
            ("none", A::None),
            ("", A::None),
        ];
        for (input, expected) in cases {
            assert_eq!(xfce_double_click(input), expected, "{input:?}");
        }
    }

    #[test]
    fn macos_double_click_table() {
        let cases = [
            (Some("Maximize"), A::ToggleMaximise),
            (Some("Minimize"), A::Minimise),
            (Some("None"), A::None),
            (Some("minimize\n"), A::Minimise),
            (None, A::ToggleMaximise),
            (Some("Bogus"), A::ToggleMaximise),
            (Some(""), A::ToggleMaximise),
        ];
        for (input, expected) in cases {
            assert_eq!(macos_double_click(input), expected, "{input:?}");
        }
    }

    #[test]
    fn fixed_platform_presets() {
        let windows = windows_preferences();
        assert_eq!(
            windows.button_layout,
            layout(&[], &[B::Minimise, B::Maximise, B::Close])
        );
        assert_eq!(windows.actions, TitlebarActions::DEFAULT);
        assert_eq!(windows.desktop_environment, DesktopEnvironment::Windows);
        assert_eq!(windows.source, LayoutSource::Platform);

        let macos = macos_preferences(A::Minimise);
        assert_eq!(
            macos.button_layout,
            layout(&[B::Close, B::Minimise, B::Maximise], &[])
        );
        assert_eq!(macos.actions.double_click, A::Minimise);
        assert_eq!(macos.actions.middle_click, A::None);
        assert_eq!(macos.actions.right_click, A::Menu);
        assert_eq!(macos.desktop_environment, DesktopEnvironment::Macos);
        assert_eq!(macos.source, LayoutSource::Platform);
    }
}
