// An in-memory provider that implements the whole `Provider` trait, for tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `MemoryProvider` is a complete tree held in memory: folders, files with content, symlinks,
//! times, permissions, volumes, and either case rule. It follows the same rules as `LocalProvider`
//! (the conformance suite in `tests/conformance.rs` runs both), plus the hooks tests need: volumes
//! to simulate `CrossesDevices`, and failure injection to simulate a full disk or a vanished file.
//!
//! One difference from a real file system: a path that passes *through* a symlink (`link/child`) is
//! not resolved; the final component of a path is never followed, as on disk.

use std::collections::{BTreeMap, HashMap};
use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::path::Component;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use waypoint_path::{windows, CaseRule, FilePath, VfsPath};
use waypoint_protocol::{Location, VfsError};

use crate::error::InjectedError;
use crate::icon::group_for;
use crate::model::{EntryKind, VolumeSpace};
use crate::names::validate_new_path;
use crate::provider::{Capabilities, Provider, ScannedEntry};
use crate::write::{FileTimes, Permissions, ReadStream, VolumeId, WriteOptions, WriteStream};
use crate::CancelToken;

/// An operation a failure can be injected into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemOp {
    Stat,
    List,
    ResolveLink,
    /// One entry read while listing a folder (an I/O error on a single directory entry).
    ListEntry,
    CreateDir,
    CreateFile,
    Rename,
    RemoveFile,
    RemoveDir,
    OpenRead,
    CreateWrite,
    /// One `read` call on a stream from `open_read`.
    Read,
    /// One `write` call on a stream from `create_write`.
    Write,
    /// `WriteStream::finish`.
    Finish,
    SetTimes,
    Permissions,
    SetPermissions,
    Symlink,
    ReadLink,
    CopyFileWithin,
}

struct Failure {
    op: MemOp,
    /// Calls to let through first.
    skip: usize,
    error: VfsError,
    sticky: bool,
}

#[derive(Clone)]
struct Node {
    kind: Kind,
    accessed: SystemTime,
    modified: SystemTime,
    mode: u32,
}

type Children = BTreeMap<OsString, (OsString, Node)>;

#[derive(Clone)]
enum Kind {
    File(Vec<u8>),
    /// Children by folded name, each with its real name.
    Dir(Children),
    Symlink(OsString),
}

impl Node {
    fn entry_kind(&self) -> EntryKind {
        match self.kind {
            Kind::File(_) => EntryKind::File,
            Kind::Dir(_) => EntryKind::Directory,
            Kind::Symlink(_) => EntryKind::Symlink,
        }
    }

    fn is_dir(&self) -> bool {
        matches!(self.kind, Kind::Dir(_))
    }
}

struct Inner {
    rule: CaseRule,
    root: Node,
    clock: u64,
    /// Subtrees on a volume of their own, as component paths from the root; the root's volume is
    /// `VolumeId(1)`.
    volumes: Vec<(Vec<OsString>, VolumeId)>,
    space: HashMap<VolumeId, VolumeSpace>,
    failures: Vec<Failure>,
    calls: HashMap<MemOp, usize>,
    fast_copy: bool,
}

/// A whole file system in memory. Cloning shares it.
#[derive(Clone)]
pub struct MemoryProvider {
    root: FilePath,
    inner: Arc<Mutex<Inner>>,
}

fn epoch(ticks: u64) -> SystemTime {
    // Start in 2023 so times are plausible, and tick a second per change so ordering is visible.
    UNIX_EPOCH + Duration::from_secs(1_700_000_000 + ticks)
}

impl Inner {
    fn tick(&mut self) -> SystemTime {
        self.clock += 1;
        epoch(self.clock)
    }

    fn new_node(&mut self, kind: Kind) -> Node {
        let now = self.tick();
        let mode = if matches!(kind, Kind::Dir(_)) {
            0o755
        } else if matches!(kind, Kind::Symlink(_)) {
            0o777
        } else {
            0o644
        };
        Node {
            kind,
            accessed: now,
            modified: now,
            mode,
        }
    }

