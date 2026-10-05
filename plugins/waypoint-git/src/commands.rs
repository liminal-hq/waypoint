// The Git plugin's commands: watch a folder's repository, stop, and ask for sidebar badges
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::{Emitter, Runtime, State, Window};
use waypoint_path::VfsPath;
use waypoint_protocol::{
    GitBadge, GitChanged, GitCommit, GitDiffStat, GitPathInfo, GitWatch, Location, PluginStatus,
    VfsError,
};
use waypoint_provider_git::{TrackEvent, TrackSink};

use crate::error::Error;
use crate::state::Git;
use crate::summary::wire;
use crate::CHANGED_EVENT;

/// A local folder's path, or `None` for a location that is not one (a revision, a server).
fn local_folder(location: &Location) -> Result<Option<PathBuf>, VfsError> {
    let path = VfsPath::from_location(location).map_err(|_| VfsError::InvalidLocation {
        input: location.uri.clone(),
    })?;
    Ok(match path {
        VfsPath::File(file) => Some(file.into_path_buf()),
        _ => None,
    })
}

/// What the plugin can do here: `status` and `watch` (or `polling-fallback` while a repository is
/// polled because the system cannot watch it) while it is on, and unavailable, with the reason,
/// while the Settings switch has it off.
#[tauri::command]
pub async fn get_status(state: State<'_, Git>) -> Result<PluginStatus, Error> {
    if !state.enabled() {
        return Ok(PluginStatus::unavailable(
            "Git status is turned off in Settings",
        ));
    }
    let mut features = vec![
        "status".to_owned(),
        "branch".to_owned(),
        "revisions".to_owned(),
    ];
    match state.degraded() {
        Some(_) => features.push("polling-fallback".to_owned()),
        None => features.push("watch".to_owned()),
    }
    let mut status = PluginStatus::available(features);
    status.reason = state.degraded();
    Ok(status)
}

/// Starts telling `window` about the repository that holds `location`, and replies with where it
/// stands. `None` when the folder is not in a working tree (or is not a local folder), or the
/// switch is off: there is nothing to watch, and the page shows no Git. Changes follow as
/// `waypoint-git://changed` events to the window.
#[tauri::command]
pub async fn git_watch<R: Runtime>(
    window: Window<R>,
    state: State<'_, Git>,
    location: Location,
) -> Result<Option<GitWatch>, Error> {
    if !state.enabled() {
        return Ok(None);
    }
    let Some(folder) = local_folder(&location)? else {
        return Ok(None);
    };
    let Some(root) = state.service.repository_of(&folder) else {
        return Ok(None);
    };
    let label = window.label().to_owned();
    // The id is known only once the watch exists, and the first events can come before then.
    let id_slot: Arc<Mutex<Option<u32>>> = Arc::default();
    let degraded = state.degraded_slot();
    let sink: TrackSink = {
        let id_slot = Arc::clone(&id_slot);
        let window = window.clone();
        Arc::new(move |event| match event {
            TrackEvent::Updated(snapshot) => {
                let Some(id) = *id_slot.lock().unwrap_or_else(|e| e.into_inner()) else {
                    return;
                };
                let changed = GitChanged {
                    id,
                    revision: snapshot.revision,
                    summary: wire(&snapshot.summary),
                };
                if let Err(error) = window.emit_to(window.label(), CHANGED_EVENT, changed) {
                    log::warn!(
                        "could not tell window={} about Git: {error}",
                        window.label()
                    );
                }
            }
            TrackEvent::Degraded { reason } => {
                *degraded.lock().unwrap_or_else(|e| e.into_inner()) = Some(reason);
            }
            TrackEvent::Failed { message } => {
                log::debug!("git status failed: {message}");
            }
        })
    };
    let subscription = state.service.subscribe(&root, sink);
    let snapshot = subscription.snapshot();
    let id = state.add_watch(&label, subscription);
    *id_slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
    let name = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string());
    let root = waypoint_path::FilePath::from_path(&root)
        .map(|path| path.to_location())
        .map_err(|_| VfsError::InvalidLocation {
            input: root.display().to_string(),
        })?;
    Ok(Some(GitWatch {
        id,
        root,
        name,
        summary: snapshot.as_ref().map(|s| wire(&s.summary)),
        revision: snapshot.map_or(0, |s| s.revision),
    }))
}

