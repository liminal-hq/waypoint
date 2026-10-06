// Reads and watches the palette on Windows through UISettings and, in a high-contrast theme, GetSysColor
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::cell::Cell;

use tauri::{AppHandle, Runtime};
use tokio::sync::mpsc::UnboundedSender;
use windows::{
    core::{w, IInspectable},
    Foundation::TypedEventHandler,
    Win32::{
        Foundation::ERROR_SUCCESS,
        Graphics::Gdi::{
            GetSysColor, COLOR_ACTIVECAPTION, COLOR_BTNFACE, COLOR_GRADIENTACTIVECAPTION,
            COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_HOTLIGHT, COLOR_WINDOW, COLOR_WINDOWFRAME,
            COLOR_WINDOWTEXT, SYS_COLOR_INDEX,
        },
        System::{
            Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
            WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
        },
    },
    UI::ViewManagement::{AccessibilitySettings, UIColorType, UISettings},
};

use super::{
    models::{PaletteColour, PaletteColours, PaletteEntry, PaletteSource},
    parse,
};
use crate::{
    appearance::models::UnavailableReason, appearance::parse as appearance, service::Readiness,
};

/// Joins the apartment of a thread that will call WinRT. A thread that already has one keeps it.
fn ensure_winrt() {
    thread_local! {
        static DONE: Cell<bool> = const { Cell::new(false) };
    }
    DONE.with(|done| {
        if !done.get() {
            // SAFETY: initialises COM for the calling thread only; a thread that already did so
            // with another model makes this fail, which leaves its own apartment in place.
            let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
            done.set(true);
        }
    });
}

fn sys_colour(index: SYS_COLOR_INDEX) -> String {
    // SAFETY: `GetSysColor` takes an index and returns a value; it touches no memory of ours.
    parse::windows_colorref(unsafe { GetSysColor(index) })
}

/// Whether a high-contrast theme is in force, in which `GetSysColor` holds the user's own colours.
fn high_contrast() -> bool {
    AccessibilitySettings::new()
        .and_then(|settings| settings.HighContrast())
        .unwrap_or(false)
}

fn ui_colour(settings: &UISettings, kind: UIColorType) -> Result<String, String> {
    settings
        .GetColorValue(kind)
        .map(|colour| appearance::hex(colour.R, colour.G, colour.B))
        .map_err(|error| error.to_string())
}

/// Whether the user asked for the accent colour on title bars (Settings, Personalisation,
/// Colours). Without it Windows 11 draws the title bar in the theme's own surface colour, which
/// no API reports apart from the window background, so the title bar stays flat.
fn accent_on_title_bars() -> bool {
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: `data` and `size` outlive the call, and `size` is the byte length of `data`.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\DWM"),
            w!("ColorPrevalence"),
            RRF_RT_REG_DWORD,
            None,
            Some(std::ptr::addr_of_mut!(data).cast()),
            Some(&mut size),
        )
    };
    status == ERROR_SUCCESS && data == 1
}

fn read_blocking() -> PaletteColours {
    ensure_winrt();
    let mut colours = PaletteColours::unavailable(
        UnavailableReason::SourceMissing,
        "Windows has no such colour outside a high-contrast theme",
    );
    match UISettings::new() {
        Ok(settings) => {
            for (target, kind) in [
                (PaletteColour::WindowBackground, UIColorType::Background),
                (PaletteColour::ViewBackground, UIColorType::Background),
                (PaletteColour::WindowForeground, UIColorType::Foreground),
                (PaletteColour::ViewForeground, UIColorType::Foreground),
                (PaletteColour::SelectionBackground, UIColorType::Accent),
                (PaletteColour::Focus, UIColorType::Accent),
            ] {
                *colours.entry_mut(target) = match ui_colour(&settings, kind) {
                    Ok(hex) => PaletteEntry::found(hex, PaletteSource::UiSettings),
                    Err(error) => PaletteEntry::missing(UnavailableReason::ReadFailed, error),
                };
            }
            if accent_on_title_bars() {
                if let Ok(hex) = ui_colour(&settings, UIColorType::Accent) {
                    colours.set(
                        PaletteColour::TitleBarBackground,
                        hex,
                        PaletteSource::UiSettings,
                    );
                }
            }
        }
        Err(error) => {
            return PaletteColours::unavailable(UnavailableReason::ReadFailed, &error.to_string())
        }
    }
    if high_contrast() {
        for (target, index) in [
            (PaletteColour::WindowBackground, COLOR_WINDOW),
            (PaletteColour::ViewBackground, COLOR_WINDOW),
            (PaletteColour::WindowForeground, COLOR_WINDOWTEXT),
            (PaletteColour::ViewForeground, COLOR_WINDOWTEXT),
            (PaletteColour::SurfaceBackground, COLOR_BTNFACE),
            (PaletteColour::SelectionBackground, COLOR_HIGHLIGHT),
            (PaletteColour::SelectionForeground, COLOR_HIGHLIGHTTEXT),
            (PaletteColour::Border, COLOR_WINDOWFRAME),
            (PaletteColour::Focus, COLOR_HOTLIGHT),
            (PaletteColour::TitleBarBackground, COLOR_ACTIVECAPTION),
            (
                PaletteColour::TitleBarBackgroundEnd,
                COLOR_GRADIENTACTIVECAPTION,
            ),
        ] {
            colours.set(target, sys_colour(index), PaletteSource::SysColor);
        }
    }
    colours
}

pub async fn read<R: Runtime>(_app: &AppHandle<R>) -> PaletteColours {
    tauri::async_runtime::spawn_blocking(read_blocking)
        .await
        .unwrap_or_else(|error| {
            PaletteColours::unavailable(UnavailableReason::ReadFailed, &error.to_string())
        })
}

/// The change handlers registered on the WinRT settings objects, removed when dropped.
pub struct Watcher {
    ui: Option<(UISettings, i64)>,
    accessibility: Option<(AccessibilitySettings, i64)>,
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
        if let Some((settings, token)) = self.ui.take() {
            let _ = settings.RemoveColorValuesChanged(token);
        }
        if let Some((settings, token)) = self.accessibility.take() {
            let _ = settings.RemoveHighContrastChanged(token);
        }
    }
}

fn notify(changed: &UnboundedSender<()>) -> TypedEventHandler<UISettings, IInspectable> {
    let changed = changed.clone();
    TypedEventHandler::new(move |_, _| {
        let _ = changed.send(());
        Ok(())
    })
}

pub fn watch<R: Runtime>(_app: &AppHandle<R>, changed: UnboundedSender<()>) -> Watcher {
    ensure_winrt();
    let ui = UISettings::new()
        .and_then(|settings| {
            let token = settings.ColorValuesChanged(&notify(&changed))?;
            Ok((settings, token))
        })
        .map_err(|error| log::warn!("cannot listen to UISettings colour changes: {error}"))
        .ok();
    let accessibility = AccessibilitySettings::new()
        .and_then(|settings| {
            let changed = changed.clone();
            let token = settings.HighContrastChanged(&TypedEventHandler::<
                AccessibilitySettings,
                IInspectable,
            >::new(move |_, _| {
                let _ = changed.send(());
                Ok(())
            }))?;
            Ok((settings, token))
        })
        .map_err(|error| log::warn!("cannot listen to high-contrast changes: {error}"))
        .ok();
    Watcher { ui, accessibility }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live read of this machine's palette. Read-only. Run with
    /// `cargo test live_palette_read -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_palette_read() {
        println!(
            "{}",
            serde_json::to_string_pretty(&read_blocking()).unwrap()
        );
    }
}