    fn key(&self, name: &OsStr) -> OsString {
        match self.rule {
            CaseRule::Sensitive => name.to_owned(),
            CaseRule::Insensitive => OsString::from(windows::fold(&name.to_string_lossy())),
        }
    }

    fn inject(&mut self, op: MemOp) -> Result<(), VfsError> {
        *self.calls.entry(op).or_default() += 1;
        let Some(index) = self.failures.iter().position(|f| f.op == op) else {
            return Ok(());
        };
        let failure = &mut self.failures[index];
        if failure.skip > 0 {
            failure.skip -= 1;
            return Ok(());
        }
        let error = failure.error.clone();
        if !failure.sticky {
            self.failures.remove(index);
        }
        Err(error)
    }

    fn node(&self, comps: &[OsString]) -> Option<&Node> {
        let mut node = &self.root;
        for name in comps {
            let Kind::Dir(children) = &node.kind else {
                return None;
            };
            node = &children.get(&self.key(name))?.1;
        }
        Some(node)
    }

    /// Why nothing is at `comps`: `NotADirectory` when a folder on the way is really a file or a
    /// link (as the operating system says), otherwise `NotFound`.
    fn missing(&self, comps: &[OsString], location: Location) -> VfsError {
        for end in 0..comps.len() {
            if self.node(&comps[..end]).is_some_and(|n| !n.is_dir()) {
                return VfsError::NotADirectory { location };
            }
        }
        VfsError::NotFound { location }
    }

    fn node_mut(&mut self, comps: &[OsString]) -> Option<&mut Node> {
        let keys: Vec<OsString> = comps.iter().map(|c| self.key(c)).collect();
        let mut node = &mut self.root;
        for key in keys {
            let Kind::Dir(children) = &mut node.kind else {
                return None;
            };
            node = &mut children.get_mut(&key)?.1;
        }
        Some(node)
    }

    /// The folder holding `comps` and the child's folded key, or why there is none.
    fn parent_mut(
        &mut self,
        comps: &[OsString],
        location: &Location,
    ) -> Result<(&mut Children, OsString), VfsError> {
        let (name, parent) = comps
            .split_last()
            .ok_or_else(|| VfsError::PermissionDenied {
                location: location.clone(),
            })?;
        let key = self.key(name);
        let node = self.node_mut(parent).ok_or_else(|| VfsError::NotFound {
            location: location.clone(),
        })?;
        match &mut node.kind {
            Kind::Dir(children) => Ok((children, key)),
            _ => Err(VfsError::NotADirectory {
                location: location.clone(),
            }),
        }
    }

    fn volume_of(&self, comps: &[OsString]) -> VolumeId {
        let keys: Vec<OsString> = comps.iter().map(|c| self.key(c)).collect();
        self.volumes
            .iter()
            .filter(|(prefix, _)| prefix.len() <= keys.len() && keys[..prefix.len()] == prefix[..])
            .max_by_key(|(prefix, _)| prefix.len())
            .map_or(VolumeId(1), |(_, id)| *id)
    }
}

