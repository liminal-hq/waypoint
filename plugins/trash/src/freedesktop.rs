// Implements the freedesktop.org Trash specification: home and per-volume trashes, `.trashinfo`, `directorysizes`, restore and empty
//
// The layout is the one Nemo, Nautilus and Dolphin use, so a file trashed here shows up in their trash and the other way round. Every environmental input arrives through `TrashEnv` and `TrashFs`, so tests run in a temporary directory.
//
// A trashed item is two things: `files/NAME` (the item itself) and `info/NAME.trashinfo` (where it came from and when). Trashing writes the `.trashinfo` first (exclusively, which also reserves the name), then renames the item into `files/`, and removes the `.trashinfo` again if the rename fails. Removing does it the other way round: the item, then its `.trashinfo`. A crash therefore leaves at worst an orphan `.trashinfo`, which is ignored and is cleaned by emptying.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod env;
mod fs;
mod trashinfo;

#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{Duration, NaiveDateTime, TimeZone};

pub use env::{default_env, parse_mountinfo, system_clock, Clock, MountInfo, TrashEnv};
pub use fs::{StdFs, TrashFs};
pub use trashinfo::{decode, encode, SizeEntry};

use crate::error::{Result, TrashError};
use crate::models::{EmptyFailure, EmptyReport, RestoreTarget, TrashReceipt, TrashedItem};
use trashinfo::{TrashInfo, INFO_SUFFIX};

/// The longest file name the common file systems allow.
const NAME_MAX: usize = 255;

/// How many collision suffixes to try before giving up on a name.
const MAX_ATTEMPTS: u32 = 100_000;

/// `directorysizes` is read, changed and replaced as a whole, so two threads of this process take turns. (Other processes may still race; the file is a cache, and a lost line is recomputed.)
static SIZES_LOCK: Mutex<()> = Mutex::new(());

/// One trash directory: the one in the home folder, or one on a volume.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TrashRoot {
    /// The trash directory, holding `files`, `info` and `directorysizes`.
    path: PathBuf,
    /// The top directory of the volume for a per-volume trash, whose `Path` values are relative to it. `None` for the home trash.
    topdir: Option<PathBuf>,
}

/// One item of a trash, as read from disk.
#[derive(Debug, Clone)]
struct Entry {
    root: TrashRoot,
    /// The name in `files/` (and, with `.trashinfo`, in `info/`).
    name: OsString,
    original_path: PathBuf,
    deleted_at: NaiveDateTime,
    size: u64,
    is_dir: bool,
}

/// The freedesktop trash over an injected environment.
pub struct Freedesktop {
    env: TrashEnv,
    fs: Arc<dyn TrashFs>,
}

impl Freedesktop {
    /// A trash over the real file system.
    pub fn new(env: TrashEnv) -> Self {
        Self::with_fs(env, Arc::new(StdFs))
    }

    /// A trash over `fs`, which tests use to fake devices and cross-device renames.
    pub fn with_fs(mut env: TrashEnv, fs: Arc<dyn TrashFs>) -> Self {
        // Compare like with like: mount points and the home folder as the file system spells them.
        let canonical =
            |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        env.data_home = canonical(&env.data_home);
        env.home_dir = canonical(&env.home_dir);
        for mount in &mut env.mounts {
            mount.mount_point = canonical(&mount.mount_point);
        }
        Freedesktop { env, fs }
    }

    pub fn env(&self) -> &TrashEnv {
        &self.env
    }

    fn home_root(&self) -> TrashRoot {
        TrashRoot {
            path: self.env.data_home.join("Trash"),
            topdir: None,
        }
    }

    // ---------------------------------------------------------------- trashing

