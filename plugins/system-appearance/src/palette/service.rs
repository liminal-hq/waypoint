// Keeps the last-known palette and emits an event when it changes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{sync::Mutex, time::Duration};

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::sync::mpsc;

use super::{
    models::{Palette, PaletteColours},
    platform,
};
use crate::service::{lock, wait_until_ready, FollowUp, DEBOUNCE, READY_TIMEOUT, SETTLE};

/// Event emitted to all windows with the new `Palette` when it changes.
pub const PALETTE_CHANGED_EVENT: &str = "system-appearance://palette-changed";

/// A theme switch updates the toolkit's own colours a moment after the setting that announces it,
/// so a read straight after the notification can still see the old theme.
const THEME_SETTLE: Duration = Duration::from_millis(250);

/// Assigns `next` its revision given the previous palette, and says whether the colours changed
/// since a baseline existed. The first reading is revision 1 and only sets the baseline; the
/// revision increases exactly when a colour or its source differs from the previous reading.
pub fn advance(previous: Option<&Palette>, next: PaletteColours) -> (Palette, bool) {
    let (revision, changed) = match previous {
        None => (1, false),
        Some(previous) if previous.colours == next => (previous.revision, false),
        Some(previous) => (previous.revision + 1, true),
    };
    (
        Palette {
            revision,
            status: next.status(),
            colours: next,
        },
        changed,
    )
}

/// Plugin state for the palette: the last reading and the platform's watcher.
#[derive(Default)]
pub struct PaletteService {
    /// Serialises reads so a slow older reading cannot overwrite a newer one.
    gate: tokio::sync::Mutex<()>,
    last: Mutex<Option<Palette>>,
    watcher: Mutex<Option<platform::Watcher>>,
}

impl PaletteService {
    /// Reads the platform now, remembers the result and emits the change event, stamped with the
    /// new revision, if the colours differ from the previous reading. The first reading only sets
    /// the baseline.
    pub async fn refresh<R: Runtime>(&self, app: &AppHandle<R>) -> Palette {
        let _turn = self.gate.lock().await;
        let next = platform::read(app).await;
        let (palette, changed) = {
            let mut last = lock(&self.last);
            let (palette, changed) = advance(last.as_ref(), next);
            *last = Some(palette.clone());
            (palette, changed)
        };
        if changed {
            if let Err(error) = app.emit(PALETTE_CHANGED_EVENT, &palette) {
                log::warn!("failed to emit palette change event: {error}");
            }
        }
        palette
    }

    /// Starts watching the platform and re-reads whenever it reports a change.
    pub fn start<R: Runtime>(&self, app: &AppHandle<R>) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut watcher = platform::watch(app, tx);
        let readiness = watcher.take_readiness();
        *lock(&self.watcher) = Some(watcher);

        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            // Read the baseline only once the watcher is listening, so a change between the two
            // cannot be missed.
            let follow_up = wait_until_ready(readiness, READY_TIMEOUT).await;
            app.state::<PaletteService>().refresh(&app).await;
            match follow_up {
                FollowUp::Nothing => {}
                FollowUp::Settle => {
                    tokio::time::sleep(SETTLE).await;
                    app.state::<PaletteService>().refresh(&app).await;
                }
                FollowUp::AwaitListening(ready) => {
                    let _ = ready.await;
                    app.state::<PaletteService>().refresh(&app).await;
                }
            }
            while rx.recv().await.is_some() {
                tokio::time::sleep(DEBOUNCE + THEME_SETTLE).await;
                while rx.try_recv().is_ok() {}
                app.state::<PaletteService>().refresh(&app).await;
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
    use crate::{
        appearance::models::UnavailableReason,
        palette::models::{PaletteColour, PaletteSource},
    };

    fn palette(window: &str) -> PaletteColours {
        let mut colours = PaletteColours::unavailable(UnavailableReason::NoSource, "none");
        colours.set(
            PaletteColour::WindowBackground,
            window.to_string(),
            PaletteSource::GtkTheme,
        );
        colours.set(
            PaletteColour::WindowForeground,
            "#ffffff".to_string(),
            PaletteSource::GtkTheme,
        );
        colours
    }

    #[test]
    fn the_event_has_its_documented_name() {
        assert_eq!(PALETTE_CHANGED_EVENT, "system-appearance://palette-changed");
    }

    #[test]
    fn the_first_reading_is_revision_one_and_not_a_change() {
        let (first, changed) = advance(None, palette("#242424"));
        assert_eq!(first.revision, 1);
        assert!(!changed);
        assert!(first.status.available);
    }

    #[test]
    fn an_identical_reading_keeps_the_revision() {
        let (first, _) = advance(None, palette("#242424"));
        let (second, changed) = advance(Some(&first), palette("#242424"));
        assert_eq!(second.revision, 1);
        assert!(!changed);
    }

    #[test]
    fn a_different_colour_raises_the_revision() {
        let (first, _) = advance(None, palette("#242424"));
        let (second, changed) = advance(Some(&first), palette("#fafafa"));
        assert_eq!(second.revision, 2);
        assert!(changed);
    }

    #[test]
    fn the_wire_format_is_the_colours_plus_a_revision_and_a_status() {
        let (first, _) = advance(None, palette("#242424"));
        let json = serde_json::to_value(&first).unwrap();
        assert_eq!(json["revision"], 1);
        assert_eq!(json["status"]["available"], true);
        assert_eq!(json["status"]["source"], "gtkTheme");
        assert_eq!(json["windowBackground"]["colour"], "#242424");
        assert_eq!(json["border"]["reason"], "noSource");
    }
}
