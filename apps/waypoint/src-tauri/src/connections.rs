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
// The remote protocols are gated by the switches on Settings → Experimental (D167): a provider is
// built here but registered only while its switch is on, and `Gate` registers and turns off
// providers as the settings change, with no restart. Adding another protocol is one more entry in
// `compose`'s gate and, when its connections have tuning of their own, one more observer in `wire`.
// The `admin` scheme of Open as Administrator is gated the same way by its own switch, and
// registered only where the system can also start the helper (`elevate`).

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_secrets::{Secret as KeyringSecret, SecretId, Secrets, SecretsError};
use waypoint_connections::{
    ConnectionStorage, ConnectionsPersistence, Credentials, KeyringUnavailable, MemoryConnections,
    SecretKind, SecretStore, CONNECTIONS_FILE, KEYRING_SERVICE,
};
use waypoint_path::ConnectionKey;
use waypoint_settings::ExperimentalSettings;
use waypoint_vfs::{Provider, ProviderRegistry, Secret};

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

/// The remote protocols that have a switch on Settings → Experimental.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Sftp,
    // The other protocols' providers are behind Cargo features, so a variant can be unused in a
    // build that leaves its provider out.
    #[allow(dead_code)]
    Smb,
    #[allow(dead_code)]
    WebDav,
    #[allow(dead_code)]
    S3,
    /// The elevated helper's `admin` scheme: not a server, but gated by its switch in the same way.
    Administrator,
}

impl Protocol {
    /// Whether the person has turned the protocol on.
    pub fn is_on(self, settings: &ExperimentalSettings) -> bool {
        match self {
            Protocol::Sftp => settings.sftp,
            Protocol::Smb => settings.smb,
            Protocol::WebDav => settings.webdav,
            Protocol::S3 => settings.s3,
            Protocol::Administrator => settings.administrator_access,
        }
    }
}

/// The providers of the build that a switch gates, and what registers them. A provider is
/// registered while its switch is on and turned off while it is not, so nothing connects, listens
/// or reads a credential for a protocol that is off, and the registry (shared with the operations
/// engine) tells an address in it apart from one nothing serves.
#[derive(Default)]
pub struct Gate {
    protocols: Vec<(Protocol, Vec<Arc<dyn Provider>>)>,
    /// One change of the registry at a time, so two quick changes of the settings cannot cross.
    applying: Mutex<()>,
}

impl Gate {
    /// Gates `providers` (every scheme one protocol serves) behind the protocol's switch.
    pub fn add(&mut self, protocol: Protocol, providers: Vec<Arc<dyn Provider>>) {
        self.protocols.push((protocol, providers));
    }

    /// Makes `registry` match `settings`: registers the providers whose switch is on and turns
    /// off the others, ending their logins first through `close_logins` (called with the scheme,
    /// while its provider is still registered). Returns whether anything changed.
    pub fn apply(
        &self,
        registry: &ProviderRegistry,
        settings: &ExperimentalSettings,
        close_logins: &dyn Fn(&str),
    ) -> bool {
        let _one_at_a_time = self.applying.lock().unwrap_or_else(|e| e.into_inner());
        let mut changed = false;
        for (protocol, providers) in &self.protocols {
            let wanted = protocol.is_on(settings);
            for provider in providers {
                let scheme = provider.scheme();
                let serving = registry.serves(scheme);
                if wanted && !serving {
                    registry.register(provider.clone());
                    changed = true;
                } else if !wanted && (serving || !registry.is_off(scheme)) {
                    if serving {
                        close_logins(scheme);
                    }
                    registry.turn_off(scheme);
                    changed = true;
                }
            }
        }
        changed
    }
}

/// What `compose` makes before the app exists.
pub struct Composed {
    pub options: tauri_plugin_waypoint_vfs::Options<Wry>,
    /// Set once the app exists, so the keyring can be reached.
    pub app: AppCell,
    gate: Arc<Gate>,
    #[cfg(feature = "sftp")]
    sftp: sftp::Sftp,
    #[cfg(feature = "smb")]
    smb: smb::Smb,
    #[cfg(feature = "webdav")]
    webdav: webdav::WebDav,
    #[cfg(feature = "s3")]
    s3: s3::S3,
}

