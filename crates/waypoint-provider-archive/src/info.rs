// What an archive says about itself and its entries, beyond what a listing shows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_vfs::EntryKind;

use crate::format::ArchiveFormat;
use crate::names::UnsafeName;

/// An archive as a whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveInfo {
    pub format: ArchiveFormat,
    /// Entries the archive lists.
    pub entries: usize,
    /// Entries that need a password to read.
    pub encrypted_entries: usize,
    /// Entries whose stored names had to be changed to be safe (see `EntryInfo::unsafe_name`).
    pub unsafe_names: usize,
    /// The archive has to be read from its start to be listed.
    pub slow_listing: bool,
    /// The archive's own comment, when it has one.
    pub comment: Option<String>,
}

/// One entry, with what extraction needs to decide about it. The name is always the safe one: it
/// never leaves the folder it is joined onto. `unsafe_name` and `raw_name` say what the archive
/// actually stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo {
    /// The last component of the safe name, as text.
    pub name: String,
    /// The safe name from the top of the archive, a component at a time, as bytes.
    pub components: Vec<Vec<u8>>,
    pub kind: EntryKind,
    /// The size once decompressed, when the archive says.
    pub size: Option<u64>,
    /// The size as stored, when the archive says (zip and 7z), so a caller can see a ratio that
    /// looks like a zip bomb before extracting.
    pub compressed_size: Option<u64>,
    /// The Unix permission bits, when the archive stores them.
    pub mode: Option<u32>,
    pub modified_ms: Option<i64>,
    /// The text a symlink holds, when the archive keeps it in its headers (tar); a zip or 7z link
    /// holds it in its data.
    pub link_target: Option<Vec<u8>>,
    /// Reading it needs a password.
    pub encrypted: bool,
    /// Why the stored name was not used as it was.
    pub unsafe_name: Option<UnsafeName>,
    /// The name as the archive stored it, when that differs from the one shown.
    pub raw_name: Option<Vec<u8>>,
    /// A folder the archive does not list, made up so what is below it can be reached.
    pub synthetic: bool,
}
