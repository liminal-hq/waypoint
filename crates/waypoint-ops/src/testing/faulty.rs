// A provider that fails, cancels or loses a path on a script, to prove an operation leaves a
// consistent tree whichever step goes wrong.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex, MutexGuard};

use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;
use waypoint_vfs::{
    CancelToken, Capabilities, FileTimes, InjectedError, Permissions, Provider, ReadStream,
    ScannedEntry, VolumeId, VolumeSpace, Watch, WatchSink, WriteOptions, WriteStream,
};

use crate::exec::remove_all;

/// A call a fault can be aimed at. `Read`, `Write` and `Finish` are the calls on the streams a
/// provider hands out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Op {
    Stat,
    List,
    ResolveLink,
    CreateDir,
    CreateFile,
    Rename,
    RemoveFile,
    RemoveDir,
    OpenRead,
    CreateWrite,
    Read,
    Write,
    Finish,
    SetTimes,
    Permissions,
    SetPermissions,
    Symlink,
    ReadLink,
}

impl Op {
    /// Whether the call can change the tree.
    pub fn is_write(self) -> bool {
        matches!(
            self,
            Op::CreateDir
                | Op::CreateFile
                | Op::Rename
                | Op::RemoveFile
                | Op::RemoveDir
                | Op::CreateWrite
                | Op::Write
                | Op::Finish
                | Op::SetTimes
                | Op::SetPermissions
                | Op::Symlink
        )
    }
}

/// The failures an operation has to survive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    PermissionDenied,
    StorageFull,
    CrossesDevices,
    Interrupted,
    NotFound,
}

impl FaultKind {
    pub const ALL: [FaultKind; 5] = [
        FaultKind::PermissionDenied,
        FaultKind::StorageFull,
        FaultKind::CrossesDevices,
        FaultKind::Interrupted,
        FaultKind::NotFound,
    ];

    fn error(self, path: &VfsPath) -> VfsError {
        let location = path.to_location();
        match self {
            FaultKind::PermissionDenied => VfsError::PermissionDenied { location },
            FaultKind::StorageFull => VfsError::StorageFull { location },
            FaultKind::CrossesDevices => VfsError::CrossesDevices {
                from: location.clone(),
                to: location,
            },
            FaultKind::Interrupted => VfsError::Io {
                message: "Interrupted system call".to_owned(),
                location: Some(location),
            },
            FaultKind::NotFound => VfsError::NotFound { location },
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Trigger {
    /// The nth call (1 is the first) of one operation.
    OpNth(Op, usize),
    /// The nth call of any operation.
    Global(usize),
}

#[derive(Clone)]
enum Action {
    Fail(FaultKind),
    Cancel(CancelToken),
    Vanish(VfsPath),
}

struct Rule {
    trigger: Trigger,
    action: Action,
    fired: bool,
}

#[derive(Default)]
struct State {
    rules: Vec<Rule>,
    per_op: HashMap<Op, usize>,
    total: usize,
    writes: usize,
    /// Paths to remove at the next call on the provider itself (a stream cannot reach it).
    deferred: Vec<VfsPath>,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Counts a call and runs the rules that fire on it. `Err` is the injected failure; `Ok`
    /// carries the paths to make vanish before the call goes on.
    fn hit(&self, op: Op, path: &VfsPath) -> Result<Vec<VfsPath>, VfsError> {
        let mut state = self.lock();
        let mut vanish = std::mem::take(&mut state.deferred);
        state.total += 1;
        let total = state.total;
        let nth = {
            let n = state.per_op.entry(op).or_default();
            *n += 1;
            *n
        };
        if op.is_write() {
            state.writes += 1;
        }
        let mut failure = None;
        for rule in &mut state.rules {
            if rule.fired {
                continue;
            }
            let fires = match rule.trigger {
                Trigger::OpNth(o, n) => o == op && n == nth,
                Trigger::Global(n) => n == total,
            };
            if !fires {
                continue;
            }
            rule.fired = true;
            match &rule.action {
                Action::Fail(kind) => failure = failure.or(Some(kind.error(path))),
                Action::Cancel(token) => token.cancel(),
                Action::Vanish(gone) => vanish.push(gone.clone()),
            }
        }
        match failure {
            Some(error) => {
                state.deferred.extend(vanish);
                Err(error)
            }
            None => Ok(vanish),
        }
    }
}

/// Wraps a provider and injects the faults a test scripts. Calls are counted whether or not a
/// fault fires: `calls` is every call, so a test can run an operation once to learn how many steps
/// it takes and then again with a fault at each step.
pub struct FaultyProvider<P> {
    inner: P,
    shared: Arc<Shared>,
}

impl<P: Provider> FaultyProvider<P> {
    pub fn new(inner: P) -> Self {
        Self {
            inner,
            shared: Arc::new(Shared::default()),
        }
    }

    pub fn inner(&self) -> &P {
        &self.inner
    }

    fn add(&self, trigger: Trigger, action: Action) {
        self.shared.lock().rules.push(Rule {
            trigger,
            action,
            fired: false,
        });
    }

    /// Fails the nth next call of `op` (1 is the first) with `kind`; the other calls succeed.
    pub fn fail_nth(&self, op: Op, n: usize, kind: FaultKind) {
        self.add(Trigger::OpNth(op, n), Action::Fail(kind));
    }

    /// Fails the nth call of any kind (1 is the first) with `kind`.
    pub fn fail_at(&self, n: usize, kind: FaultKind) {
        self.add(Trigger::Global(n), Action::Fail(kind));
    }

    /// Flips `token` at the nth call of any kind; that call and the ones after go on as normal.
    pub fn cancel_at(&self, n: usize, token: &CancelToken) {
        self.add(Trigger::Global(n), Action::Cancel(token.clone()));
    }

    /// Removes `path` (and anything below it) from the wrapped provider at the nth call of any
    /// kind, as if another program had deleted it. The call itself then runs against the changed
    /// tree.
    pub fn vanish_at(&self, n: usize, path: &VfsPath) {
        self.add(Trigger::Global(n), Action::Vanish(path.clone()));
    }

    /// How many calls of any kind have been made.
    pub fn calls(&self) -> usize {
        self.shared.lock().total
    }

    pub fn calls_of(&self, op: Op) -> usize {
        self.shared.lock().per_op.get(&op).copied().unwrap_or(0)
    }

    /// How many calls that can change the tree have been made.
    pub fn write_calls(&self) -> usize {
        self.shared.lock().writes
    }

    /// Forgets every script and every count.
    pub fn reset(&self) {
        *self.shared.lock() = State::default();
    }

    fn hit(&self, op: Op, path: &VfsPath) -> Result<(), VfsError> {
        for gone in self.shared.hit(op, path)? {
            let _ = remove_all(&self.inner, &gone);
        }
        Ok(())
    }
}

struct FaultyRead {
    inner: ReadStream,
    shared: Arc<Shared>,
    path: VfsPath,
}

impl Read for FaultyRead {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.shared
            .hit(Op::Read, &self.path)
            .map_err(|e| InjectedError(e).into_io())?;
        self.inner.read(buf)
    }
}

struct FaultyWrite {
    inner: Box<dyn WriteStream>,
    shared: Arc<Shared>,
    path: VfsPath,
}

impl Write for FaultyWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.shared
            .hit(Op::Write, &self.path)
            .map_err(|e| InjectedError(e).into_io())?;
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl WriteStream for FaultyWrite {
    fn finish(self: Box<Self>, sync: bool) -> Result<(), VfsError> {
        let this = *self;
        this.shared.hit(Op::Finish, &this.path)?;
        this.inner.finish(sync)
    }
}

impl<P: Provider> Provider for FaultyProvider<P> {
    fn scheme(&self) -> &'static str {
        self.inner.scheme()
    }

    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        self.hit(Op::Stat, path)?;
        self.inner.stat(path)
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        inline_link_budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        self.hit(Op::List, path)?;
        self.inner.list(path, cancel, inline_link_budget, progress)
    }

