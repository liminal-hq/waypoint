// The Windows backend: DWM's materials through Tauri's `set_effects`, and the build number through `RtlGetVersion`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::utils::config::WindowEffectsConfig;
use tauri::window::{Color, Effect, EffectsBuilder};
use tauri::{AppHandle, Runtime, Window};
use windows::Wdk::System::SystemServices::RtlGetVersion;
use windows::Win32::System::SystemInformation::OSVERSIONINFOW;

use crate::backend::{Backend, BoxFuture};
use crate::error::{Result, WindowEffectsError};
use crate::models::{Effects, Insets, Reason};
use crate::request::{windows_effect, WindowsEffect};
use crate::status::{Desktop, Environment, SessionType};

pub struct Platform;

impl Platform {
    pub fn new() -> Self {
        Platform
    }
}

/// The Windows build number. `GetVersionEx` reports Windows 11 as Windows 10 for an app with no manifest; `RtlGetVersion` does not.
pub fn build_number() -> Option<u32> {
    let mut info = OSVERSIONINFOW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is a valid `OSVERSIONINFOW` with its size set, which is all `RtlGetVersion` needs.
    let status = unsafe { RtlGetVersion(&mut info) };
    status.is_ok().then_some(info.dwBuildNumber)
}

pub fn probe_environment() -> Environment {
    Environment {
        session_type: SessionType::Windows,
        desktop: Desktop::Other,
        has_composite: true,
        wayland_globals: Vec::new(),
        build_number: build_number(),
    }
}

/// Tauri's description of a material.
fn config(effect: WindowsEffect) -> WindowEffectsConfig {
    let builder = EffectsBuilder::new();
    match effect {
        WindowsEffect::MicaDark => builder.effect(Effect::MicaDark),
        WindowsEffect::MicaLight => builder.effect(Effect::MicaLight),
        WindowsEffect::Acrylic { tint } => builder
            .effect(Effect::Acrylic)
            .color(Color(tint.0, tint.1, tint.2, tint.3)),
        WindowsEffect::Blur { tint } => builder
            .effect(Effect::Blur)
            .color(Color(tint.0, tint.1, tint.2, tint.3)),
    }
    .build()
}

fn failed(error: tauri::Error) -> WindowEffectsError {
    WindowEffectsError::failed(error.to_string())
}

impl<R: Runtime> Backend<R> for Platform {
    fn environment(&self, _app: &AppHandle<R>) -> BoxFuture<'_, Environment> {
        Box::pin(async { probe_environment() })
    }

    fn apply(
        &self,
        window: Window<R>,
        _env: Environment,
        effects: Effects,
    ) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let config = windows_effect(&effects).map(config);
            // Tauri runs this on the main thread and does not report DWM's own failure, so a rejection is the window going away.
            window.set_effects(config).map_err(failed)
        })
    }

    fn clear(&self, window: Window<R>, _env: Environment) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move { window.set_effects(None).map_err(failed) })
    }

    fn set_shadow_inset(&self, _window: Window<R>, _insets: Insets) -> BoxFuture<'_, Result<()>> {
        Box::pin(async {
            Err(WindowEffectsError::Unsupported {
                reason: Reason::GtkOnly,
                message: "the shadow inset is a GTK feature; Windows draws the shadow itself"
                    .to_string(),
            })
        })
    }
}
