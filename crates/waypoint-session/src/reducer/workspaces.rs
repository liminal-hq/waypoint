// Workspace commands: save a group as a workspace, rename, delete, switch and edit its folders.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashSet;

use waypoint_protocol::Location;

use crate::model::{Workspace, WorkspaceId};
use crate::reducer::{Command, SessionError};
use crate::store::{workspace_key, Store};

/// The name trimmed, or an error when nothing is left.
fn clean(name: &str) -> Result<String, SessionError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(SessionError::Invalid("a workspace needs a name"));
    }
    Ok(name.to_string())
}

fn position(store: &Store, workspace: WorkspaceId) -> Result<usize, SessionError> {
    store
        .workspaces
        .iter()
        .position(|w| w.id == workspace)
        .ok_or(SessionError::UnknownWorkspace(workspace.0))
}

/// Fails when another workspace (not `except`) already has this name.
fn ensure_free(store: &Store, name: &str, except: Option<WorkspaceId>) -> Result<(), SessionError> {
    let key = workspace_key(name);
    if store
        .workspaces
        .iter()
        .any(|w| Some(w.id) != except && workspace_key(&w.name) == key)
    {
        return Err(SessionError::WorkspaceNameTaken(name.to_string()));
    }
    Ok(())
}

fn unique(locations: impl IntoIterator<Item = Location>) -> Vec<Location> {
    let mut seen = HashSet::new();
    locations
        .into_iter()
        .filter(|l| seen.insert(l.uri.clone()))
        .collect()
}

pub(crate) fn apply(store: &mut Store, window: &str, command: Command) -> Result<(), SessionError> {
    // Workspaces are global but their events go to windows, so a command needs a live caller:
    // otherwise a change could happen with nobody told.
    let wi = store.window_index(window)?;
    match command {
        Command::SaveGroupAsWorkspace { group, name } => {
            let w = &store.windows[wi];
            let group = w
                .group(group)
                .ok_or(SessionError::UnknownGroup(group.0))?
                .clone();
            let name = clean(name.as_deref().unwrap_or(&group.name))?;
            ensure_free(store, &name, None)?;
            let locations = unique(
                w.tabs
                    .iter()
                    .filter(|t| t.group == Some(group.id))
                    .map(|t| t.location.clone()),
            );
            let id = WorkspaceId(store.next_workspace);
            store.next_workspace += 1;
            store.workspaces.push(Workspace {
                id,
                name,
                locations,
            });
        }
        Command::RenameWorkspace { workspace, name } => {
            let at = position(store, workspace)?;
            let name = clean(&name)?;
            ensure_free(store, &name, Some(workspace))?;
            store.workspaces[at].name = name;
        }
        Command::DeleteWorkspace { workspace } => {
            let at = position(store, workspace)?;
            store.workspaces.remove(at);
            // `settle` sends every window that had it back to the bookmarks.
        }
        Command::SetActiveWorkspace { workspace } => {
            if let Some(id) = workspace {
                position(store, id)?;
            }
            store.windows[wi].workspace = workspace;
        }
        Command::SetWorkspaceLocations {
            workspace,
            locations,
        } => {
            let at = position(store, workspace)?;
            store.workspaces[at].locations = unique(locations);
        }
        _ => unreachable!("routed by `reduce`"),
    }
    Ok(())
}