impl MemoryProvider {
    /// An empty tree rooted at `root` (an absolute path that need not exist on disk; tests use a
    /// temporary directory's path so the same paths work against `LocalProvider`).
    pub fn new(root: FilePath, rule: CaseRule) -> Self {
        let mut inner = Inner {
            rule,
            root: Node {
                kind: Kind::Dir(BTreeMap::new()),
                accessed: epoch(0),
                modified: epoch(0),
                mode: 0o755,
            },
            clock: 0,
            volumes: Vec::new(),
            space: HashMap::new(),
            failures: Vec::new(),
            calls: HashMap::new(),
            fast_copy: false,
        };
        inner.clock = 0;
        Self {
            root,
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The path components below the root, or `NotFound` for a path outside it.
    fn comps(&self, path: &VfsPath) -> Result<Vec<OsString>, VfsError> {
        let VfsPath::File(file) = path;
        let outside = || VfsError::NotFound {
            location: path.to_location(),
        };
        let below = file
            .as_path()
            .strip_prefix(self.root.as_path())
            .map_err(|_| outside())?;
        Ok(below
            .components()
            .filter_map(|c| match c {
                Component::Normal(name) => Some(name.to_owned()),
                _ => None,
            })
            .collect())
    }

    // Configuration and inspection, for tests.

    /// Puts the subtree at `path` on its own volume, so renames across its edge fail with
    /// `CrossesDevices`. The root's volume is `VolumeId(1)`.
    pub fn set_volume(&self, path: &VfsPath, volume: VolumeId) {
        let comps = self.comps(path).expect("a volume path lies under the root");
        let mut inner = self.lock();
        let keys = comps.iter().map(|c| inner.key(c)).collect();
        inner.volumes.retain(|(prefix, _)| *prefix != keys);
        inner.volumes.push((keys, volume));
    }

    /// What `free_space` reports for a volume.
    pub fn set_space(&self, volume: VolumeId, space: VolumeSpace) {
        self.lock().space.insert(volume, space);
    }

    /// Lets `copy_file_within` copy within a volume (as a reflink would); off by default, when it
    /// returns `None`.
    pub fn enable_fast_copy(&self, on: bool) {
        self.lock().fast_copy = on;
    }

    /// Makes the next call of `op` fail with `error`.
    pub fn fail_next(&self, op: MemOp, error: VfsError) {
        self.fail_nth(op, 1, error);
    }

    /// Makes the `n`th next call of `op` (1 is the next) fail with `error`; the others succeed.
    pub fn fail_nth(&self, op: MemOp, n: usize, error: VfsError) {
        self.lock().failures.push(Failure {
            op,
            skip: n.saturating_sub(1),
            error,
            sticky: false,
        });
    }

    /// Makes every call of `op` fail with `error` until `clear_failures`.
    pub fn fail_always(&self, op: MemOp, error: VfsError) {
        self.lock().failures.push(Failure {
            op,
            skip: 0,
            error,
            sticky: true,
        });
    }

    pub fn clear_failures(&self) {
        self.lock().failures.clear();
    }

    /// How many times `op` has been called (injected failures included).
    pub fn calls(&self, op: MemOp) -> usize {
        self.lock().calls.get(&op).copied().unwrap_or(0)
    }

    /// Creates a file with content, and any missing parent folders, as a test fixture. It does not
    /// pass through failure injection or name validation.
    pub fn put_file(&self, path: &VfsPath, content: &[u8]) {
        let comps = self.comps(path).expect("a fixture lies under the root");
        let mut inner = self.lock();
        let node = inner.new_node(Kind::File(content.to_vec()));
        Self::put(&mut inner, &comps, node);
    }

    /// Creates a folder and any missing parents, as a test fixture.
    pub fn put_dir(&self, path: &VfsPath) {
        let comps = self.comps(path).expect("a fixture lies under the root");
        let mut inner = self.lock();
        let node = inner.new_node(Kind::Dir(BTreeMap::new()));
        Self::put(&mut inner, &comps, node);
    }

    fn put(inner: &mut Inner, comps: &[OsString], node: Node) {
        for end in 1..comps.len() {
            if inner.node(&comps[..end]).is_none() {
                let dir = inner.new_node(Kind::Dir(BTreeMap::new()));
                Self::insert(inner, &comps[..end], dir);
            }
        }
        if !comps.is_empty() {
            Self::insert(inner, comps, node);
        }
    }

    fn insert(inner: &mut Inner, comps: &[OsString], node: Node) {
        let (name, parent) = comps.split_last().expect("not the root");
        let key = inner.key(name);
        let parent = inner.node_mut(parent).expect("the parent was made");
        if let Kind::Dir(children) = &mut parent.kind {
            children.insert(key, (name.clone(), node));
        }
    }

    /// The content of a file, or `None` when there is no such file.
    pub fn file_content(&self, path: &VfsPath) -> Option<Vec<u8>> {
        let comps = self.comps(path).ok()?;
        match &self.lock().node(&comps)?.kind {
            Kind::File(bytes) => Some(bytes.clone()),
            _ => None,
        }
    }

    fn entry(
        inner: &Inner,
        name: &OsStr,
        node: &Node,
        parent: &[OsString],
        root: &FilePath,
        resolve: bool,
    ) -> ScannedEntry {
        let kind = node.entry_kind();
        let mut link_target = None;
        let mut link_pending = false;
        let mut shown = node;
        if let Kind::Symlink(text) = &node.kind {
            if resolve {
                let target = Self::follow(inner, parent, text, root, 0);
                link_target = target.map(Node::entry_kind);
                if let Some(target) = target {
                    shown = target;
                }
            } else {
                link_pending = true;
            }
        }
        let size = match (&shown.kind, kind) {
            (Kind::File(bytes), EntryKind::File | EntryKind::Symlink) => Some(bytes.len() as u64),
            _ => None,
        };
        let to_ms = |time: SystemTime| match time.duration_since(UNIX_EPOCH) {
            Ok(after) => after.as_millis() as i64,
            Err(before) => -(before.duration().as_millis() as i64),
        };
        ScannedEntry {
            name: name.to_owned(),
            kind,
            link_target,
            link_pending,
            group: group_for(name.as_encoded_bytes(), kind, link_target),
            size,
            modified_ms: Some(to_ms(shown.modified)),
            hidden: name.as_encoded_bytes().first() == Some(&b'.'),
        }
    }

    /// The component path a symlink in the folder `parent` holding `text` names, whether or not
    /// anything is there; `None` when it leaves the tree.
    fn target_comps(parent: &[OsString], text: &OsStr, root: &FilePath) -> Option<Vec<OsString>> {
        let mut base = root.clone();
        for name in parent {
            base = base.join(name).ok()?;
        }
        let target = base.join(text).ok()?;
        let below = target.as_path().strip_prefix(root.as_path()).ok()?;
        Some(
            below
                .components()
                .filter_map(|c| match c {
                    Component::Normal(name) => Some(name.to_owned()),
                    _ => None,
                })
                .collect(),
        )
    }

    /// What a symlink in the folder `parent` pointing at `text` leads to, following chains of links.
    fn follow<'a>(
        inner: &'a Inner,
        parent: &[OsString],
        text: &OsStr,
        root: &FilePath,
        depth: usize,
    ) -> Option<&'a Node> {
        if depth > 8 {
            return None;
        }
        let comps = Self::target_comps(parent, text, root)?;
        let node = inner.node(&comps)?;
        match &node.kind {
            Kind::Symlink(next) => {
                let (_, up) = comps.split_last()?;
                Self::follow(inner, up, next, root, depth + 1)
            }
            _ => Some(node),
        }
    }

