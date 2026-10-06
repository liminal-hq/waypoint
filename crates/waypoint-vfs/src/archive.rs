// What the operations engine needs from a provider that serves archives, beyond the `Provider`
// trait: a catalogue of an archive's entries with what extraction must decide on, and writers for
// making archives. The archive provider implements them; the engine reaches them through traits so
// neither crate depends on the other.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::Read;

use waypoint_path::VfsPath;
use waypoint_protocol::{Location, VfsError};

use crate::write::WriteStream;
use crate::{CancelToken, EntryKind};

/// Why a name an archive stores could not be used as written. The name shown, and every path built
/// from it, is the safe one; this says what was changed, so an extraction can leave such an entry
/// out instead of trusting the stored name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnsafeName {
    /// The name starts at the root (`/etc/passwd`) or a drive (`C:\Windows`).
    Absolute,
    /// A component is `..`, so the name climbs out of the folder it is extracted into.
    Traversal,
    /// The name holds a NUL or another control character.
    ControlCharacters,
    /// The entry sits below a name the archive made a link or a file, so extracting it would write
    /// through the link.
    ThroughLink,
}

/// What stands for `/` in the one flat name an archive lists for an entry whose stored name has a
/// `..` in it (U+2215, DIVISION SLASH).
pub const FLAT_SLASH: char = '\u{2215}';

/// What a stored name that is only `..` is listed as.
pub const FLAT_DOT_DOT: &str = "%2E%2E";

/// One component of an archive path, as a person reads the name the archive stored: the listing's
/// flat form `..\u{2215}evil.txt` is `../evil.txt` and `%2E%2E` is `..`. The listing and the
/// notice of what an extraction leaves out share this, so they agree on every name but the one
/// character (`\u{2215}` or `/`) the listing cannot use as a separator.
pub fn stored_name_shown(component: &[u8]) -> String {
    if component == FLAT_DOT_DOT.as_bytes() {
        return "..".to_owned();
    }
    String::from_utf8_lossy(component).replace(FLAT_SLASH, "/")
}

/// One entry of an archive, as extraction plans with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntryInfo {
    /// The entry's safe location inside the archive.
    pub path: VfsPath,
    pub kind: EntryKind,
    /// The size once decompressed, when the archive says.
    pub size: Option<u64>,
    /// The size as stored, when the archive says, so a ratio that looks like a bomb shows before
    /// anything is read.
    pub compressed_size: Option<u64>,
    /// The Unix permission bits, when the archive stores them.
    pub mode: Option<u32>,
    pub modified_ms: Option<i64>,
    /// The text a symlink holds, when the archive keeps it in its headers (tar). A zip or 7z link
    /// holds it in its data: `Provider::read_link` reads it.
    pub link_target: Option<Vec<u8>>,
    /// Reading it needs a password.
    pub encrypted: bool,
    pub unsafe_name: Option<UnsafeName>,
    /// A folder the archive does not list, made up so what is below it can be reached.
    pub synthetic: bool,
}

/// Why an archive cannot be rewritten to add, rename or delete entries (D170). Each is a reason a
/// person can act on, so the app words each one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ArchiveRefusal {
    /// Waypoint reads this format but cannot write it (zstd-compressed tar).
    ReadOnlyFormat { format: String },
    /// Entries (or the header) need a password, and an archive is never rewritten without being
    /// able to keep what it holds readable.
    Encrypted,
    /// The archive is inside another archive, which is read only.
    Nested,
    /// Some entry names were changed to be safe when the archive was read, so writing them back
    /// would rename what the archive stores.
    UnsafeNames,
    /// The place the archive file is kept cannot replace a file in one step.
    NoAtomicReplace,
    /// The place the archive file is kept cannot be written to.
    ContainerReadOnly,
}

/// Whether an archive can be rewritten, and as what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Writability {
    /// It can, in this format (the one it already has).
    Writable(ArchiveKind),
    Refused(ArchiveRefusal),
}

/// The entries of an archive. Implemented by the archive provider.
pub trait ArchiveCatalog: Send + Sync {
    /// Whether the archive whose top is `archive` can be rewritten: its format has a writer, nothing
    /// in it is encrypted, no name was changed on reading, and the file's own provider can write and
    /// replace a file.
    fn writability(&self, archive: &VfsPath, cancel: &CancelToken)
        -> Result<Writability, VfsError>;

    /// Every entry of the archive whose top is `archive` (an `archive:` path with no inner path),
    /// in the order the archive lists them, folders it omits made up before what is in them.
    /// `progress` receives the entries read so far while the archive is read.
    fn archive_entries(
        &self,
        archive: &VfsPath,
        cancel: &CancelToken,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ArchiveEntryInfo>, VfsError>;
}

/// The formats an archive can be made in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchiveKind {
    Zip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    SevenZ,
}

impl ArchiveKind {
    pub const ALL: [ArchiveKind; 6] = [
        ArchiveKind::Zip,
        ArchiveKind::Tar,
        ArchiveKind::TarGz,
        ArchiveKind::TarBz2,
        ArchiveKind::TarXz,
        ArchiveKind::SevenZ,
    ];

    /// The extension of the files it makes, with its dot.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Zip => ".zip",
            Self::Tar => ".tar",
            Self::TarGz => ".tar.gz",
            Self::TarBz2 => ".tar.bz2",
            Self::TarXz => ".tar.xz",
            Self::SevenZ => ".7z",
        }
    }
}

/// What an archive records about an entry besides its name and data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EntryAttrs {
    /// The Unix permission bits, when the source has them.
    pub mode: Option<u32>,
    pub modified_ms: Option<i64>,
}

/// An archive being written, one entry at a time. Names are relative, `/`-separated bytes. Nothing
/// is final until `finish` succeeds; dropping the builder instead leaves a truncated archive in the
/// stream it was given, which the caller discards.
pub trait ArchiveBuilder: Send {
    /// Adds a folder.
    fn add_dir(&mut self, name: &[u8], attrs: EntryAttrs) -> Result<(), VfsError>;

    /// Adds a file of exactly `size` bytes read from `data`. A reader that fails (a cancel, a lost
    /// connection) fails the call with its typed error; one that ends early or runs on is an error
    /// too.
    fn add_file(
        &mut self,
        name: &[u8],
        size: u64,
        attrs: EntryAttrs,
        data: &mut dyn Read,
    ) -> Result<(), VfsError>;

    /// Adds a symlink holding the text `target`.
    fn add_symlink(
        &mut self,
        name: &[u8],
        target: &[u8],
        attrs: EntryAttrs,
    ) -> Result<(), VfsError>;

    /// Writes whatever ends the archive and closes the stream (`sync` asks the storage to commit it).
    fn finish(self: Box<Self>, sync: bool) -> Result<(), VfsError>;
}

/// Makes archives. Implemented by the archive provider.
pub trait ArchiveWriters: Send + Sync {
    /// The formats this can make.
    fn kinds(&self) -> Vec<ArchiveKind>;

    /// Starts an archive of `kind` written to `out`. `location` is the archive file being made,
    /// which the errors of writing it name.
    fn begin(
        &self,
        kind: ArchiveKind,
        out: Box<dyn WriteStream>,
        location: Location,
    ) -> Result<Box<dyn ArchiveBuilder>, VfsError>;
}