    /// Moves one file or folder (or link: the link, not what it points to) to the trash it belongs in.
    pub fn trash(&self, path: &Path) -> Result<TrashReceipt> {
        if !path.is_absolute() {
            return Err(TrashError::io("the path must be absolute"));
        }
        // Drop a trailing slash and `.` parts, and refuse `..` at the end, which names no file.
        let normal: PathBuf = path.components().collect();
        let name = match normal.file_name() {
            Some(name) if normal.components().next_back() != Some(Component::ParentDir) => {
                name.to_os_string()
            }
            _ => return Err(refused("the path does not name a file or folder")),
        };
        let parent = normal
            .parent()
            .ok_or_else(|| refused("the root cannot be trashed"))?;
        // Resolve links in the folders above, but never in the last part: a link is trashed itself.
        let target = std::fs::canonicalize(parent)?.join(&name);
        std::fs::symlink_metadata(&target)?;
        self.refuse_protected(&target)?;

        let root = self.trash_root_for(&target)?;
        let recorded: OsString = match &root.topdir {
            Some(topdir) => target
                .strip_prefix(topdir)
                .map_err(|_| TrashError::io("the file is outside its volume"))?
                .as_os_str()
                .to_os_string(),
            None => target.as_os_str().to_os_string(),
        };
        let deleted_at = (self.env.now)();
        let final_name = self.place(&root, &target, &name, &recorded, &deleted_at)?;

        self.record_directory_size(&root, &final_name);
        Ok(TrashReceipt {
            trash_id: make_id(&root.path, &final_name),
            original_path: target,
            deleted_at: to_unix(&deleted_at),
        })
    }

    /// Refuses the paths a trash must never swallow: the home folder and its parents, a mount point, and anything in, or holding, a trash.
    fn refuse_protected(&self, target: &Path) -> Result<()> {
        if self.env.home_dir.starts_with(target) {
            return Err(refused("the home folder and its parents cannot be trashed"));
        }
        // A mount point, or a folder with a volume mounted somewhere inside it: moving that away would hide the volume.
        if self
            .env
            .mounts
            .iter()
            .any(|mount| mount.mount_point.starts_with(target))
        {
            return Err(refused(
                "a mount point, or a folder holding one, cannot be trashed",
            ));
        }
        let home_trash = self.home_root().path;
        if target.starts_with(&home_trash) || home_trash.starts_with(target) {
            return Err(refused("the trash cannot be trashed"));
        }
        for mount in &self.env.mounts {
            let shared = mount.mount_point.join(".Trash");
            let own = mount.mount_point.join(format!(".Trash-{}", self.env.uid));
            if target.starts_with(&shared) || target.starts_with(&own) {
                return Err(refused("the trash cannot be trashed"));
            }
        }
        Ok(())
    }

    /// Picks the trash for a file: the home trash if the file is on the home trash's device, otherwise the trash of the file's own volume. A file is never copied across devices, so a volume with no usable trash is an error.
    fn trash_root_for(&self, target: &Path) -> Result<TrashRoot> {
        let file_device = self.fs.device_id(target)?;
        let home = self.home_root();
        if file_device == self.home_device(&home)? {
            ensure_trash_dir(&home.path).map_err(|error| TrashError::TrashUnavailable {
                reason: format!("cannot create {}: {error}", home.path.display()),
            })?;
            return Ok(home);
        }
        let mount = self
            .env
            .mounts
            .iter()
            .filter(|mount| {
                mount.device_id == file_device && target.starts_with(&mount.mount_point)
            })
            .max_by_key(|mount| mount.mount_point.components().count())
            .ok_or_else(|| TrashError::TrashUnavailable {
                reason: format!("no mounted volume was found for {}", target.display()),
            })?;
        self.volume_trash(&mount.mount_point)
    }

    /// The device the home trash is (or would be) on: that of its nearest folder that exists. Asking does not create anything.
    fn home_device(&self, home: &TrashRoot) -> Result<u64> {
        let existing = home
            .path
            .ancestors()
            .find(|ancestor| ancestor.exists())
            .ok_or_else(|| TrashError::TrashUnavailable {
                reason: format!("{} has no existing parent", home.path.display()),
            })?;
        Ok(self.fs.device_id(existing)?)
    }

    /// The trash for a volume, created if needed: `$topdir/.Trash/$uid` when `.Trash` exists, is a real folder (not a link) and has the sticky bit, otherwise `$topdir/.Trash-$uid`.
    fn volume_trash(&self, topdir: &Path) -> Result<TrashRoot> {
        if let Some(shared) = self.shared_trash_dir(topdir) {
            match ensure_trash_dir(&shared) {
                Ok(()) => {
                    return Ok(TrashRoot {
                        path: shared,
                        topdir: Some(topdir.to_path_buf()),
                    })
                }
                Err(error) => log::warn!(
                    "cannot use {}: {error}; falling back to the per-user trash",
                    shared.display()
                ),
            }
        }
        let own = topdir.join(format!(".Trash-{}", self.env.uid));
        ensure_trash_dir(&own).map_err(|error| TrashError::TrashUnavailable {
            reason: format!("cannot create {}: {error}", own.display()),
        })?;
        Ok(TrashRoot {
            path: own,
            topdir: Some(topdir.to_path_buf()),
        })
    }