    /// The node at `comps`, with a final symlink followed.
    fn node_following(&self, inner: &Inner, comps: &[OsString]) -> Option<Node> {
        let node = inner.node(comps)?;
        match &node.kind {
            Kind::Symlink(text) => {
                let (_, up) = comps.split_last()?;
                Self::follow(inner, up, text, &self.root, 0).cloned()
            }
            _ => Some(node.clone()),
        }
    }

    /// Creates `node` at `path` if its parent exists and the name is free.
    fn create(&self, op: MemOp, path: &VfsPath, kind: Kind) -> Result<(), VfsError> {
        let mut inner = self.lock();
        inner.inject(op)?;
        validate_new_path(path, inner.rule)?;
        let location = path.to_location();
        let comps = self.comps(path)?;
        let node = inner.new_node(kind);
        let (children, key) = inner.parent_mut(&comps, &location)?;
        if children.contains_key(&key) {
            return Err(VfsError::AlreadyExists { location });
        }
        let name = comps
            .last()
            .expect("a path with a parent has a name")
            .clone();
        children.insert(key, (name, node));
        Ok(())
    }
}

struct MemRead {
    inner: Arc<Mutex<Inner>>,
    data: io::Cursor<Vec<u8>>,
}

impl Read for MemRead {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner
            .inject(MemOp::Read)
            .map_err(|e| InjectedError(e).into_io())?;
        drop(inner);
        self.data.read(buf)
    }
}

