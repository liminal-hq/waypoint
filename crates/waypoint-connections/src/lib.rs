// Waypoint's saved connections and connection manager, with no Tauri dependency
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The connections of milestone 6 (A81, `docs/architecture/remote-locations.md`): the saved
//! connections a person keeps in the Network section, checked against the canonical addresses of
//! `waypoint-path` and saved as a versioned document with one earlier generation; the store's one
//! writer, revision and granular events (`ConnectionsHub`); where logins come from (`Credentials`,
//! the app's `CredentialSource`: this session's answers, then the keyring behind `SecretStore`);
//! and the `ConnectionManager`, which connects with the person's answers, follows the state of
//! every login, and closes the idle ones. Nothing here holds a secret in a file, an event or a log.

mod credentials;
mod hub;
mod manager;
mod model;
mod storage;
mod store;
mod wire;

pub use credentials::{
    Credentials, KeyringUnavailable, MemorySecrets, SecretKind, SecretStore, KEYRING_SERVICE,
};
pub use hub::{host_override, ConnectionsHub, HostOverride};
pub use manager::{
    is_connection_error, ConnectionManager, ConnectionStatus, Remembered, IDLE_TIMEOUT, SWEEP_EVERY,
};
pub use model::{
    check_draft, AuthMethod, Checked, ConnectionDraft, ConnectionEntry, ConnectionOptions, DavAuth,
    DavPreset, DraftError, RecentServer, RemoteThumbnails, SavedConnection, MAX_FIELD_BYTES,
    MAX_NAME_CHARS, THUMBNAIL_MAX_MB_DEFAULT, THUMBNAIL_MAX_MB_MAX,
};
pub use storage::{
    plan_connections, ConnectionStorage, ConnectionsPersistence, MemoryConnections,
    CONNECTIONS_FILE, CONNECTIONS_FILE_ID, CONNECTIONS_KEY, CONNECTIONS_PREVIOUS_KEY,
};
pub use store::{
    ConnectionChange, Connections, ConnectionsChanged, ConnectionsDocument, ConnectionsError,
    ConnectionsSnapshot, CONNECTIONS_VERSION, MAX_CONNECTIONS, MAX_RECENT,
};
pub use wire::{
    parse_address, parse_address_gated, AnswerInput, ConnectionSupport, ConnectionsOverview,
    NoKeyring, ParsedAddress, ProtocolsChanged, SuggestedServer, TestedConnection,
};
