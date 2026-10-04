// `SmbProvider` on Windows: Windows' own SMB client. An `smb://` location is the UNC path
// `\\host\share\path`, served by the local provider's code; `WNetAddConnection2` signs in with a
// password the app has, `NetShareEnum` lists the shares of `smb://host/`, and domain logins,
// Kerberos, signing and encryption are the operating system's, as in Explorer. Type-checked from
// Linux with `cargo xwin check`; not verified at runtime.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{HashMap, HashSet};
use std::os::windows::ffi::OsStrExt;
use std::sync::{Arc, Mutex, MutexGuard};

use waypoint_path::{CaseRule, ConnectionKey, FilePath, RemotePath, VfsPath};
use waypoint_protocol::{AuthPrompt, ConnectionState, Location, VfsError};
use waypoint_vfs::{
    CancelToken, Capabilities, ConnectAnswer, Credential, FileTimes, LocalProvider,
    PermissionModel, Provider, ReadStream, ScannedEntry, VolumeId, VolumeSpace, WriteOptions,
    WriteStream,
};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::NetworkManagement::NetManagement::NetApiBufferFree;
use windows::Win32::NetworkManagement::WNet::{
    WNetAddConnection2W, WNetCancelConnection2W, NETRESOURCEW, NET_CONNECT_FLAGS, RESOURCETYPE_DISK,
};
use windows::Win32::Storage::FileSystem::{NetShareEnum, SHARE_INFO_1, STYPE_MASK, STYPE_SPECIAL};

use crate::entries::folder;
use crate::failure::SmbFailure;
use crate::options::SmbConfig;
use crate::paths::{remote, split_login, target, Target};
use crate::unc::{failure_from_win32, unc_path, unc_server};

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

