// Reads the hour format Apple platforms generate for the user's locale, shared by macOS and iOS
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use objc2_foundation::{NSDateFormatter, NSLocale, NSString};

use crate::parse;

/// The date format Foundation produces for the template `j`, which asks for the locale's
/// preferred hour format. It resolves with the user's "24-Hour Time" override from System
/// Settings (for example `h a` or `HH`). The auto-updating locale is used so that a long-running
/// process sees a change without restarting.
fn hour_pattern() -> Option<String> {
    let locale = NSLocale::autoupdatingCurrentLocale();
    let pattern = NSDateFormatter::dateFormatFromTemplate_options_locale(
        &NSString::from_str("j"),
        0,
        Some(&locale),
    )?;
    Some(pattern.to_string())
}

/// Whether the user's clock is 24-hour, or why it could not be told.
pub fn is_24_hour() -> Result<bool, String> {
    let pattern = hour_pattern().ok_or("Foundation returned no hour pattern")?;
    parse::macos_hour_template(&pattern)
        .ok_or_else(|| format!("the hour pattern {pattern:?} has no hour field"))
}