/// Stops one watch. Stopping one that is already gone is quiet.
#[tauri::command]
pub async fn git_unwatch<R: Runtime>(
    window: Window<R>,
    state: State<'_, Git>,
    id: u32,
) -> Result<(), Error> {
    state.remove_watch(window.label(), id);
    Ok(())
}

/// How many paths have changed inside each of `locations` that is a folder in a working tree, for
/// the badges the sidebar draws. A folder with nothing changed, outside a repository or not local
/// has no entry. Repositories that are not being watched are read once and the reading is kept for
/// a few seconds.
#[tauri::command]
pub async fn git_badges(
    state: State<'_, Git>,
    locations: Vec<Location>,
) -> Result<Vec<GitBadge>, Error> {
    if !state.enabled() {
        return Ok(Vec::new());
    }
    let mut work: Vec<(String, PathBuf, PathBuf)> = Vec::new();
    for location in &locations {
        let Some(folder) = local_folder(location)? else {
            continue;
        };
        if let Some(root) = state.service.repository_of(&folder) {
            work.push((location.uri.clone(), folder, root));
        }
    }
    let source = state.badge_source();
    tauri::async_runtime::spawn_blocking(move || {
        work.into_iter()
            .filter_map(|(uri, folder, root)| {
                let status = source.status(&root)?;
                let rel = folder.strip_prefix(&root).ok()?;
                let badge = status.badge(&waypoint_provider_git::rel_bytes(rel))?;
                Some(GitBadge {
                    uri,
                    changed: badge.changed,
                    conflicted: badge.conflicted,
                })
            })
            .collect()
    })
    .await
    .map_err(|error| Error::Internal(error.to_string()))
}

/// The newest commits that changed a file or folder and how much of it has changed since `HEAD`,
/// for the Inspector's Git tab. `None` when `location` is not in a working tree (or not local) or
/// Git is off. The path is read on a blocking thread and the search is bounded
/// (`waypoint_provider_git::SCAN_CAP` commits, `DIFF_FILE_CAP` files).
#[tauri::command]
pub async fn git_path_info(
    state: State<'_, Git>,
    location: Location,
    limit: Option<u32>,
) -> Result<Option<GitPathInfo>, Error> {
    if !state.enabled() {
        return Ok(None);
    }
    let Some(folder) = local_folder(&location)? else {
        return Ok(None);
    };
    let Some(root) = state.service.repository_of(&folder) else {
        return Ok(None);
    };
    let source = state.badge_source();
    let limit = limit.unwrap_or(5).clamp(1, 50) as usize;
    tauri::async_runtime::spawn_blocking(move || {
        let rel = folder
            .strip_prefix(&root)
            .map(waypoint_provider_git::rel_bytes)
            .unwrap_or_default();
        let cancel = waypoint_vfs::CancelToken::new();
        let found = waypoint_provider_git::history(&root, &rel, limit, &cancel)
            .map_err(|error| Error::from(crate::state::failure(&location, &error)))?;
        let diff = match source.status(&root) {
            Some(status) => {
                waypoint_provider_git::diff_stat(&root, &status, &rel, &cancel).unwrap_or_default()
            }
            None => Default::default(),
        };
        Ok(Some(GitPathInfo {
            commits: found
                .commits
                .into_iter()
                .map(|c| GitCommit {
                    short: c.id.chars().take(8).collect(),
                    id: c.id,
                    summary: c.summary,
                    author: c.author,
                    time_ms: c.time_ms,
                })
                .collect(),
            truncated: found.truncated,
            diff: GitDiffStat {
                files: diff.files,
                added: diff.added,
                removed: diff.removed,
                binary: diff.binary,
                partial: diff.partial,
            },
        }))
    })
    .await
    .map_err(|error| Error::Internal(error.to_string()))?
}
