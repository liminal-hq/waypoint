// What happened to a path: the commits that changed it, and what has changed in it since `HEAD`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;

use gix::bstr::ByteSlice;
use gix::diff::blob::{sources::byte_lines, Algorithm, Diff, InternedInput};
use waypoint_vfs::CancelToken;

use crate::names::os_string;
use crate::status::{RepoStatus, StatusError};

/// The most commits looked at to find the ones that touched a path: a path nothing has changed
/// for longer than this is reported as "older than what was searched".
pub const SCAN_CAP: usize = 5_000;

/// The most changed files a diff stat reads, and the largest file it reads (bigger is counted as
/// not read, and the stat says it is partial).
pub const DIFF_FILE_CAP: usize = 500;
pub const DIFF_FILE_BYTES: u64 = 1024 * 1024;

/// One commit, as the Inspector lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    /// The full id in hexadecimal.
    pub id: String,
    /// The first line of the message.
    pub summary: String,
    pub author: String,
    /// When it was committed, in milliseconds since the Unix epoch.
    pub time_ms: i64,
}

/// The newest commits that changed a path.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct History {
    pub commits: Vec<CommitInfo>,
    /// The search stopped at `SCAN_CAP` commits before it found `limit`, so older ones may exist.
    pub truncated: bool,
}

fn failed(error: impl std::fmt::Display) -> StatusError {
    StatusError::Failed(error.to_string())
}

fn components(rel: &[u8]) -> Vec<&[u8]> {
    rel.split(|&b| b == b'/')
        .filter(|p| !p.is_empty())
        .collect()
}

/// The id of what `parts` names in a commit's tree, if anything.
fn entry_at(
    repo: &gix::Repository,
    commit: &gix::Commit<'_>,
    parts: &[&[u8]],
) -> Result<Option<gix::ObjectId>, StatusError> {
    let tree = commit.tree().map_err(failed)?;
    if parts.is_empty() {
        return Ok(Some(tree.id));
    }
    let _ = repo;
    Ok(tree
        .lookup_entry(parts.iter().copied())
        .map_err(failed)?
        .map(|entry| entry.object_id()))
}

/// The newest `limit` commits that changed the file or folder at `rel` (relative to the working
/// folder, `/`-separated; empty for the whole repository): a commit counts when what `rel` names
/// differs from what it was in the commit's first parent. A repository with no commits has none.
pub fn history(
    root: &Path,
    rel: &[u8],
    limit: usize,
    cancel: &CancelToken,
) -> Result<History, StatusError> {
    let repo = crate::repository::open(root).map_err(StatusError::Failed)?;
    let Ok(head) = repo.head_id() else {
        return Ok(History::default());
    };
    let parts = components(rel);
    let walk = repo
        .rev_walk([head.detach()])
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            Default::default(),
        ))
        .all()
        .map_err(failed)?;
    let mut out = History::default();
    for (seen, info) in walk.enumerate() {
        if cancel.is_cancelled() {
            return Err(StatusError::Cancelled);
        }
        if seen >= SCAN_CAP {
            out.truncated = true;
            break;
        }
        let info = info.map_err(failed)?;
        let commit = repo.find_commit(info.id).map_err(failed)?;
        let touched = if parts.is_empty() {
            true
        } else {
            let now = entry_at(&repo, &commit, &parts)?;
            match commit.parent_ids().next() {
                None => now.is_some(),
                Some(parent) => {
                    let parent = repo.find_commit(parent).map_err(failed)?;
                    now != entry_at(&repo, &parent, &parts)?
                }
            }
        };
        if !touched {
            continue;
        }
        let message = commit.message_raw_sloppy();
        let summary = message
            .lines()
            .next()
            .map(|line| line.to_str_lossy().trim().to_owned())
            .unwrap_or_default();
        let author = commit
            .author()
            .map(|a| a.name.to_str_lossy().into_owned())
            .unwrap_or_default();
        out.commits.push(CommitInfo {
            id: commit.id.to_string(),
            summary,
            author,
            time_ms: commit.time().map(|t| t.seconds * 1000).unwrap_or(0),
        });
        if out.commits.len() >= limit {
            return Ok(out);
        }
    }
    Ok(out)
}

/// How much changed under a path since `HEAD`, as `git diff --stat HEAD` counts it: lines added and
/// removed over the files that differ, working tree against `HEAD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiffStat {
    pub files: u32,
    pub added: u32,
    pub removed: u32,
    /// Files with content that is not text (counted in `files`, not in the lines).
    pub binary: u32,
    /// Some files were not read: more than `DIFF_FILE_CAP`, or larger than `DIFF_FILE_BYTES`.
    pub partial: bool,
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(8000)].contains(&0)
}

/// Counts the changes under `rel` (empty for the whole repository) from `status`, which says
/// which files differ; untracked folders and ignored paths are not counted.
pub fn diff_stat(
    root: &Path,
    status: &RepoStatus,
    rel: &[u8],
    cancel: &CancelToken,
) -> Result<DiffStat, StatusError> {
    let repo = crate::repository::open(root).map_err(StatusError::Failed)?;
    let head_tree = repo
        .head_commit()
        .ok()
        .and_then(|commit| commit.tree().ok());
    let mut stat = DiffStat::default();
    for (path, entry) in status.iter() {
        let under = rel.is_empty()
            || path == rel
            || (path.len() > rel.len() && path.starts_with(rel) && path[rel.len()] == b'/');
        let changed = entry.is_changed() && !entry.is_conflicted();
        if !under || !changed {
            continue;
        }
        if cancel.is_cancelled() {
            return Err(StatusError::Cancelled);
        }
        let on_disk = root.join(os_string_path(path));
        let metadata = std::fs::symlink_metadata(&on_disk).ok();
        // A folder (an untracked one is a single entry) has no lines of its own to count.
        if metadata.as_ref().is_some_and(|m| m.is_dir()) {
            continue;
        }
        if stat.files as usize >= DIFF_FILE_CAP {
            stat.partial = true;
            break;
        }
        let new = match &metadata {
            Some(m) if m.is_file() => {
                if m.len() > DIFF_FILE_BYTES {
                    stat.files += 1;
                    stat.partial = true;
                    continue;
                }
                std::fs::read(&on_disk).ok()
            }
            _ => None,
        };
        let old = match &head_tree {
            Some(tree) => tree
                .lookup_entry(components(path))
                .map_err(failed)?
                .filter(|entry| entry.mode().is_blob())
                .and_then(|entry| repo.find_object(entry.object_id()).ok())
                .map(|object| object.detach().data),
            None => None,
        };
        if old.is_none() && new.is_none() {
            continue;
        }
        let (old, new) = (old.unwrap_or_default(), new.unwrap_or_default());
        if old.len() as u64 > DIFF_FILE_BYTES {
            stat.files += 1;
            stat.partial = true;
            continue;
        }
        stat.files += 1;
        if is_binary(&old) || is_binary(&new) {
            stat.binary += 1;
            continue;
        }
        let input = InternedInput::new(byte_lines(&old), byte_lines(&new));
        let diff = Diff::compute(Algorithm::Histogram, &input);
        stat.added += diff.count_additions();
        stat.removed += diff.count_removals();
    }
    Ok(stat)
}

fn os_string_path(rel: &[u8]) -> std::path::PathBuf {
    components(rel).into_iter().map(os_string).collect()
}
