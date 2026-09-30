// Waypoint's path type: the scheme-shaped `VfsPath`, of which only `file` is implemented, and the
// platform rules behind it. The Linux rules (`posix`) and the Windows rules (`windows`) are pure
// functions over bytes and strings, so both are tested on every platform.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod encoding;
mod error;
mod file_path;
pub mod posix;
mod vfs_path;
pub mod windows;

pub use error::PathError;
pub use file_path::{CaseRule, FilePath};
pub use vfs_path::VfsPath;

#[cfg(test)]
mod properties;
