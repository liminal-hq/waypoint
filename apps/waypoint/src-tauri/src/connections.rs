// Composes the remote providers, the keyring and the saved connections for the file system plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The vfs plugin holds the provider registry and the saved connections, and calls no other plugin
// (A4, A81). This is where the pieces meet: each provider crate the build includes (behind its
// Cargo feature, A85) is built with the one `Credentials` source, which reads this session's
// answers and then the keyring through `KeyringSecrets` over the reusable `secrets` plugin; the
// saved connections live in `connections.json` beside the other documents; and the SSH options a
// saved connection sets (a key file, a jump host) are laid over `~/.ssh/config` for its host.
// Registering another protocol is one more provider in `providers` and, when its connections have
// tuning of their own, one more observer in `wire`.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_secrets::{Secret as KeyringSecret, SecretId, Secrets, SecretsError};
use waypoint_connections::{
    ConnectionStorage, ConnectionsPersistence, Credentials, KeyringUnavailable, MemoryConnections,
    SecretKind, SecretStore, CONNECTIONS_FILE, KEYRING_SERVICE,
};
use waypoint_path::ConnectionKey;
use waypoint_vfs::{Provider, Secret};

use crate::storage::FileKeyValue;

/// How long a keyring call may take, the desktop's own unlock prompt included, before the login
/// is asked for instead.
const KEYRING_WAIT: Duration = Duration::from_secs(120);

/// The handle the keyring adapter reaches the app through: the plugins are built before the app
/// exists.
pub type AppCell = Arc<OnceLock<AppHandle<Wry>>>;

/// The keyring as the saved connections see it, over the `secrets` plugin. The plugin's API is
/// asynchronous and the connection code is not (a provider asks for a credential from inside its
/// own runtime), so each call runs on a thread of its own and is waited for with a bound.
pub struct KeyringSecrets {
    app: AppCell,
}

fn kind(kind: SecretKind) -> tauri_plugin_secrets::SecretKind {
    match kind {
        SecretKind::Password => tauri_plugin_secrets::SecretKind::Password,
        SecretKind::Passphrase => tauri_plugin_secrets::SecretKind::Passphrase,
        SecretKind::AccessKey => tauri_plugin_secrets::SecretKind::Key,
    }
}

fn id(key: &ConnectionKey, of: SecretKind) -> SecretId {
    SecretId::new(KEYRING_SERVICE, key.as_str(), kind(of))
}

fn unavailable(error: &SecretsError) -> KeyringUnavailable {
    match error {
        SecretsError::Locked | SecretsError::Dismissed => KeyringUnavailable::Locked,
        SecretsError::NoKeyring | SecretsError::Unsupported => KeyringUnavailable::NoKeyring,
        _ => KeyringUnavailable::Failed,
    }
}

impl KeyringSecrets {
    fn secrets(&self) -> Result<Secrets, KeyringUnavailable> {
        self.app
            .get()
            .and_then(|app| app.try_state::<Secrets>())
            .map(|secrets| secrets.inner().clone())
            .ok_or(KeyringUnavailable::NoKeyring)
    }

    /// Runs `call` to its end on a thread of its own, waiting at most `KEYRING_WAIT`.
    fn run<T: Send + 'static>(
        &self,
        call: impl FnOnce(Secrets) -> Result<T, KeyringUnavailable> + Send + 'static,
    ) -> Result<T, KeyringUnavailable> {
        let secrets = self.secrets()?;
        let (send, receive) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("waypoint-keyring".to_owned())
            .spawn(move || {
                let _ = send.send(call(secrets));
            });
        if spawned.is_err() {
            return Err(KeyringUnavailable::Failed);
        }
        receive
            .recv_timeout(KEYRING_WAIT)
            .unwrap_or(Err(KeyringUnavailable::Locked))
    }
}

impl SecretStore for KeyringSecrets {
    fn status(&self) -> Result<(), KeyringUnavailable> {
        self.run(|secrets| {
            let status = tauri::async_runtime::block_on(secrets.get_status());
            if status.has(tauri_plugin_secrets::FEATURE_STORE) {
                Ok(())
            } else {
                Err(match status.reason {
                    Some(tauri_plugin_secrets::Reason::Locked) => KeyringUnavailable::Locked,
                    Some(tauri_plugin_secrets::Reason::Failed) => KeyringUnavailable::Failed,
                    _ => KeyringUnavailable::NoKeyring,
                })
            }
        })
    }

    fn store(
        &self,
        key: &ConnectionKey,
        of: SecretKind,
        secret: Secret,
    ) -> Result<(), KeyringUnavailable> {
        let id = id(key, of);
        let label = format!("Waypoint login for {key}");
        let value = KeyringSecret::from_bytes(secret.expose().to_vec());
        self.run(move |secrets| {
            tauri::async_runtime::block_on(secrets.store(&id, Some(label), value))
                .map_err(|e| unavailable(&e))
        })
    }

