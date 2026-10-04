// Waypoint's SMB provider: `smb://` locations, the share browser and what this build can do.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `SmbProvider` implements `waypoint_vfs::Provider` for `smb://[domain;user@]host[:port]/share/path`
//! locations (`docs/architecture/remote-locations.md`). `smb://host/` is the share browser: a
//! folder of the server's shares, listed before one is chosen.
//!
//! On Linux the provider speaks SMB 2 and 3 itself through the pure Rust `smb2` client, behind
//! this crate's `client` feature (spike #278). It keeps one session per `ConnectionKey`, opened
//! lazily by the first call that needs it and reopened after it drops, logs in with NTLM (a user
//! name, an optional domain and a password), and never waits on a person inside a call: a login it
//! cannot complete with the app's `CredentialSource` fails with `AuthRequired`, for the app to ask
//! and answer through `Provider::connect`. Signing and encryption are negotiated by the library.
//! Kerberos is not offered by the library's client, and `availability` says so.
//!
//! On Windows SMB is the operating system's own client: an `smb://` location is the UNC path
//! `\\host\share\path` served by the local provider's code, `WNetAddConnection2` signs in with the
//! person's password and `NetShareEnum` is the share browser, so domain logins, Kerberos and
//! signing work as in Explorer (A99). That path is type-checked from Linux and is not verified at
//! runtime.
//!
//! Nothing here stores a secret: a password comes from the `CredentialSource` or the person's
//! answer for the one connection attempt that needs it.

mod availability;
#[cfg(any(windows, feature = "client"))]
mod entries;
mod failure;
mod options;
mod paths;
mod unc;

#[cfg(windows)]
mod os;

#[cfg(all(feature = "client", not(windows)))]
mod changes;
#[cfg(all(feature = "client", not(windows)))]
mod errors;
#[cfg(all(feature = "client", not(windows)))]
mod listing;
#[cfg(all(feature = "client", not(windows)))]
mod pool;
#[cfg(all(feature = "client", not(windows)))]
mod provider;
#[cfg(all(feature = "client", not(windows)))]
mod read;
#[cfg(all(feature = "client", not(windows)))]
mod session;
#[cfg(all(feature = "client", not(windows)))]
mod write;

pub use availability::{availability, Availability, Engine, Support};
pub use failure::SmbFailure;
pub use options::{SmbConfig, SmbOptions};
#[cfg(windows)]
pub use os::SmbProvider;
#[cfg(all(feature = "client", not(windows)))]
pub use provider::SmbProvider;
