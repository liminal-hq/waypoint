// A repository's summary as the window reads it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_protocol::{GitHeadKind, GitOperation, GitSummary};
use waypoint_provider_git::{HeadState, InProgress, RepoSummary};

fn operation(op: InProgress) -> GitOperation {
    match op {
        InProgress::Merge => GitOperation::Merge,
        InProgress::Rebase => GitOperation::Rebase,
        InProgress::CherryPick => GitOperation::CherryPick,
        InProgress::Revert => GitOperation::Revert,
        InProgress::Bisect => GitOperation::Bisect,
    }
}

/// The wire form of a summary.
pub fn wire(summary: &RepoSummary) -> GitSummary {
    let (head_kind, head) = match &summary.head {
        HeadState::Branch(name) => (GitHeadKind::Branch, name.clone()),
        HeadState::Detached(short) => (GitHeadKind::Detached, short.clone()),
        HeadState::Unborn(name) => (GitHeadKind::Unborn, name.clone()),
    };
    let distance = summary.ahead_behind;
    GitSummary {
        head_kind,
        head,
        commit: summary.commit.clone(),
        upstream: summary.upstream.clone(),
        ahead: distance.map(|d| d.ahead),
        behind: distance.map(|d| d.behind),
        distance_capped: distance.is_some_and(|d| d.capped),
        staged: summary.counts.staged,
        unstaged: summary.counts.unstaged,
        untracked: summary.counts.untracked,
        conflicted: summary.counts.conflicted,
        operation: summary.in_progress.map(operation),
    }
}
