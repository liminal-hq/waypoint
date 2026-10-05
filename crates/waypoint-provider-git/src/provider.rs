// `GitProvider`: a revision of a local repository as read-only folders.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Cursor;
use std::sync::Mutex;

use gix::object::tree::EntryKind as TreeKind;
use waypoint_path::{CaseRule, GitPath, VfsPath};
use waypoint_protocol::VfsError;
use waypoint_vfs::{
    group_for_scan, CancelToken, Capabilities, EntryKind, IconGroup, Provider, ReadStream,
    ScannedEntry,
};

use crate::errors::{corrupt, not_found, unsupported};
use crate::names::{os_bytes, os_string, resolve_in_tree};

/// What an entry of a revision is, finer than the listing's `EntryKind`: Git tells a plain file
/// from an executable one and a submodule from everything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Tree,
    Blob,
    Executable,
    Symlink,
    /// A commit of another repository, recorded where the submodule is. Its contents are not in
    /// this repository, so it cannot be opened here.
    Submodule,
}

/// What `GitProvider::describe` says about one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitEntryInfo {
    pub kind: ObjectKind,
    /// The object's id in hexadecimal (for a submodule, the commit it is pinned at).
    pub id: String,
    /// The commit the revision resolved to, so a name such as `main` can be shown with the exact
    /// commit being browsed. `None` for a repository with no commits yet.
    pub commit: Option<String>,
}

/// Serves `git+file:` locations. It holds nothing but opened repositories, so a provider is cheap
/// and one is shared by every tab.
#[derive(Default)]
pub struct GitProvider {
    repos: Mutex<HashMap<std::path::PathBuf, gix::ThreadSafeRepository>>,
}

/// A revision resolved to a tree.
struct View {
    repo: gix::Repository,
    /// `None` for a repository with no commits: an empty root.
    tree: Option<gix::ObjectId>,
    commit: Option<gix::ObjectId>,
    time_ms: Option<i64>,
}

struct Found {
    kind: ObjectKind,
    id: gix::ObjectId,
}

fn kind_of(kind: TreeKind) -> ObjectKind {
    match kind {
        TreeKind::Tree => ObjectKind::Tree,
        TreeKind::Blob => ObjectKind::Blob,
        TreeKind::BlobExecutable => ObjectKind::Executable,
        TreeKind::Link => ObjectKind::Symlink,
        TreeKind::Commit => ObjectKind::Submodule,
    }
}

fn entry_kind(kind: ObjectKind) -> EntryKind {
    match kind {
        ObjectKind::Tree => EntryKind::Directory,
        ObjectKind::Blob | ObjectKind::Executable => EntryKind::File,
        ObjectKind::Symlink => EntryKind::Symlink,
        ObjectKind::Submodule => EntryKind::Other,
    }
}

fn git_of(path: &VfsPath) -> Result<&GitPath, VfsError> {
    match path {
        VfsPath::Git(git) => Ok(git),
        other => Err(VfsError::InvalidLocation {
            input: other.display(),
        }),
    }
}

impl GitProvider {
    pub fn new() -> Self {
        Self::default()
    }

    fn open(&self, git: &GitPath, path: &VfsPath) -> Result<gix::Repository, VfsError> {
        let root = git.repo().as_path().to_path_buf();
        let mut repos = self.repos.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(shared) = repos.get(&root) {
            return Ok(shared.to_thread_local());
        }
        let shared = crate::repository::open(&root)
            .map_err(|error| {
                if crate::repository::find_repository(&root).as_deref() == Some(&root) {
                    corrupt(path, &error)
                } else {
                    not_found(path)
                }
            })?
            .into_sync();
        let local = shared.to_thread_local();
        // A folder that stops being a repository is dropped from the cache on its next failed open.
        if repos.len() > 32 {
            repos.clear();
        }
        repos.insert(root, shared);
        Ok(local)
    }

    fn view(&self, git: &GitPath, path: &VfsPath) -> Result<View, VfsError> {
        let repo = self.open(git, path)?;
        let commit_id = match git.rev() {
            None => match repo.head_id() {
                Ok(id) => Some(id.detach()),
                Err(_) if repo.head().map(|h| h.is_unborn()).unwrap_or(false) => None,
                Err(error) => return Err(corrupt(path, &error)),
            },
            Some(rev) => {
                let id = repo.rev_parse_single(rev).map_err(|error| {
                    log::debug!("git: no revision {rev:?}: {error}");
                    not_found(path)
                })?;
                Some(id.detach())
            }
        };
        let Some(commit_id) = commit_id else {
            return Ok(View {
                repo,
                tree: None,
                commit: None,
                time_ms: None,
            });
        };
        let (tree, time_ms) = {
            let commit = repo
                .find_object(commit_id)
                .and_then(|object| object.peel_to_commit())
                .map_err(|_| not_found(path))?;
            (
                commit.tree_id().map_err(|e| corrupt(path, &e))?.detach(),
                commit.time().ok().map(|time| time.seconds * 1000),
            )
        };
        Ok(View {
            repo,
            tree: Some(tree),
            commit: Some(commit_id),
            time_ms,
        })
    }

