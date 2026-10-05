// The branch, upstream distance and state of a repository.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use support::*;
use waypoint_provider_git::{
    compute, summarize, HeadState, InProgress, RepoSummary, StatusOptions,
};
use waypoint_vfs::CancelToken;

fn summary(repo: &Repo) -> RepoSummary {
    let status = compute(
        repo.path(),
        &StatusOptions::default(),
        &CancelToken::new(),
        None,
    )
    .unwrap();
    summarize(repo.path(), &status).unwrap()
}

#[test]
fn a_branch_with_commits_reports_its_name_and_commit() {
    let Some(repo) = repo() else { return };
    repo.write("a", "a");
    let head = repo.commit_all("one");
    let s = summary(&repo);
    assert_eq!(s.head, HeadState::Branch("main".into()));
    assert_eq!(s.head.label(), "main");
    assert_eq!(s.commit.as_deref(), Some(head.as_str()));
    assert_eq!(s.upstream, None);
    assert_eq!(s.ahead_behind, None);
    assert!(!s.is_dirty());
    assert_eq!(s.in_progress, None);
}

#[test]
fn a_dirty_tree_says_so_with_counts() {
    let Some(repo) = repo() else { return };
    repo.write("a", "a");
    repo.commit_all("one");
    repo.write("a", "changed");
    repo.write("new", "n");
    let s = summary(&repo);
    assert!(s.is_dirty());
    assert_eq!(s.counts.unstaged, 1);
    assert_eq!(s.counts.untracked, 1);
}

#[test]
fn a_repository_with_no_commits_is_on_an_unborn_branch() {
    let Some(repo) = repo() else { return };
    let s = summary(&repo);
    assert_eq!(s.head, HeadState::Unborn("main".into()));
    assert_eq!(s.commit, None);
}

#[test]
fn a_detached_head_shows_the_short_commit() {
    let Some(repo) = repo() else { return };
    repo.write("a", "a");
    let first = repo.commit_all("one");
    repo.write("a", "b");
    repo.commit_all("two");
    repo.git(&["checkout", "-q", "--detach", &first]);
    let s = summary(&repo);
    assert_eq!(s.head, HeadState::Detached(first[..8].to_owned()));
    assert_eq!(s.commit.as_deref(), Some(first.as_str()));
}

#[test]
fn ahead_and_behind_count_the_commits_from_the_upstream() {
    let Some(upstream) = repo() else { return };
    upstream.write("a", "a");
    upstream.commit_all("one");
    let Some(clone) = repo() else { return };
    // A clone made by hand, so the test needs nothing but `git`.
    clone.git(&["remote", "add", "origin", upstream.path().to_str().unwrap()]);
    clone.git(&["fetch", "-q", "origin"]);
    clone.git(&["checkout", "-q", "-B", "main", "origin/main"]);
    clone.git(&["branch", "--set-upstream-to=origin/main", "main"]);
    let s = summary(&clone);
    assert_eq!(s.upstream.as_deref(), Some("origin/main"));
    let level = s.ahead_behind.unwrap();
    assert_eq!((level.ahead, level.behind, level.capped), (0, 0, false));

    clone.write("local", "l");
    clone.commit_all("local one");
    clone.write("local", "ll");
    clone.commit_all("local two");
    upstream.write("a", "a2");
    upstream.commit_all("remote one");
    upstream.write("a", "a3");
    upstream.commit_all("remote two");
    upstream.write("a", "a4");
    upstream.commit_all("remote three");
    clone.git(&["fetch", "-q", "origin"]);
    let s = summary(&clone);
    let distance = s.ahead_behind.unwrap();
    assert_eq!((distance.ahead, distance.behind), (2, 3));
}

#[test]
fn a_merge_in_progress_is_named() {
    let Some(repo) = repo() else { return };
    repo.write("f", "base\n");
    repo.commit_all("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("f", "other\n");
    repo.commit_all("other");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("f", "main\n");
    repo.commit_all("main");
    let _ = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["merge", "other"])
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_AUTHOR_NAME", "a")
        .env("GIT_AUTHOR_EMAIL", "a@example.test")
        .env("GIT_COMMITTER_NAME", "a")
        .env("GIT_COMMITTER_EMAIL", "a@example.test")
        .output()
        .unwrap();
    let s = summary(&repo);
    assert_eq!(s.in_progress, Some(InProgress::Merge));
    assert_eq!(s.counts.conflicted, 1);
}
