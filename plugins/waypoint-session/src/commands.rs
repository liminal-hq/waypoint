// Implements the IPC commands exposed by the session plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Every command acts on the calling window's session unless it names a target (`close_window`'s
// `target`, `move_tabs`' `to`). Each is one `Command` on the store, so it is atomic and its events
// reach the windows they belong to in revision order (see `Sessions::run`).

use tauri::{Manager, Runtime, State, WebviewWindow};
use waypoint_protocol::{Location, PluginStatus};
use waypoint_session::{
    Command, Geometry, GroupId, GroupSort, MoveTo, MoveWhat, Outcome, PairId, PairLayout,
    SessionError, SessionEvent, SessionSnapshot, TabColour, TabHints, TabId, ViewPrefs,
};

use crate::error::Error;
use crate::sessions::{move_target, Sessions};

fn run<R: Runtime>(
    window: &WebviewWindow<R>,
    sessions: &Sessions<R>,
    command: Command,
) -> Result<Outcome, Error> {
    sessions.run(window.app_handle(), window.label(), command)
}

/// The first event of the calling window that `pick` takes, or an internal error naming `what`.
fn from_events<R: Runtime, T>(
    window: &WebviewWindow<R>,
    outcome: &Outcome,
    what: &str,
    pick: impl Fn(&SessionEvent) -> Option<T>,
) -> Result<T, Error> {
    outcome
        .events_for(window.label())
        .find_map(pick)
        .ok_or_else(|| Error::Internal(format!("{what} produced no event")))
}

/// The calling window's whole session at its current revision. A main window the store does not
/// hold yet (the first run) is registered first.
#[tauri::command]
pub async fn get_snapshot<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
) -> Result<SessionSnapshot, Error> {
    sessions.snapshot(window.app_handle(), window.label())
}

// Tabs.

/// Opens a tab and returns its id. The first tab of a window is always activated.
#[tauri::command]
pub async fn open_tab<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    location: Location,
    after: Option<TabId>,
    activate: bool,
) -> Result<TabId, Error> {
    let outcome = run(
        &window,
        &sessions,
        Command::Open {
            location,
            after,
            activate,
        },
    )?;
    from_events(&window, &outcome, "opening a tab", |event| match event {
        SessionEvent::TabOpened { tab, .. } => Some(tab.id),
        _ => None,
    })
}

#[tauri::command]
pub async fn close_tab<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    tab: TabId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::Close { tab }).map(drop)
}

#[tauri::command]
pub async fn activate_tab<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    tab: TabId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::Activate { tab }).map(drop)
}

#[tauri::command]
pub async fn move_tab<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
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
    sessions: State<'_, Sessions<R>>,
    tab: TabId,
    location: Location,
) -> Result<(), Error> {
    run(&window, &sessions, Command::Navigate { tab, location }).map(drop)
}

#[tauri::command]
pub async fn back<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    tab: TabId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::Back { tab }).map(drop)
}

#[tauri::command]
pub async fn forward<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    tab: TabId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::Forward { tab }).map(drop)
}

/// Pins or unpins a tab (with its pair or group).
#[tauri::command]
pub async fn pin_tab<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    tab: TabId,
    pinned: bool,
) -> Result<(), Error> {
    run(&window, &sessions, Command::Pin { tab, pinned }).map(drop)
}

#[tauri::command]
pub async fn set_tab_colour<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    tab: TabId,
    colour: Option<TabColour>,
) -> Result<(), Error> {
    run(&window, &sessions, Command::SetColour { tab, colour }).map(drop)
}

#[tauri::command]
pub async fn set_tab_hints<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    tab: TabId,
    hints: TabHints,
) -> Result<(), Error> {
    run(&window, &sessions, Command::SetHints { tab, hints }).map(drop)
}

