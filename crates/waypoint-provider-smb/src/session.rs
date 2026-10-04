// One SMB session: reaching the server, logging in with NTLM, and the shares connected on it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use smb2::{ClientConfig, SmbClient, Tree};
use waypoint_path::RemotePath;
use waypoint_protocol::{AuthPrompt, Location, VfsError};
use waypoint_vfs::{ConnectAnswer, Credential, Secret};

use crate::errors::{from_login, from_smb2};
use crate::failure::SmbFailure;
use crate::options::{SmbConfig, SmbOptions};
use crate::paths::{socket_address, split_login};

/// An open, logged-in session: the library's client behind a lock (its calls take `&mut self`, so
/// one request runs at a time on the control connection) and the shares connected on it. File
/// streams own clones of the connection and run beside it.
pub(crate) struct Session {
    client: tokio::sync::Mutex<SmbClient>,
    trees: Mutex<HashMap<String, Tree>>,
    pub options: SmbOptions,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// The failure of a request that took longer than the connection's timeout.
pub(crate) fn timed_out(location: &Location) -> VfsError {
    VfsError::Timeout {
        location: location.clone(),
    }
}

impl Session {
    pub(crate) fn is_closed(&self) -> bool {
        // A busy client is in use, so it is open.
        self.client
            .try_lock()
            .map(|client| client.is_disconnected())
            .unwrap_or(false)
    }

    /// The control connection and the share's tree, connecting the share on first use. A share the
    /// server does not have is `NotFound` at `location`.
    pub(crate) async fn share(
        &self,
        share: &str,
        location: &Location,
    ) -> Result<(tokio::sync::MutexGuard<'_, SmbClient>, Tree), VfsError> {
        let mut client = self.client.lock().await;
        let key = share.to_lowercase();
        let cached = lock(&self.trees).get(&key).cloned();
        let tree = match cached {
            Some(tree) => tree,
            None => {
                let tree = tokio::time::timeout(self.options.timeout, client.connect_share(share))
                    .await
                    .map_err(|_| timed_out(location))?
                    .map_err(|error| from_smb2(&error, location))?;
                lock(&self.trees).insert(key, tree.clone());
                tree
            }
        };
        Ok((client, tree))
    }

    /// The control connection, for the requests that are not about one share.
    pub(crate) async fn control(&self) -> tokio::sync::MutexGuard<'_, SmbClient> {
        self.client.lock().await
    }

    pub(crate) async fn close(&self) {
        let trees: Vec<Tree> = lock(&self.trees).drain().map(|(_, tree)| tree).collect();
        let mut client = self.client.lock().await;
        for tree in trees {
            let _ =
                tokio::time::timeout(Duration::from_secs(2), client.disconnect_share(&tree)).await;
        }
    }
}

/// A login: who, and with what.
struct Login {
    domain: String,
    user: String,
    password: Secret,
    /// Whether a password came from the person or the credential source (a guest session offers
    /// none).
    offered: bool,
}

/// What opening a session needs besides its target.
pub(crate) struct Opening<'a> {
    pub config: &'a SmbConfig,
    pub options: SmbOptions,
    pub answer: Option<&'a ConnectAnswer>,
    pub location: &'a Location,
}

impl Opening<'_> {
    /// The login to try: the person's answer, then the credential source, then (for a location
    /// that names no user) a guest session. A location that names a user and has no password
    /// stops here with the question to ask.
    fn login(&self, remote: &RemotePath) -> Result<Login, VfsError> {
        let named = remote.authority().user.clone();
        let key = remote.connection_key();
        let prompt = AuthPrompt::Password {
            user: named.clone(),
        };
        let answer = match self.answer {
            Some(ConnectAnswer::Credential(credential)) => Some(credential.clone()),
            _ => self.config.credentials.credential(&key, &prompt),
        };
        match answer {
            Some(Credential::Password { user, password }) => {
                let who = user.or(named).unwrap_or_default();
                let (domain, user) = split_login(&who);
                Ok(Login {
                    domain,
                    user,
                    password,
                    offered: true,
                })
            }
            _ => {
                match named {
                    None => Ok(Login {
                        domain: String::new(),
                        user: String::new(),
                        password: Secret::new(Vec::new()),
                        offered: false,
                    }),
                    Some(user) => Err(SmbFailure::CredentialsNeeded { user: Some(user) }
                        .into_error(self.location)),
                }
            }
        }
    }

    pub(crate) async fn open(&self, remote: &RemotePath) -> Result<Session, VfsError> {
        let login = self.login(remote)?;
        let Some(password) = login.password.expose_str() else {
            return Err(SmbFailure::BadCredentials.into_error(self.location));
        };
        let shown = remote.authority().user.clone().or_else(|| {
            (!login.user.is_empty()).then(|| {
                if login.domain.is_empty() {
                    login.user.clone()
                } else {
                    format!("{};{}", login.domain, login.user)
                }
            })
        });
        let config = ClientConfig {
            addr: socket_address(remote),
            timeout: self.options.timeout,
            username: login.user.clone(),
            password: password.to_owned(),
            domain: login.domain.clone(),
            auto_reconnect: true,
            ..ClientConfig::default()
        };
        let connected = tokio::time::timeout(
            self.options.timeout + Duration::from_secs(1),
            SmbClient::connect(config),
        )
        .await
        .map_err(|_| timed_out(self.location))?;
        match connected {
            Ok(client) => Ok(Session {
                client: tokio::sync::Mutex::new(client),
                trees: Mutex::new(HashMap::new()),
                options: self.options,
            }),
            Err(error) => {
                let (error, rejected) =
                    from_login(&error, shown.as_deref(), login.offered, self.location);
                if rejected {
                    self.config.credentials.rejected(
                        &remote.connection_key(),
                        &AuthPrompt::Password { user: shown },
                    );
                }
                Err(error)
            }
        }
    }
}