    /// `$topdir/.Trash/$uid`, if `$topdir/.Trash` passes the spec's checks.
    fn shared_trash_dir(&self, topdir: &Path) -> Option<PathBuf> {
        let shared = topdir.join(".Trash");
        let metadata = std::fs::symlink_metadata(&shared).ok()?;
        let usable = metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.permissions().mode() & 0o1000 != 0;
        usable.then(|| shared.join(self.env.uid.to_string()))
    }

    /// Reserves a unique name in `root`, moves `source` there and returns the name. The `.trashinfo` is created first and exclusively, and removed again if anything after it fails.
    fn place(
        &self,
        root: &TrashRoot,
        source: &Path,
        base: &OsStr,
        recorded_path: &OsStr,
        deleted_at: &NaiveDateTime,
    ) -> Result<OsString> {
        let content = trashinfo::format_info(&trashinfo::encode_os(recorded_path), deleted_at);
        let files = root.path.join("files");
        let info = root.path.join("info");
        for attempt in 0..MAX_ATTEMPTS {
            let name = candidate_name(base, attempt);
            let info_path = info.join(info_file_name(&name));
            let files_path = files.join(&name);
            let mut file = match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&info_path)
            {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            };
            // An item without a `.trashinfo` (left by a crash or another tool) holds the name too.
            if std::fs::symlink_metadata(&files_path).is_ok() {
                drop(file);
                let _ = std::fs::remove_file(&info_path);
                continue;
            }
            if let Err(error) = file
                .write_all(content.as_bytes())
                .and_then(|()| file.flush())
            {
                drop(file);
                let _ = std::fs::remove_file(&info_path);
                return Err(error.into());
            }
            drop(file);
            match self.fs.rename_noreplace(source, &files_path) {
                Ok(()) => return Ok(name),
                Err(error) => {
                    // Roll back: without the item there must be no `.trashinfo` either.
                    let _ = std::fs::remove_file(&info_path);
                    if error.kind() == io::ErrorKind::AlreadyExists {
                        continue;
                    }
                    return Err(rename_error(&error));
                }
            }
        }
        Err(TrashError::io("no free name in the trash"))
    }

    // ----------------------------------------------------------- directorysizes

    /// Adds a trashed folder to the `directorysizes` cache. The cache is optional, so a failure is logged and not reported.
    fn record_directory_size(&self, root: &TrashRoot, name: &OsStr) {
        let files_path = root.path.join("files").join(name);
        let is_dir = std::fs::symlink_metadata(&files_path)
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false);
        if !is_dir {
            return;
        }
        let Some(mtime) = info_mtime(&root.path.join("info").join(info_file_name(name))) else {
            return;
        };
        let entry = SizeEntry {
            size: tree_size(&files_path),
            mtime,
            name: name.to_os_string(),
        };
        if let Err(error) = update_sizes(&root.path, |entries| {
            entries.retain(|existing| existing.name != entry.name);
            entries.push(entry);
        }) {
            log::warn!(
                "cannot update {}/directorysizes: {error}",
                root.path.display()
            );
        }
    }

    // ----------------------------------------------------------------- listing

    /// The trash directories that exist: the home trash, then `.Trash/$uid` and `.Trash-$uid` of every mount.
    fn existing_roots(&self) -> Vec<TrashRoot> {
        let mut seen = HashSet::new();
        let mut roots = Vec::new();
        let mut push = |root: TrashRoot| {
            if is_real_dir(&root.path) && seen.insert(root.path.clone()) {
                roots.push(root);
            }
        };
        push(self.home_root());
        for mount in &self.env.mounts {
            if let Some(shared) = self.shared_trash_dir(&mount.mount_point) {
                push(TrashRoot {
                    path: shared,
                    topdir: Some(mount.mount_point.clone()),
                });
            }
            push(TrashRoot {
                path: mount.mount_point.join(format!(".Trash-{}", self.env.uid)),
                topdir: Some(mount.mount_point.clone()),
            });
        }
        roots
    }

    /// Finds the root a receipt names, but only among the trashes this environment knows. A receipt comes from outside, so it never reaches an arbitrary directory.
    fn root_for_id(&self, trash_root: &Path) -> Option<TrashRoot> {
        self.existing_roots()
            .into_iter()
            .find(|root| root.path == trash_root)
    }

    fn entries_in(&self, root: &TrashRoot) -> Vec<Entry> {
        let Some((files, info)) = open_root(&root.path) else {
            return Vec::new();
        };
        let Ok(read) = std::fs::read_dir(&info) else {
            return Vec::new();
        };
        let sizes = read_sizes(&root.path);
        let mut entries = Vec::new();
        for dirent in read.flatten() {
            let file_name = dirent.file_name();
            let Some(name) = strip_info_suffix(&file_name) else {
                continue;
            };
            let Ok(text) = std::fs::read(dirent.path()) else {
                continue;
            };
            let Some(parsed) = trashinfo::parse_info(&String::from_utf8_lossy(&text)) else {
                log::warn!("skipping unreadable {}", dirent.path().display());
                continue;
            };
            let Ok(metadata) = std::fs::symlink_metadata(files.join(&name)) else {
                continue; // an orphan `.trashinfo`
            };
            let TrashInfo { path, deleted_at } = parsed;
            let mtime = info_mtime(&dirent.path());
            let deleted_at = deleted_at
                .or_else(|| mtime.and_then(from_unix))
                .unwrap_or_else(|| (self.env.now)());
            let original_path = match &root.topdir {
                Some(topdir) => topdir.join(&path),
                None => PathBuf::from(&path),
            };
            let is_dir = metadata.is_dir();
            let size = if is_dir {
                sizes
                    .iter()
                    .find(|entry| entry.name == name && Some(entry.mtime) == mtime)
                    .map_or_else(|| tree_size(&files.join(&name)), |entry| entry.size)
            } else {
                metadata.len()
            };
            entries.push(Entry {
                root: root.clone(),
                name,
                original_path,
                deleted_at,
                size,
                is_dir,
            });
        }
        entries
    }

    fn all_entries(&self) -> Vec<Entry> {
        let mut entries: Vec<Entry> = self
            .existing_roots()
            .iter()
            .flat_map(|root| self.entries_in(root))
            .collect();
        entries.sort_by(|a, b| {
            a.deleted_at
                .cmp(&b.deleted_at)
                .then_with(|| a.root.path.cmp(&b.root.path))
                .then_with(|| a.name.cmp(&b.name))
        });
        entries
    }

    /// Every item in the home trash and in the trash of every mount.
    pub fn list(&self) -> Vec<TrashedItem> {
        self.all_entries()
            .into_iter()
            .map(|entry| {
                let display = entry
                    .original_path
                    .file_name()
                    .unwrap_or(&entry.name)
                    .to_string_lossy()
                    .into_owned();
                TrashedItem {
                    receipt: receipt_of(&entry),
                    name: display,
                    original_path: entry.original_path,
                    deleted_at: to_unix(&entry.deleted_at),
                    size: entry.size,
                    is_dir: entry.is_dir,
                }
            })
            .collect()
    }

    // ----------------------------------------------------------------- removing

    /// Looks a receipt up: its trash, its name, and the item's `.trashinfo`.
    fn resolve(&self, trash_id: &str) -> Result<(TrashRoot, OsString)> {
        let (root_path, name) = parse_id(trash_id).ok_or(TrashError::NotFound)?;
        let root = self.root_for_id(&root_path).ok_or(TrashError::NotFound)?;
        Ok((root, name))
    }

    /// Puts an item back, at its original path or at the given one. Never overwrites: a taken destination is `OriginExists` and a missing folder is `OriginMissingParent`.
    pub fn restore(&self, trash_id: &str, target: &RestoreTarget) -> Result<TrashReceipt> {
        let (root, name) = self.resolve(trash_id)?;
        let (files, info) = open_root(&root.path).ok_or(TrashError::NotFound)?;
        let info_path = info.join(info_file_name(&name));
        let source = files.join(&name);
        let text = std::fs::read(&info_path)?;
        let parsed = trashinfo::parse_info(&String::from_utf8_lossy(&text))
            .ok_or_else(|| TrashError::io("the item's .trashinfo is unreadable"))?;
        std::fs::symlink_metadata(&source)?;

        let original = match &root.topdir {
            Some(topdir) => topdir.join(&parsed.path),
            None => PathBuf::from(&parsed.path),
        };
        let destination = match target {
            RestoreTarget::Original => original.clone(),
            RestoreTarget::Path { path } => path.clone(),
        };
        if !destination.is_absolute() {
            return Err(TrashError::io("the destination must be an absolute path"));
        }
        if std::fs::symlink_metadata(&destination).is_ok() {
            return Err(TrashError::OriginExists { path: destination });
        }
        let parent = destination
            .parent()
            .ok_or_else(|| TrashError::io("the destination has no folder"))?;
        if !parent.is_dir() {
            return Err(TrashError::OriginMissingParent {
                path: parent.to_path_buf(),
            });
        }
        if let Err(error) = self.fs.rename_noreplace(&source, &destination) {
            return Err(match error.kind() {
                io::ErrorKind::AlreadyExists => TrashError::OriginExists { path: destination },
                io::ErrorKind::NotFound if !parent.is_dir() => TrashError::OriginMissingParent {
                    path: parent.to_path_buf(),
                },
                _ if error.raw_os_error() == Some(libc::EXDEV) => TrashError::io(
                    "an item cannot be restored to another volume than the one it is trashed on",
                ),
                _ => rename_error(&error),
            });
        }
        if let Err(error) = std::fs::remove_file(&info_path) {
            log::warn!("cannot remove {}: {error}", info_path.display());
        }
        self.forget_directory_size(&root, &name);
        Ok(TrashReceipt {
            trash_id: trash_id.to_string(),
            original_path: destination,
            deleted_at: parsed
                .deleted_at
                .map_or_else(|| to_unix(&(self.env.now)()), |date| to_unix(&date)),
        })
    }

    /// Removes one item for good: the item first, then its `.trashinfo`.
    pub fn delete(&self, trash_id: &str) -> Result<()> {
        let (root, name) = self.resolve(trash_id)?;
        let (files, info) = open_root(&root.path).ok_or(TrashError::NotFound)?;
        let item = files.join(&name);
        let info_path = info.join(info_file_name(&name));
        let had_item = std::fs::symlink_metadata(&item).is_ok();
        let had_info = std::fs::symlink_metadata(&info_path).is_ok();
        if !had_item && !had_info {
            return Err(TrashError::NotFound);
        }
        if had_item {
            remove_tree(&item)?;
        }
        remove_if_exists(&info_path)?;
        self.forget_directory_size(&root, &name);
        Ok(())
    }

    fn forget_directory_size(&self, root: &TrashRoot, name: &OsStr) {
        if let Err(error) = update_sizes(&root.path, |entries| {
            entries.retain(|entry| entry.name != name);
        }) {
            log::warn!(
                "cannot update {}/directorysizes: {error}",
                root.path.display()
            );
        }
    }

    /// Empties the trash, or only the items trashed `older_than_days` or more days ago. Each item is removed on its own (the item, then its `.trashinfo`); one that cannot be removed is reported and the rest still go.
    pub fn empty(&self, older_than_days: Option<u32>) -> EmptyReport {
        let mut report = EmptyReport::default();
        match older_than_days {
            Some(days) => {
                let cutoff = (self.env.now)() - Duration::days(i64::from(days));
                for entry in self.all_entries() {
                    if entry.deleted_at > cutoff {
                        continue;
                    }
                    self.remove_entry(&entry.root, &entry.name, &mut report);
                }
            }
            None => {
                for root in self.existing_roots() {
                    self.empty_root(&root, &mut report);
                }
            }
        }
        report
    }

    fn remove_entry(&self, root: &TrashRoot, name: &OsStr, report: &mut EmptyReport) {
        match self.delete(&make_id(&root.path, name)) {
            Ok(()) => report.removed += 1,
            Err(error) => report.failed.push(EmptyFailure {
                trash_id: make_id(&root.path, name),
                error,
            }),
        }
    }

    /// Removes everything in one trash: the items (including any without a `.trashinfo`), then the `.trashinfo` files, then the size cache.
    fn empty_root(&self, root: &TrashRoot, report: &mut EmptyReport) {
        let Some((files, info)) = open_root(&root.path) else {
            return;
        };
        let mut failed_names: HashSet<OsString> = HashSet::new();
        if let Ok(read) = std::fs::read_dir(&files) {
            for dirent in read.flatten() {
                let name = dirent.file_name();
                match remove_tree(&dirent.path()) {
                    Ok(()) => report.removed += 1,
                    Err(error) => {
                        failed_names.insert(name.clone());
                        report.failed.push(EmptyFailure {
                            trash_id: make_id(&root.path, &name),
                            error: TrashError::from(error),
                        });
                    }
                }
            }
        }
        if let Ok(read) = std::fs::read_dir(&info) {
            for dirent in read.flatten() {
                let file_name = dirent.file_name();
                // Keep the `.trashinfo` of an item that could not be removed: it is still in the trash.
                if strip_info_suffix(&file_name).is_some_and(|name| failed_names.contains(&name)) {
                    continue;
                }
                if let Err(error) = std::fs::remove_file(dirent.path()) {
                    if error.kind() != io::ErrorKind::NotFound {
                        log::warn!("cannot remove {}: {error}", dirent.path().display());
                    }
                }
            }
        }
        if failed_names.is_empty() {
            let _ = std::fs::remove_file(root.path.join("directorysizes"));
        }
    }
}

