// OpenSSH's client files: the `~/.ssh/config` options a file manager needs, and the
// `known_hosts` file, read and written as OpenSSH does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A small crate with no knowledge of any application, so it can be published or moved to a
//! shared workspace:
//!
//! - `SshConfig` reads `Host`, `HostName`, `User`, `Port`, `IdentityFile` and `ProxyJump` from a
//!   client configuration, with `Include` (globs in the file name), `Host` pattern lists with `*`,
//!   `?` and negation, `Match all`, quoted arguments and OpenSSH's first-value-wins rule. Other
//!   `Match` blocks are skipped, and every other keyword is ignored.
//! - `KnownHostsFile` checks a server's key against the user's and the system's `known_hosts`
//!   (plain, wildcard, negated and hashed host names, `[host]:port`, `@revoked`), appends a
//!   trusted key and replaces a changed one without taking trust from any other host.

mod config;
mod known_hosts;
mod pattern;

pub use config::{parse_jumps, HostConfig, Jump, SshConfig};
pub use known_hosts::{host_label, KnownHostsFile, Lookup, RecordedKey};
