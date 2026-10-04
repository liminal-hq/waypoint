// Waypoint's path type: the scheme-shaped `VfsPath` (local files, the Trash, servers, archives and
// Git revisions) and the platform rules behind it. The Linux rules (`posix`) and the Windows rules
// (`windows`) are pure functions over bytes and strings, so both are tested on every platform.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod archive_path;
mod encoding;
mod error;
mod file_path;
mod git_path;
pub mod posix;
mod remote_path;
mod segments;
mod trash_path;
mod vfs_path;
pub mod windows;

pub use archive_path::{ArchivePath, ARCHIVE_SCHEME};
pub use error::PathError;
pub use file_path::{CaseRule, FilePath};
pub use git_path::{GitPath, GIT_SCHEME};
pub use remote_path::{Authority, ConnectionKey, Endpoint, Host, RemotePath, RemoteScheme};
pub use trash_path::{TrashPath, TRASH_SCHEME};
pub use vfs_path::VfsPath;

#[cfg(test)]
mod properties;