// ------------------------------------------------------------------ receipts

fn make_id(root: &Path, name: &OsStr) -> String {
    format!(
        "{}|{}",
        trashinfo::encode_os(root.as_os_str()),
        trashinfo::encode_os(name)
    )
}

fn parse_id(id: &str) -> Option<(PathBuf, OsString)> {
    let (root, name) = id.split_once('|')?;
    let root = PathBuf::from(trashinfo::decode_os(root));
    let name = trashinfo::decode_os(name);
    // The name is one file name: it must not climb or descend.
    let single = name.as_bytes();
    if single.is_empty() || single == b"." || single == b".." || single.contains(&b'/') {
        return None;
    }
    root.is_absolute().then_some((root, name))
}

fn receipt_of(entry: &Entry) -> TrashReceipt {
    TrashReceipt {
        trash_id: make_id(&entry.root.path, &entry.name),
        original_path: entry.original_path.clone(),
        deleted_at: to_unix(&entry.deleted_at),
    }
}

// ------------------------------------------------------------------- helpers

fn refused(reason: &str) -> TrashError {
    TrashError::Refused {
        reason: reason.to_string(),
    }
}

fn rename_error(error: &io::Error) -> TrashError {
    match error.raw_os_error() {
        Some(libc::EXDEV) => TrashError::TrashUnavailable {
            reason: "the file is on another file system than its trash, and moving it would mean copying it"
                .to_string(),
        },
        Some(libc::EACCES) | Some(libc::EPERM) | Some(libc::EROFS) => TrashError::PermissionDenied,
        _ => TrashError::from_io(error),
    }
}

