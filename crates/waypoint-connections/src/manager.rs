// The connection manager: the state of every login, connecting with the person's answers, idle timeouts and the events the windows follow
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Connecting is lazy (A78): any provider call opens its session. The manager does not stand in
// the way of those calls; it is told what they found (`observe`), so the Network section and a
// tab can say "Connecting", "Connected" or why not. Explicit Connect and Reconnect go through
// `connect`, with the person's answer to the question the last attempt asked. A login nobody has
// used for the idle timeout (five minutes, D146) is closed by `sweep`, and reopened by the next
// call. Every change of state is one `ConnectionStatus` with the manager's revision.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_path::{ConnectionKey, VfsPath, ELEVATED_SCHEME};
use waypoint_protocol::{ConnectionState, VfsError};
use waypoint_vfs::{CancelToken, ConnectAnswer, Provider, ProviderRegistry};

use crate::credentials::{Credentials, KeyringUnavailable, SecretKind};
use crate::store::root_of;

/// How long a login nobody uses stays open (D146).
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// How often the idle sweep looks.
pub const SWEEP_EVERY: Duration = Duration::from_secs(30);

/// How often a login shown as failed is asked whether its provider has reconnected.
pub const RECHECK_EVERY: Duration = Duration::from_secs(2);

/// The state of one login, as every window hears it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ConnectionStatus {
    /// The login (`sftp://me@nas.lan`).
    pub key: String,
    pub state: ConnectionState,
    /// The manager's revision after this change; a window ignores an older one.
    #[ts(type = "number")]
    pub revision: u64,
}

/// What became of "Remember" on a login that connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum Remembered {
    /// Nothing was asked to be remembered.
    No,
    /// The secret is in the keyring.
    Kept,
    /// The keyring could not keep it; it lasts for this session.
    SessionOnly { why: KeyringUnavailable },
}

/// Whether an error is about the connection rather than the file asked for: it changes the state
/// the Network section and the tab show.
pub fn is_connection_error(error: &VfsError) -> bool {
    matches!(
        error,
        VfsError::Disconnected { .. }
            | VfsError::Unreachable { .. }
            | VfsError::Timeout { .. }
            | VfsError::AuthRequired { .. }
            | VfsError::AuthFailed { .. }
            | VfsError::HostKeyUnknown { .. }
            | VfsError::HostKeyChanged { .. }
            | VfsError::CertificateUntrusted { .. }
    )
}

struct Tracked {
    state: ConnectionState,
    /// Open listings on this login; it is never idle while any is open.
    users: usize,
    last_used: Instant,
}

type Sink = Arc<dyn Fn(ConnectionStatus) + Send + Sync>;

/// The connection manager. Shared (`Arc`) by the plugin's commands and the idle sweep.
pub struct ConnectionManager {
    registry: Arc<ProviderRegistry>,
    credentials: Arc<Credentials>,
    tracked: Mutex<HashMap<ConnectionKey, Tracked>>,
    revision: AtomicU64,
    sink: RwLock<Option<Sink>>,
    idle: Duration,
}

impl ConnectionManager {
    pub fn new(registry: Arc<ProviderRegistry>, credentials: Arc<Credentials>) -> Self {
        Self {
            registry,
            credentials,
            tracked: Mutex::new(HashMap::new()),
            revision: AtomicU64::new(0),
            sink: RwLock::new(None),
            idle: IDLE_TIMEOUT,
        }
    }

    /// The same manager with another idle timeout.
    pub fn with_idle_timeout(mut self, idle: Duration) -> Self {
        self.idle = idle;
        self
    }