fn wide(text: &str) -> Vec<u16> {
    std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// The shares of a server, without the administrative ones and without printers and pipes.
fn share_names(server: &str) -> Result<Vec<String>, u32> {
    let server = wide(server);
    let mut buffer: *mut u8 = std::ptr::null_mut();
    let (mut read, mut total) = (0u32, 0u32);
    // SAFETY: the arguments follow `NetShareEnum`'s contract; the buffer it allocates is read only
    // for `read` entries and released with `NetApiBufferFree`.
    let status = unsafe {
        NetShareEnum(
            PCWSTR(server.as_ptr()),
            1,
            &mut buffer,
            u32::MAX,
            &mut read,
            &mut total,
            None,
        )
    };
    if status != 0 {
        return Err(status);
    }
    let mut names = Vec::new();
    // SAFETY: on success `buffer` points at `read` `SHARE_INFO_1` records with valid, NUL-ended
    // names, and stays valid until it is freed below.
    unsafe {
        let records = std::slice::from_raw_parts(buffer as *const SHARE_INFO_1, read as usize);
        for record in records {
            let kind = record.shi1_type.0;
            let disk = kind & STYPE_MASK.0 == 0;
            let special = kind & STYPE_SPECIAL.0 != 0;
            if disk && !special {
                if let Ok(name) = record.shi1_netname.to_string() {
                    names.push(name);
                }
            }
        }
        let _ = NetApiBufferFree(Some(buffer as *const std::ffi::c_void));
    }
    Ok(names)
}

/// Signs in to a server with a user name and password, for every later path to it.
fn add_connection(server: &str, user: &str, password: &str) -> u32 {
    let mut remote_name = wide(&format!(r"{server}\IPC$"));
    let resource = NETRESOURCEW {
        dwType: RESOURCETYPE_DISK,
        lpRemoteName: PWSTR(remote_name.as_mut_ptr()),
        ..NETRESOURCEW::default()
    };
    let (user, password) = (wide(user), wide(password));
    // SAFETY: the strings are NUL-ended and live through the call.
    let status: WIN32_ERROR = unsafe {
        WNetAddConnection2W(
            &resource,
            PCWSTR(password.as_ptr()),
            PCWSTR(user.as_ptr()),
            NET_CONNECT_FLAGS(0),
        )
    };
    status.0
}

fn cancel_connection(server: &str) {
    let name = wide(&format!(r"{server}\IPC$"));
    // SAFETY: the name is NUL-ended and lives through the call.
    let _ = unsafe { WNetCancelConnection2W(PCWSTR(name.as_ptr()), NET_CONNECT_FLAGS(0), false) };
}

/// The error with its location in `smb://` terms, since the local provider names the UNC path.
fn relocate(error: VfsError, location: &Location) -> VfsError {
    let at = location.clone();
    match error {
        VfsError::NotFound { .. } => VfsError::NotFound { location: at },
        VfsError::PermissionDenied { .. } => VfsError::PermissionDenied { location: at },
        VfsError::NotADirectory { .. } => VfsError::NotADirectory { location: at },
        VfsError::AlreadyExists { .. } => VfsError::AlreadyExists { location: at },
        VfsError::NotEmpty { .. } => VfsError::NotEmpty { location: at },
        VfsError::IsADirectory { .. } => VfsError::IsADirectory { location: at },
        VfsError::StorageFull { .. } => VfsError::StorageFull { location: at },
        VfsError::ReadOnly { .. } => VfsError::ReadOnly { location: at },
        VfsError::InUse { .. } => VfsError::InUse { location: at },
        VfsError::Io { message, .. } => VfsError::Io {
            message,
            location: Some(at),
        },
        other => other,
    }
}

/// The Windows SMB provider. Cloning shares its connections.
#[derive(Clone)]
pub struct SmbProvider {
    inner: Arc<Inner>,
}

struct Inner {
    config: SmbConfig,
    local: LocalProvider,
    /// Servers a password has been sent to (or that need none), so it is sent once.
    signed_in: Mutex<HashSet<ConnectionKey>>,
    states: Mutex<HashMap<ConnectionKey, ConnectionState>>,
}

impl SmbProvider {
    pub fn new(config: SmbConfig) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                local: LocalProvider::new(),
                signed_in: Mutex::new(HashSet::new()),
                states: Mutex::new(HashMap::new()),
            }),
        }
    }

    fn set_state(&self, key: &ConnectionKey, state: ConnectionState) {
        lock(&self.inner.states).insert(key.clone(), state);
    }

    /// Signs in with a password the person gave or the credential source has, once per server;
    /// without one the operating system's own login (single sign-on, saved credentials) is used.
    fn sign_in(
        &self,
        path: &RemotePath,
        location: &Location,
        answer: Option<&ConnectAnswer>,
    ) -> Result<(), VfsError> {
        let key = path.connection_key();
        if answer.is_none() && lock(&self.inner.signed_in).contains(&key) {
            return Ok(());
        }
        let named = path.authority().user.clone();
        let prompt = AuthPrompt::Password {
            user: named.clone(),
        };
        let credential = match answer {
            Some(ConnectAnswer::Credential(credential)) => Some(credential.clone()),
            _ => self.inner.config.credentials.credential(&key, &prompt),
        };
        if let Some(Credential::Password { user, password }) = credential {
            let (domain, user) = split_login(&user.or(named.clone()).unwrap_or_default());
            let who = if domain.is_empty() {
                user
            } else {
                format!(r"{domain}\{user}")
            };
            let Some(password) = password.expose_str() else {
                return Err(SmbFailure::BadCredentials.into_error(location));
            };
            let server = unc_server(path)?;
            self.set_state(&key, ConnectionState::Connecting);
            let status = add_connection(&server, &who, password);
            if let Some(failure) =
                failure_from_win32(status, named.clone(), true).filter(|_| status != 0)
            {
                if matches!(failure, SmbFailure::BadCredentials) {
                    self.inner.config.credentials.rejected(&key, &prompt);
                }
                let error = failure.into_error(location);
                self.set_state(
                    &key,
                    ConnectionState::Failed {
                        error: error.clone(),
                    },
                );
                return Err(error);
            }
        }
        lock(&self.inner.signed_in).insert(key.clone());
        self.set_state(&key, ConnectionState::Connected);
        Ok(())
    }

    /// The local file path of a location inside a share.
    fn file(path: &RemotePath) -> Result<VfsPath, VfsError> {
        let unc = unc_path(path)?;
        FilePath::from_path(unc)
            .map(VfsPath::File)
            .map_err(|error| VfsError::InvalidLocation {
                input: error.to_string(),
            })
    }

    /// Runs `op` on the UNC path of `path` with the local provider, signing in first and giving
    /// the errors back in `smb://` terms. An access error on a server nothing was sent a password
    /// to is the server asking for a login.
    fn through<T>(
        &self,
        path: &VfsPath,
        op: impl FnOnce(&LocalProvider, &VfsPath) -> Result<T, VfsError>,
    ) -> Result<T, VfsError> {
        let remote = remote(path)?;
        let location = path.to_location();
        if matches!(target(remote)?, Target::Shares) {
            return Err(VfsError::Unsupported {
                what: "the list of shares has no folder on disk".to_owned(),
            });
        }
        self.sign_in(remote, &location, None)?;
        let file = Self::file(remote)?;
        op(&self.inner.local, &file).map_err(|error| match relocate(error, &location) {
            VfsError::PermissionDenied { .. } if self.sent_no_password(remote) => {
                SmbFailure::CredentialsNeeded {
                    user: remote.authority().user.clone(),
                }
                .into_error(&location)
            }
            other => other,
        })
    }

    fn sent_no_password(&self, remote: &RemotePath) -> bool {
        let prompt = AuthPrompt::Password {
            user: remote.authority().user.clone(),
        };
        self.inner
            .config
            .credentials
            .credential(&remote.connection_key(), &prompt)
            .is_none()
    }

    /// The shares of the server `path` is on, as a folder of shares.
    fn shares(&self, path: &VfsPath) -> Result<Vec<ScannedEntry>, VfsError> {
        let remote = remote(path)?;
        let location = path.to_location();
        self.sign_in(remote, &location, None)?;
        let server = unc_server(remote)?;
        match share_names(&server) {
            Ok(names) => Ok(names.iter().map(|name| folder(name)).collect()),
            Err(code) => {
                let named = remote.authority().user.clone();
                Err(failure_from_win32(code, named, false)
                    .unwrap_or(SmbFailure::Unsupported("Windows could not list the shares"))
                    .into_error(&location))
            }
        }
    }
}