/// The providers, the credential source and the storage the vfs plugin is built with.
pub fn compose(archives: &crate::archives::Archives) -> Composed {
    let app: AppCell = Arc::new(OnceLock::new());
    let credentials = Arc::new(Credentials::new(Arc::new(KeyringSecrets {
        app: app.clone(),
    })));
    #[allow(unused_mut)]
    let mut providers: Vec<Arc<dyn Provider>> = Vec::new();
    #[allow(unused_mut)]
    let mut gate = Gate::default();
    #[cfg(feature = "sftp")]
    let sftp = {
        let sftp = sftp::Sftp::new(credentials.clone());
        gate.add(Protocol::Sftp, vec![Arc::new(sftp.provider.clone())]);
        sftp
    };
    #[cfg(feature = "smb")]
    let smb = {
        let smb = smb::Smb::new(credentials.clone());
        gate.add(Protocol::Smb, vec![Arc::new(smb.provider.clone())]);
        smb
    };
    #[cfg(feature = "webdav")]
    let webdav = {
        let webdav = webdav::WebDav::new(credentials.clone());
        gate.add(
            Protocol::WebDav,
            vec![Arc::new(webdav.davs.clone()), Arc::new(webdav.dav.clone())],
        );
        webdav
    };
    #[cfg(feature = "s3")]
    let s3 = {
        let s3 = s3::S3::new(credentials.clone());
        gate.add(Protocol::S3, vec![Arc::new(s3.provider.clone())]);
        s3
    };
    // Open as Administrator: the provider is built here and registered while its switch is on and
    // the system can start the helper. It connects only when asked, and starts nothing until then.
    gate.add(
        Protocol::Administrator,
        vec![Arc::new(waypoint_elevated::ElevatedProvider::new(
            Box::new(crate::elevate::ElevateLauncher::new(app.clone())),
        ))],
    );
    // A revision of a local repository browses read-only like any other location (A85).
    #[cfg(feature = "git")]
    providers.push(Arc::new(waypoint_provider_git::GitProvider::new()));
    // Archives are served beside the servers, and reach server-held archive files through the same registry (A91).
    #[cfg(feature = "archive")]
    providers.push(archives.provider.clone());
    #[cfg(not(feature = "archive"))]
    let _ = archives;
    let storage: tauri_plugin_waypoint_vfs::StorageFactory<Wry> = Box::new(storage);
    Composed {
        options: tauri_plugin_waypoint_vfs::Options {
            providers,
            credentials: Some(credentials),
            storage: Some(storage),
            suggestions: Some(Arc::new(|| openssh_config::SshConfig::for_user().aliases())),
        },
        app,
        gate: Arc::new(gate),
        #[cfg(feature = "sftp")]
        sftp,
        #[cfg(feature = "smb")]
        smb,
        #[cfg(feature = "webdav")]
        webdav,
        #[cfg(feature = "s3")]
        s3,
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
        let registry = vfs.remote().clone();
        let manager = vfs.connections().map(|hub| hub.manager().clone());
        // Before any window exists, the switches decide which protocols a restored tab can open;
        // after, each change registers or turns off providers and tells every window.
        let gate = self.gate.clone();
        let apply = {
            let (app, registry) = (app.clone(), registry.clone());
            move |settings: &ExperimentalSettings| {
                // A switch that is on does nothing where the system cannot start the helper.
                let settings = ExperimentalSettings {
                    administrator_access: settings.administrator_access
                        && crate::elevate::available(&app),
                    ..*settings
                };
                let changed = gate.apply(&registry, &settings, &|scheme| {
                    if let Some(manager) = &manager {
                        manager.close_scheme(scheme);
                    }
                });
                if changed {
                    tauri_plugin_waypoint_vfs::announce_protocols(&app);
                }
            }
        };
        apply(&crate::settings::current(app).experimental);
        if let Some(store) = app.try_state::<tauri_plugin_waypoint_settings::SettingsStore<Wry>>() {
            store.on_change(move |settings| apply(&settings.experimental));
        }
        let Some(hub) = vfs.connections() else {
            return;
        };
        #[cfg(feature = "sftp")]
        self.sftp.follow(hub);
        #[cfg(feature = "smb")]
        self.smb.follow(hub);
        #[cfg(feature = "webdav")]
        self.webdav.follow(hub);
        #[cfg(feature = "s3")]
        self.s3.follow(hub);
        #[cfg(not(any(feature = "sftp", feature = "smb", feature = "webdav", feature = "s3")))]
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

#[cfg(feature = "smb")]
mod smb {
    //! The SMB provider (#293): the pure Rust client on Linux and the operating system's own on
    //! Windows (A99, A100), signing in with the one credential source, and each saved connection's
    //! tuning.

    use std::sync::Arc;
    use std::time::Duration;

    use waypoint_connections::{ConnectionsHub, Credentials, SavedConnection};
    use waypoint_protocol::PluginStatus;
    use waypoint_provider_smb::{
        availability, Engine, SmbConfig, SmbOptions, SmbProvider, Support,
    };

    /// A saved connection's tuning as the provider takes it. On Windows the system's client has
    /// none to tune, so only the tests read it there.
    #[cfg_attr(windows, allow(dead_code))]
    pub(super) fn options_of(saved: &SavedConnection) -> SmbOptions {
        let options = &saved.draft.options;
        let mut tuned = SmbOptions::default();
        if let Some(seconds) = options.timeout_seconds {
            tuned.timeout = Duration::from_secs(u64::from(seconds));
        }
        if let Some(requests) = options.transfer_requests {
            tuned.read_requests = requests as usize;
        }
        tuned
    }

    /// What this build can do, for the Services panel: the logins and protections that work, and
    /// the first one that does not, with the provider's own reason.
    pub(super) fn detail() -> PluginStatus {
        let found = availability();
        if found.engine == Engine::None {
            return PluginStatus::unavailable("this build of Waypoint has no SMB client");
        }
        let all = [
            ("ntlm", found.ntlm),
            ("kerberos", found.kerberos),
            ("signing", found.signing),
            ("encryption", found.encryption),
            ("share-browser", found.share_browser),
        ];
        let reason = all.iter().find_map(|(_, support)| match support {
            Support::Unsupported { reason } => Some((*reason).to_owned()),
            Support::Supported => None,
        });
        PluginStatus {
            available: true,
            reason,
            features: all
                .iter()
                .filter(|(_, support)| support.is_supported())
                .map(|(name, _)| (*name).to_owned())
                .collect(),
        }
    }

    pub struct Smb {
        pub provider: SmbProvider,
    }

    impl Smb {
        pub fn new(credentials: Arc<Credentials>) -> Self {
            Self {
                provider: SmbProvider::new(SmbConfig::new().with_credentials(credentials)),
            }
        }

        /// Follows the saved connections' tuning. On Windows the system's client does the work and
        /// has none to tune.
        pub fn follow(&self, hub: &ConnectionsHub) {
            #[cfg(not(windows))]
            {
                let provider = self.provider.clone();
                hub.observe(move |saved| {
                    for connection in saved.iter().filter(|c| c.draft.scheme == "smb") {
                        if let Some(key) = connection.key() {
                            provider.set_options(&key, options_of(connection));
                        }
                    }
                });
            }
            #[cfg(windows)]
            let _ = hub;
        }
    }
}

#[cfg(feature = "webdav")]
mod webdav {
    //! The WebDAV providers (#294): one for `davs://` and one for `dav://`, signing in with the one
    //! credential source, and each saved connection's sign-in, dialect and tuning.

    use std::sync::Arc;
    use std::time::Duration;

    use waypoint_connections::{
        AuthMethod, ConnectionsHub, Credentials, DavAuth, DavPreset, SavedConnection,
    };
    use waypoint_protocol::PluginStatus;
    use waypoint_provider_webdav::{AuthMode, Preset, WebDavConfig, WebDavOptions, WebDavProvider};

    /// How a saved connection answers a server that asks who is calling.
    fn auth_mode(saved: &SavedConnection) -> AuthMode {
        match saved.draft.auth {
            AuthMethod::Token => AuthMode::Bearer,
            AuthMethod::Password => match saved.draft.options.dav_auth {
                Some(DavAuth::Basic) => AuthMode::Basic,
                Some(DavAuth::Digest) => AuthMode::Digest,
                Some(DavAuth::Auto) | None => AuthMode::Auto,
            },
            AuthMethod::Auto | AuthMethod::KeyFile => AuthMode::Auto,
        }
    }

    /// A saved connection's tuning, sign-in and dialect as the provider takes them.
    pub(super) fn options_of(saved: &SavedConnection) -> WebDavOptions {
        let options = &saved.draft.options;
        let mut tuned = WebDavOptions::default().with_auth(auth_mode(saved));
        if let Some(seconds) = options.timeout_seconds {
            tuned = tuned.with_timeout(Duration::from_secs(u64::from(seconds)));
        }
        if let Some(requests) = options.transfer_requests {
            tuned = tuned.with_max_requests(requests as usize);
        }
        tuned.with_preset(match options.dav_preset {
            Some(DavPreset::Generic) => Preset::Generic,
            Some(DavPreset::Nextcloud) => Preset::Nextcloud,
            Some(DavPreset::Auto) | None => Preset::Auto,
        })
    }

    pub(super) fn detail() -> PluginStatus {
        PluginStatus::available(
            ["basic", "digest", "bearer", "nextcloud", "tls"]
                .map(str::to_owned)
                .to_vec(),
        )
    }

    pub struct WebDav {
        pub davs: WebDavProvider,
        pub dav: WebDavProvider,
    }

    impl WebDav {
        pub fn new(credentials: Arc<Credentials>) -> Self {
            let config = WebDavConfig::new().with_credentials(credentials);
            Self {
                davs: WebDavProvider::davs(config.clone()),
                dav: WebDavProvider::dav(config),
            }
        }

        /// Follows the saved connections: their sign-in, dialect and tuning, for the provider of
        /// each one's scheme.
        pub fn follow(&self, hub: &ConnectionsHub) {
            let (davs, dav) = (self.davs.clone(), self.dav.clone());
            hub.observe(move |saved| {
                for connection in saved.iter() {
                    let provider = match connection.draft.scheme.as_str() {
                        "davs" => &davs,
                        "dav" => &dav,
                        _ => continue,
                    };
                    if let Some(key) = connection.key() {
                        provider.set_options(&key, options_of(connection));
                    }
                }
            });
        }
    }
}

#[cfg(feature = "s3")]
mod s3 {
    //! The S3 provider (#295): buckets on AWS and on S3-compatible services, signing in with the
    //! one credential source, and each saved connection's access key id, region and addressing.
    //! The AWS environment and `~/.aws` files are not offered: the access key is the connection's
    //! own.

    use std::sync::Arc;

    use waypoint_connections::{ConnectionsHub, Credentials, SavedConnection};
    use waypoint_protocol::PluginStatus;
    use waypoint_provider_s3::{S3Config, S3Options, S3Provider};

    /// A saved connection's key id, region and addressing as the provider takes them. The
    /// endpoint is part of the connection's address, so it is not an option.
    pub(super) fn options_of(saved: &SavedConnection) -> S3Options {
        let options = &saved.draft.options;
        S3Options {
            region: options.s3_region.clone(),
            path_style: options.s3_path_style,
            access_key_id: saved.draft.user.clone(),
            ..S3Options::default()
        }
    }

    pub(super) fn detail() -> PluginStatus {
        PluginStatus::available(
            [
                "multipart",
                "server-copy",
                "resume",
                "storage-class",
                "presets",
                "tls",
            ]
            .map(str::to_owned)
            .to_vec(),
        )
    }

    pub struct S3 {
        pub provider: S3Provider,
    }

    impl S3 {
        pub fn new(credentials: Arc<Credentials>) -> Self {
            Self {
                provider: S3Provider::new(S3Config::new(credentials)),
            }
        }

        /// Follows the saved connections: each one's access key id, region and addressing.
        pub fn follow(&self, hub: &ConnectionsHub) {
            let provider = self.provider.clone();
            hub.observe(move |saved| {
                for connection in saved.iter().filter(|c| c.draft.scheme == "s3") {
                    if let Some(key) = connection.key() {
                        provider.set_options(&key, options_of(connection));
                    }
                }
            });
        }
    }
}

/// What each remote protocol this build includes can do, for the Services panel: the logins and
/// protections that work and the first that does not, in the provider's own words. A protocol the
/// build leaves out is not listed. Whether it is turned on is the vfs plugin's `connection_support`.
#[tauri::command]
pub fn get_protocol_details() -> std::collections::HashMap<String, waypoint_protocol::PluginStatus>
{
    #[allow(unused_mut)]
    let mut details = std::collections::HashMap::new();
    #[cfg(feature = "smb")]
    details.insert("smb".to_owned(), smb::detail());
    #[cfg(feature = "webdav")]
    details.insert("webdav".to_owned(), webdav::detail());
    #[cfg(feature = "s3")]
    details.insert("s3".to_owned(), s3::detail());
    details
}

/// The address of a person's files on a Nextcloud server, from the address they know and their
/// account's id (`davs://alice@cloud.example.com/remote.php/dav/files/alice`), for the Connect
/// dialog's Nextcloud preset. The dialog reads it back through `parse_address_text`, so the host,
/// user and folder it fills in are the canonical ones.
#[tauri::command]
pub fn nextcloud_address(
    server: String,
    user: String,
) -> Result<String, waypoint_protocol::VfsError> {
    #[cfg(feature = "webdav")]
    {
        waypoint_provider_webdav::nextcloud_root(&server, &user).map(|path| path.to_uri())
    }
    #[cfg(not(feature = "webdav"))]
    {
        let _ = (server, user);
        Err(waypoint_protocol::VfsError::Unsupported {
            what: "WebDAV".to_owned(),
        })
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

    fn fake(scheme: waypoint_path::RemoteScheme) -> Arc<dyn Provider> {
        Arc::new(waypoint_vfs::FakeRemoteProvider::new(
            scheme,
            waypoint_path::CaseRule::Sensitive,
        ))
    }

    fn gate_of_two() -> Gate {
        let mut gate = Gate::default();
        gate.add(
            Protocol::Sftp,
            vec![fake(waypoint_path::RemoteScheme::Sftp)],
        );
        gate.add(
            Protocol::WebDav,
            vec![
                fake(waypoint_path::RemoteScheme::Dav),
                fake(waypoint_path::RemoteScheme::Davs),
            ],
        );
        gate
    }

    #[test]
    fn a_fresh_profile_registers_no_remote_protocol_and_marks_each_one_off() {
        let (gate, registry) = (gate_of_two(), ProviderRegistry::new());
        assert!(gate.apply(&registry, &ExperimentalSettings::default(), &|_| {}));
        assert!(registry.schemes().is_empty());
        assert_eq!(registry.off_schemes(), ["dav", "davs", "sftp"]);
        // Nothing changes the second time.
        assert!(!gate.apply(&registry, &ExperimentalSettings::default(), &|_| {}));
    }

    #[test]
    fn each_switch_turns_on_its_own_protocol_and_nothing_else() {
        let (gate, registry) = (gate_of_two(), ProviderRegistry::new());
        gate.apply(&registry, &ExperimentalSettings::default(), &|_| {});
        let sftp = ExperimentalSettings {
            sftp: true,
            ..ExperimentalSettings::default()
        };
        assert!(gate.apply(&registry, &sftp, &|_| {}));
        assert_eq!(registry.schemes(), ["sftp"]);
        assert_eq!(registry.off_schemes(), ["dav", "davs"]);
        // A protocol with two schemes turns on and off as one.
        let webdav = ExperimentalSettings {
            webdav: true,
            ..ExperimentalSettings::default()
        };
        assert!(gate.apply(&registry, &webdav, &|_| {}));
        assert_eq!(registry.schemes(), ["dav", "davs"]);
        assert_eq!(registry.off_schemes(), ["sftp"]);
    }

    #[test]
    fn turning_a_switch_off_ends_its_logins_first_and_only_for_what_was_serving() {
        let (gate, registry) = (gate_of_two(), ProviderRegistry::new());
        let both = ExperimentalSettings {
            sftp: true,
            webdav: true,
            ..ExperimentalSettings::default()
        };
        gate.apply(&registry, &both, &|_| {});
        let closed = Mutex::new(Vec::new());
        let sftp_only = ExperimentalSettings {
            sftp: true,
            ..ExperimentalSettings::default()
        };
        assert!(gate.apply(&registry, &sftp_only, &|scheme| {
            // The provider is still registered while its logins end.
            assert!(registry.serves(scheme));
            closed.lock().unwrap().push(scheme.to_owned());
        }));
        assert_eq!(*closed.lock().unwrap(), ["dav", "davs"]);
        assert_eq!(registry.schemes(), ["sftp"]);
        assert!(registry.is_off("dav") && registry.is_off("davs"));
    }

    fn administrator_gate() -> Gate {
        let launcher = waypoint_elevated::testing::loopback::LoopbackLauncher::new(
            Arc::new(waypoint_vfs::LocalProvider::new()),
            waypoint_elevated::ServeConfig::default(),
        );
        let mut gate = Gate::default();
        gate.add(
            Protocol::Administrator,
            vec![Arc::new(waypoint_elevated::ElevatedProvider::new(
                Box::new(launcher),
            ))],
        );
        gate
    }

    #[test]
    fn administrator_access_is_off_until_its_switch_is_on_and_fails_as_a_protocol_that_is_off() {
        let (gate, registry) = (administrator_gate(), ProviderRegistry::new());
        assert!(gate.apply(&registry, &ExperimentalSettings::default(), &|_| {}));
        assert!(registry.schemes().is_empty());
        assert_eq!(registry.off_schemes(), ["admin"]);
        let admin = waypoint_path::VfsPath::File(
            waypoint_path::FilePath::from_path(std::env::temp_dir()).unwrap(),
        )
        .elevated()
        .unwrap();
        assert!(matches!(
            registry.for_path(&admin),
            Err(waypoint_protocol::VfsError::ProtocolOff { scheme }) if scheme == "admin"
        ));
        let on = ExperimentalSettings {
            administrator_access: true,
            ..ExperimentalSettings::default()
        };
        assert!(gate.apply(&registry, &on, &|_| {}));
        assert_eq!(registry.schemes(), ["admin"]);
        assert!(registry.for_path(&admin).is_ok());
        // Turning it off ends its connection first, while the provider is still registered.
        let closed = Mutex::new(Vec::new());
        assert!(
            gate.apply(&registry, &ExperimentalSettings::default(), &|scheme| {
                assert!(registry.serves(scheme));
                closed.lock().unwrap().push(scheme.to_owned());
            })
        );
        assert_eq!(*closed.lock().unwrap(), ["admin"]);
        assert!(registry.for_path(&admin).is_err());
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

    #[cfg(feature = "smb")]
    #[test]
    fn an_smb_connections_tuning_reaches_its_provider_and_the_panel_says_what_it_can_do() {
        let saved = SavedConnection {
            id: "c1".into(),
            draft: check_draft(&ConnectionDraft {
                scheme: "smb".into(),
                host: "files.lan".into(),
                user: Some("WORK;me".into()),
                options: ConnectionOptions {
                    timeout_seconds: Some(12),
                    transfer_requests: Some(4),
                    ..ConnectionOptions::default()
                },
                ..ConnectionDraft::default()
            })
            .unwrap()
            .draft,
        };
        let options = smb::options_of(&saved);
        assert_eq!(options.timeout, Duration::from_secs(12));
        assert_eq!(options.read_requests, 4);
        let detail = smb::detail();
        assert!(detail.available);
        assert!(detail.features.iter().any(|f| f == "ntlm"));
        assert!(detail.features.iter().any(|f| f == "share-browser"));
        if !cfg!(windows) {
            // The reason is the provider's own, for what the Linux client cannot do.
            assert!(detail
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains("Kerberos"));
        }
    }

    #[cfg(feature = "webdav")]
    #[test]
    fn a_webdav_connections_sign_in_dialect_and_tuning_reach_its_provider() {
        use waypoint_connections::{AuthMethod, DavAuth, DavPreset};
        use waypoint_provider_webdav::{AuthMode, Preset};
        let saved = |auth, dav_auth, dav_preset| SavedConnection {
            id: "c1".into(),
            draft: check_draft(&ConnectionDraft {
                scheme: "davs".into(),
                host: "cloud.example.com".into(),
                auth,
                options: ConnectionOptions {
                    timeout_seconds: Some(20),
                    transfer_requests: Some(3),
                    dav_auth,
                    dav_preset,
                    ..ConnectionOptions::default()
                },
                ..ConnectionDraft::default()
            })
            .unwrap()
            .draft,
        };
        let basic = webdav::options_of(&saved(
            AuthMethod::Password,
            Some(DavAuth::Basic),
            Some(DavPreset::Nextcloud),
        ));
        assert_eq!(basic.auth, AuthMode::Basic);
        assert_eq!(basic.preset, Preset::Nextcloud);
        assert_eq!(basic.timeout, Duration::from_secs(20));
        assert_eq!(basic.max_requests, 3);
        let token = webdav::options_of(&saved(AuthMethod::Token, Some(DavAuth::Digest), None));
        assert_eq!(
            token.auth,
            AuthMode::Bearer,
            "a token ignores the password type"
        );
        assert_eq!(token.preset, Preset::Auto);
        let digest = webdav::options_of(&saved(AuthMethod::Password, Some(DavAuth::Digest), None));
        assert_eq!(digest.auth, AuthMode::Digest);
        let auto = webdav::options_of(&saved(AuthMethod::Auto, Some(DavAuth::Basic), None));
        assert_eq!(
            auto.auth,
            AuthMode::Auto,
            "the password type is for a password"
        );
    }

    #[cfg(feature = "s3")]
    #[test]
    fn an_s3_connections_key_id_region_and_addressing_reach_its_provider() {
        let saved = |user: Option<&str>, path_style| SavedConnection {
            id: "c1".into(),
            draft: check_draft(&ConnectionDraft {
                scheme: "s3".into(),
                host: "photos".into(),
                user: user.map(str::to_owned),
                options: ConnectionOptions {
                    s3_endpoint: Some("https://s3.us-west-004.backblazeb2.com".into()),
                    s3_region: Some("us-west-004".into()),
                    s3_path_style: path_style,
                    ..ConnectionOptions::default()
                },
                ..ConnectionDraft::default()
            })
            .unwrap()
            .draft,
        };
        let options = s3::options_of(&saved(Some("004abc"), Some(true)));
        assert_eq!(options.access_key_id.as_deref(), Some("004abc"));
        assert_eq!(options.region.as_deref(), Some("us-west-004"));
        assert_eq!(options.path_style, Some(true));
        let unnamed = s3::options_of(&saved(None, None));
        assert_eq!(unnamed.access_key_id, None);
        assert_eq!(unnamed.path_style, None);
    }

    #[cfg(all(feature = "smb", feature = "webdav", feature = "s3"))]
    #[test]
    fn the_details_list_the_protocols_this_build_includes() {
        let details = get_protocol_details();
        assert!(details.contains_key("smb") && details.contains_key("webdav"));
        assert!(details["s3"].features.iter().any(|f| f == "storage-class"));
        assert!(details["webdav"].features.iter().any(|f| f == "bearer"));
    }

    #[cfg(feature = "webdav")]
    #[test]
    fn the_nextcloud_preset_writes_the_address_of_a_persons_files() {
        assert_eq!(
            nextcloud_address("cloud.example.com".into(), "alice".into()).unwrap(),
            "davs://alice@cloud.example.com/remote.php/dav/files/alice"
        );
        assert_eq!(
            nextcloud_address(
                "https://cloud.example.com/nextcloud/".into(),
                "alice".into()
            )
            .unwrap(),
            "davs://alice@cloud.example.com/nextcloud/remote.php/dav/files/alice"
        );
        assert!(nextcloud_address("".into(), "alice".into()).is_err());
        assert!(nextcloud_address("cloud.example.com".into(), "".into()).is_err());
    }

    #[test]
    fn the_real_providers_register_behind_their_switches_and_no_other_does() {
        // Each Cargo feature adds its provider to the gate; with every switch off none is served.
        let (app, registry) = (
            Arc::new(OnceLock::<AppHandle<Wry>>::new()),
            ProviderRegistry::new(),
        );
        let credentials = Arc::new(Credentials::new(Arc::new(KeyringSecrets { app })));
        let mut gate = Gate::default();
        #[cfg(feature = "smb")]
        gate.add(
            Protocol::Smb,
            vec![Arc::new(smb::Smb::new(credentials.clone()).provider)],
        );
        #[cfg(feature = "webdav")]
        {
            let dav = webdav::WebDav::new(credentials.clone());
            gate.add(
                Protocol::WebDav,
                vec![Arc::new(dav.davs), Arc::new(dav.dav)],
            );
        }
        #[cfg(feature = "s3")]
        gate.add(
            Protocol::S3,
            vec![Arc::new(s3::S3::new(credentials.clone()).provider)],
        );
        let _ = credentials;
        gate.apply(&registry, &ExperimentalSettings::default(), &|_| {});
        assert!(registry.schemes().is_empty());
        gate.apply(
            &registry,
            &ExperimentalSettings {
                smb: true,
                webdav: true,
                s3: true,
                ..ExperimentalSettings::default()
            },
            &|_| {},
        );
        let mut served = registry.schemes();
        served.sort_unstable();
        let expected: Vec<&str> = [
            cfg!(feature = "webdav").then_some("dav"),
            cfg!(feature = "webdav").then_some("davs"),
            cfg!(feature = "s3").then_some("s3"),
            cfg!(feature = "smb").then_some("smb"),
        ]
        .into_iter()
        .flatten()
        .collect();
        assert_eq!(served, expected);
    }
}