    fn fetch(
        &self,
        key: &ConnectionKey,
        of: SecretKind,
    ) -> Result<Option<Secret>, KeyringUnavailable> {
        let id = id(key, of);
        self.run(move |secrets| {
            tauri::async_runtime::block_on(secrets.fetch(&id))
                .map(|found| found.map(|secret| Secret::new(secret.expose_bytes().to_vec())))
                .map_err(|e| unavailable(&e))
        })
    }

    fn forget(&self, key: &ConnectionKey) -> Result<usize, KeyringUnavailable> {
        let account = key.as_str().to_owned();
        self.run(move |secrets| {
            tauri::async_runtime::block_on(secrets.delete_account(KEYRING_SERVICE, &account))
                .map_err(|e| unavailable(&e))
        })
    }
}

/// What `compose` makes before the app exists.
pub struct Composed {
    pub options: tauri_plugin_waypoint_vfs::Options<Wry>,
    /// Set once the app exists, so the keyring can be reached.
    pub app: AppCell,
    #[cfg(feature = "sftp")]
    sftp: sftp::Sftp,
}

/// The providers, the credential source and the storage the vfs plugin is built with.
pub fn compose() -> Composed {
    let app: AppCell = Arc::new(OnceLock::new());
    let credentials = Arc::new(Credentials::new(Arc::new(KeyringSecrets {
        app: app.clone(),
    })));
    #[allow(unused_mut)]
    let mut providers: Vec<Arc<dyn Provider>> = Vec::new();
    #[cfg(feature = "sftp")]
    let sftp = {
        let sftp = sftp::Sftp::new(credentials.clone());
        providers.push(Arc::new(sftp.provider.clone()));
        sftp
    };
    let storage: tauri_plugin_waypoint_vfs::StorageFactory<Wry> = Box::new(storage);
    Composed {
        options: tauri_plugin_waypoint_vfs::Options {
            providers,
            credentials: Some(credentials),
            storage: Some(storage),
            suggestions: Some(Arc::new(|| openssh_config::SshConfig::for_user().aliases())),
        },
        app,
        #[cfg(feature = "sftp")]
        sftp,
    }
}

/// The saved connections' storage (`connections.json`, key `connections`). A file that cannot be
/// opened leaves them in memory for this run, with a warning: they must never stop the app.
fn storage(app: &AppHandle<Wry>) -> Arc<dyn ConnectionStorage> {
    match FileKeyValue::open_file(app, CONNECTIONS_FILE) {
        Ok(kv) => Arc::new(ConnectionsPersistence::new(crate::settings::SettingsFile(
            kv,
        ))),
        Err(e) => {
            log::warn!("could not open the connections file, connections will not be saved: {e}");
            Arc::new(MemoryConnections::default())
        }
    }
}

impl Composed {
    /// Connects the providers to the saved connections once the plugin is set up: each provider
    /// follows the options and SSH settings the saved connections give their logins.
    pub fn wire(&self, app: &AppHandle<Wry>) {
        let _ = self.app.set(app.clone());
        let Some(vfs) = app.try_state::<tauri_plugin_waypoint_vfs::Vfs>() else {
            return;
        };
        let Some(hub) = vfs.connections() else {
            return;
        };
        #[cfg(feature = "sftp")]
        self.sftp.follow(hub);
        #[cfg(not(feature = "sftp"))]
        let _ = hub;
    }
}

#[cfg(feature = "sftp")]
mod sftp {
    //! The SFTP provider (#289): `~/.ssh/known_hosts` for host keys, `~/.ssh/config` with the
    //! saved connections' key files and jump hosts laid over it, and each saved connection's
    //! tuning.

    use std::path::PathBuf;
    use std::sync::{Arc, RwLock};
    use std::time::Duration;

    use openssh_config::{parse_jumps, HostConfig, SshConfig};
    use waypoint_connections::{host_override, ConnectionsHub, Credentials, SavedConnection};
    use waypoint_provider_sftp::{
        KnownHosts, MemoryKnownHosts, OpenSshKnownHosts, SftpConfig, SftpOptions, SftpProvider,
        SshConfigSource,
    };

    /// `~/.ssh/config`, read at each lookup (a lookup is one connect), with what the saved
    /// connections of a host add: their key file replaces the file's `IdentityFile`s, and their
    /// jump host its `ProxyJump`.
    #[derive(Default)]
    pub struct SavedSshConfig {
        saved: RwLock<Vec<SavedConnection>>,
    }

    impl SavedSshConfig {
        fn set(&self, saved: &[SavedConnection]) {
            *self.saved.write().unwrap_or_else(|e| e.into_inner()) = saved.to_vec();
        }
    }

    /// A saved key file as a path: `~/` is the home folder.
    fn expand(path: &str) -> PathBuf {
        match (path.strip_prefix("~/"), std::env::home_dir()) {
            (Some(rest), Some(home)) => home.join(rest),
            _ => PathBuf::from(path),
        }
    }