/// Brings a closed tab back (the newest when `tab` is `None`) and returns its id, or `None` when
/// there was nothing to reopen.
#[tauri::command]
pub async fn reopen_tab<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    tab: Option<TabId>,
) -> Result<Option<TabId>, Error> {
    let outcome = run(&window, &sessions, Command::Reopen { tab })?;
    Ok(outcome.events.iter().find_map(|e| match &e.event {
        SessionEvent::TabReopened { tab, .. } => Some(tab.id),
        _ => None,
    }))
}

// Groups.

/// Groups the tabs (and the rest of their pairs) and returns the new group's id.
#[tauri::command]
pub async fn create_group<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    tabs: Vec<TabId>,
    name: Option<String>,
) -> Result<GroupId, Error> {
    let outcome = run(&window, &sessions, Command::CreateGroup { tabs, name })?;
    from_events(&window, &outcome, "creating a group", |event| match event {
        SessionEvent::GroupCreated { group, .. } => Some(group.id),
        _ => None,
    })
}

#[tauri::command]
pub async fn add_to_group<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    tab: TabId,
    group: GroupId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::AddToGroup { tab, group }).map(drop)
}

#[tauri::command]
pub async fn remove_from_group<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    tab: TabId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::RemoveFromGroup { tab }).map(drop)
}

#[tauri::command]
pub async fn rename_group<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    group: GroupId,
    name: String,
) -> Result<(), Error> {
    run(&window, &sessions, Command::RenameGroup { group, name }).map(drop)
}

#[tauri::command]
pub async fn set_group_colour<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    group: GroupId,
    colour: Option<TabColour>,
) -> Result<(), Error> {
    run(
        &window,
        &sessions,
        Command::SetGroupColour { group, colour },
    )
    .map(drop)
}

#[tauri::command]
pub async fn set_group_collapsed<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    group: GroupId,
    collapsed: bool,
) -> Result<(), Error> {
    run(
        &window,
        &sessions,
        Command::SetGroupCollapsed { group, collapsed },
    )
    .map(drop)
}

/// Collapses every other group in the window.
#[tauri::command]
pub async fn collapse_other_groups<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    group: GroupId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::CollapseOthers { group }).map(drop)
}

#[tauri::command]
pub async fn sort_group<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    group: GroupId,
    by: GroupSort,
) -> Result<(), Error> {
    run(&window, &sessions, Command::SortGroup { group, by }).map(drop)
}

/// Copies a group after itself and returns the copy's id.
#[tauri::command]
pub async fn duplicate_group<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    group: GroupId,
) -> Result<GroupId, Error> {
    let outcome = run(&window, &sessions, Command::DuplicateGroup { group })?;
    from_events(
        &window,
        &outcome,
        "duplicating a group",
        |event| match event {
            SessionEvent::GroupCreated { group, .. } => Some(group.id),
            _ => None,
        },
    )
}

#[tauri::command]
pub async fn move_group<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    group: GroupId,
    index: u32,
) -> Result<(), Error> {
    run(
        &window,
        &sessions,
        Command::MoveGroup {
            group,
            index: index as usize,
        },
    )
    .map(drop)
}

#[tauri::command]
pub async fn ungroup<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    group: GroupId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::Ungroup { group }).map(drop)
}

#[tauri::command]
pub async fn close_group<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    group: GroupId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::CloseGroup { group }).map(drop)
}

// Pairs.

/// Pairs two or more tabs and returns the new pair's id.
#[tauri::command]
pub async fn join_pair<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    tabs: Vec<TabId>,
    layout: PairLayout,
) -> Result<PairId, Error> {
    let outcome = run(&window, &sessions, Command::JoinPair { tabs, layout })?;
    from_events(&window, &outcome, "joining a pair", |event| match event {
        SessionEvent::PairCreated { pair, .. } => Some(pair.id),
        _ => None,
    })
}

#[tauri::command]
pub async fn separate_pair<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    pair: PairId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::SeparatePair { pair }).map(drop)
}

#[tauri::command]
pub async fn set_pair_layout<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    pair: PairId,
    layout: PairLayout,
) -> Result<(), Error> {
    run(&window, &sessions, Command::SetPairLayout { pair, layout }).map(drop)
}