/// Seconds since the Unix epoch for a local time, the way the trash writes `DeletionDate`. A time that does not exist (a clock change) or exists twice takes the earlier reading.
pub(crate) fn to_unix(local: &NaiveDateTime) -> i64 {
    chrono::Local
        .from_local_datetime(local)
        .earliest()
        .map_or_else(|| local.and_utc().timestamp(), |time| time.timestamp())
}

fn from_unix(seconds: i64) -> Option<NaiveDateTime> {
    chrono::Local
        .timestamp_opt(seconds, 0)
        .earliest()
        .map(|time| time.naive_local())
}

fn is_real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir())
}

/// The `files` and `info` folders of a trash, only if both are real folders (a link there could point anywhere).
fn open_root(root: &Path) -> Option<(PathBuf, PathBuf)> {
    let files = root.join("files");
    let info = root.join("info");
    (is_real_dir(root) && is_real_dir(&files) && is_real_dir(&info)).then_some((files, info))
}

/// Creates a trash directory and its `files` and `info` folders, with mode 0700, as the spec asks. A link in the way is an error.
fn ensure_trash_dir(root: &Path) -> io::Result<()> {
    if let Some(parent) = root.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    for dir in [root.to_path_buf(), root.join("files"), root.join("info")] {
        match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        if !is_real_dir(&dir) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} is not a folder", dir.display()),
            ));
        }
    }
    Ok(())
}