struct MemWrite {
    inner: Arc<Mutex<Inner>>,
    comps: Vec<OsString>,
    location: Location,
}

impl Write for MemWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner
            .inject(MemOp::Write)
            .map_err(|e| InjectedError(e).into_io())?;
        let now = inner.tick();
        let node = inner
            .node_mut(&self.comps)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        let Kind::File(bytes) = &mut node.kind else {
            return Err(io::Error::from(io::ErrorKind::NotFound));
        };
        bytes.extend_from_slice(buf);
        node.modified = now;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl WriteStream for MemWrite {
    fn finish(self: Box<Self>, _sync: bool) -> Result<(), VfsError> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .inject(MemOp::Finish)
            .map_err(|e| match e {
                VfsError::Io {
                    location: None,
                    message,
                } => VfsError::Io {
                    message,
                    location: Some(self.location.clone()),
                },
                other => other,
            })
    }
}

impl Provider for MemoryProvider {
    fn scheme(&self) -> &'static str {
        "file"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            watch: false,
            case_rule: self.lock().rule,
        }
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        let mut inner = self.lock();
        inner.inject(MemOp::Stat)?;
        let comps = self.comps(path)?;
        let node = inner
            .node(&comps)
            .ok_or_else(|| inner.missing(&comps, path.to_location()))?;
        let (name, parent) = comps
            .split_last()
            .map_or((OsString::new(), &comps[..0]), |(n, p)| (n.clone(), p));
        Ok(Self::entry(&inner, &name, node, parent, &self.root, true))
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        inline_link_budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        let mut inner = self.lock();
        inner.inject(MemOp::List)?;
        let comps = self.comps(path)?;
        let location = path.to_location();
        let node = inner
            .node(&comps)
            .ok_or_else(|| inner.missing(&comps, location.clone()))?;
        let Kind::Dir(children) = &node.kind else {
            return Err(VfsError::NotADirectory { location });
        };
        let mut budget = inline_link_budget;
        let mut entries = Vec::new();
        for (name, child) in children.values() {
            if cancel.is_cancelled() {
                return Err(VfsError::Cancelled);
            }
            let resolve = matches!(child.kind, Kind::Symlink(_)) && budget > 0;
            if resolve {
                budget -= 1;
            }
            entries.push(Self::entry(
                &inner, name, child, &comps, &self.root, resolve,
            ));
        }
        // One check per entry read, so a test can fail a single entry mid-listing. The entries are
        // built first because the injection needs the lock mutably.
        for _ in 0..entries.len() {
            inner.inject(MemOp::ListEntry)?;
        }
        progress(entries.len() as u32);
        Ok(entries)
    }

    fn resolve_link(
        &self,
        folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        let mut inner = self.lock();
        inner.inject(MemOp::ResolveLink)?;
        let mut comps = self.comps(folder)?;
        comps.push(entry.name.clone());
        let node = inner
            .node(&comps)
            .ok_or_else(|| inner.missing(&comps, folder.to_location()))?;
        let parent = &comps[..comps.len() - 1];
        Ok(Self::entry(
            &inner,
            &entry.name,
            node,
            parent,
            &self.root,
            true,
        ))
    }

    fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.create(MemOp::CreateDir, path, Kind::Dir(BTreeMap::new()))
    }

    fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.create(MemOp::CreateFile, path, Kind::File(Vec::new()))
    }

    fn rename(&self, from: &VfsPath, to: &VfsPath, overwrite: bool) -> Result<(), VfsError> {
        let mut inner = self.lock();
        inner.inject(MemOp::Rename)?;
        validate_new_path(to, inner.rule)?;
        let (from_loc, to_loc) = (from.to_location(), to.to_location());
        let (f, t) = (self.comps(from)?, self.comps(to)?);
        // The kernel resolves both parent folders (the source's first) before it looks at either
        // name, so a bad parent is reported ahead of a missing source.
        let (Some((_, f_parent)), Some((_, t_parent))) = (f.split_last(), t.split_last()) else {
            return Err(VfsError::PermissionDenied { location: to_loc });
        };
        for (parent, whole, location) in [(f_parent, &f, &from_loc), (t_parent, &t, &to_loc)] {
            match inner.node(parent).map(Node::is_dir) {
                Some(true) => {}
                Some(false) => {
                    return Err(VfsError::NotADirectory {
                        location: location.clone(),
                    })
                }
                None => return Err(inner.missing(whole, location.clone())),
            }
        }
        let source = inner.node(&f).ok_or_else(|| VfsError::NotFound {
            location: from_loc.clone(),
        })?;
        let source_is_dir = source.is_dir();
        if inner.volume_of(&f) != inner.volume_of(&t) {
            return Err(VfsError::CrossesDevices {
                from: from_loc,
                to: to_loc,
            });
        }
        let same_entry =
            f.len() == t.len() && f.iter().zip(&t).all(|(a, b)| inner.key(a) == inner.key(b));
        if same_entry {
            // The same entry under two spellings (a case-only rename) is allowed; the same spelling
            // is an existing target.
            if f == t && !overwrite {
                return Err(VfsError::AlreadyExists { location: to_loc });
            }
        } else {
            // `RENAME_NOREPLACE` reports an existing target before anything else.
            if !overwrite && inner.node(&t).is_some() {
                return Err(VfsError::AlreadyExists { location: to_loc });
            }
            if source_is_dir && t.len() > f.len() && {
                let (head, _) = t.split_at(f.len());
                head.iter()
                    .zip(&f)
                    .all(|(a, b)| inner.key(a) == inner.key(b))
            } {
                return Err(VfsError::Io {
                    message: "a folder cannot be moved into itself".to_owned(),
                    location: Some(from_loc),
                });
            }
            if let Some(existing) = inner.node(&t) {
                if !overwrite {
                    return Err(VfsError::AlreadyExists { location: to_loc });
                }
                match (&existing.kind, source_is_dir) {
                    (Kind::Dir(children), true) if !children.is_empty() => {
                        return Err(VfsError::NotEmpty { location: to_loc });
                    }
                    (Kind::Dir(_), false) => {
                        return Err(VfsError::IsADirectory { location: to_loc });
                    }
                    (Kind::File(_) | Kind::Symlink(_), true) => {
                        return Err(VfsError::NotADirectory { location: to_loc });
                    }
                    _ => {}
                }
            }
        }
        let (children, key) = inner.parent_mut(&f, &from_loc)?;
        let (_, node) = children.remove(&key).expect("the source was found");
        let (children, key) = inner.parent_mut(&t, &to_loc)?;
        let name = t.last().expect("a target has a name").clone();
        children.insert(key, (name, node));
        Ok(())
    }

    fn remove_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        let mut inner = self.lock();
        inner.inject(MemOp::RemoveFile)?;
        let location = path.to_location();
        let comps = self.comps(path)?;
        let (children, key) = inner.parent_mut(&comps, &location)?;
        match children.get(&key).map(|(_, n)| n.is_dir()) {
            None => Err(VfsError::NotFound { location }),
            Some(true) => Err(VfsError::IsADirectory { location }),
            Some(false) => {
                children.remove(&key);
                Ok(())
            }
        }
    }

    fn remove_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let mut inner = self.lock();
        inner.inject(MemOp::RemoveDir)?;
        let location = path.to_location();
        let comps = self.comps(path)?;
        let (children, key) = inner.parent_mut(&comps, &location)?;
        match children.get(&key).map(|(_, n)| &n.kind) {
            None => Err(VfsError::NotFound { location }),
            Some(Kind::Dir(held)) if !held.is_empty() => Err(VfsError::NotEmpty { location }),
            Some(Kind::Dir(_)) => {
                children.remove(&key);
                Ok(())
            }
            Some(_) => Err(VfsError::NotADirectory { location }),
        }
    }

    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        let mut inner = self.lock();
        inner.inject(MemOp::OpenRead)?;
        let location = path.to_location();
        let comps = self.comps(path)?;
        let node = self
            .node_following(&inner, &comps)
            .ok_or_else(|| inner.missing(&comps, location.clone()))?;
        match node.kind {
            Kind::File(bytes) => Ok(Box::new(MemRead {
                inner: self.inner.clone(),
                data: io::Cursor::new(bytes),
            })),
            _ => Err(VfsError::IsADirectory { location }),
        }
    }

    fn create_write(
        &self,
        path: &VfsPath,
        options: WriteOptions,
    ) -> Result<Box<dyn WriteStream>, VfsError> {
        let mut inner = self.lock();
        inner.inject(MemOp::CreateWrite)?;
        validate_new_path(path, inner.rule)?;
        let location = path.to_location();
        let mut comps = self.comps(path)?;
        if options.exclusive && inner.node(&comps).is_some() {
            return Err(VfsError::AlreadyExists { location });
        }
        // A non-exclusive write follows a symlink in the final component and writes (or creates)
        // what it points at, as `open(2)` does; a link is never replaced by the file.
        let mut hops = 0;
        while let Some(Kind::Symlink(text)) = inner.node(&comps).map(|n| &n.kind) {
            hops += 1;
            let followed = comps
                .split_last()
                .and_then(|(_, up)| Self::target_comps(up, text, &self.root));
            match followed {
                Some(next) if hops <= 8 => comps = next,
                _ => {
                    return Err(VfsError::Io {
                        message: "too many levels of symbolic links".to_owned(),
                        location: Some(location),
                    })
                }
            }
        }
        let existing = inner.node(&comps).map(|n| n.entry_kind());
        match existing {
            Some(EntryKind::Directory) => return Err(VfsError::IsADirectory { location }),
            Some(_) => {
                let now = inner.tick();
                let node = inner.node_mut(&comps).expect("just found");
                node.kind = Kind::File(Vec::new());
                node.modified = now;
            }
            None => {
                let mut node = inner.new_node(Kind::File(Vec::new()));
                if let Some(mode) = options.mode {
                    node.mode = mode & 0o7777;
                }
                let (children, key) = inner.parent_mut(&comps, &location)?;
                let name = comps
                    .last()
                    .expect("a path with a parent has a name")
                    .clone();
                children.insert(key, (name, node));
            }
        }
        Ok(Box::new(MemWrite {
            inner: self.inner.clone(),
            comps,
            location,
        }))
    }

    fn set_times(&self, path: &VfsPath, times: FileTimes) -> Result<(), VfsError> {
        let mut inner = self.lock();
        inner.inject(MemOp::SetTimes)?;
        let comps = self.comps(path)?;
        let missing = inner.missing(&comps, path.to_location());
        let node = inner.node_mut(&comps).ok_or(missing)?;
        if let Some(accessed) = times.accessed {
            node.accessed = accessed;
        }
        if let Some(modified) = times.modified {
            node.modified = modified;
        }
        Ok(())
    }

    fn permissions(&self, path: &VfsPath) -> Result<Permissions, VfsError> {
        let mut inner = self.lock();
        inner.inject(MemOp::Permissions)?;
        let comps = self.comps(path)?;
        let node = self
            .node_following(&inner, &comps)
            .ok_or_else(|| inner.missing(&comps, path.to_location()))?;
        Ok(Permissions {
            mode: Some(node.mode),
            readonly: node.mode & 0o222 == 0,
        })
    }

    fn set_permissions(&self, path: &VfsPath, permissions: Permissions) -> Result<(), VfsError> {
        let mut inner = self.lock();
        inner.inject(MemOp::SetPermissions)?;
        let comps = self.comps(path)?;
        let missing = inner.missing(&comps, path.to_location());
        let node = inner.node_mut(&comps).ok_or(missing)?;
        if matches!(node.kind, Kind::Symlink(_)) {
            return Err(VfsError::Unsupported {
                what: "setting the permissions of a symlink".to_owned(),
            });
        }
        node.mode = match permissions.mode {
            Some(mode) => mode & 0o7777,
            None if permissions.readonly => node.mode & !0o222,
            None => node.mode | 0o200,
        };
        Ok(())
    }

    fn symlink(&self, link: &VfsPath, target: &OsStr) -> Result<(), VfsError> {
        self.create(MemOp::Symlink, link, Kind::Symlink(target.to_owned()))
    }

    fn read_link(&self, path: &VfsPath) -> Result<OsString, VfsError> {
        let mut inner = self.lock();
        inner.inject(MemOp::ReadLink)?;
        let location = path.to_location();
        let comps = self.comps(path)?;
        match inner.node(&comps).map(|n| &n.kind) {
            Some(Kind::Symlink(text)) => Ok(text.clone()),
            Some(_) => Err(VfsError::Io {
                message: "not a symbolic link".to_owned(),
                location: Some(location),
            }),
            None => Err(VfsError::NotFound { location }),
        }
    }

    fn canonicalize(&self, path: &VfsPath) -> Result<VfsPath, VfsError> {
        let inner = self.lock();
        let location = path.to_location();
        let mut pending: Vec<OsString> = self.comps(path)?;
        pending.reverse();
        let mut done: Vec<OsString> = Vec::new();
        let mut hops = 0;
        while let Some(name) = pending.pop() {
            done.push(name);
            let node = inner
                .node(&done)
                .ok_or_else(|| inner.missing(&done, location.clone()))?;
            if let Kind::Symlink(text) = &node.kind {
                hops += 1;
                let (_, up) = done.split_last().expect("just pushed");
                let target = Self::target_comps(up, text, &self.root);
                match target {
                    Some(target) if hops <= 40 => {
                        done.clear();
                        pending.extend(target.into_iter().rev());
                    }
                    _ => {
                        return Err(VfsError::Io {
                            message: "too many levels of symbolic links".to_owned(),
                            location: Some(location),
                        })
                    }
                }
            }
        }
        let mut resolved = self.root.clone();
        for name in &done {
            resolved = resolved.join(name).map_err(|_| VfsError::InvalidLocation {
                input: name.to_string_lossy().into_owned(),
            })?;
        }
        Ok(VfsPath::File(resolved))
    }

    fn volume_id(&self, path: &VfsPath) -> Option<VolumeId> {
        let comps = self.comps(path).ok()?;
        let inner = self.lock();
        inner.node(&comps)?;
        Some(inner.volume_of(&comps))
    }

    fn free_space(&self, path: &VfsPath) -> Option<VolumeSpace> {
        let comps = self.comps(path).ok()?;
        let inner = self.lock();
        inner.node(&comps)?;
        inner.space.get(&inner.volume_of(&comps)).copied()
    }

    fn copy_file_within(
        &self,
        src: &VfsPath,
        dst: &VfsPath,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Option<Result<u64, VfsError>> {
        let mut inner = self.lock();
        if !inner.fast_copy {
            return None;
        }
        if let Err(error) = inner.inject(MemOp::CopyFileWithin) {
            return Some(Err(error));
        }
        let (from, to) = (self.comps(src).ok()?, self.comps(dst).ok()?);
        let Some(Kind::File(bytes)) = inner.node(&from).map(|n| n.kind.clone()) else {
            return None;
        };
        if inner.volume_of(&from) != inner.volume_of(&to) {
            return None;
        }
        if cancel.is_cancelled() {
            return Some(Err(VfsError::Cancelled));
        }
        let location = dst.to_location();
        if let Err(error) = validate_new_path(dst, inner.rule) {
            return Some(Err(error));
        }
        let len = bytes.len() as u64;
        let node = inner.new_node(Kind::File(bytes));
        let (children, key) = match inner.parent_mut(&to, &location) {
            Ok(found) => found,
            Err(error) => return Some(Err(error)),
        };
        if children.contains_key(&key) {
            return Some(Err(VfsError::AlreadyExists { location }));
        }
        let name = to.last().expect("a path with a parent has a name").clone();
        children.insert(key, (name, node));
        drop(inner);
        progress(len);
        Some(Ok(len))
    }
}
