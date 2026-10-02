// Keeps the last-known appearance preferences and emits an event when they change
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::sync::mpsc;

use super::{
    models::{AppearanceFeatureStatus, AppearancePreferences, AppearanceValues},
    platform,
    resolve::Resolution,
};
use crate::service::{lock, wait_until_ready, FollowUp, DEBOUNCE, READY_TIMEOUT, SETTLE};

/// Event emitted to all windows with the new `AppearancePreferences` when they change.
pub const APPEARANCE_CHANGED_EVENT: &str = "system-appearance://appearance-changed";

/// A reading of the platform with the revision it was assigned.
#[derive(Debug, Clone)]
pub struct Reading {
    pub preferences: AppearancePreferences,
    pub status: Vec<AppearanceFeatureStatus>,
}

/// Assigns `next` its revision given the previous reading, and says whether the values changed
/// since a baseline existed. The first reading is revision 1 and only sets the baseline; the
/// revision increases exactly when the values (or the sources that won) differ from the previous
/// reading. The availability list is always the latest.
pub fn advance(previous: Option<&Reading>, next: Resolution) -> (Reading, bool) {
    let (revision, changed) = revision_after(previous.map(|p| &p.preferences), &next.values);
    (
        Reading {
            preferences: AppearancePreferences {
                revision,
                values: next.values,
            },
            status: next.status,
        },
        changed,
    )
}

fn revision_after(
    previous: Option<&AppearancePreferences>,
    next: &AppearanceValues,
) -> (u32, bool) {
    match previous {
        None => (1, false),
        Some(previous) if &previous.values == next => (previous.revision, false),
        Some(previous) => (previous.revision + 1, true),
    }
}

/// Plugin state for the appearance preferences: the last reading and the platform's watcher.
#[derive(Default)]
pub struct AppearanceService {
    /// Serialises reads so a slow older reading cannot overwrite a newer one.
    gate: tokio::sync::Mutex<()>,
    last: Mutex<Option<Reading>>,
    watcher: Mutex<Option<platform::Watcher>>,
}

impl AppearanceService {
    /// Reads the platform now, remembers the result and emits the change event, stamped with the
    /// new revision, if the values differ from the previous reading. The first reading only sets
    /// the baseline.
    pub async fn refresh<R: Runtime>(&self, app: &AppHandle<R>) -> Reading {
        let _turn = self.gate.lock().await;
        let next = platform::read().await;
        let (reading, changed) = {
            let mut last = lock(&self.last);
            let (reading, changed) = advance(last.as_ref(), next);
            *last = Some(reading.clone());
            (reading, changed)
        };
        if changed {
            if let Err(error) = app.emit(APPEARANCE_CHANGED_EVENT, &reading.preferences) {
                log::warn!("failed to emit appearance change event: {error}");
            }
        }
        reading
    }

    /// Starts watching the platform and re-reads whenever it reports a change.
    pub fn start<R: Runtime>(&self, app: &AppHandle<R>) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut watcher = platform::watch(tx);
        let readiness = watcher.take_readiness();
        *lock(&self.watcher) = Some(watcher);

        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            // Read the baseline only once the watcher is listening, so a change between the two
            // cannot be missed.
            let follow_up = wait_until_ready(readiness, READY_TIMEOUT).await;
            app.state::<AppearanceService>().refresh(&app).await;
            match follow_up {
                FollowUp::Nothing => {}
                FollowUp::Settle => {
                    tokio::time::sleep(SETTLE).await;
                    app.state::<AppearanceService>().refresh(&app).await;
                }
                FollowUp::AwaitListening(ready) => {
                    let _ = ready.await;
                    app.state::<AppearanceService>().refresh(&app).await;
                }
            }
            while rx.recv().await.is_some() {
                tokio::time::sleep(DEBOUNCE).await;
                while rx.try_recv().is_ok() {}
                app.state::<AppearanceService>().refresh(&app).await;
            }
        });
    }

    /// Stops the platform watcher, releasing any child processes.
    pub fn stop(&self) {
        lock(&self.watcher).take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::models::{AppearanceSource, ColourScheme, Contrast};
    use crate::appearance::resolve::{resolve, Partial, SourceReading};

    fn resolution(scheme: ColourScheme, text_scale: f64) -> Resolution {
        let mut portal = SourceReading::new(AppearanceSource::Portal);
        portal.values = Partial {
            colour_scheme: Some(scheme),
            text_scale: Some(text_scale),
            ..Partial::default()
        };
        resolve(&[portal])
    }

    #[test]
    fn the_event_has_its_documented_name() {
        assert_eq!(
            APPEARANCE_CHANGED_EVENT,
            "system-appearance://appearance-changed"
        );
    }

    #[test]
    fn the_first_reading_is_revision_one_and_not_a_change() {
        let (reading, changed) = advance(None, resolution(ColourScheme::Dark, 1.0));
        assert_eq!(reading.preferences.revision, 1);
        assert!(!changed);
    }

    #[test]
    fn an_identical_reading_keeps_the_revision() {
        let (first, _) = advance(None, resolution(ColourScheme::Dark, 1.0));
        let (second, changed) = advance(Some(&first), resolution(ColourScheme::Dark, 1.0));
        assert_eq!(second.preferences.revision, 1);
        assert!(!changed);
    }

    #[test]
    fn each_different_reading_increases_the_revision() {
        let (first, _) = advance(None, resolution(ColourScheme::Dark, 1.0));
        let (second, changed) = advance(Some(&first), resolution(ColourScheme::Light, 1.0));
        assert_eq!(second.preferences.revision, 2);
        assert!(changed);
        let (third, changed) = advance(Some(&second), resolution(ColourScheme::Light, 1.25));
        assert_eq!(third.preferences.revision, 3);
        assert!(changed);
    }

    #[test]
    fn a_new_winning_source_is_a_change() {
        let (first, _) = advance(None, resolution(ColourScheme::Dark, 1.0));
        let mut gsettings = SourceReading::new(AppearanceSource::Gsettings);
        gsettings.values = Partial {
            colour_scheme: Some(ColourScheme::Dark),
            text_scale: Some(1.0),
            ..Partial::default()
        };
        let (second, changed) = advance(Some(&first), resolve(&[gsettings]));
        assert!(changed);
        assert_eq!(second.preferences.revision, 2);
    }

    #[test]
    fn the_wire_format_is_the_values_object_plus_a_revision() {
        let (reading, _) = advance(None, resolution(ColourScheme::Dark, 1.5));
        let json = serde_json::to_value(&reading.preferences).unwrap();
        assert_eq!(json["revision"], 1);
        assert_eq!(json["colourScheme"], "dark");
        assert_eq!(json["accent"], serde_json::Value::Null);
        assert_eq!(
            json["contrast"],
            serde_json::to_value(Contrast::Normal).unwrap()
        );
        assert_eq!(json["reducedMotion"], false);
        assert_eq!(json["reducedTransparency"], false);
        assert_eq!(json["textScale"], 1.5);
        assert_eq!(json["iconTheme"], serde_json::Value::Null);
        assert_eq!(json["sources"]["colourScheme"], "portal");
        assert_eq!(json["sources"]["accent"], serde_json::Value::Null);
    }
}