fn info_file_name(name: &OsStr) -> OsString {
    let mut file = name.to_os_string();
    file.push(INFO_SUFFIX);
    file
}

fn strip_info_suffix(file_name: &OsStr) -> Option<OsString> {
    let bytes = file_name.as_bytes();
    let stem = bytes.strip_suffix(INFO_SUFFIX.as_bytes())?;
    (!stem.is_empty()).then(|| OsString::from_vec(stem.to_vec()))
}

fn info_mtime(path: &Path) -> Option<i64> {
    std::fs::symlink_metadata(path)
        .ok()
        .map(|metadata| metadata.mtime())
}

/// The name for the `attempt`th try: `name`, then `name.2`, with the number before the extension (`photo.2.jpg`), shortened so that its `.trashinfo` still fits a file name.
fn candidate_name(base: &OsStr, attempt: u32) -> OsString {
    let bytes = base.as_bytes();
    let (stem, extension) = match bytes.iter().rposition(|byte| *byte == b'.') {
        Some(dot) if dot > 0 => bytes.split_at(dot),
        _ => (bytes, &bytes[bytes.len()..]),
    };
    let suffix = if attempt == 0 {
        String::new()
    } else {
        format!(".{}", attempt + 1)
    };
    let budget = NAME_MAX - INFO_SUFFIX.len();
    // An extension that cannot fit with a stem is dropped rather than kept as the whole name.
    let extension = if extension.len() + suffix.len() >= budget {
        &extension[..0]
    } else {
        extension
    };
    let room = budget - suffix.len() - extension.len();
    let mut stem_end = stem.len().min(room);
    // Do not cut a UTF-8 character in half.
    while stem_end > 0 && stem_end < stem.len() && (stem[stem_end] & 0xC0) == 0x80 {
        stem_end -= 1;
    }
    let mut out = stem[..stem_end].to_vec();
    out.extend_from_slice(suffix.as_bytes());
    out.extend_from_slice(extension);
    OsString::from_vec(out)
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Removes a file, a link (not its target) or a whole folder tree. A folder that denies access (mode 0500, say) is made writable first, so an item the user could trash can also be emptied.
fn remove_tree(path: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return std::fs::remove_file(path);
    }
    let mode = metadata.permissions().mode();
    if mode & 0o700 != 0o700 {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode | 0o700))?;
    }
    for dirent in std::fs::read_dir(path)? {
        remove_tree(&dirent?.path())?;
    }
    std::fs::remove_dir(path)
}

