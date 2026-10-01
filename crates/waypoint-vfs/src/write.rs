// The types the write primitives of a `Provider` speak.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{Read, Write};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use waypoint_protocol::VfsError;

/// Identifies a volume (a file system) within one provider, so a caller can tell whether a rename
/// is atomic (same volume) or a copy and remove (different volumes).
///
/// Two ids from the same provider are equal exactly when the locations are on the same volume;
/// comparing ids from different providers means nothing. On Unix it is `st_dev`; on Windows the
/// volume serial number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VolumeId(pub u64);

/// How `Provider::create_write` opens its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WriteOptions {
    /// Fail with `AlreadyExists` instead of replacing what is there. Operations always set this for
    /// partial files, so a stray name collision can never clobber data. Without it, an existing
    /// file is truncated, and a symlink is followed as `open(2)` does.
    pub exclusive: bool,
    /// The Unix permission bits of a file this call creates (ignored where the file exists, and on
    /// Windows). `None` takes the platform default (`0o666` less the umask).
    pub mode: Option<u32>,
}

impl WriteOptions {
    /// Create a new file, failing if the name is taken.
    pub const fn exclusive() -> Self {
        Self {
            exclusive: true,
            mode: None,
        }
    }

    /// Create the file or truncate it.
    pub const fn truncate() -> Self {
        Self {
            exclusive: false,
            mode: None,
        }
    }
}

/// The times `Provider::set_times` changes. A time left `None` is left as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileTimes {
    pub accessed: Option<SystemTime>,
    pub modified: Option<SystemTime>,
}

/// What `Provider::permissions` reports and `set_permissions` applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    /// The Unix permission bits (setuid, setgid and sticky included), or `None` where the platform
    /// has none (Windows).
    pub mode: Option<u32>,
    /// Whether the entry cannot be written to: no write bit is set, or the read-only attribute is.
    pub readonly: bool,
}

/// A stream of a file's bytes, as `Provider::open_read` returns it.
pub type ReadStream = Box<dyn Read + Send>;

/// A stream a file is written through, as `Provider::create_write` returns it. Errors come back as
/// `io::Error`; `from_io` turns them into typed ones.
pub trait WriteStream: Write + Send {
    /// Flushes and closes the file, reporting a failure that only shows at close (a full disk, a
    /// network file system). With `sync` it also asks the storage to commit the data before
    /// returning, which a verified copy wants and a scratch file does not. Dropping the stream
    /// instead of calling this closes it without reporting.
    fn finish(self: Box<Self>, sync: bool) -> Result<(), VfsError>;
}
