// Reads the facts about the system that the availability check needs, behind a trait so the check runs without polkit or root
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

/// What kind of thing a path is, as the check cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    File,
    Directory,
    Other,
}

/// The facts about one path that decide whether it can be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileFacts {
    pub kind: FileKind,
    /// The owner's user id.
    pub uid: u32,
    /// The permission bits (`st_mode & 0o7777`).
    pub mode: u32,
}

/// The environment the availability check reads. The real one is [`SystemProbe`]; tests substitute a fake.
pub trait Probe {
    /// The facts about `path` without following a final symbolic link, or `None` when nothing is there or it cannot be read.
    fn facts(&self, path: &Path) -> Option<FileFacts>;
    /// The facts about `path` after following symbolic links (a link to a folder is that folder).
    fn facts_following(&self, path: &Path) -> Option<FileFacts>;
    /// The value of an environment variable, when set and valid text.
    fn env(&self, name: &str) -> Option<String>;
    /// Finds an executable by name on `PATH`.
    fn which(&self, name: &str) -> Option<PathBuf>;
}

/// The real system.
#[cfg(unix)]
pub struct SystemProbe;

#[cfg(unix)]
impl SystemProbe {
    fn describe(metadata: std::io::Result<std::fs::Metadata>) -> Option<FileFacts> {
        use std::os::unix::fs::MetadataExt;
        let metadata = metadata.ok()?;
        let kind = if metadata.is_file() {
            FileKind::File
        } else if metadata.is_dir() {
            FileKind::Directory
        } else {
            FileKind::Other
        };
        Some(FileFacts {
            kind,
            uid: metadata.uid(),
            mode: metadata.mode() & 0o7777,
        })
    }
}

#[cfg(unix)]
impl Probe for SystemProbe {
    fn facts(&self, path: &Path) -> Option<FileFacts> {
        Self::describe(std::fs::symlink_metadata(path))
    }

    fn facts_following(&self, path: &Path) -> Option<FileFacts> {
        Self::describe(std::fs::metadata(path))
    }

    fn env(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }

    fn which(&self, name: &str) -> Option<PathBuf> {
        let paths = std::env::var_os("PATH")?;
        std::env::split_paths(&paths)
            .filter(|dir| dir.is_absolute())
            .map(|dir| dir.join(name))
            .find(|candidate| {
                self.facts_following(candidate)
                    .is_some_and(|facts| facts.kind == FileKind::File && facts.mode & 0o111 != 0)
            })
    }
}
