// Waypoint's WebDAV provider: `dav://` and `davs://` locations over HTTP, with a Nextcloud preset.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `WebDavProvider` implements `waypoint_vfs::Provider` for `davs://[user@]host[:port]/path`
//! (HTTPS) and `dav://…` (plain HTTP, for local servers) locations
//! (`docs/architecture/remote-locations.md`). One provider serves one scheme.
//!
//! The client is a thin layer over `reqwest` and `quick-xml` (spike #278): a listing is one
//! `PROPFIND` of depth 1 whose `207` body is parsed as it streams and handed over in batches, a read
//! is a `GET` with a `Range` that resumes after a dropped connection, and a write is a `PUT`,
//! `MKCOL`, `MOVE`, `COPY` or `DELETE` guarded by `If-None-Match`, `If-Match` and `Overwrite` so it
//! cannot clobber what another writer did meanwhile.
//!
//! Logins are Basic, Digest or a bearer token, taken from the app's `CredentialSource` (or the
//! person's answer to `connect`) and held in memory only for as long as the session lives. A
//! certificate the system does not trust fails with `CertificateUntrusted`, showing it, and is
//! trusted only by an explicit `TrustCertificate` answer or a pin the app supplies, by fingerprint,
//! for as long as the provider lives. A `429` or a `503` with `Retry-After` is `RateLimited`.
//!
//! The provider is synchronous, as the trait is, and runs the asynchronous client on a small Tokio
//! runtime of its own, so callers are worker threads, never the runtime's own.

mod auth;
mod client;
mod entry;
mod errors;
mod listing;
mod options;
mod paths;
mod pool;
mod preset;
mod provider;
mod read;
mod spool;
mod stream;
mod tls;
mod write;
mod xml;

pub use options::{AuthMode, Preset, WebDavConfig, WebDavOptions};
pub use preset::nextcloud_root;
pub use provider::WebDavProvider;