    /// Finds what `inner` names in the view's tree.
    fn find(&self, view: &View, inner: &[Vec<u8>], path: &VfsPath) -> Result<Found, VfsError> {
        let Some(root) = view.tree else {
            return if inner.is_empty() {
                Ok(Found {
                    kind: ObjectKind::Tree,
                    id: gix::ObjectId::empty_tree(view.repo.object_hash()),
                })
            } else {
                Err(not_found(path))
            };
        };
        if inner.is_empty() {
            return Ok(Found {
                kind: ObjectKind::Tree,
                id: root,
            });
        }
        let tree = view.repo.find_tree(root).map_err(|e| corrupt(path, &e))?;
        let wanted: Vec<&[u8]> = inner.iter().map(|part| part.as_slice()).collect();
        // `lookup_entry` goes through intermediate trees; a blob in the middle is "not found".
        match tree.lookup_entry(wanted) {
            Ok(Some(entry)) => Ok(Found {
                kind: kind_of(entry.mode().kind()),
                id: entry.object_id(),
            }),
            Ok(None) => Err(not_found(path)),
            Err(error) => Err(corrupt(path, &error)),
        }
    }

    fn blob(&self, view: &View, id: gix::ObjectId, path: &VfsPath) -> Result<Vec<u8>, VfsError> {
        let object = view.repo.find_object(id).map_err(|e| corrupt(path, &e))?;
        Ok(object.detach().data)
    }

    fn scanned(
        &self,
        view: &View,
        name: &[u8],
        kind: ObjectKind,
        id: gix::ObjectId,
        link: Option<(Option<EntryKind>, bool)>,
        path: &VfsPath,
    ) -> Result<ScannedEntry, VfsError> {
        let size = match kind {
            ObjectKind::Blob | ObjectKind::Executable | ObjectKind::Symlink => Some(
                view.repo
                    .find_header(id)
                    .map_err(|e| corrupt(path, &e))?
                    .size(),
            ),
            ObjectKind::Tree | ObjectKind::Submodule => None,
        };
        let (link_target, link_pending) = link.unwrap_or((None, false));
        let ek = entry_kind(kind);
        let group = if kind == ObjectKind::Submodule {
            IconGroup::Folder
        } else {
            group_for_scan(
                name,
                ek,
                link_target,
                link_pending,
                kind == ObjectKind::Executable,
            )
        };
        Ok(ScannedEntry {
            name: os_string(name),
            kind: ek,
            link_target,
            link_pending,
            group,
            special: None,
            size,
            modified_ms: view.time_ms,
            hidden: name.first() == Some(&b'.'),
            trashed: None,
        })
    }

    /// The kind of what a link points at, following links up to a small depth.
    fn link_kind(
        &self,
        view: &View,
        folder: &[Vec<u8>],
        link_id: gix::ObjectId,
        path: &VfsPath,
    ) -> Result<Option<EntryKind>, VfsError> {
        let mut folder = folder.to_vec();
        let mut id = link_id;
        for _ in 0..8 {
            let text = self.blob(view, id, path)?;
            let Some(parts) = resolve_in_tree(&folder, &text) else {
                return Ok(None);
            };
            if parts.is_empty() {
                return Ok(Some(EntryKind::Directory));
            }
            let Ok(found) = self.find(view, &parts, path) else {
                return Ok(None);
            };
            match found.kind {
                ObjectKind::Symlink => {
                    folder = parts[..parts.len() - 1].to_vec();
                    id = found.id;
                }
                other => return Ok(Some(entry_kind(other))),
            }
        }
        Ok(None)
    }

    /// Describes an entry for the Inspector and the status overlay: its kind exactly, its id, and
    /// the commit the revision resolved to.
    pub fn describe(&self, path: &VfsPath) -> Result<GitEntryInfo, VfsError> {
        let git = git_of(path)?;
        let view = self.view(git, path)?;
        let found = self.find(&view, git.inner(), path)?;
        Ok(GitEntryInfo {
            kind: found.kind,
            id: found.id.to_string(),
            commit: view.commit.map(|id| id.to_string()),
        })
    }

    fn entry_in(
        &self,
        view: &View,
        inner: &[Vec<u8>],
        found: &Found,
        path: &VfsPath,
        resolve: bool,
    ) -> Result<ScannedEntry, VfsError> {
        let name: Vec<u8> = inner.last().cloned().unwrap_or_else(|| {
            // The top of a revision is named for its repository.
            path.file_name()
                .map(|n| os_bytes(&n))
                .unwrap_or_else(|| b"/".to_vec())
        });
        let link = (found.kind == ObjectKind::Symlink).then(|| {
            if resolve {
                let folder = &inner[..inner.len().saturating_sub(1)];
                (
                    self.link_kind(view, folder, found.id, path).ok().flatten(),
                    false,
                )
            } else {
                (None, true)
            }
        });
        self.scanned(view, &name, found.kind, found.id, link, path)
    }
}

