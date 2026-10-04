// The one-line state of a repository: which branch, how far from its upstream, whether it is dirty.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;

use super::compute::StatusError;
use super::model::{RepoStatus, StatusCounts};

/// How many commits ahead or behind are counted before the count stops (and is shown as "at
/// least"), so a branch a million commits from its upstream does not stall the status bar.
pub const AHEAD_BEHIND_CAP: usize = 10_000;

/// What `HEAD` is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadState {
    /// On a branch (`main`).
    Branch(String),
    /// On a commit that no branch names; the abbreviated id.
    Detached(String),
    /// On a branch with no commits yet.
    Unborn(String),
}

impl HeadState {
    /// What the status bar calls it: the branch, or the short commit.
    pub fn label(&self) -> &str {
        match self {
            HeadState::Branch(name) | HeadState::Unborn(name) | HeadState::Detached(name) => name,
        }
    }
}

/// An operation Git is part-way through, which the status bar says so the person is not surprised
/// by a conflict or a detached head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InProgress {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect,
}

/// The distance from the upstream branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AheadBehind {
    pub ahead: u32,
    pub behind: u32,
    /// A count stopped at `AHEAD_BEHIND_CAP`: the real number is at least this.
    pub capped: bool,
}

/// Everything the branch item and the Inspector's header show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSummary {
    pub head: HeadState,
    /// The commit `HEAD` is at, in full; `None` before the first commit.
    pub commit: Option<String>,
    /// The upstream branch (`origin/main`), when the branch tracks one that exists.
    pub upstream: Option<String>,
    pub ahead_behind: Option<AheadBehind>,
    pub counts: StatusCounts,
    pub in_progress: Option<InProgress>,
}

impl RepoSummary {
    pub fn is_dirty(&self) -> bool {
        self.counts.is_dirty()
    }
}

fn failed(error: impl std::fmt::Display) -> StatusError {
    StatusError::Failed(error.to_string())
}

fn in_progress(git_dir: &Path) -> Option<InProgress> {
    let has = |name: &str| git_dir.join(name).exists();
    if has("rebase-merge") || has("rebase-apply") {
        Some(InProgress::Rebase)
    } else if has("MERGE_HEAD") {
        Some(InProgress::Merge)
    } else if has("CHERRY_PICK_HEAD") {
        Some(InProgress::CherryPick)
    } else if has("REVERT_HEAD") {
        Some(InProgress::Revert)
    } else if has("BISECT_LOG") {
        Some(InProgress::Bisect)
    } else {
        None
    }
}

fn count_from(
    repo: &gix::Repository,
    from: gix::ObjectId,
    hidden: gix::ObjectId,
) -> Option<(u32, bool)> {
    let walk = repo.rev_walk([from]).with_hidden([hidden]).all().ok()?;
    let mut count = 0usize;
    for info in walk {
        info.ok()?;
        count += 1;
        if count >= AHEAD_BEHIND_CAP {
            return Some((count as u32, true));
        }
    }
    Some((count as u32, false))
}

/// Reads `HEAD`, its upstream and the operation in progress, and joins them to the counts of
/// `status`.
pub fn summarize(root: &Path, status: &RepoStatus) -> Result<RepoSummary, StatusError> {
    let repo = crate::repository::open(root).map_err(StatusError::Failed)?;
    let head = repo.head().map_err(failed)?;
    let commit = head.id().map(|id| id.to_string());
    let state = if head.is_unborn() {
        HeadState::Unborn(
            head.referent_name()
                .map(|name| name.shorten().to_string())
                .unwrap_or_else(|| "HEAD".to_owned()),
        )
    } else if let Some(name) = head.referent_name() {
        HeadState::Branch(name.shorten().to_string())
    } else {
        let short = commit
            .as_deref()
            .map(|id| id[..id.len().min(8)].to_owned())
            .unwrap_or_default();
        HeadState::Detached(short)
    };

    let mut upstream = None;
    let mut ahead_behind = None;
    if let (Some(name), Some(head_id)) = (head.referent_name(), head.id()) {
        let tracking = repo
            .branch_remote_tracking_ref_name(name, gix::remote::Direction::Fetch)
            .and_then(Result::ok);
        if let Some(tracking) = tracking {
            if let Ok(mut reference) = repo.find_reference(tracking.as_ref()) {
                if let Ok(up_id) = reference.peel_to_id() {
                    upstream = Some(tracking.shorten().to_string());
                    let head_id = head_id.detach();
                    let up_id = up_id.detach();
                    if head_id != up_id {
                        let ahead = count_from(&repo, head_id, up_id);
                        let behind = count_from(&repo, up_id, head_id);
                        if let (Some((ahead, a_cap)), Some((behind, b_cap))) = (ahead, behind) {
                            ahead_behind = Some(AheadBehind {
                                ahead,
                                behind,
                                capped: a_cap || b_cap,
                            });
                        }
                    } else {
                        ahead_behind = Some(AheadBehind {
                            ahead: 0,
                            behind: 0,
                            capped: false,
                        });
                    }
                }
            }
        }
    }
    Ok(RepoSummary {
        head: state,
        commit,
        upstream,
        ahead_behind,
        counts: status.counts(),
        in_progress: crate::repository::git_dir_of(root)
            .as_deref()
            .and_then(in_progress),
    })
}
