// One SSH session with its SFTP channel: reaching the server (through jump hosts when asked),
// checking its key, logging in and starting the SFTP subsystem.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use russh::client::{self, Handle};
use russh_sftp::client::RawSftpSession;
use waypoint_path::{ConnectionKey, Host};
use waypoint_protocol::{AuthPrompt, Location, VfsError};
use waypoint_vfs::{ConnectAnswer, Credential};

use crate::auth::{AuthStop, Login};
use crate::client::{Client, HostCheck, SessionTrust};
use crate::errors::{from_connect_io, from_russh, from_sftp};
use crate::options::{SftpConfig, SftpOptions};

/// Where one SSH hop goes and who logs in there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    /// The login, for credentials and the session pool.
    pub key: ConnectionKey,
    /// The name or address connected to (an SSH config alias already resolved).
    pub host: String,
    pub port: u16,
    pub user: String,
    pub identity_files: Vec<PathBuf>,
    /// The jump hosts to go through first, in order.
    pub jumps: Vec<Target>,
}

/// A host as a socket address reads it.
pub(crate) fn host_name(host: &Host) -> String {
    match host {
        Host::Name(name) => name.clone(),
        Host::Ipv4(addr) => addr.to_string(),
        Host::Ipv6 { addr, zone: None } => addr.to_string(),
        Host::Ipv6 {
            addr,
            zone: Some(zone),
        } => format!("{addr}%{zone}"),
    }
}

/// Which optional requests the server announced.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Extensions {
    /// `posix-rename@openssh.com`: a rename that replaces its target atomically.
    pub posix_rename: bool,
    /// `fsync@openssh.com`: commit a file's data to storage.
    pub fsync: bool,
    /// `lsetstat@openssh.com`: set attributes without following a symlink.
    pub lsetstat: bool,
    /// `statvfs@openssh.com`: free and total space.
    pub statvfs: bool,
    /// The server is OpenSSH (it announces `@openssh.com` extensions), which takes the two paths
    /// of `SSH_FXP_SYMLINK` in the opposite order from the draft.
    pub openssh: bool,
}

/// An open, logged-in session.
pub(crate) struct Session {
    pub sftp: RawSftpSession,
    pub extensions: Extensions,
    pub options: SftpOptions,
    handle: Handle<Client>,
    /// The sessions of the jump hosts this one runs through; dropping them closes it.
    _hops: Vec<Handle<Client>>,
}

impl Session {
    pub(crate) fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }

    pub(crate) async fn close(&self) {
        let _ = self.sftp.close_session();
        let _ = self
            .handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
    }
}

/// A login left waiting for keyboard-interactive answers (see `AuthStop::Waiting`).
pub(crate) struct Pending {
    handle: Handle<Client>,
    hops: Vec<Handle<Client>>,
    count: usize,
}

/// Why opening a session stopped.
pub(crate) enum OpenStop {
    Error(VfsError),
    Waiting(Box<Pending>, VfsError),
}

/// What opening a session needs besides its target.
pub(crate) struct Opening<'a> {
    pub config: &'a SftpConfig,
    pub options: SftpOptions,
    pub session_trust: &'a Arc<SessionTrust>,
    pub answer: Option<&'a ConnectAnswer>,
    pub location: &'a Location,
}