    fn resolve_link(
        &self,
        folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        self.hit(Op::ResolveLink, folder)?;
        self.inner.resolve_link(folder, entry)
    }

    fn watch(&self, path: &VfsPath, sink: WatchSink) -> Result<Box<dyn Watch>, VfsError> {
        self.inner.watch(path, sink)
    }

    fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.hit(Op::CreateDir, path)?;
        self.inner.create_dir(path)
    }

    fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.hit(Op::CreateFile, path)?;
        self.inner.create_file(path)
    }

    fn rename(&self, from: &VfsPath, to: &VfsPath, overwrite: bool) -> Result<(), VfsError> {
        self.hit(Op::Rename, from)?;
        self.inner.rename(from, to, overwrite)
    }

    fn remove_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.hit(Op::RemoveFile, path)?;
        self.inner.remove_file(path)
    }

    fn remove_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.hit(Op::RemoveDir, path)?;
        self.inner.remove_dir(path)
    }

    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        self.hit(Op::OpenRead, path)?;
        let inner = self.inner.open_read(path)?;
        Ok(Box::new(FaultyRead {
            inner,
            shared: self.shared.clone(),
            path: path.clone(),
        }))
    }

    fn create_write(
        &self,
        path: &VfsPath,
        options: WriteOptions,
    ) -> Result<Box<dyn WriteStream>, VfsError> {
        self.hit(Op::CreateWrite, path)?;
        let inner = self.inner.create_write(path, options)?;
        Ok(Box::new(FaultyWrite {
            inner,
            shared: self.shared.clone(),
            path: path.clone(),
        }))
    }

    fn set_times(&self, path: &VfsPath, times: FileTimes) -> Result<(), VfsError> {
        self.hit(Op::SetTimes, path)?;
        self.inner.set_times(path, times)
    }

    fn permissions(&self, path: &VfsPath) -> Result<Permissions, VfsError> {
        self.hit(Op::Permissions, path)?;
        self.inner.permissions(path)
    }

    fn set_permissions(&self, path: &VfsPath, permissions: Permissions) -> Result<(), VfsError> {
        self.hit(Op::SetPermissions, path)?;
        self.inner.set_permissions(path, permissions)
    }

    fn symlink(&self, link: &VfsPath, target: &OsStr) -> Result<(), VfsError> {
        self.hit(Op::Symlink, link)?;
        self.inner.symlink(link, target)
    }

    fn read_link(&self, path: &VfsPath) -> Result<OsString, VfsError> {
        self.hit(Op::ReadLink, path)?;
        self.inner.read_link(path)
    }

    fn canonicalize(&self, path: &VfsPath) -> Result<VfsPath, VfsError> {
        // Not a counted call: resolving links is a read the planner makes, and scripted faults
        // count the operations the executors make.
        self.inner.canonicalize(path)
    }

    fn volume_id(&self, path: &VfsPath) -> Option<VolumeId> {
        self.inner.volume_id(path)
    }

    fn free_space(&self, path: &VfsPath) -> Option<VolumeSpace> {
        self.inner.free_space(path)
    }

    fn copy_file_within(
        &self,
        src: &VfsPath,
        dst: &VfsPath,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Option<Result<u64, VfsError>> {
        self.inner.copy_file_within(src, dst, progress, cancel)
    }
}
