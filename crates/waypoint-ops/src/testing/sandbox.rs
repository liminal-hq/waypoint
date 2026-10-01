// A provider that panics on any path outside its root, so a test that wanders off its temporary
// directory fails loudly instead of touching the real file system.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::{OsStr, OsString};
use std::path::Component;

use waypoint_path::{FilePath, VfsPath};
use waypoint_protocol::VfsError;
use waypoint_vfs::{
    CancelToken, Capabilities, EntryKind, FileTimes, Permissions, Provider, ReadStream,
    ScannedEntry, VolumeId, VolumeSpace, Watch, WatchSink, WriteOptions, WriteStream,
};

/// How many links a path may pass through before the sandbox calls it a loop.
const MAX_LINKS: usize = 16;

/// Wraps a provider and checks every path it is asked about: it must lie inside `root` once
/// normalised (so `..` cannot climb out) and, following every symlink on the way, must still lie
/// inside it (so a link cannot lead out). Nothing outside the root is ever passed to the wrapped
/// provider; the check panics first.
pub struct SandboxProvider<P> {
    inner: P,
    root: FilePath,
}

impl<P: Provider> SandboxProvider<P> {
    pub fn new(inner: P, root: &VfsPath) -> Self {
        let VfsPath::File(root) = root;
        Self {
            inner,
            root: root.clone(),
        }
    }

    pub fn inner(&self) -> &P {
        &self.inner
    }

    pub fn root(&self) -> VfsPath {
        VfsPath::File(self.root.clone())
    }

    fn escape(&self, path: &VfsPath, why: &str) -> ! {
        panic!(
            "sandbox: {} {why}; the sandbox root is {}",
            path.display(),
            self.root.display()
        )
    }

    /// Panics unless `path` is inside the root, through every link it passes. A symlink in the
    /// final component is followed only when `follow_final` (the operation acts through it).
    fn guard(&self, path: &VfsPath, follow_final: bool) {
        self.check(path, follow_final, 0);
    }

    fn check(&self, path: &VfsPath, follow_final: bool, links: usize) {
        if links > MAX_LINKS {
            self.escape(path, "passes through too many symlinks");
        }
        let VfsPath::File(file) = path;
        let Ok(relative) = file.as_path().strip_prefix(self.root.as_path()) else {
            self.escape(path, "is outside the sandbox");
        };
        let names: Vec<OsString> = relative
            .components()
            .filter_map(|c| match c {
                Component::Normal(name) => Some(name.to_owned()),
                _ => None,
            })
            .collect();
        let mut here = VfsPath::File(self.root.clone());
        for (index, name) in names.iter().enumerate() {
            here = match here.join(name) {
                Ok(next) => next,
                Err(_) => self.escape(path, "cannot be joined"),
            };
            if index + 1 == names.len() && !follow_final {
                break;
            }
            match self.inner.stat(&here) {
                Ok(entry) if entry.kind == EntryKind::Symlink => {
                    let Ok(text) = self.inner.read_link(&here) else {
                        return;
                    };
                    let Some(parent) = here.parent() else { return };
                    let Ok(mut resolved) = parent.join(&text) else {
                        self.escape(path, "follows a link that cannot be resolved");
                    };
                    for rest in &names[index + 1..] {
                        resolved = match resolved.join(rest) {
                            Ok(next) => next,
                            Err(_) => self.escape(path, "cannot be joined"),
                        };
                    }
                    self.check(&resolved, follow_final, links + 1);
                    return;
                }
                Ok(_) => {}
                // Nothing exists here, so there is nothing further to follow.
                Err(_) => break,
            }
        }
    }
}

impl<P: Provider> Provider for SandboxProvider<P> {
    fn scheme(&self) -> &'static str {
        self.inner.scheme()
    }

    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        self.guard(path, false);
        self.inner.stat(path)
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        inline_link_budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        self.guard(path, true);
        self.inner.list(path, cancel, inline_link_budget, progress)
    }

    fn resolve_link(
        &self,
        folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        self.guard(folder, true);
        self.inner.resolve_link(folder, entry)
    }

    fn watch(&self, path: &VfsPath, sink: WatchSink) -> Result<Box<dyn Watch>, VfsError> {
        self.guard(path, true);
        self.inner.watch(path, sink)
    }

    fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.guard(path, false);
        self.inner.create_dir(path)
    }

    fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.guard(path, false);
        self.inner.create_file(path)
    }

    fn rename(&self, from: &VfsPath, to: &VfsPath, overwrite: bool) -> Result<(), VfsError> {
        self.guard(from, false);
        self.guard(to, false);
        self.inner.rename(from, to, overwrite)
    }

    fn remove_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.guard(path, false);
        self.inner.remove_file(path)
    }

    fn remove_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.guard(path, false);
        self.inner.remove_dir(path)
    }

    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        self.guard(path, true);
        self.inner.open_read(path)
    }

    fn create_write(
        &self,
        path: &VfsPath,
        options: WriteOptions,
    ) -> Result<Box<dyn WriteStream>, VfsError> {
        // An exclusive create never follows a link in its last component; a truncating one does.
        self.guard(path, !options.exclusive);
        self.inner.create_write(path, options)
    }

    fn set_times(&self, path: &VfsPath, times: FileTimes) -> Result<(), VfsError> {
        self.guard(path, false);
        self.inner.set_times(path, times)
    }

    fn permissions(&self, path: &VfsPath) -> Result<Permissions, VfsError> {
        self.guard(path, true);
        self.inner.permissions(path)
    }

    fn set_permissions(&self, path: &VfsPath, permissions: Permissions) -> Result<(), VfsError> {
        self.guard(path, true);
        self.inner.set_permissions(path, permissions)
    }

    fn symlink(&self, link: &VfsPath, target: &OsStr) -> Result<(), VfsError> {
        self.guard(link, false);
        self.inner.symlink(link, target)
    }

    fn read_link(&self, path: &VfsPath) -> Result<OsString, VfsError> {
        self.guard(path, false);
        self.inner.read_link(path)
    }

    fn volume_id(&self, path: &VfsPath) -> Option<VolumeId> {
        self.guard(path, true);
        self.inner.volume_id(path)
    }

    fn free_space(&self, path: &VfsPath) -> Option<VolumeSpace> {
        self.guard(path, true);
        self.inner.free_space(path)
    }

    fn copy_file_within(
        &self,
        src: &VfsPath,
        dst: &VfsPath,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Option<Result<u64, VfsError>> {
        self.guard(src, true);
        self.guard(dst, false);
        self.inner.copy_file_within(src, dst, progress, cancel)
    }
}
