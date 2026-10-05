// Reads and watches the appearance preferences on Windows through UISettings, AccessibilitySettings and the registry
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{cell::Cell, time::Duration};

use tokio::sync::mpsc::UnboundedSender;
use windows::{
    core::{w, IInspectable},
    Foundation::TypedEventHandler,
    Win32::{
        Foundation::ERROR_SUCCESS,
        System::{
            Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
            WinRT::{RoInitialize, RO_INIT_MULTITHREADED, RO_INIT_SINGLETHREADED, RO_INIT_TYPE},
        },
    },
    UI::ViewManagement::{
        AccessibilitySettings, UIColorType, UISettings, UISettingsAnimationsEnabledChangedEventArgs,
    },
};

use super::{
    models::{AppearanceFeature, AppearanceSource, Contrast, UnavailableReason},
    parse,
    resolve::{resolve, Resolution, SourceReading},
};
use crate::service::Readiness;

/// How often the preferences are read again, for changes no event announces (the registry's light
/// and dark setting among them). A tick that finds nothing new is silent.
const POLL_INTERVAL: Duration = Duration::from_secs(10);

/// Joins the apartment of a thread that will call WinRT. A thread that already has one keeps it.
///
/// `read_blocking` runs on a tokio blocking-pool thread with no message loop, so it joins the
/// multithreaded apartment; `watch` runs on the application's main thread, which `tao` later
/// initialises single-threaded for its window, so it must join that same apartment first or
/// `tao`'s `OleInitialize` fails with `RPC_E_CHANGED_MODE`.
fn ensure_winrt(model: RO_INIT_TYPE) {
    thread_local! {
        static DONE: Cell<bool> = const { Cell::new(false) };
    }
    DONE.with(|done| {
        if !done.get() {
            // SAFETY: initialises COM for the calling thread only; a thread that already did so
            // with another model makes this fail, which leaves its own apartment in place.
            let _ = unsafe { RoInitialize(model) };
            done.set(true);
        }
    });
}

fn fail(error: &windows::core::Error) -> (UnavailableReason, String) {
    (UnavailableReason::ReadFailed, error.to_string())
}

/// Reads `UISettings`: accent, animations, advanced effects and the text scale.
fn ui_settings() -> SourceReading {
    let mut reading = SourceReading::new(AppearanceSource::UiSettings);
    let settings = match UISettings::new() {
        Ok(settings) => settings,
        Err(error) => {
            return SourceReading::failed(
                AppearanceSource::UiSettings,
                UnavailableReason::ReadFailed,
                error.to_string(),
            )
        }
    };
    match settings.GetColorValue(UIColorType::Accent) {
        Ok(colour) => {
            reading.values.accent = Some(parse::windows_accent(colour.R, colour.G, colour.B))
        }
        Err(error) => {
            let (reason, detail) = fail(&error);
            reading.miss(AppearanceFeature::Accent, reason, detail);
        }
    }
    match settings.AnimationsEnabled() {
        Ok(enabled) => reading.values.reduced_motion = Some(!enabled),
        Err(error) => {
            let (reason, detail) = fail(&error);
            reading.miss(AppearanceFeature::ReducedMotion, reason, detail);
        }
    }
    // Without advanced effects Windows draws no transparency or blur.
    match settings.AdvancedEffectsEnabled() {
        Ok(enabled) => reading.values.reduced_transparency = Some(!enabled),
        Err(error) => {
            let (reason, detail) = fail(&error);
            reading.miss(AppearanceFeature::ReducedTransparency, reason, detail);
        }
    }
    match settings.TextScaleFactor() {
        Ok(factor) => match parse::text_scale(factor) {
            Some(scale) => reading.values.text_scale = Some(scale),
            None => reading.miss(
                AppearanceFeature::TextScale,
                UnavailableReason::ReadFailed,
                format!("UISettings reported a text scale of {factor}"),
            ),
        },
        Err(error) => {
            let (reason, detail) = fail(&error);
            reading.miss(AppearanceFeature::TextScale, reason, detail);
        }
    }
    reading.miss(
        AppearanceFeature::IconTheme,
        UnavailableReason::PlatformUnsupported,
        "Windows has no icon theme setting",
    );
    reading
}

/// Reads `AccessibilitySettings.HighContrast`.
fn accessibility() -> SourceReading {
    let mut reading = SourceReading::new(AppearanceSource::AccessibilitySettings);
    match AccessibilitySettings::new().and_then(|settings| settings.HighContrast()) {
        Ok(high) => {
            reading.values.contrast = Some(if high {
                Contrast::More
            } else {
                Contrast::Normal
            })
        }
        Err(error) => {
            let (reason, detail) = fail(&error);
            reading.miss(AppearanceFeature::Contrast, reason, detail);
        }
    }
    reading
}

