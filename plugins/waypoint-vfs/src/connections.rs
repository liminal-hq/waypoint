// The connection commands and events: saved connections, connecting with the person's answers, and the state of every login
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The plugin holds the `ConnectionsHub` (A81) beside the provider registry. A window edits the
// saved connections, connects, tests and disconnects through these commands; every window hears
// `waypoint-vfs://connections` after an edit and `waypoint-vfs://connection-state` after a login
// changes state. A secret crosses IPC once, inside an `AnswerInput`, and never comes back.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use waypoint_connections::{
    check_draft, parse_address_gated, AnswerInput, ConnectionDraft, ConnectionEntry,
    ConnectionStatus, ConnectionSupport, ConnectionsError, ConnectionsHub, ConnectionsOverview,
    KeyringUnavailable, ParsedAddress, ProtocolsChanged, Remembered, SuggestedServer,
    TestedConnection,
};
use waypoint_path::{ConnectionKey, RemoteScheme, VfsPath};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{CancelToken, ConnectAnswer};

use crate::commands::Vfs;
use crate::error::Error;

/// Sent to every window after the saved connections change (`ConnectionsChanged`).
pub const CONNECTIONS_EVENT: &str = "waypoint-vfs://connections";

/// Sent to every window after a login changes state (`ConnectionStatus`).
pub const CONNECTION_STATE_EVENT: &str = "waypoint-vfs://connection-state";

/// Sent to every window after a remote protocol is turned on or off (`ProtocolsChanged`).
pub const PROTOCOLS_EVENT: &str = "waypoint-vfs://protocols";

/// Tells every window which protocols are on and which are off now. The app calls it after it
/// registers or turns off a provider (D167).
pub fn announce_protocols<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<Vfs>() else {
        return;
    };
    let (schemes, off) = connectable_protocols(&state);
    let change = ProtocolsChanged { schemes, off };
    if let Err(error) = app.emit(PROTOCOLS_EVENT, change) {
        log::warn!("could not send the protocols that are on: {error}");
    }
}

/// The protocols a connection can be made to, those that are on and those turned off. Registered
/// schemes also include locations nobody connects to (`archive`, `git+file`); they are left out.
pub(crate) fn connectable_protocols(state: &Vfs) -> (Vec<String>, Vec<String>) {
    (
        connectable(state.remote().schemes()),
        connectable(state.remote().off_schemes()),
    )
}

/// The schemes of `schemes` that a connection can be made to.
pub(crate) fn connectable(schemes: Vec<&'static str>) -> Vec<String> {
    schemes
        .into_iter()
        .filter(|scheme| RemoteScheme::from_name(scheme).is_some())
        .map(str::to_owned)
        .collect()
}

/// Points the hub's events at every window of `app`.
pub(crate) fn wire_events<R: Runtime>(app: &AppHandle<R>, hub: &ConnectionsHub) {
    let to_windows = app.clone();
    hub.set_sink(move |change| {
        if let Err(error) = to_windows.emit(CONNECTIONS_EVENT, change) {
            log::warn!("could not send a connections change: {error}");
        }
    });
    let to_windows = app.clone();
    hub.manager().set_sink(move |status| {
        if let Err(error) = to_windows.emit(CONNECTION_STATE_EVENT, &status) {
            log::warn!("could not send a connection state: {error}");
        }
    });
}

fn hub(state: &Vfs) -> Result<Arc<ConnectionsHub>, Error> {
    state.connections().cloned().ok_or_else(|| {
        VfsError::Unsupported {
            what: "saved connections".to_owned(),
        }
        .into()
    })
}

/// The login of a server location; `InvalidLocation` for anything else.
fn key_of(state: &Vfs, location: &Location) -> Result<ConnectionKey, Error> {
    let invalid = || VfsError::InvalidLocation {
        input: location.uri.clone(),
    };
    let path = VfsPath::from_location(location).map_err(|_| invalid())?;
    if !matches!(path, VfsPath::Remote(_)) {
        return Err(invalid().into());
    }
    state.remote().for_path(&path)?;
    path.connection_key().ok_or_else(|| invalid().into())
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, Error> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| Error::Internal(error.to_string()))
}

/// The saved connections and recent servers, with the state of every login the manager knows.
#[tauri::command]
pub async fn list_connections(state: State<'_, Vfs>) -> Result<ConnectionsOverview, Error> {
    let hub = hub(&state)?;
    Ok(ConnectionsOverview {
        connections: hub.snapshot(),
        statuses: hub.manager().statuses(),
    })
}

/// The protocols that can be connected to here, and whether a login can be remembered. Asking
/// the keyring never asks it to unlock.
#[tauri::command]
pub async fn connection_support(state: State<'_, Vfs>) -> Result<ConnectionSupport, Error> {
    let hub = hub(&state)?;
    let (schemes, off) = connectable_protocols(&state);
    blocking(move || {
        ConnectionSupport::new(schemes, off, hub.manager().credentials().keyring().as_ref())
    })
    .await
}

/// Hosts of `~/.ssh/config` to offer in the Connect dialog.
#[tauri::command]
pub async fn suggested_servers(state: State<'_, Vfs>) -> Result<Vec<SuggestedServer>, Error> {
    let Some(suggest) = state.suggestions() else {
        return Ok(Vec::new());
    };
    blocking(move || SuggestedServer::from_aliases(suggest())).await
}

