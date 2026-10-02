// Reads and watches the appearance preferences through the xdg-desktop-portal Settings interface
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;

use ashpd::{
    desktop::settings::Settings,
    zvariant::{OwnedValue, Value},
};
use futures_util::StreamExt;
use tokio::sync::{mpsc::UnboundedSender, oneshot};

use crate::{
    appearance::{
        models::{AppearanceFeature, AppearanceSource, Contrast, UnavailableReason},
        parse,
        resolve::{Partial, SourceReading},
    },
    linux::portal::TaskGuard,
};

const APPEARANCE: &str = "org.freedesktop.appearance";
const GNOME_INTERFACE: &str = "org.gnome.desktop.interface";
const GNOME_A11Y: &str = "org.gnome.desktop.a11y.interface";

/// The namespaces read, and whose changes are watched.
const NAMESPACES: [&str; 3] = [APPEARANCE, GNOME_INTERFACE, GNOME_A11Y];

type Namespaces = HashMap<String, HashMap<String, OwnedValue>>;

/// Looks through the variant wrapping a portal value may arrive in.
fn unwrapped<'a, 'b>(value: &'a Value<'b>) -> &'a Value<'b> {
    match value {
        Value::Value(inner) => unwrapped(inner),
        other => other,
    }
}

fn as_u32(value: &Value<'_>) -> Option<u32> {
    match unwrapped(value) {
        Value::U32(number) => Some(*number),
        _ => None,
    }
}

fn as_bool(value: &Value<'_>) -> Option<bool> {
    match unwrapped(value) {
        Value::Bool(flag) => Some(*flag),
        _ => None,
    }
}

fn as_f64(value: &Value<'_>) -> Option<f64> {
    match unwrapped(value) {
        Value::F64(number) => Some(*number),
        _ => None,
    }
}

fn as_string(value: &Value<'_>) -> Option<String> {
    match unwrapped(value) {
        Value::Str(text) => Some(text.to_string()),
        _ => None,
    }
}

/// The three channels of the portal's `(ddd)` accent colour.
fn as_triple(value: &Value<'_>) -> Option<(f64, f64, f64)> {
    let Value::Structure(structure) = unwrapped(value) else {
        return None;
    };
    match structure.fields() {
        [red, green, blue] => Some((as_f64(red)?, as_f64(green)?, as_f64(blue)?)),
        _ => None,
    }
}

/// What the portal holds for one key: absent, present but not understood, or a value.
enum Found<T> {
    Absent,
    Unreadable,
    Value(T),
}

