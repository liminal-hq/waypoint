// Waypoint's archive provider: `archive:` locations served from zip, tar and 7z files.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `ArchiveProvider` implements `waypoint_vfs::Provider` for `archive:{container URI}!/inner`
//! locations (`docs/architecture/remote-locations.md`, D152): a zip, a tar (plain, or compressed
//! with gzip, bzip2, xz or zstd) or a 7z file browses as a read-only folder, wherever the file is
//! (any provider, a server or another archive included).
//!
//! Listing never extracts. A zip is listed from its central directory (a few ranged reads, however
//! many entries), a plain tar by seeking from header to header, a 7z from its header; a compressed
//! tar has no index, so it is read once from its start, with progress, a cap on the entries, a
//! cancel and an `ArchiveNotice::SlowListing` before a big one starts. The listing of each archive
//! stays in memory until it is replaced or the archive changes.
//!
//! Names are never trusted. Every name is split into components that cannot leave the folder they
//! are joined onto (a leading `/` or drive is dropped, a `..` is shown as `%2E%2E`, control
//! characters are replaced), and `EntryInfo::unsafe_name` records what was changed, so an
//! extraction refuses or skips such an entry. An entry stored below a link is flagged too.
//!
//! Reads are streams: each entry is decoded on a thread of its own and handed over in chunks, so
//! the caller reads at its own pace, a damaged entry ends in an error rather than a short file,
//! and dropping the stream stops the decoder. Encrypted entries and headers fail with
//! `AuthRequired` (`Passphrase`) until `ArchiveProvider::unlock` is given the password, and
//! `AuthFailed` when it is wrong. A damaged archive is `Corrupt`.
//!
//! Everything is pure Rust (`zip`, `tar`, `flate2` with `zlib-rs`, `bzip2` with `libbz2-rs-sys`,
//! `lzma-rust2`, `ruzstd`, `sevenz-rust2`), so the crate builds for Windows without a C compiler.

mod compress;
mod errors;
mod format;
mod index;
mod info;
mod names;
mod options;
mod provider;
mod sevenz;
mod source;
mod stream;
mod tar_scan;
mod write;
mod zip_read;
mod zip_scan;

pub use format::{ArchiveFormat, TarCompression};
pub use info::{ArchiveInfo, EntryInfo};
pub use options::{ArchiveNotice, ArchiveOptions, ContainerSource, NoticeSink};
pub use provider::ArchiveProvider;
pub use waypoint_vfs::UnsafeName;