/// Reads a typed server address into the dialog's fields, for live checking as it is typed.
#[tauri::command]
pub async fn parse_address_text(
    state: State<'_, Vfs>,
    text: String,
) -> Result<ParsedAddress, Error> {
    let remote = state.remote().clone();
    Ok(parse_address_gated(
        &text,
        &|scheme| remote.serves(scheme),
        &|scheme| remote.is_off(scheme),
    )?)
}

#[tauri::command]
pub async fn add_connection(
    state: State<'_, Vfs>,
    draft: ConnectionDraft,
) -> Result<ConnectionEntry, Error> {
    Ok(hub(&state)?.add(&draft)?)
}

#[tauri::command]
pub async fn update_connection(
    state: State<'_, Vfs>,
    id: String,
    draft: ConnectionDraft,
) -> Result<ConnectionEntry, Error> {
    Ok(hub(&state)?.update(&id, &draft)?)
}

/// Saves a copy of a connection right after it, under `name` (the page words "… (copy)").
#[tauri::command]
pub async fn duplicate_connection(
    state: State<'_, Vfs>,
    id: String,
    name: String,
) -> Result<ConnectionEntry, Error> {
    Ok(hub(&state)?.duplicate(&id, |_| name)?)
}

/// Forgets a saved connection; with `forget_login` its remembered secrets go too. Resolves with
/// why the keyring could not forget them, or `null`.
#[tauri::command]
pub async fn remove_connection(
    state: State<'_, Vfs>,
    id: String,
    forget_login: bool,
) -> Result<Option<KeyringUnavailable>, Error> {
    let hub = hub(&state)?;
    blocking(move || hub.remove(&id, forget_login))
        .await?
        .map_err(Error::from)
}

#[tauri::command]
pub async fn move_connection(state: State<'_, Vfs>, id: String, to: u32) -> Result<(), Error> {
    Ok(hub(&state)?.move_to(&id, to as usize)?)
}

/// Forgets one recent server, or all of them with `null`, with the secrets remembered for their
/// logins. Resolves with why the keyring could not forget them, or `null`.
#[tauri::command]
pub async fn forget_recent_server(
    state: State<'_, Vfs>,
    key: Option<String>,
) -> Result<Option<KeyringUnavailable>, Error> {
    let hub = hub(&state)?;
    blocking(move || hub.forget_recent(key.as_deref())).await
}

/// Forgets the remembered secrets of a server's login, in the keyring and in this session.
#[tauri::command]
pub async fn forget_login(
    state: State<'_, Vfs>,
    location: Location,
) -> Result<Option<KeyringUnavailable>, Error> {
    let hub = hub(&state)?;
    let key = key_of(&state, &location)?;
    blocking(move || hub.forget_login(&key).err()).await
}

/// Connects a server's login now, with the person's answer to the question its last attempt asked
/// (none to retry, as Reconnect does). With `remember`, a credential the server accepts is put in
/// the keyring. Resolves with what became of "Remember"; rejects with the `VfsError` that says what
/// is still needed.
#[tauri::command]
pub async fn connect(
    state: State<'_, Vfs>,
    location: Location,
    answer: Option<AnswerInput>,
    remember: Option<bool>,
) -> Result<Remembered, Error> {
    let hub = hub(&state)?;
    let key = key_of(&state, &location)?;
    let answer = answer.map(ConnectAnswer::from);
    blocking(move || hub.connect(&key, answer, remember.unwrap_or(false), &CancelToken::new()))
        .await?
        .map_err(Error::from)
}

/// Tries a draft's server without saving it: connects its login (which stays open until it is
/// idle, so saving and opening it next costs nothing) and resolves with what became of "Remember"
/// and where the draft opens; rejects as `connect` does.
#[tauri::command]
pub async fn test_connection(
    state: State<'_, Vfs>,
    draft: ConnectionDraft,
    answer: Option<AnswerInput>,
    remember: Option<bool>,
) -> Result<TestedConnection, Error> {
    let hub = hub(&state)?;
    let checked = check_draft(&draft).map_err(ConnectionsError::from)?;
    let location = VfsPath::Remote(checked.start).to_location();
    let root = VfsPath::Remote(checked.root);
    state.remote().for_path(&root)?;
    let key = root
        .connection_key()
        .ok_or_else(|| VfsError::InvalidLocation {
            input: root.to_uri(),
        })?;
    let answer = answer.map(ConnectAnswer::from);
    let login = key.as_str().to_owned();
    let remembered =
        blocking(move || hub.connect(&key, answer, remember.unwrap_or(false), &CancelToken::new()))
            .await??;
    Ok(TestedConnection {
        remembered,
        key: login,
        location,
    })
}

/// Closes a server's login. Listings on it fail with `Disconnected` and show Reconnect.
#[tauri::command]
pub async fn disconnect(state: State<'_, Vfs>, location: Location) -> Result<(), Error> {
    let hub = hub(&state)?;
    let key = key_of(&state, &location)?;
    blocking(move || hub.manager().disconnect(&key))
        .await?
        .map_err(Error::from)
}

/// The state of the login a location belongs to, or `null` for a location with no login (a local
/// folder).
#[tauri::command]
pub async fn connection_state(
    state: State<'_, Vfs>,
    location: Location,
) -> Result<Option<ConnectionStatus>, Error> {
    let Ok(key) = key_of(&state, &location) else {
        return Ok(None);
    };
    let Some(hub) = state.connections() else {
        return Ok(None);
    };
    Ok(Some(ConnectionStatus {
        key: key.as_str().to_owned(),
        state: hub.manager().state(&key),
        revision: 0,
    }))
}