fn find<T>(
    namespaces: &Namespaces,
    namespace: &str,
    key: &str,
    parse: impl Fn(&Value<'_>) -> Option<T>,
) -> Found<T> {
    match namespaces.get(namespace).and_then(|keys| keys.get(key)) {
        None => Found::Absent,
        Some(value) => parse(value).map_or(Found::Unreadable, Found::Value),
    }
}

/// Returns the value of a standard-namespace key, recording why it is missing when it is not there.
fn record<T>(
    reading: &mut SourceReading,
    feature: AppearanceFeature,
    key: &str,
    found: Found<T>,
) -> Option<T> {
    match found {
        Found::Value(value) => Some(value),
        Found::Absent => {
            reading.miss(
                feature,
                UnavailableReason::SourceMissing,
                format!("the portal does not expose {APPEARANCE} {key}"),
            );
            None
        }
        Found::Unreadable => {
            reading.miss(
                feature,
                UnavailableReason::ReadFailed,
                format!("the portal's {APPEARANCE} {key} is not a value this plugin understands"),
            );
            None
        }
    }
}

/// Turns what the portal answered into a reading.
///
/// `org.freedesktop.appearance` is the standard source and comes first. The GNOME namespaces the
/// portal passes through fill the gaps (text scale and icon theme exist nowhere else, and older
/// portals lack `contrast` and `reduced-motion`), and every value taken from them is still a
/// portal value.
pub fn interpret(namespaces: &Namespaces) -> SourceReading {
    let mut reading = SourceReading::new(AppearanceSource::Portal);
    let mut standard = Partial::default();
    let mut gnome = Partial::default();

    if let Some(value) = record(
        &mut reading,
        AppearanceFeature::ColourScheme,
        "color-scheme",
        find(namespaces, APPEARANCE, "color-scheme", as_u32),
    ) {
        standard.colour_scheme = Some(parse::portal_colour_scheme(value));
    }
    if let Some((red, green, blue)) = record(
        &mut reading,
        AppearanceFeature::Accent,
        "accent-color",
        find(namespaces, APPEARANCE, "accent-color", as_triple),
    ) {
        standard.accent = parse::portal_accent(red, green, blue);
        if standard.accent.is_none() {
            reading.miss(
                AppearanceFeature::Accent,
                UnavailableReason::NoSource,
                "the desktop has no accent colour set",
            );
        }
    }
    if let Some(value) = record(
        &mut reading,
        AppearanceFeature::Contrast,
        "contrast",
        find(namespaces, APPEARANCE, "contrast", as_u32),
    ) {
        standard.contrast = Some(parse::portal_contrast(value));
    }
    if let Some(value) = record(
        &mut reading,
        AppearanceFeature::ReducedMotion,
        "reduced-motion",
        find(namespaces, APPEARANCE, "reduced-motion", as_u32),
    ) {
        standard.reduced_motion = Some(parse::portal_reduced_motion(value));
    }

    if let Found::Value(name) = find(namespaces, GNOME_INTERFACE, "color-scheme", as_string) {
        gnome.colour_scheme = parse::gsettings_colour_scheme(&name);
    }
    if let Found::Value(name) = find(namespaces, GNOME_INTERFACE, "accent-color", as_string) {
        gnome.accent = parse::gnome_accent_name(&name);
    }
    if let Found::Value(enabled) = find(namespaces, GNOME_INTERFACE, "enable-animations", as_bool) {
        gnome.reduced_motion = Some(!enabled);
    }
    if let Found::Value(scale) = find(namespaces, GNOME_INTERFACE, "text-scaling-factor", as_f64) {
        gnome.text_scale = parse::text_scale(scale);
    }
    if let Found::Value(name) = find(namespaces, GNOME_INTERFACE, "icon-theme", as_string) {
        gnome.icon_theme = parse::gsettings_name(&name);
    }
    if let Found::Value(high) = find(namespaces, GNOME_A11Y, "high-contrast", as_bool) {
        gnome.contrast = Some(if high {
            Contrast::More
        } else {
            Contrast::Normal
        });
    }

    reading.values = standard.or(gnome);
    // Features the portal passes through only on GNOME-family desktops, and one no desktop offers.
    for (feature, namespace, key) in [
        (
            AppearanceFeature::TextScale,
            GNOME_INTERFACE,
            "text-scaling-factor",
        ),
        (AppearanceFeature::IconTheme, GNOME_INTERFACE, "icon-theme"),
    ] {
        if !reading.values.has(feature) {
            reading.miss(
                feature,
                UnavailableReason::SourceMissing,
                format!("the portal does not pass through {namespace} {key}"),
            );
        }
    }
    reading.miss(
        AppearanceFeature::ReducedTransparency,
        UnavailableReason::NoSource,
        "the portal has no reduced-transparency setting",
    );
    reading
}

/// Reads every namespace this plugin uses from the portal in one call.
pub async fn read() -> SourceReading {
    let settings = match Settings::new().await {
        Ok(settings) => settings,
        Err(error) => {
            return SourceReading::failed(
                AppearanceSource::Portal,
                UnavailableReason::PortalUnavailable,
                error.to_string(),
            )
        }
    };
    match settings.read_all(&NAMESPACES).await {
        Ok(namespaces) => interpret(&namespaces),
        Err(error) => SourceReading::failed(
            AppearanceSource::Portal,
            UnavailableReason::PortalUnavailable,
            error.to_string(),
        ),
    }
}

/// Notifies on every portal `SettingChanged` signal in a namespace this plugin reads.
///
/// The second value resolves once the signal stream is installed, so the caller can read the
/// baseline only after a change can no longer slip past it. It resolves with an error, rather
/// than hanging, if the portal cannot be reached.
pub fn watch(changed: UnboundedSender<()>) -> (Vec<Box<dyn Send>>, oneshot::Receiver<()>) {
    let (ready_tx, ready_rx) = oneshot::channel();
    let task = tauri::async_runtime::spawn(async move {
        // The stream borrows the proxy, so the proxy lives as long as the task.
        let settings = match Settings::new().await {
            Ok(settings) => settings,
            Err(error) => {
                log::warn!("cannot listen to the portal for appearance changes: {error}");
                return;
            }
        };
        let mut stream = match settings.receive_setting_changed().await {
            Ok(stream) => Box::pin(stream),
            Err(error) => {
                log::warn!("cannot listen to the portal for appearance changes: {error}");
                return;
            }
        };
        let _ = ready_tx.send(());
        while let Some(setting) = stream.next().await {
            if NAMESPACES.contains(&setting.namespace()) && changed.send(()).is_err() {
                break;
            }
        }
    });
    (vec![Box::new(TaskGuard(task))], ready_rx)
}

#[cfg(test)]
mod tests {
    use ashpd::zvariant::StructureBuilder;

    use super::*;
    use crate::appearance::models::ColourScheme;

    fn owned(value: impl Into<Value<'static>>) -> OwnedValue {
        OwnedValue::try_from(value.into()).unwrap()
    }

    fn accent(red: f64, green: f64, blue: f64) -> OwnedValue {
        let structure = StructureBuilder::new()
            .add_field(red)
            .add_field(green)
            .add_field(blue)
            .build()
            .unwrap();
        owned(Value::Structure(structure))
    }

    fn namespace(entries: Vec<(&str, OwnedValue)>) -> HashMap<String, OwnedValue> {
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect()
    }

    fn gnome_like() -> Namespaces {
        let mut namespaces = Namespaces::new();
        namespaces.insert(
            APPEARANCE.to_string(),
            namespace(vec![
                ("color-scheme", owned(1u32)),
                ("contrast", owned(0u32)),
                ("reduced-motion", owned(0u32)),
                (
                    "accent-color",
                    accent(0.207_843_14, 0.517_647_1, 0.894_117_65),
                ),
            ]),
        );
        namespaces.insert(
            GNOME_INTERFACE.to_string(),
            namespace(vec![
                ("enable-animations", owned(true)),
                ("text-scaling-factor", owned(1.25f64)),
                ("icon-theme", owned("Mint-Y-Red")),
            ]),
        );
        namespaces
    }

    #[test]
    fn a_full_gnome_answer_resolves_every_portal_feature() {
        let reading = interpret(&gnome_like());
        let values = &reading.values;
        assert_eq!(values.colour_scheme, Some(ColourScheme::Dark));
        assert_eq!(values.accent.as_deref(), Some("#3584e4"));
        assert_eq!(values.contrast, Some(Contrast::Normal));
        assert_eq!(values.reduced_motion, Some(false));
        assert_eq!(values.text_scale, Some(1.25));
        assert_eq!(values.icon_theme.as_deref(), Some("Mint-Y-Red"));
        assert_eq!(values.reduced_transparency, None);
        assert_eq!(reading.source, AppearanceSource::Portal);
    }

    #[test]
    fn the_standard_namespace_wins_over_the_gnome_ones() {
        let mut namespaces = gnome_like();
        let appearance = namespaces.get_mut(APPEARANCE).unwrap();
        appearance.insert("reduced-motion".to_string(), owned(1u32));
        // The GNOME key says animations are on, which would mean no reduced motion.
        let reading = interpret(&namespaces);
        assert_eq!(reading.values.reduced_motion, Some(true));
    }

    #[test]
    fn gnome_keys_fill_what_an_older_portal_lacks() {
        let mut namespaces = Namespaces::new();
        namespaces.insert(
            APPEARANCE.to_string(),
            namespace(vec![("color-scheme", owned(2u32))]),
        );
        namespaces.insert(
            GNOME_INTERFACE.to_string(),
            namespace(vec![
                ("enable-animations", owned(false)),
                ("accent-color", owned("purple")),
            ]),
        );
        namespaces.insert(
            GNOME_A11Y.to_string(),
            namespace(vec![("high-contrast", owned(true))]),
        );
        let reading = interpret(&namespaces);
        assert_eq!(reading.values.colour_scheme, Some(ColourScheme::Light));
        assert_eq!(reading.values.reduced_motion, Some(true));
        assert_eq!(reading.values.contrast, Some(Contrast::More));
        assert_eq!(reading.values.accent.as_deref(), Some("#9141ac"));
    }

    #[test]
    fn a_missing_key_is_a_miss_with_the_key_named() {
        let mut namespaces = Namespaces::new();
        namespaces.insert(
            APPEARANCE.to_string(),
            namespace(vec![("color-scheme", owned(0u32))]),
        );
        let reading = interpret(&namespaces);
        let miss = reading
            .misses
            .iter()
            .find(|miss| miss.feature == AppearanceFeature::Contrast)
            .expect("contrast should be a miss");
        assert_eq!(miss.reason, UnavailableReason::SourceMissing);
        assert!(miss.detail.contains("contrast"), "{}", miss.detail);
        assert!(reading
            .misses
            .iter()
            .any(|m| m.feature == AppearanceFeature::IconTheme));
    }

    #[test]
    fn an_out_of_range_accent_means_none_is_set() {
        let mut namespaces = gnome_like();
        namespaces
            .get_mut(APPEARANCE)
            .unwrap()
            .insert("accent-color".to_string(), accent(-1.0, -1.0, -1.0));
        let reading = interpret(&namespaces);
        assert_eq!(reading.values.accent, None);
        assert!(reading
            .misses
            .iter()
            .any(|m| m.feature == AppearanceFeature::Accent
                && m.reason == UnavailableReason::NoSource));
    }

    #[test]
    fn a_value_of_the_wrong_type_is_unreadable() {
        let mut namespaces = Namespaces::new();
        namespaces.insert(
            APPEARANCE.to_string(),
            namespace(vec![("color-scheme", owned("dark"))]),
        );
        let reading = interpret(&namespaces);
        assert_eq!(reading.values.colour_scheme, None);
        let miss = reading
            .misses
            .iter()
            .find(|m| m.feature == AppearanceFeature::ColourScheme)
            .unwrap();
        assert_eq!(miss.reason, UnavailableReason::ReadFailed);
    }

    #[test]
    fn values_wrapped_in_variants_are_unwrapped() {
        let wrapped = owned(Value::Value(Box::new(Value::U32(1))));
        let mut namespaces = Namespaces::new();
        namespaces.insert(
            APPEARANCE.to_string(),
            namespace(vec![("color-scheme", wrapped)]),
        );
        assert_eq!(
            interpret(&namespaces).values.colour_scheme,
            Some(ColourScheme::Dark)
        );
    }

    /// Live smoke test against the real portal; run with `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_portal_appearance_read() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let reading = runtime.block_on(read());
        println!("{reading:#?}");
    }
}
