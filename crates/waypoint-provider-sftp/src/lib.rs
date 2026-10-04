// Waypoint's SFTP provider: `sftp://` locations served over SSH through `russh` and `russh-sftp`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `SftpProvider` implements `waypoint_vfs::Provider` for `sftp://[user@]host[:port]/path`
//! locations (`docs/architecture/remote-locations.md`). It keeps one SSH session per
//! `ConnectionKey`, opened lazily by the first call that needs it and reopened after it drops, and
//! it never waits on a person inside a call: a login it cannot complete with the SSH agent, key
//! files and the app's `CredentialSource` fails with `AuthRequired`, and a server key it does not
//! know fails with `HostKeyUnknown`, for the app to ask and answer through `Provider::connect`.
//!
//! The provider is synchronous, as the trait is, and runs the asynchronous SSH library on a small
//! Tokio runtime of its own. Listings keep a window of `readdir` requests in flight and hand each
//! reply's entries over as they arrive; reads keep a window of 32 KiB requests in flight
//! (spike #278, `milestone-6-spikes.md`).
//!
//! Nothing here stores a secret: passwords and passphrases come from the `CredentialSource` or the
//! person's answer for the one connection attempt that needs them.

mod auth;
mod client;
mod errors;
mod host_keys;
mod known_hosts_file;
mod listing;
mod options;
mod paths;
mod pool;
mod provider;
mod read;
mod session;
mod ssh_config;

pub use host_keys::{HostKeyCheck, KnownHosts, MemoryKnownHosts, ServerKey};
pub use known_hosts_file::OpenSshKnownHosts;
pub use openssh_config::{HostConfig, Jump, SshConfig};
pub use options::{AgentSource, SftpConfig, SftpOptions};
pub use provider::SftpProvider;
pub use ssh_config::SshConfigSource;
