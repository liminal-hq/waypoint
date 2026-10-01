// Reports the toplevel drag unavailable where there is no Wayland
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Runtime};

use super::State;
use crate::{
    models::{ToplevelBeginReport, ToplevelBeginState},
    status::{Platform, ToplevelProbe},
};

pub fn probe<R: Runtime>(_app: &AppHandle<R>, _platform: Platform) -> ToplevelProbe {
    ToplevelProbe::NotWayland
}

pub fn begin<R: Runtime>(
    _app: &AppHandle<R>,
    _state: &Arc<State>,
    _source: &str,
    _window: &str,
    _payload: &Value,
    _grab: (i32, i32),
    _mime: &str,
) -> ToplevelBeginReport {
    ToplevelBeginReport {
        state: ToplevelBeginState::Unavailable,
        reason: ToplevelProbe::NotWayland.reason().map(str::to_string),
    }
}

pub fn cancel() {}

pub fn install_drop_target<R: Runtime>(_window: &tauri::Window<R>, _mime: &str) {}
