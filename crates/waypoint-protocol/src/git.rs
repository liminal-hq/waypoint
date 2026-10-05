// What the Git plugin tells the window about a repository: where `HEAD` is, how far it is from its
// upstream, and how much is changed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Location;

/// What `HEAD` is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum GitHeadKind {
    /// On a branch.
    Branch,
    /// On a commit no branch names.
    Detached,
    /// On a branch with no commits yet.
    Unborn,
}

/// What Git is part-way through, which the status bar says so a conflict or a detached head is not
/// a surprise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum GitOperation {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect,
}

/// The state of one repository: what the branch item of the status bar and the Inspector's Git tab
/// show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct GitSummary {
    pub head_kind: GitHeadKind,
    /// The branch, or the abbreviated commit of a detached head.
    pub head: String,
    /// The commit `HEAD` is at, in full; none before the first commit.
    pub commit: Option<String>,
    /// The upstream branch (`origin/main`), when the branch tracks one that exists.
    pub upstream: Option<String>,
    /// Commits `HEAD` has that the upstream does not; none without an upstream.
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    /// Whether `ahead` or `behind` stopped counting at a cap: the real number is at least it.
    pub distance_capped: bool,
    /// How many paths have a change staged for the next commit.
    pub staged: u32,
    /// How many have a change that is not staged (untracked files are counted apart).
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicted: u32,
    pub operation: Option<GitOperation>,
}

impl GitSummary {
    /// Whether anything is the matter, by the rule `git status` uses for a clean tree.
    pub fn is_dirty(&self) -> bool {
        self.staged + self.unstaged + self.untracked + self.conflicted > 0
    }
}

/// The reply to watching a folder: the repository it is in, and its state so far.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct GitWatch {
    /// Names the watch for `git_unwatch` and in `GitChanged`.
    pub id: u32,
    /// The repository's working folder.
    pub root: Location,
    /// Its name, as the last folder of the path reads.
    pub name: String,
    /// The state, or none while the first status is still being computed (a `GitChanged` follows).
    pub summary: Option<GitSummary>,
    #[ts(type = "number")]
    pub revision: u64,
}

/// A repository's state changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct GitChanged {
    pub id: u32,
    #[ts(type = "number")]
    pub revision: u64,
    pub summary: GitSummary,
}

/// The status of every folder a window asked about, for the sidebar's badges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct GitBadge {
    /// The location asked about, as given.
    pub uri: String,
    /// How many changed paths are inside, at any depth; ignored files are not counted.
    pub changed: u32,
    pub conflicted: u32,
}

/// One commit, as the Inspector lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct GitCommit {
    /// The full id in hexadecimal.
    pub id: String,
    /// The abbreviated id.
    pub short: String,
    /// The first line of the message.
    pub summary: String,
    pub author: String,
    /// When it was committed, in milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub time_ms: i64,
}

/// How much changed under a path since `HEAD`, as `git diff --stat HEAD` counts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct GitDiffStat {
    pub files: u32,
    pub added: u32,
    pub removed: u32,
    /// Files whose content is not text: counted in `files`, not in the lines.
    pub binary: u32,
    /// Some files were not read (too many, or too large), so the counts are a floor.
    pub partial: bool,
}

/// What the Inspector's Git tab shows about one file or folder, read once the selection settles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct GitPathInfo {
    /// The newest commits that changed the path, newest first.
    pub commits: Vec<GitCommit>,
    /// The search stopped before it reached the oldest, so older ones may exist.
    pub truncated: bool,
    pub diff: GitDiffStat,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_summary_serialises_in_camel_case_and_knows_when_it_is_dirty() {
        let summary = GitSummary {
            head_kind: GitHeadKind::Branch,
            head: "main".into(),
            commit: Some("abc".into()),
            upstream: Some("origin/main".into()),
            ahead: Some(2),
            behind: Some(0),
            distance_capped: false,
            staged: 1,
            unstaged: 0,
            untracked: 0,
            conflicted: 0,
            operation: None,
        };
        assert!(summary.is_dirty());
        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["headKind"], "branch");
        assert_eq!(json["distanceCapped"], false);
        let clean = GitSummary {
            staged: 0,
            ..summary
        };
        assert!(!clean.is_dirty());
    }
}