impl Opening<'_> {
    fn credential(&self) -> Option<&Credential> {
        match self.answer {
            Some(ConnectAnswer::Credential(credential)) => Some(credential),
            _ => None,
        }
    }

    fn ssh_config(&self) -> Arc<client::Config> {
        Arc::new(client::Config {
            window_size: self.options.window_size,
            keepalive_interval: Some(Duration::from_secs(15)),
            keepalive_max: 3,
            ..client::Config::default()
        })
    }

    /// Opens a session to `target`, or answers the login `pending` left waiting.
    pub(crate) async fn open(
        &self,
        target: &Target,
        pending: Option<Box<Pending>>,
    ) -> Result<Session, OpenStop> {
        let resumable =
            pending.filter(|pending| !pending.handle.is_closed() && self.credential().is_some());
        let (handle, hops) = match resumable.map(|pending| *pending) {
            Some(Pending {
                mut handle,
                hops,
                count,
            }) => {
                let login = self.login(target, self.credential());
                match login.resume(&mut handle, count).await {
                    Ok(()) => (handle, hops),
                    Err(stop) => return Err(self.stopped(stop, handle, hops)),
                }
            }
            None => self.reach(target).await?,
        };
        self.start_sftp(handle, hops).await.map_err(OpenStop::Error)
    }

    fn login<'b>(&'b self, target: &'b Target, answer: Option<&'b Credential>) -> Login<'b> {
        Login {
            key: &target.key,
            user: &target.user,
            identity_files: &target.identity_files,
            agent: &self.config.agent,
            credentials: self.config.credentials.as_ref(),
            answer,
            location: self.location,
        }
    }

    fn stopped(
        &self,
        stop: AuthStop,
        handle: Handle<Client>,
        hops: Vec<Handle<Client>>,
    ) -> OpenStop {
        match stop {
            AuthStop::Error(error) => OpenStop::Error(error),
            AuthStop::Waiting { prompt, count } => OpenStop::Waiting(
                Box::new(Pending {
                    handle,
                    hops,
                    count,
                }),
                auth_required(self.location, prompt),
            ),
        }
    }

    /// Connects through every jump host to the target and logs in on each.
    async fn reach(
        &self,
        target: &Target,
    ) -> Result<(Handle<Client>, Vec<Handle<Client>>), OpenStop> {
        let mut hops: Vec<Handle<Client>> = Vec::new();
        let chain: Vec<&Target> = target.jumps.iter().chain(std::iter::once(target)).collect();
        for (n, hop) in chain.iter().enumerate() {
            let last = n + 1 == chain.len();
            let mut handle = match hops.last() {
                None => self.handshake_tcp(hop).await?,
                Some(previous) => {
                    let channel = previous
                        .channel_open_direct_tcpip(
                            hop.host.clone(),
                            u32::from(hop.port),
                            "127.0.0.1",
                            0,
                        )
                        .await
                        .map_err(|error| OpenStop::Error(from_russh(&error, self.location)))?;
                    self.handshake(hop, channel.into_stream()).await?
                }
            };
            // The person's credential answers the target; a jump host logs in with what it finds.
            let answer = if last { self.credential() } else { None };
            let login = self.login(hop, answer);
            match login.authenticate(&mut handle).await {
                Ok(()) => {}
                Err(AuthStop::Waiting { prompt, count }) if last => {
                    return Err(OpenStop::Waiting(
                        Box::new(Pending {
                            handle,
                            hops,
                            count,
                        }),
                        auth_required(self.location, prompt),
                    ))
                }
                Err(AuthStop::Waiting { prompt, .. }) => {
                    return Err(OpenStop::Error(auth_required(self.location, prompt)))
                }
                Err(AuthStop::Error(error)) => return Err(OpenStop::Error(error)),
            }
            if last {
                return Ok((handle, hops));
            }
            hops.push(handle);
        }
        unreachable!("the chain always ends at the target")
    }

    async fn handshake_tcp(&self, hop: &Target) -> Result<Handle<Client>, OpenStop> {
        let location = self.location;
        let addresses: Vec<_> = tokio::net::lookup_host((hop.host.as_str(), hop.port))
            .await
            .map_err(|error| {
                log::debug!("sftp: cannot resolve {}: {error}", hop.host);
                OpenStop::Error(VfsError::Unreachable {
                    location: location.clone(),
                    reason: waypoint_protocol::UnreachableReason::NameNotResolved,
                })
            })?
            .collect();
        if addresses.is_empty() {
            return Err(OpenStop::Error(VfsError::Unreachable {
                location: location.clone(),
                reason: waypoint_protocol::UnreachableReason::NameNotResolved,
            }));
        }
        let socket = tokio::time::timeout(
            self.options.timeout,
            tokio::net::TcpStream::connect(&addresses[..]),
        )
        .await
        .map_err(|_| {
            OpenStop::Error(VfsError::Timeout {
                location: location.clone(),
            })
        })?
        .map_err(|error| OpenStop::Error(from_connect_io(&error, location)))?;
        let _ = socket.set_nodelay(true);
        self.handshake(hop, socket).await
    }

    async fn handshake<S>(&self, hop: &Target, stream: S) -> Result<Handle<Client>, OpenStop>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let check = Arc::new(HostCheck::new(
            self.config.known_hosts.clone(),
            &hop.host,
            hop.port,
            self.answer,
            self.session_trust.clone(),
        ));
        let client = Client {
            check: check.clone(),
        };
        let connecting = client::connect_stream(self.ssh_config(), stream, client);
        match tokio::time::timeout(self.options.timeout, connecting).await {
            Err(_) => Err(OpenStop::Error(VfsError::Timeout {
                location: self.location.clone(),
            })),
            Ok(Ok(handle)) => Ok(handle),
            Ok(Err(error)) => Err(OpenStop::Error(
                check
                    .refusal(self.location)
                    .unwrap_or_else(|| from_russh(&error, self.location)),
            )),
        }
    }

    async fn start_sftp(
        &self,
        handle: Handle<Client>,
        hops: Vec<Handle<Client>>,
    ) -> Result<Session, VfsError> {
        let location = self.location;
        let channel = handle
            .channel_open_session()
            .await
            .map_err(|error| from_russh(&error, location))?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|error| from_russh(&error, location))?;
        let config = russh_sftp::client::Config {
            request_timeout_secs: self.options.timeout.as_secs().max(1),
            ..russh_sftp::client::Config::default()
        };
        let sftp = RawSftpSession::new_with_config(channel.into_stream(), config);
        let version = sftp
            .init()
            .await
            .map_err(|error| from_sftp(&error, location))?;
        let announced = |name: &str| version.extensions.contains_key(name);
        let extensions = Extensions {
            posix_rename: announced("posix-rename@openssh.com"),
            fsync: announced("fsync@openssh.com"),
            lsetstat: announced("lsetstat@openssh.com"),
            statvfs: announced("statvfs@openssh.com"),
            openssh: version
                .extensions
                .keys()
                .any(|name| name.ends_with("@openssh.com")),
        };
        log::debug!("sftp: version {} with {extensions:?}", version.version);
        Ok(Session {
            sftp,
            extensions,
            options: self.options,
            handle,
            _hops: hops,
        })
    }
}

fn auth_required(location: &Location, prompt: AuthPrompt) -> VfsError {
    VfsError::AuthRequired {
        location: location.clone(),
        prompt: Box::new(prompt),
    }
}

#[cfg(test)]
mod tests {
    use waypoint_path::VfsPath;

    #[test]
    fn a_location_names_its_host_port_and_user() {
        let VfsPath::Remote(path) = VfsPath::from_uri("sftp://me@NAS.lan:2222/srv").unwrap() else {
            panic!()
        };
        let target = crate::ssh_config::resolve(&path, None, Some(&[]), &Vec::new);
        assert_eq!(target.host, "nas.lan");
        assert_eq!(target.port, 2222);
        assert_eq!(target.user, "me");
        assert_eq!(target.key.as_str(), "sftp://me@nas.lan:2222");
        let VfsPath::Remote(path) = VfsPath::from_uri("sftp://[::1]/").unwrap() else {
            panic!()
        };
        let target = crate::ssh_config::resolve(&path, None, Some(&[]), &Vec::new);
        assert_eq!(target.host, "::1");
        assert_eq!(target.port, 22);
        assert!(!target.user.is_empty());
    }
}