    pub(super) fn layered(
        config: HostConfig,
        saved: &[SavedConnection],
        alias: &str,
    ) -> HostConfig {
        let mut config = config;
        if let Some(over) = host_override(saved, alias) {
            if let Some(file) = over.key_file {
                config.identity_files = vec![expand(&file)];
            }
            if let Some(jump) = over.jump_host {
                config.proxy_jump = parse_jumps(&jump);
            }
        }
        config
    }

    impl SshConfigSource for SavedSshConfig {
        fn host(&self, alias: &str) -> HostConfig {
            let file = SshConfig::for_user().host(alias);
            let saved = self.saved.read().unwrap_or_else(|e| e.into_inner());
            layered(file, &saved, alias)
        }
    }

    /// A saved connection's tuning as the provider takes it.
    pub(super) fn options_of(saved: &SavedConnection) -> SftpOptions {
        let options = &saved.draft.options;
        let mut tuned = SftpOptions::default();
        if let Some(seconds) = options.timeout_seconds {
            tuned.timeout = Duration::from_secs(u64::from(seconds));
        }
        if let Some(requests) = options.listing_requests {
            tuned.listing_requests = requests as usize;
        }
        if let Some(requests) = options.transfer_requests {
            tuned.read_requests = requests as usize;
            tuned.write_requests = requests as usize;
        }
        if let Some(kib) = options.window_kib {
            tuned.window_size = kib.saturating_mul(1024);
        }
        tuned
    }

    pub struct Sftp {
        pub provider: SftpProvider,
        config: Arc<SavedSshConfig>,
    }

    impl Sftp {
        pub fn new(credentials: Arc<Credentials>) -> Self {
            let known_hosts: Arc<dyn KnownHosts> = match OpenSshKnownHosts::for_user() {
                Some(file) => Arc::new(file),
                None => {
                    log::warn!("no home folder: SSH host keys are trusted for this run only");
                    Arc::new(MemoryKnownHosts::new())
                }
            };
            let config = Arc::new(SavedSshConfig::default());
            let provider = SftpProvider::new(
                SftpConfig::new(known_hosts)
                    .with_credentials(credentials)
                    .with_ssh_config(config.clone()),
            );
            Self { provider, config }
        }

        /// Follows the saved connections: their SSH settings and their tuning.
        pub fn follow(&self, hub: &ConnectionsHub) {
            let (config, provider) = (self.config.clone(), self.provider.clone());
            hub.observe(move |saved| {
                config.set(saved);
                for connection in saved.iter().filter(|c| c.draft.scheme == "sftp") {
                    if let Some(key) = connection.key() {
                        provider.set_options(&key, options_of(connection));
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waypoint_connections::{check_draft, ConnectionDraft, ConnectionOptions, SavedConnection};

    #[test]
    fn a_keyring_that_is_not_there_yet_is_no_keyring() {
        let keyring = KeyringSecrets {
            app: Arc::new(OnceLock::new()),
        };
        assert_eq!(keyring.status(), Err(KeyringUnavailable::NoKeyring));
    }

    #[test]
    fn keyring_items_are_filed_under_the_login() {
        let key = waypoint_path::VfsPath::from_uri("sftp://me@nas.lan/")
            .unwrap()
            .connection_key()
            .unwrap();
        let filed = id(&key, SecretKind::Password);
        assert_eq!(filed.service, "connection");
        assert_eq!(filed.account, "sftp://me@nas.lan");
        assert_eq!(filed.kind, tauri_plugin_secrets::SecretKind::Password);
    }

    #[cfg(feature = "sftp")]
    #[test]
    fn saved_ssh_settings_and_tuning_reach_the_provider() {
        let saved = SavedConnection {
            id: "c1".into(),
            draft: check_draft(&ConnectionDraft {
                scheme: "sftp".into(),
                host: "nas.lan".into(),
                key_file: Some("/keys/work".into()),
                jump_host: Some("me@bastion:2200".into()),
                options: ConnectionOptions {
                    timeout_seconds: Some(10),
                    window_kib: Some(1024),
                    ..ConnectionOptions::default()
                },
                ..ConnectionDraft::default()
            })
            .unwrap()
            .draft,
        };
        let layered = sftp::layered(
            openssh_config::HostConfig::default(),
            std::slice::from_ref(&saved),
            "NAS.lan",
        );
        assert_eq!(
            layered.identity_files,
            [std::path::PathBuf::from("/keys/work")]
        );
        assert_eq!(layered.proxy_jump[0].host, "bastion");
        assert_eq!(layered.proxy_jump[0].port, Some(2200));
        let options = sftp::options_of(&saved);
        assert_eq!(options.timeout, Duration::from_secs(10));
        assert_eq!(options.window_size, 1024 * 1024);
    }
}