/// The total size of the files under `path`, not following links.
fn tree_size(path: &Path) -> u64 {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if !metadata.is_dir() {
        return metadata.len();
    }
    std::fs::read_dir(path)
        .map(|read| read.flatten().map(|dirent| tree_size(&dirent.path())).sum())
        .unwrap_or(0)
}

fn read_sizes(root: &Path) -> Vec<SizeEntry> {
    std::fs::read(root.join("directorysizes"))
        .map(|bytes| trashinfo::parse_sizes(&String::from_utf8_lossy(&bytes)))
        .unwrap_or_default()
}

/// Rewrites `directorysizes` through a temporary file and a rename, so a reader never sees half a file.
fn update_sizes(root: &Path, change: impl FnOnce(&mut Vec<SizeEntry>)) -> io::Result<()> {
    let _guard = SIZES_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut entries = read_sizes(root);
    let before = entries.clone();
    change(&mut entries);
    if entries == before {
        return Ok(());
    }
    let target = root.join("directorysizes");
    if entries.is_empty() {
        return remove_if_exists(&target);
    }
    let temporary = root.join(format!("directorysizes.{}.tmp", std::process::id()));
    let mut text = String::new();
    for entry in &entries {
        text.push_str(&trashinfo::format_size_entry(entry));
        text.push('\n');
    }
    let result =
        std::fs::write(&temporary, text).and_then(|()| std::fs::rename(&temporary, &target));
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}
