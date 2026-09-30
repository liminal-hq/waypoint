// Implements the IPC commands exposed by the session plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{Emitter, Runtime, State, WebviewWindow};
use waypoint_protocol::{Location, PluginStatus};
use waypoint_session::{Command, SessionEvent, SessionSnapshot, TabId};

use crate::error::Error;
use crate::sessions::Sessions;
use crate::EVENT;

/// Applies a command to the calling window's session and emits each event it produced to that
/// window only. The lock is released before emitting.
fn run<R: Runtime>(
    window: &WebviewWindow<R>,
    sessions: &Sessions,
    command: Command,
) -> Result<Vec<SessionEvent>, Error> {
    let events = sessions.dispatch(window.label(), command)?;
    for event in &events {
        window
            .emit_to(window.label(), EVENT, event)
            .map_err(|e| Error::Internal(e.to_string()))?;
    }
    Ok(events)
}

/// The calling window's whole session at its current revision.
#[tauri::command]
pub async fn get_snapshot<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions>,
) -> Result<SessionSnapshot, Error> {
    Ok(sessions.snapshot(window.label()))
}

/// Opens a tab and returns its id. The first tab of a window is always activated.
#[tauri::command]
pub async fn open_tab<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions>,
    location: Location,
    after: Option<TabId>,
    activate: bool,
) -> Result<TabId, Error> {
    let events = run(
        &window,
        &sessions,
        Command::Open {
            location,
            after,
            activate,
        },
    )?;
    events
        .iter()
        .find_map(|event| match event {
            SessionEvent::TabOpened { tab, .. } => Some(tab.id),
            _ => None,
        })
        .ok_or_else(|| Error::Internal("opening a tab produced no event".into()))
}

#[tauri::command]
pub async fn close_tab<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions>,
    tab: TabId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::Close { tab }).map(drop)
}

#[tauri::command]
pub async fn activate_tab<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions>,
    tab: TabId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::Activate { tab }).map(drop)
}

#[tauri::command]
pub async fn move_tab<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions>,
    tab: TabId,
    index: u32,
) -> Result<(), Error> {
    run(
        &window,
        &sessions,
        Command::Move {
            tab,
            index: index as usize,
        },
    )
    .map(drop)
}

#[tauri::command]
pub async fn navigate<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions>,
    tab: TabId,
    location: Location,
) -> Result<(), Error> {
    run(&window, &sessions, Command::Navigate { tab, location }).map(drop)
}

#[tauri::command]
pub async fn back<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions>,
    tab: TabId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::Back { tab }).map(drop)
}

#[tauri::command]
pub async fn forward<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions>,
    tab: TabId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::Forward { tab }).map(drop)
}

/// Tab state is in memory and platform-independent, so the plugin is always available.
#[tauri::command]
pub async fn get_status() -> Result<PluginStatus, Error> {
    Ok(PluginStatus::available(vec![
        "tabs".to_string(),
        "history".to_string(),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_available() {
        let status = tauri::async_runtime::block_on(get_status()).unwrap();
        assert!(status.available);
        assert!(status.features.contains(&"tabs".to_string()));
    }
}