#[tauri::command]
pub async fn set_pair_sizes<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    pair: PairId,
    sizes: Vec<u32>,
) -> Result<(), Error> {
    run(&window, &sessions, Command::SetPairSizes { pair, sizes }).map(drop)
}

#[tauri::command]
pub async fn swap_panes<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    pair: PairId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::SwapPanes { pair }).map(drop)
}

/// Splits a tab into a pair with a copy of itself, or undoes such a split.
#[tauri::command]
pub async fn toggle_split<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    tab: TabId,
) -> Result<(), Error> {
    run(&window, &sessions, Command::ToggleSplit { tab }).map(drop)
}

// Windows.

/// Makes a window (with a first tab at `location` when given) and returns its label. The window
/// factory creates it; when that fails nothing changes and the command fails.
#[tauri::command]
pub async fn open_window<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    location: Option<Location>,
    geometry: Option<Geometry>,
) -> Result<String, Error> {
    let outcome = run(
        &window,
        &sessions,
        Command::OpenWindow { location, geometry },
    )?;
    outcome
        .windows_opened()
        .into_iter()
        .next()
        .ok_or_else(|| Error::Internal("opening a window produced no event".into()))
}

/// Closes a window's session (its tabs go to the closed list) and then its webview: the calling
/// window, or `target` when given. The store's change comes first, so a webview that cannot be
/// destroyed still leaves a consistent store.
#[tauri::command]
pub async fn close_window<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    target: Option<String>,
) -> Result<(), Error> {
    let label = target.unwrap_or_else(|| window.label().to_string());
    if sessions.with_store(|s| s.window(&label).is_none()) {
        return Err(SessionError::UnknownWindow(label).into());
    }
    // `Sessions::run` destroys the webview of every window the store closes.
    sessions.run(window.app_handle(), &label, Command::CloseWindow)?;
    Ok(())
}

#[tauri::command]
pub async fn set_geometry<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    geometry: Geometry,
) -> Result<(), Error> {
    run(&window, &sessions, Command::SetGeometry { geometry }).map(drop)
}

#[tauri::command]
pub async fn set_view<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    view: ViewPrefs,
) -> Result<(), Error> {
    run(&window, &sessions, Command::SetView { view }).map(drop)
}

/// Moves tabs, a group or a pair out of the calling window into an existing window or a new one
/// and returns the label of the window they went to. A new window is created by the window
/// factory, and when that fails nothing moves.
#[tauri::command]
pub async fn move_tabs<R: Runtime>(
    window: WebviewWindow<R>,
    sessions: State<'_, Sessions<R>>,
    what: MoveWhat,
    to: MoveTo,
) -> Result<String, Error> {
    if let MoveTo::NewWindow {
        label: Some(label), ..
    } = &to
    {
        // A window's label is the store's to allocate; a caller cannot pick one.
        return Err(Error::Internal(format!(
            "a new window cannot be given the label `{label}`"
        )));
    }
    let outcome = run(
        &window,
        &sessions,
        Command::MoveTabs {
            what,
            to: to.clone(),
        },
    )?;
    move_target(&to, &outcome)
        .ok_or_else(|| Error::Internal("moving tabs produced no target window".into()))
}

/// Session state is in memory and platform-independent, so the plugin is always available.
#[tauri::command]
pub async fn get_status() -> Result<PluginStatus, Error> {
    Ok(PluginStatus::available(
        [
            "tabs", "history", "pin", "colour", "closed", "mru", "groups", "pairs", "windows",
        ]
        .map(String::from)
        .to_vec(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_available() {
        let status = tauri::async_runtime::block_on(get_status()).unwrap();
        assert!(status.available);
        for feature in ["tabs", "groups", "pairs", "windows", "closed", "mru", "pin"] {
            assert!(status.features.contains(&feature.to_string()), "{feature}");
        }
    }
}
