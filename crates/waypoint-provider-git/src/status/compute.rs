// Computing the status of a working tree, once, with gitoxide.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::path::Path;

use gix::bstr::{BString, ByteSlice};
use gix::status::{Item, UntrackedFiles};
use waypoint_vfs::CancelToken;

use super::model::{Change, EntryStatus, RepoStatus};

/// Why a status could not be computed.
#[derive(Debug, thiserror::Error)]
pub enum StatusError {
    /// The folder is not a working tree (not a repository, or a bare one).
    #[error("this folder is not a Git working tree")]
    NotARepository,
    /// Another status was asked for, or the tracking stopped.
    #[error("the status was cancelled")]
    Cancelled,
    /// Git could not be read: a damaged index, a broken configuration, an unreadable file.
    #[error("Git could not be read: {0}")]
    Failed(String),
}

/// How untracked files are reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UntrackedMode {
    /// Not at all (`status.showUntrackedFiles=no`): the cheapest, as no folder is walked beyond
    /// what the index lists.
    Hidden,
    /// A folder with nothing tracked is one entry, as `git status` shows it.
    #[default]
    Collapsed,
    /// Every untracked file on its own.
    Files,
}

/// What to compute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusOptions {
    pub untracked: UntrackedMode,
    /// Report ignored paths (as `!` and dimmed rows). Ignored folders are one entry each, and the
    /// walk does not go into them either way, so this costs a little, not a walk of `target/`.
    pub ignored: bool,
    /// Detect a staged rename (a file moved and added in the same change) rather than showing a
    /// delete and an add. Costs a similarity pass over the staged adds, so it is the first thing a
    /// huge change turns off.
    pub renames: bool,
}

impl Default for StatusOptions {
    fn default() -> Self {
        Self {
            untracked: UntrackedMode::Collapsed,
            ignored: true,
            renames: true,
        }
    }
}

fn failed(error: impl std::fmt::Display) -> StatusError {
    StatusError::Failed(error.to_string())
}

/// A pathspec that matches `rel` literally (so a name with `*` or `[` is itself) and everything
/// under it.
fn literal(rel: &[u8]) -> BString {
    let mut spec = BString::from(":(literal)");
    spec.extend_from_slice(rel);
    spec
}

fn staged_change(change: &gix::diff::index::Change) -> Option<(BString, Change)> {
    use gix::diff::index::Change as C;
    Some(match change {
        C::Addition { location, .. } => (location.clone().into_owned(), Change::Added),
        C::Deletion { location, .. } => (location.clone().into_owned(), Change::Deleted),
        C::Modification {
            location,
            previous_entry_mode,
            entry_mode,
            ..
        } => (
            location.clone().into_owned(),
            if (*previous_entry_mode == gix::index::entry::Mode::SYMLINK)
                != (*entry_mode == gix::index::entry::Mode::SYMLINK)
            {
                Change::TypeChanged
            } else {
                Change::Modified
            },
        ),
        C::Rewrite { location, copy, .. } => (
            location.clone().into_owned(),
            if *copy { Change::Added } else { Change::Renamed },
        ),
    })
}

fn worktree_change(item: &gix::status::index_worktree::Item) -> Option<(BString, Change)> {
    use gix::status::index_worktree::Item as I;
    use gix::status::plumbing::index_as_worktree::{Change as W, EntryStatus as S};
    match item {
        I::Modification {
            rela_path, status, ..
        } => {
            let change = match status {
                S::Conflict { .. } => Change::Conflicted,
                S::Change(W::Removed) => Change::Deleted,
                S::Change(W::Type { .. }) => Change::TypeChanged,
                S::Change(W::Modification { .. } | W::SubmoduleModification(_)) => {
                    Change::Modified
                }
                S::NeedsUpdate(_) => return None,
                S::IntentToAdd => Change::Added,
            };
            Some((rela_path.clone(), change))
        }
        I::DirectoryContents { entry, .. } => match entry.status {
            gix::dir::entry::Status::Untracked => {
                Some((entry.rela_path.clone(), Change::Untracked))
            }
            gix::dir::entry::Status::Ignored(_) => Some((entry.rela_path.clone(), Change::Ignored)),
            _ => None,
        },
        I::Rewrite { dirwalk_entry, .. } => {
            Some((dirwalk_entry.rela_path.clone(), Change::Renamed))
        }
    }
}

/// Computes the status of the working tree at `root`, or of only the paths in `scope` (relative,
/// `/`-separated) when it is given. Never writes: not the index (the refreshed stat data `git
/// status` would store is dropped), not a lock. `cancel` stops it between files.
pub fn compute(
    root: &Path,
    options: &StatusOptions,
    cancel: &CancelToken,
    scope: Option<&[Vec<u8>]>,
) -> Result<RepoStatus, StatusError> {
    if cancel.is_cancelled() {
        return Err(StatusError::Cancelled);
    }
    let repo = crate::repository::open(root).map_err(StatusError::Failed)?;
    if repo.workdir().is_none() {
        return Err(StatusError::NotARepository);
    }
    let untracked = match options.untracked {
        UntrackedMode::Hidden => UntrackedFiles::None,
        UntrackedMode::Collapsed => UntrackedFiles::Collapsed,
        UntrackedMode::Files => UntrackedFiles::Files,
    };
    let rename_mode = if options.renames {
        gix::status::tree_index::TrackRenames::AsConfigured
    } else {
        gix::status::tree_index::TrackRenames::Disabled
    };
    let ignored = options.ignored;
    let platform = repo
        .status(gix::progress::Discard)
        .map_err(failed)?
        .untracked_files(untracked)
        .dirwalk_options(|walk| {
            walk.emit_ignored(
                ignored.then_some(gix::dir::walk::EmissionMode::CollapseDirectory),
            )
        })
        .index_worktree_rewrites(None)
        .tree_index_track_renames(rename_mode)
        .should_interrupt_owned(cancel.flag());
    let patterns: Vec<BString> = scope
        .map(|paths| paths.iter().map(|rel| literal(rel)).collect())
        .unwrap_or_default();
    let iter = platform.into_iter(patterns).map_err(failed)?;

    let mut entries: BTreeMap<Vec<u8>, EntryStatus> = BTreeMap::new();
    for item in iter {
        if cancel.is_cancelled() {
            return Err(StatusError::Cancelled);
        }
        let item = item.map_err(failed)?;
        match &item {
            Item::TreeIndex(change) => {
                if let Some((path, change)) = staged_change(change) {
                    entries
                        .entry(path.as_bytes().to_vec())
                        .or_default()
                        .staged = Some(change);
                }
            }
            Item::IndexWorktree(item) => {
                if let Some((path, change)) = worktree_change(item) {
                    let mut path = path.as_bytes().to_vec();
                    // A folder is reported with the separator it ends in on some platforms.
                    while path.last() == Some(&b'/') {
                        path.pop();
                    }
                    entries.entry(path).or_default().unstaged = Some(change);
                }
            }
        }
    }
    if cancel.is_cancelled() {
        return Err(StatusError::Cancelled);
    }
    // A conflicted path has no staged side: it is unmerged, not staged.
    for status in entries.values_mut() {
        if status.unstaged == Some(Change::Conflicted) {
            status.staged = None;
        }
    }
    Ok(RepoStatus::new(root.to_path_buf(), entries))
}