    /// Where state changes go (the plugin's event to every window).
    pub fn set_sink(&self, sink: impl Fn(ConnectionStatus) + Send + Sync + 'static) {
        *self.sink.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(sink));
    }

    pub fn registry(&self) -> &Arc<ProviderRegistry> {
        &self.registry
    }

    pub fn credentials(&self) -> &Arc<Credentials> {
        &self.credentials
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<ConnectionKey, Tracked>> {
        self.tracked.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The provider of a login, or `Unsupported` when no provider serves its scheme.
    pub fn provider(&self, key: &ConnectionKey) -> Result<Arc<dyn Provider>, VfsError> {
        // The elevated helper's one connection is not a server's root, and has a scheme of its own.
        if *key == ConnectionKey::elevated() {
            return self.registry.for_scheme(ELEVATED_SCHEME);
        }
        let root = root_of(key.as_str()).ok_or_else(|| VfsError::InvalidLocation {
            input: key.as_str().to_owned(),
        })?;
        self.registry.for_path(&VfsPath::Remote(root))
    }

    /// The login a location belongs to, when its provider has sessions.
    pub fn key_of(&self, path: &VfsPath) -> Option<ConnectionKey> {
        self.registry.for_path(path).ok()?.connection_key(path)
    }

    /// Sets a login's state and tells the windows, when it changed.
    fn set(&self, key: &ConnectionKey, state: ConnectionState) {
        let status = {
            let mut tracked = self.lock();
            let entry = tracked.entry(key.clone()).or_insert_with(|| Tracked {
                state: ConnectionState::Idle,
                users: 0,
                last_used: Instant::now(),
            });
            entry.last_used = Instant::now();
            if entry.state == state {
                return;
            }
            entry.state = state.clone();
            ConnectionStatus {
                key: key.as_str().to_owned(),
                state,
                revision: self.revision.fetch_add(1, Ordering::AcqRel) + 1,
            }
        };
        let sink = self.sink.read().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(sink) = sink {
            sink(status);
        }
    }

    /// The state of one login.
    pub fn state(&self, key: &ConnectionKey) -> ConnectionState {
        self.lock()
            .get(key)
            .map(|t| t.state.clone())
            .unwrap_or(ConnectionState::Idle)
    }

    /// Every login the manager knows, for a window that has just opened.
    pub fn statuses(&self) -> Vec<ConnectionStatus> {
        let revision = self.revision.load(Ordering::Acquire);
        let mut all: Vec<_> = self
            .lock()
            .iter()
            .map(|(key, tracked)| ConnectionStatus {
                key: key.as_str().to_owned(),
                state: tracked.state.clone(),
                revision,
            })
            .collect();
        all.sort_by(|a, b| a.key.cmp(&b.key));
        all
    }

    /// Opens a login now, with the person's answer to the last question. A credential answer is
    /// kept for the session first (so the provider's own lookups find it too); with `remember`
    /// it is put in the keyring once the server has accepted it, never before, so a mistyped
    /// password is never remembered.
    pub fn connect(
        &self,
        key: &ConnectionKey,
        answer: Option<ConnectAnswer>,
        remember: bool,
        cancel: &CancelToken,
    ) -> Result<Remembered, VfsError> {
        let provider = self.provider(key)?;
        let answered = match &answer {
            Some(ConnectAnswer::Credential(credential)) => SecretKind::of_credential(credential),
            _ => None,
        };
        if let Some((kind, secret)) = &answered {
            self.credentials.answer(key, *kind, secret.clone());
        }
        self.set(key, ConnectionState::Connecting);
        match provider.connect(key, answer, cancel) {
            Ok(()) => {
                self.set(key, ConnectionState::Connected);
                Ok(match (&answered, remember) {
                    (Some((kind, secret)), true) => {
                        match self.credentials.keyring().store(key, *kind, secret.clone()) {
                            Ok(()) => Remembered::Kept,
                            Err(why) => Remembered::SessionOnly { why },
                        }
                    }
                    _ => Remembered::No,
                })
            }
            Err(VfsError::Cancelled) => {
                self.set(key, ConnectionState::Idle);
                Err(VfsError::Cancelled)
            }
            Err(error) => {
                if let (VfsError::AuthFailed { .. }, Some((kind, _))) = (&error, &answered) {
                    self.credentials.refuse(key, *kind);
                }
                self.set(
                    key,
                    ConnectionState::Failed {
                        error: error.clone(),
                    },
                );
                Err(error)
            }
        }
    }

    /// Closes a login. Calls in flight end with `Disconnected`; the next call reconnects.
    pub fn disconnect(&self, key: &ConnectionKey) -> Result<(), VfsError> {
        let provider = self.provider(key)?;
        provider.disconnect(key);
        self.set(key, ConnectionState::Idle);
        Ok(())
    }

    /// Closes every login of `scheme` and forgets its state, for a protocol that is being turned
    /// off (D167): call it while the provider is still registered, so it can end the sessions.
    /// Each login's windows hear it go idle.
    pub fn close_scheme(&self, scheme: &str) {
        let prefix = format!("{scheme}://");
        let keys: Vec<ConnectionKey> = self
            .lock()
            .keys()
            .filter(|key| {
                key.as_str().starts_with(&prefix)
                    || (scheme == ELEVATED_SCHEME && **key == ConnectionKey::elevated())
            })
            .cloned()
            .collect();
        for key in keys {
            if let Ok(provider) = self.provider(&key) {
                provider.disconnect(&key);
            }
            self.set(&key, ConnectionState::Idle);
        }
    }

    /// What a call on a login found: success means the session works, a connection error is the
    /// login's new state, and any other error (a missing file) says nothing about the login.
    pub fn observe(&self, key: &ConnectionKey, outcome: Result<(), &VfsError>) {
        match outcome {
            Ok(()) => self.set(key, ConnectionState::Connected),
            Err(VfsError::Cancelled) => {}
            Err(error) if is_connection_error(error) => self.set(
                key,
                ConnectionState::Failed {
                    error: error.clone(),
                },
            ),
            Err(_) => self.set(key, ConnectionState::Connected),
        }
    }

    /// A listing on `key` opened: the login is in use until `release`.
    pub fn acquire(&self, key: &ConnectionKey) {
        let mut tracked = self.lock();
        let entry = tracked.entry(key.clone()).or_insert_with(|| Tracked {
            state: ConnectionState::Idle,
            users: 0,
            last_used: Instant::now(),
        });
        entry.users += 1;
        entry.last_used = Instant::now();
    }

    /// A listing on `key` closed.
    pub fn release(&self, key: &ConnectionKey) {
        if let Some(entry) = self.lock().get_mut(key) {
            entry.users = entry.users.saturating_sub(1);
            entry.last_used = Instant::now();
        }
    }

    /// Notes a use of `key` that does not change its state (an operation, a preview).
    pub fn touch(&self, key: &ConnectionKey) {
        if let Some(entry) = self.lock().get_mut(key) {
            entry.last_used = Instant::now();
        }
    }

    /// Closes every connected login nobody has used since `now - idle` and that no listing holds,
    /// and takes up what the providers found on their own (a session that dropped). Returns the
    /// logins it closed.
    pub fn sweep(&self, now: Instant) -> Vec<ConnectionKey> {
        let candidates: Vec<(ConnectionKey, bool)> = self
            .lock()
            .iter()
            .filter(|(_, t)| t.state == ConnectionState::Connected)
            .map(|(key, t)| {
                let idle = t.users == 0 && now.saturating_duration_since(t.last_used) >= self.idle;
                (key.clone(), idle)
            })
            .collect();
        let mut closed = Vec::new();
        for (key, idle) in candidates {
            let Ok(provider) = self.provider(&key) else {
                continue;
            };
            if idle {
                log::debug!("closing an idle connection");
                provider.disconnect(&key);
                self.set(&key, ConnectionState::Idle);
                closed.push(key);
                continue;
            }
            match provider.connection_state(&key) {
                state @ (ConnectionState::Idle | ConnectionState::Failed { .. }) => {
                    self.set(&key, state)
                }
                ConnectionState::Connecting | ConnectionState::Connected => {}
            }
        }
        closed
    }

    /// Takes up a login the provider says works again while the manager still shows it failed: a
    /// transfer reconnects through the registry on its own, and nothing else would tell the
    /// Network section and the tab's dot.
    pub fn refresh_failed(&self) {
        let failed: Vec<ConnectionKey> = self
            .lock()
            .iter()
            .filter(|(_, t)| matches!(t.state, ConnectionState::Failed { .. }))
            .map(|(key, _)| key.clone())
            .collect();
        for key in failed {
            let Ok(provider) = self.provider(&key) else {
                continue;
            };
            if provider.connection_state(&key) == ConnectionState::Connected {
                self.set(&key, ConnectionState::Connected);
            }
        }
    }

    /// Runs `sweep` every `every` on a thread of its own, for as long as the manager lives, and
    /// `refresh_failed` every `RECHECK_EVERY` between, so a reconnected login shows within seconds.
    pub fn spawn_sweeper(self: &Arc<Self>, every: Duration) {
        let weak: Weak<Self> = Arc::downgrade(self);
        let spawned = std::thread::Builder::new()
            .name("waypoint-connections-idle".to_owned())
            .spawn(move || {
                let tick = every.min(RECHECK_EVERY);
                let mut last_sweep = Instant::now();
                loop {
                    std::thread::sleep(tick);
                    let Some(manager) = weak.upgrade() else { break };
                    manager.refresh_failed();
                    if last_sweep.elapsed() >= every {
                        last_sweep = Instant::now();
                        manager.sweep(Instant::now());
                    }
                }
            });
        if let Err(error) = spawned {
            log::warn!("could not start the idle connection sweep: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::{MemorySecrets, SecretStore};
    use std::sync::Mutex as StdMutex;
    use waypoint_path::{CaseRule, RemoteScheme};
    use waypoint_protocol::{HostKey, UnreachableReason};
    use waypoint_vfs::{Credential, FakeRemoteProvider, RemoteFault, Secret};

    struct Fixture {
        manager: Arc<ConnectionManager>,
        server: FakeRemoteProvider,
        keyring: Arc<MemorySecrets>,
        events: Arc<StdMutex<Vec<ConnectionStatus>>>,
        key: ConnectionKey,
    }

    fn fixture() -> Fixture {
        let keyring = Arc::new(MemorySecrets::new());
        let credentials = Arc::new(Credentials::new(keyring.clone()));
        let server = FakeRemoteProvider::new(RemoteScheme::Sftp, CaseRule::Sensitive)
            .with_credentials(credentials.clone());
        let registry = ProviderRegistry::new();
        registry.register(Arc::new(server.clone()));
        let manager = Arc::new(ConnectionManager::new(Arc::new(registry), credentials));
        let events = Arc::new(StdMutex::new(Vec::new()));
        let seen = events.clone();
        manager.set_sink(move |status| seen.lock().unwrap().push(status));
        Fixture {
            manager,
            server,
            keyring,
            events,
            key: root_of("sftp://me@nas.lan").unwrap().connection_key(),
        }
    }

    fn states(fx: &Fixture) -> Vec<String> {
        fx.events
            .lock()
            .unwrap()
            .iter()
            .map(|s| match &s.state {
                ConnectionState::Idle => "idle".to_owned(),
                ConnectionState::Connecting => "connecting".to_owned(),
                ConnectionState::Connected => "connected".to_owned(),
                ConnectionState::Failed { error } => format!("failed:{error:?}")
                    .split([' ', '{'])
                    .next()
                    .unwrap()
                    .to_owned(),
            })
            .collect()
    }

    fn password(text: &str) -> ConnectAnswer {
        password_of("me", text)
    }

    fn password_of(user: &str, text: &str) -> ConnectAnswer {
        ConnectAnswer::Credential(Credential::Password {
            user: Some(user.into()),
            password: Secret::from(text),
        })
    }

    #[test]
    fn a_password_login_asks_then_connects_and_is_remembered_only_once_accepted() {
        let fx = fixture();
        fx.server.require_password(Some("me"), "hunter2");
        let cancel = CancelToken::new();
        let asked = fx
            .manager
            .connect(&fx.key, None, false, &cancel)
            .unwrap_err();
        assert!(matches!(asked, VfsError::AuthRequired { .. }));
        let wrong = fx
            .manager
            .connect(&fx.key, Some(password("nope")), true, &cancel)
            .unwrap_err();
        assert!(matches!(wrong, VfsError::AuthFailed { .. }));
        assert!(fx.keyring.is_empty(), "a refused password is never kept");
        let kept = fx
            .manager
            .connect(&fx.key, Some(password("hunter2")), true, &cancel)
            .unwrap();
        assert_eq!(kept, Remembered::Kept);
        assert_eq!(fx.keyring.len(), 1);
        assert_eq!(fx.manager.state(&fx.key), ConnectionState::Connected);
        assert_eq!(
            states(&fx),
            [
                "connecting",
                "failed:AuthRequired",
                "connecting",
                "failed:AuthFailed",
                "connecting",
                "connected"
            ]
        );
        let revisions: Vec<u64> = fx
            .events
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.revision)
            .collect();
        assert!(revisions.windows(2).all(|w| w[1] == w[0] + 1));
    }

    #[test]
    fn a_remembered_password_connects_without_a_question() {
        let fx = fixture();
        fx.server.require_password(Some("me"), "hunter2");
        fx.keyring
            .store(&fx.key, SecretKind::Password, Secret::from("hunter2"))
            .unwrap();
        fx.manager
            .connect(&fx.key, None, false, &CancelToken::new())
            .unwrap();
        assert_eq!(fx.manager.state(&fx.key), ConnectionState::Connected);
    }

    #[test]
    fn without_a_keyring_remember_lasts_for_the_session_and_says_why() {
        let fx = fixture();
        fx.server.require_password(Some("me"), "hunter2");
        fx.keyring.fail_with(Some(KeyringUnavailable::NoKeyring));
        let kept = fx
            .manager
            .connect(
                &fx.key,
                Some(password("hunter2")),
                true,
                &CancelToken::new(),
            )
            .unwrap();
        assert_eq!(
            kept,
            Remembered::SessionOnly {
                why: KeyringUnavailable::NoKeyring
            }
        );
        // The session's answer still serves the next login.
        fx.manager.disconnect(&fx.key).unwrap();
        fx.manager
            .connect(&fx.key, None, false, &CancelToken::new())
            .unwrap();
    }

    #[test]
    fn an_unknown_host_key_is_answered_by_trusting_its_fingerprint() {
        let fx = fixture();
        fx.server.require_host_key(HostKey {
            host: "nas.lan".into(),
            algorithm: "ssh-ed25519".into(),
            fingerprint: "SHA256:abc".into(),
        });
        let cancel = CancelToken::new();
        assert!(matches!(
            fx.manager.connect(&fx.key, None, false, &cancel),
            Err(VfsError::HostKeyUnknown { .. })
        ));
        fx.manager
            .connect(
                &fx.key,
                Some(ConnectAnswer::TrustHostKey {
                    fingerprint: "SHA256:abc".into(),
                    remember: true,
                }),
                false,
                &cancel,
            )
            .unwrap();
        assert_eq!(fx.manager.state(&fx.key), ConnectionState::Connected);
    }

    #[test]
    fn calls_report_the_state_and_offline_is_a_state() {
        let fx = fixture();
        fx.manager.observe(&fx.key, Ok(()));
        assert_eq!(fx.manager.state(&fx.key), ConnectionState::Connected);
        let offline = VfsError::Unreachable {
            location: waypoint_protocol::Location::new("x", "sftp://me@nas.lan/"),
            reason: UnreachableReason::Offline,
        };
        fx.manager.observe(&fx.key, Err(&offline));
        assert!(matches!(
            fx.manager.state(&fx.key),
            ConnectionState::Failed { .. }
        ));
        // A missing file says nothing bad about the login.
        let missing = VfsError::NotFound {
            location: waypoint_protocol::Location::new("x", "sftp://me@nas.lan/x"),
        };
        fx.manager.observe(&fx.key, Err(&missing));
        assert_eq!(fx.manager.state(&fx.key), ConnectionState::Connected);
        fx.server
            .set_fault(Some(RemoteFault::Unreachable(UnreachableReason::Refused)));
        assert!(matches!(
            fx.manager
                .connect(&fx.key, None, false, &CancelToken::new()),
            Err(VfsError::Unreachable { .. })
        ));
        fx.server.set_fault(None);
        fx.manager
            .connect(&fx.key, None, false, &CancelToken::new())
            .unwrap();
    }

    #[test]
    fn an_idle_login_is_closed_unless_a_listing_holds_it() {
        let fx = fixture();
        fx.manager
            .connect(&fx.key, None, false, &CancelToken::new())
            .unwrap();
        let later = Instant::now() + IDLE_TIMEOUT + Duration::from_secs(1);
        fx.manager.acquire(&fx.key);
        assert!(fx.manager.sweep(later).is_empty());
        fx.manager.release(&fx.key);
        assert!(fx.manager.sweep(Instant::now()).is_empty(), "not idle yet");
        assert_eq!(fx.manager.sweep(later), vec![fx.key.clone()]);
        assert_eq!(fx.manager.state(&fx.key), ConnectionState::Idle);
        assert_eq!(
            fx.server.connection_state(&fx.key),
            ConnectionState::Idle,
            "the provider's session is closed too"
        );
    }

    #[test]
    fn a_login_a_transfer_reconnected_is_connected_again_without_a_call_of_the_pages() {
        let fx = fixture();
        fx.server.put_dir(&fx.server.root("me@nas.lan"));
        fx.manager
            .connect(&fx.key, None, false, &CancelToken::new())
            .unwrap();
        // The connection drops while a transfer runs, and the manager hears of it.
        fx.server
            .set_fault(Some(RemoteFault::Unreachable(UnreachableReason::Refused)));
        let root = fx.server.root("me@nas.lan");
        let lost = fx.server.stat(&root).unwrap_err();
        fx.manager.observe(&fx.key, Err(&lost));
        assert!(matches!(
            fx.manager.state(&fx.key),
            ConnectionState::Failed { .. }
        ));
        // The transfer reconnects through the registry, which the manager is not part of.
        fx.server.set_fault(None);
        fx.server.stat(&root).unwrap();
        assert_eq!(
            fx.server.connection_state(&fx.key),
            ConnectionState::Connected
        );
        fx.manager.refresh_failed();
        assert_eq!(fx.manager.state(&fx.key), ConnectionState::Connected);
        assert_eq!(states(&fx).last().map(String::as_str), Some("connected"));
    }

    #[test]
    fn a_scheme_nobody_serves_is_unsupported() {
        let fx = fixture();
        let smb = root_of("smb://files").unwrap().connection_key();
        assert!(matches!(
            fx.manager.connect(&smb, None, false, &CancelToken::new()),
            Err(VfsError::Unsupported { .. })
        ));
        assert_eq!(fx.manager.statuses().len(), 0);
    }

    #[test]
    fn turning_a_protocol_off_ends_its_logins_and_refuses_new_ones() {
        let fx = fixture();
        fx.manager
            .connect(&fx.key, None, false, &CancelToken::new())
            .unwrap();
        assert_eq!(fx.manager.state(&fx.key), ConnectionState::Connected);
        fx.manager.close_scheme("sftp");
        assert_eq!(fx.manager.state(&fx.key), ConnectionState::Idle);
        assert_eq!(
            fx.server.connection_state(&fx.key),
            ConnectionState::Idle,
            "the provider's session is closed too"
        );
        fx.manager.registry().turn_off("sftp");
        assert!(matches!(
            fx.manager
                .connect(&fx.key, None, false, &CancelToken::new()),
            Err(VfsError::ProtocolOff { scheme }) if scheme == "sftp"
        ));
        // A login of another scheme is left alone.
        fx.manager.close_scheme("smb");
    }

    /// The manager over a fake server of `scheme`, the way the app composes a provider of it.
    fn fixture_of(scheme: RemoteScheme, login: &str) -> Fixture {
        let keyring = Arc::new(MemorySecrets::new());
        let credentials = Arc::new(Credentials::new(keyring.clone()));
        let server = FakeRemoteProvider::new(scheme, CaseRule::Sensitive)
            .with_credentials(credentials.clone());
        let registry = ProviderRegistry::new();
        registry.register(Arc::new(server.clone()));
        let manager = Arc::new(ConnectionManager::new(Arc::new(registry), credentials));
        let events = Arc::new(StdMutex::new(Vec::new()));
        let seen = events.clone();
        manager.set_sink(move |status| seen.lock().unwrap().push(status));
        Fixture {
            manager,
            server,
            keyring,
            events,
            key: root_of(login).unwrap().connection_key(),
        }
    }

    #[test]
    fn an_smb_domain_login_asks_for_a_password_and_remembers_it_under_its_own_key() {
        let fx = fixture_of(RemoteScheme::Smb, "smb://WORK;me@files.lan");
        assert_eq!(fx.key.as_str(), "smb://WORK;me@files.lan");
        fx.server.require_password(Some("WORK;me"), "hunter2");
        let asked = fx
            .manager
            .connect(&fx.key, None, false, &CancelToken::new())
            .unwrap_err();
        assert!(matches!(asked, VfsError::AuthRequired { .. }));
        let kept = fx
            .manager
            .connect(
                &fx.key,
                Some(password_of("WORK;me", "hunter2")),
                true,
                &CancelToken::new(),
            )
            .unwrap();
        assert_eq!(kept, Remembered::Kept);
        assert_eq!(fx.manager.state(&fx.key), ConnectionState::Connected);
        // The next login needs no question, and a login of another user of the server is another one.
        fx.manager.disconnect(&fx.key).unwrap();
        fx.manager
            .connect(&fx.key, None, false, &CancelToken::new())
            .unwrap();
        let other = root_of("smb://WORK;you@files.lan")
            .unwrap()
            .connection_key();
        assert_ne!(other, fx.key);
        assert_eq!(fx.manager.state(&other), ConnectionState::Idle);
    }

    #[test]
    fn a_webdav_server_is_a_login_of_its_own_scheme_and_host() {
        let fx = fixture_of(RemoteScheme::Davs, "davs://alice@cloud.example.com");
        fx.server.require_password(Some("alice"), "app-password");
        fx.manager
            .connect(
                &fx.key,
                Some(password_of("alice", "app-password")),
                false,
                &CancelToken::new(),
            )
            .unwrap();
        assert_eq!(fx.manager.state(&fx.key), ConnectionState::Connected);
        // Plain HTTP is not served by this provider, and says so rather than connecting.
        let dav = root_of("dav://alice@cloud.example.com")
            .unwrap()
            .connection_key();
        assert!(matches!(
            fx.manager.connect(&dav, None, false, &CancelToken::new()),
            Err(VfsError::Unsupported { .. })
        ));
    }

    /// A launcher that waits for the cancel of the call that connects, as the system's prompt does.
    struct Prompt {
        waiting: Arc<std::sync::atomic::AtomicBool>,
    }

    impl waypoint_elevated::Launcher for Prompt {
        fn launch(&self, cancel: &CancelToken) -> Result<waypoint_elevated::Transport, VfsError> {
            self.waiting.store(true, Ordering::SeqCst);
            while !cancel.is_cancelled() {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(VfsError::Cancelled)
        }
    }

    fn elevated_manager(launcher: Box<dyn waypoint_elevated::Launcher>) -> Arc<ConnectionManager> {
        let registry = ProviderRegistry::new();
        registry.register(Arc::new(waypoint_elevated::ElevatedProvider::new(launcher)));
        let credentials = Arc::new(Credentials::new(Arc::new(MemorySecrets::new())));
        Arc::new(ConnectionManager::new(Arc::new(registry), credentials))
    }

    #[test]
    fn the_elevated_login_connects_and_disconnects_through_its_provider() {
        let launcher = waypoint_elevated::testing::loopback::LoopbackLauncher::new(
            Arc::new(waypoint_vfs::LocalProvider::new()),
            waypoint_elevated::ServeConfig::default(),
        );
        let manager = elevated_manager(Box::new(launcher.clone()));
        let key = ConnectionKey::elevated();
        assert_eq!(manager.state(&key), ConnectionState::Idle);
        manager
            .connect(&key, None, false, &CancelToken::new())
            .unwrap();
        assert_eq!(manager.state(&key), ConnectionState::Connected);
        assert_eq!(launcher.launches(), 1);
        manager.disconnect(&key).unwrap();
        assert_eq!(manager.state(&key), ConnectionState::Idle);
        // Turning the switch off closes it, and refuses new connections as a protocol that is off.
        manager
            .connect(&key, None, false, &CancelToken::new())
            .unwrap();
        manager.close_scheme(ELEVATED_SCHEME);
        assert_eq!(manager.state(&key), ConnectionState::Idle);
        manager.registry().turn_off(ELEVATED_SCHEME);
        assert!(matches!(
            manager.connect(&key, None, false, &CancelToken::new()),
            Err(VfsError::ProtocolOff { scheme }) if scheme == "admin"
        ));
    }

    #[test]
    fn the_cancel_of_a_connect_reaches_the_elevated_launcher() {
        let waiting = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let manager = elevated_manager(Box::new(Prompt {
            waiting: waiting.clone(),
        }));
        let key = ConnectionKey::elevated();
        let cancel = CancelToken::new();
        let result = std::thread::scope(|scope| {
            let call = scope.spawn(|| manager.connect(&key, None, false, &cancel));
            let started = Instant::now();
            while !waiting.load(Ordering::SeqCst) {
                assert!(started.elapsed() < Duration::from_secs(5), "never launched");
                std::thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(manager.state(&key), ConnectionState::Connecting);
            cancel.cancel();
            call.join().unwrap()
        });
        assert!(matches!(result, Err(VfsError::Cancelled)));
        assert_eq!(manager.state(&key), ConnectionState::Idle);
    }
}