impl Provider for SmbProvider {
    fn scheme(&self) -> &'static str {
        "smb"
    }

    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::local();
        caps.remote = true;
        caps.watch = false;
        caps.case_rule = CaseRule::Insensitive;
        caps.symlinks = false;
        caps.permissions = PermissionModel::ReadOnlyFlag;
        caps
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        if matches!(target(remote(path)?)?, Target::Shares) {
            return Ok(folder(""));
        }
        self.through(path, |local, file| local.stat(file))
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        inline_link_budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        if matches!(target(remote(path)?)?, Target::Shares) {
            let shares = self.shares(path)?;
            progress(u32::try_from(shares.len()).unwrap_or(u32::MAX));
            return Ok(shares);
        }
        self.through(path, |local, file| {
            local.list(file, cancel, inline_link_budget, progress)
        })
    }

    fn resolve_link(
        &self,
        _folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        Ok(entry.clone())
    }

    fn connection_key(&self, path: &VfsPath) -> Option<ConnectionKey> {
        remote(path).ok().map(RemotePath::connection_key)
    }

    fn connection_state(&self, key: &ConnectionKey) -> ConnectionState {
        lock(&self.inner.states)
            .get(key)
            .cloned()
            .unwrap_or(ConnectionState::Idle)
    }

    fn connect(
        &self,
        key: &ConnectionKey,
        answer: Option<ConnectAnswer>,
        _cancel: &CancelToken,
    ) -> Result<(), VfsError> {
        let root = VfsPath::from_uri(key.as_str()).map_err(|_| VfsError::InvalidLocation {
            input: key.to_string(),
        })?;
        let remote = remote(&root)?;
        self.sign_in(remote, &root.to_location(), answer.as_ref())
    }

    fn disconnect(&self, key: &ConnectionKey) {
        if let Ok(VfsPath::Remote(root)) = VfsPath::from_uri(key.as_str()) {
            if let Ok(server) = unc_server(&root) {
                cancel_connection(&server);
            }
        }
        lock(&self.inner.signed_in).remove(key);
        self.set_state(key, ConnectionState::Idle);
    }

    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        self.through(path, |local, file| local.open_read(file))
    }

    fn open_read_at(&self, path: &VfsPath, start: u64) -> Result<ReadStream, VfsError> {
        self.through(path, |local, file| local.open_read_at(file, start))
    }

    fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.through(path, |local, file| local.create_dir(file))
    }

    fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.through(path, |local, file| local.create_file(file))
    }

    fn rename(&self, from: &VfsPath, to: &VfsPath, overwrite: bool) -> Result<(), VfsError> {
        let target_file = Self::file(remote(to)?)?;
        self.through(from, |local, file| {
            local.rename(file, &target_file, overwrite)
        })
    }

    fn remove_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.through(path, |local, file| local.remove_file(file))
    }

    fn remove_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.through(path, |local, file| local.remove_dir(file))
    }

    fn create_write(
        &self,
        path: &VfsPath,
        options: WriteOptions,
    ) -> Result<Box<dyn WriteStream>, VfsError> {
        self.through(path, |local, file| local.create_write(file, options))
    }

    fn set_times(&self, path: &VfsPath, times: FileTimes) -> Result<(), VfsError> {
        self.through(path, |local, file| local.set_times(file, times))
    }

    fn volume_id(&self, path: &VfsPath) -> Option<VolumeId> {
        let file = Self::file(remote(path).ok()?).ok()?;
        self.inner.local.volume_id(&file)
    }

    fn free_space(&self, path: &VfsPath) -> Option<VolumeSpace> {
        let file = Self::file(remote(path).ok()?).ok()?;
        self.inner.local.free_space(&file)
    }

    fn copy_file_within(
        &self,
        src: &VfsPath,
        dst: &VfsPath,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Option<Result<u64, VfsError>> {
        let (from, to) = (remote(src).ok()?, remote(dst).ok()?);
        if from.connection_key() != to.connection_key() {
            return None;
        }
        let (from, to) = (Self::file(from).ok()?, Self::file(to).ok()?);
        let location = dst.to_location();
        self.inner
            .local
            .copy_file_within(&from, &to, progress, cancel)
            .map(|result| result.map_err(|error| relocate(error, &location)))
    }
}
