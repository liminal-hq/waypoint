// The global shortcut's accelerator: the text a person types and what makes it acceptable
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The accepted words are the intersection of what the two registration paths understand, so a
// value that validates here registers on every platform that offers the shortcut: the Windows
// hotkey thread of `desktop-integration` and the X11 grab and Wayland portal path (Tauri's
// accelerator syntax). It is a pure function so the settings, the integration glue and the tests
// agree on one rule.

/// The accelerator used while none is set (D118 keeps the shortcut itself off until enabled).
pub const DEFAULT_ACCELERATOR: &str = "Ctrl+Alt+W";

/// The longest accelerator text.
pub const ACCELERATOR_MAX_LEN: usize = 64;

/// Why an accelerator was refused, as a short sentence for the row under the field.
pub const ACCELERATOR_HINT: &str = "write it like Ctrl+Alt+W";

/// The modifier words, in any letter case.
const MODIFIERS: [&str; 7] = [
    "ctrl",
    "control",
    "alt",
    "option",
    "shift",
    "super",
    "cmdorctrl",
];

/// Named keys besides letters, digits and F1 to F24.
const NAMED_KEYS: [&str; 15] = [
    "space",
    "enter",
    "tab",
    "escape",
    "backspace",
    "delete",
    "insert",
    "home",
    "end",
    "pageup",
    "pagedown",
    "up",
    "down",
    "left",
    "right",
];

/// Whether `word` (lower case) names a modifier.
fn is_modifier(word: &str) -> bool {
    MODIFIERS.contains(&word) || word == "commandorcontrol"
}

fn is_key(word: &str) -> bool {
    let mut chars = word.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return c.is_ascii_alphanumeric();
    }
    if let Some(number) = word.strip_prefix('f').and_then(|n| n.parse::<u32>().ok()) {
        return (1..=24).contains(&number) && !word[1..].starts_with('0');
    }
    NAMED_KEYS.contains(&word)
}

/// Whether `text` is an accelerator: one or more distinct modifiers and exactly one key, joined
/// by `+`, such as `Ctrl+Alt+W`, `Super+Space` or `CmdOrCtrl+Shift+F5`. A bare key is refused, as
/// it would take that key from every application. The key may not be a modifier, and the same
/// modifier twice (`Ctrl+Control+W`) is refused. Surrounding space around each word is allowed.
pub fn validate_accelerator(text: &str) -> Result<(), &'static str> {
    if text.trim().is_empty()
        || text.len() > ACCELERATOR_MAX_LEN
        || text.chars().any(char::is_control)
    {
        return Err(ACCELERATOR_HINT);
    }
    let lower = text.to_ascii_lowercase();
    let words: Vec<&str> = lower.split('+').map(str::trim).collect();
    // A word is never empty: `Ctrl+`, `+W` and `Ctrl++W` have an empty one.
    if words.iter().any(|word| word.is_empty()) {
        return Err(ACCELERATOR_HINT);
    }
    let Some((key, modifiers)) = words.split_last() else {
        return Err(ACCELERATOR_HINT);
    };
    if modifiers.is_empty() || !is_key(key) || !modifiers.iter().all(|word| is_modifier(word)) {
        return Err(ACCELERATOR_HINT);
    }
    // `Ctrl` and `Control` (or `CmdOrCtrl`) are one modifier, so they may not both appear.
    fn group(word: &str) -> &str {
        match word {
            "ctrl" | "control" | "cmdorctrl" | "commandorcontrol" => "ctrl",
            "alt" | "option" => "alt",
            other => other,
        }
    }
    let mut seen: Vec<&str> = Vec::new();
    for word in modifiers {
        let group = group(word);
        if seen.contains(&group) {
            return Err(ACCELERATOR_HINT);
        }
        seen.push(group);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_valid() {
        assert_eq!(validate_accelerator(DEFAULT_ACCELERATOR), Ok(()));
    }

    #[test]
    fn accepts_modifiers_with_one_key() {
        for good in [
            "Ctrl+Alt+W",
            "ctrl+alt+w",
            "Super+Space",
            "CmdOrCtrl+Shift+F5",
            "Alt+PageDown",
            "Ctrl+Shift+Left",
            "Control+1",
            "Ctrl + Alt + K",
            "Ctrl+F24",
            "CommandOrControl+Enter",
        ] {
            assert_eq!(validate_accelerator(good), Ok(()), "{good:?}");
        }
    }

    #[test]
    fn refuses_a_bare_key_or_only_modifiers() {
        for bad in ["W", "F5", "Space", "Ctrl", "Ctrl+Alt", "Shift+"] {
            assert!(validate_accelerator(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn refuses_unknown_and_doubled_words() {
        for bad in [
            "Ctrl+Nope",
            "Ctrl+A+B",
            "Ctrl+F25",
            "Ctrl+F0",
            "Ctrl+F05",
            "Ctrl+Control+W",
            "CmdOrCtrl+Ctrl+W",
            "Alt+Option+W",
            "Ctrl+Alt+Alt+W",
            "Ctrl++W",
            "+W",
            "Ctrl+Alt+W+",
            "W+Ctrl",
        ] {
            assert!(validate_accelerator(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn refuses_empty_long_and_control_characters() {
        assert!(validate_accelerator("").is_err());
        assert!(validate_accelerator("   ").is_err());
        assert!(
            validate_accelerator(&format!("Ctrl+{}", "a".repeat(ACCELERATOR_MAX_LEN))).is_err()
        );
        assert!(validate_accelerator("Ctrl+Alt+W\n").is_err());
        assert!(validate_accelerator("Ctrl+\u{0}W").is_err());
    }
}