impl Provider for GitProvider {
    fn scheme(&self) -> &'static str {
        waypoint_path::GIT_SCHEME
    }

    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::new(CaseRule::Sensitive);
        caps.symlinks = true;
        caps.range_read = true;
        caps
    }

    fn read_only(&self) -> bool {
        true
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        let git = git_of(path)?;
        let view = self.view(git, path)?;
        let found = self.find(&view, git.inner(), path)?;
        self.entry_in(&view, git.inner(), &found, path, true)
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        inline_link_budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        let git = git_of(path)?;
        let view = self.view(git, path)?;
        let found = self.find(&view, git.inner(), path)?;
        match found.kind {
            ObjectKind::Tree => {}
            ObjectKind::Submodule => {
                return Err(unsupported(
                    "a submodule's files are in its own repository, not in this one",
                ))
            }
            _ => {
                return Err(VfsError::NotADirectory {
                    location: path.to_location(),
                })
            }
        }
        let tree = view
            .repo
            .find_tree(found.id)
            .map_err(|e| corrupt(path, &e))?;
        let mut out = Vec::new();
        let mut budget = inline_link_budget;
        for entry in tree.iter() {
            if cancel.is_cancelled() {
                return Err(VfsError::Cancelled);
            }
            let entry = entry.map_err(|e| corrupt(path, &e))?;
            let kind = kind_of(entry.mode().kind());
            let link = (kind == ObjectKind::Symlink).then(|| {
                if budget > 0 {
                    budget -= 1;
                    (
                        self.link_kind(&view, git.inner(), entry.oid().to_owned(), path)
                            .ok()
                            .flatten(),
                        false,
                    )
                } else {
                    (None, true)
                }
            });
            out.push(self.scanned(
                &view,
                entry.filename(),
                kind,
                entry.oid().to_owned(),
                link,
                path,
            )?);
            if out.len() % 512 == 0 {
                progress(out.len() as u32);
            }
        }
        progress(out.len() as u32);
        Ok(out)
    }

    fn resolve_link(
        &self,
        folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        let path = folder
            .join(&entry.name)
            .map_err(|_| VfsError::InvalidLocation {
                input: folder.display(),
            })?;
        let git = git_of(&path)?;
        let view = self.view(git, &path)?;
        let found = self.find(&view, git.inner(), &path)?;
        if found.kind != ObjectKind::Symlink {
            return Err(not_found(&path));
        }
        self.entry_in(&view, git.inner(), &found, &path, true)
    }

    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        self.open_read_at(path, 0)
    }

    fn open_read_at(&self, path: &VfsPath, start: u64) -> Result<ReadStream, VfsError> {
        let git = git_of(path)?;
        let view = self.view(git, path)?;
        let found = self.find(&view, git.inner(), path)?;
        match found.kind {
            ObjectKind::Tree => Err(VfsError::IsADirectory {
                location: path.to_location(),
            }),
            ObjectKind::Submodule => Err(unsupported("reading a submodule's commit as a file")),
            // A link is read as the file it points at (as a file manager opens it); a link that
            // cannot be followed in the tree reads as its own text.
            ObjectKind::Symlink => {
                let target = self
                    .read_link(path)
                    .ok()
                    .and_then(|text| {
                        let parent = git.inner()[..git.inner().len().saturating_sub(1)].to_vec();
                        resolve_in_tree(&parent, &os_bytes(&text))
                    })
                    .and_then(|parts| self.find(&view, &parts, path).ok())
                    .filter(|found| {
                        matches!(found.kind, ObjectKind::Blob | ObjectKind::Executable)
                    });
                let id = target.map(|found| found.id).unwrap_or(found.id);
                self.stream(&view, id, start, path)
            }
            _ => self.stream(&view, found.id, start, path),
        }
    }

    fn read_link(&self, path: &VfsPath) -> Result<OsString, VfsError> {
        let git = git_of(path)?;
        let view = self.view(git, path)?;
        let found = self.find(&view, git.inner(), path)?;
        if found.kind != ObjectKind::Symlink {
            return Err(VfsError::InvalidLocation {
                input: path.display(),
            });
        }
        Ok(os_string(&self.blob(&view, found.id, path)?))
    }
}

impl GitProvider {
    fn stream(
        &self,
        view: &View,
        id: gix::ObjectId,
        start: u64,
        path: &VfsPath,
    ) -> Result<ReadStream, VfsError> {
        let data = self.blob(view, id, path)?;
        let mut cursor = Cursor::new(data);
        cursor.set_position(start.min(cursor.get_ref().len() as u64));
        Ok(Box::new(cursor))
    }
}