/// Reads the light or dark setting for apps from the registry.
fn registry() -> SourceReading {
    let mut reading = SourceReading::new(AppearanceSource::Registry);
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: `data` and `size` outlive the call, and `size` is the byte length of `data`.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(std::ptr::addr_of_mut!(data).cast()),
            Some(&mut size),
        )
    };
    if status == ERROR_SUCCESS {
        reading.values.colour_scheme = Some(parse::windows_colour_scheme(data));
    } else {
        reading.miss(
            AppearanceFeature::ColourScheme,
            UnavailableReason::SourceMissing,
            format!("AppsUseLightTheme could not be read (error {})", status.0),
        );
    }
    reading
}

fn read_blocking() -> Resolution {
    ensure_winrt(RO_INIT_MULTITHREADED);
    resolve(&[registry(), ui_settings(), accessibility()])
}

pub async fn read() -> Resolution {
    tauri::async_runtime::spawn_blocking(read_blocking)
        .await
        .unwrap_or_else(|error| {
            resolve(&[SourceReading::failed(
                AppearanceSource::UiSettings,
                UnavailableReason::ReadFailed,
                error.to_string(),
            )])
        })
}

/// Aborts the polling task when dropped.
struct Poller(tauri::async_runtime::JoinHandle<()>);

impl Drop for Poller {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// The change handlers registered on the WinRT settings objects, removed when dropped.
pub struct Watcher {
    ui: Option<(UISettings, Vec<i64>)>,
    accessibility: Option<(AccessibilitySettings, i64)>,
    _poller: Poller,
}

// SAFETY: `UISettings` and `AccessibilitySettings` are agile WinRT objects, which may be used
// from any thread; the watcher only removes its handlers from them when dropped.
unsafe impl Send for Watcher {}

impl Watcher {
    /// Nothing is awaited: the handlers are registered before `watch` returns.
    pub fn take_readiness(&mut self) -> Readiness {
        Readiness::Listening
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        if let Some((settings, tokens)) = self.ui.take() {
            let [colours, text, effects, animations] = tokens[..] else {
                return;
            };
            let _ = settings.RemoveColorValuesChanged(colours);
            let _ = settings.RemoveTextScaleFactorChanged(text);
            let _ = settings.RemoveAdvancedEffectsEnabledChanged(effects);
            let _ = settings.RemoveAnimationsEnabledChanged(animations);
        }
        if let Some((settings, token)) = self.accessibility.take() {
            let _ = settings.RemoveHighContrastChanged(token);
        }
    }
}

fn watch_ui(changed: &UnboundedSender<()>) -> windows::core::Result<(UISettings, Vec<i64>)> {
    let settings = UISettings::new()?;
    let notify = |changed: &UnboundedSender<()>| {
        let changed = changed.clone();
        TypedEventHandler::<UISettings, IInspectable>::new(move |_, _| {
            let _ = changed.send(());
            Ok(())
        })
    };
    let colours = settings.ColorValuesChanged(&notify(changed))?;
    let text = settings.TextScaleFactorChanged(&notify(changed))?;
    let effects = settings.AdvancedEffectsEnabledChanged(&notify(changed))?;
    let animations = {
        let changed = changed.clone();
        settings.AnimationsEnabledChanged(&TypedEventHandler::<
            UISettings,
            UISettingsAnimationsEnabledChangedEventArgs,
        >::new(move |_, _| {
            let _ = changed.send(());
            Ok(())
        }))?
    };
    Ok((settings, vec![colours, text, effects, animations]))
}

fn watch_accessibility(
    changed: &UnboundedSender<()>,
) -> windows::core::Result<(AccessibilitySettings, i64)> {
    let settings = AccessibilitySettings::new()?;
    let changed = changed.clone();
    let token = settings.HighContrastChanged(&TypedEventHandler::<
        AccessibilitySettings,
        IInspectable,
    >::new(move |_, _| {
        let _ = changed.send(());
        Ok(())
    }))?;
    Ok((settings, token))
}

pub fn watch(changed: UnboundedSender<()>) -> Watcher {
    ensure_winrt(RO_INIT_SINGLETHREADED);
    let ui = watch_ui(&changed)
        .map_err(|error| log::warn!("cannot listen to UISettings changes: {error}"))
        .ok();
    let accessibility = watch_accessibility(&changed)
        .map_err(|error| log::warn!("cannot listen to high-contrast changes: {error}"))
        .ok();
    let poll = changed;
    let poller = Poller(tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(POLL_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // The first tick fires at once; the baseline read already covers it.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            if poll.send(()).is_err() {
                break;
            }
        }
    }));
    Watcher {
        ui,
        accessibility,
        _poller: poller,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live read of this machine's appearance preferences; prints what Windows reports every two
    /// seconds for `WATCH_SECONDS` (default 10), so the settings can be changed while it runs.
    /// Read-only. Run with `cargo test live_appearance_read -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_appearance_read() {
        let seconds: u64 = std::env::var("WATCH_SECONDS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(10);
        for _ in 0..=seconds / 2 {
            let resolution = read_blocking();
            println!(
                "{}",
                serde_json::to_string_pretty(&resolution.values).unwrap()
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&resolution.status).unwrap()
            );
            std::thread::sleep(Duration::from_secs(2));
        }
    }
}
