// The commits that touched a path and the diff stat since `HEAD`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use support::*;
use waypoint_provider_git::{compute, diff_stat, history, StatusOptions};
use waypoint_vfs::CancelToken;

fn log(repo: &Repo, rel: &str, limit: usize) -> Vec<String> {
    history(repo.path(), rel.as_bytes(), limit, &CancelToken::new())
        .unwrap()
        .commits
        .into_iter()
        .map(|c| c.summary)
        .collect()
}

#[test]
fn the_commits_that_changed_a_file_come_newest_first() {
    let Some(repo) = repo() else { return };
    repo.write("a.txt", "1");
    repo.write("b.txt", "1");
    repo.commit_at("add both", "2026-01-01T00:00:00Z");
    repo.write("a.txt", "2");
    repo.commit_at("change a", "2026-01-02T00:00:00Z");
    repo.write("b.txt", "2");
    repo.commit_at("change b", "2026-01-03T00:00:00Z");
    repo.write("a.txt", "3");
    repo.commit_at("change a again", "2026-01-04T00:00:00Z");
    assert_eq!(
        log(&repo, "a.txt", 10),
        ["change a again", "change a", "add both"]
    );
    assert_eq!(log(&repo, "b.txt", 10), ["change b", "add both"]);
    assert_eq!(log(&repo, "a.txt", 2), ["change a again", "change a"]);
    assert_eq!(
        log(&repo, "", 3),
        ["change a again", "change b", "change a"]
    );
    assert!(log(&repo, "missing", 5).is_empty());
}

#[test]
fn a_folder_is_touched_by_any_change_inside_it_and_a_commit_has_its_details() {
    let Some(repo) = repo() else { return };
    repo.write("src/lib.rs", "1");
    repo.write("top.txt", "1");
    repo.commit_at("first", "2026-01-01T00:00:00Z");
    repo.write("top.txt", "2");
    repo.commit_at("top only", "2026-01-02T00:00:00Z");
    repo.write("src/deep/x.rs", "x");
    let id = repo.commit_at("deep file\n\nwith a body", "2026-01-03T00:00:00Z");
    assert_eq!(log(&repo, "src", 10), ["deep file", "first"]);
    let found = history(repo.path(), b"src", 1, &CancelToken::new()).unwrap();
    let commit = &found.commits[0];
    assert_eq!(commit.id, id);
    assert_eq!(commit.author, "Ada Tester");
    assert_eq!(commit.time_ms, 1_767_398_400_000);
    assert!(!found.truncated);
}

#[test]
fn a_repository_with_no_commits_has_no_history() {
    let Some(repo) = repo() else { return };
    assert!(log(&repo, "", 5).is_empty());
}

#[test]
fn a_cancelled_search_says_so() {
    let Some(repo) = repo() else { return };
    repo.write("a", "1");
    repo.commit_all("one");
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        history(repo.path(), b"a", 5, &cancel),
        Err(waypoint_provider_git::StatusError::Cancelled)
    ));
}

fn stat(repo: &Repo, rel: &str) -> waypoint_provider_git::DiffStat {
    let status = compute(
        repo.path(),
        &StatusOptions::default(),
        &CancelToken::new(),
        None,
    )
    .unwrap();
    diff_stat(repo.path(), &status, rel.as_bytes(), &CancelToken::new()).unwrap()
}

#[test]
fn the_diff_stat_counts_lines_like_git_diff_stat() {
    let Some(repo) = repo() else { return };
    repo.write("a.txt", "one\ntwo\nthree\n");
    repo.write("gone.txt", "x\ny\n");
    repo.write("dir/b.txt", "1\n2\n");
    repo.commit_all("base");
    repo.write("a.txt", "one\nTWO\nthree\nfour\n");
    repo.remove("gone.txt");
    repo.write("dir/b.txt", "1\n2\n3\n");
    repo.write("new.txt", "n1\nn2\n");
    // Git's own answer for the same tree.
    let numstat = repo.git(&["diff", "HEAD", "--numstat"]);
    let (mut added, mut removed) = (0u32, 0u32);
    for line in numstat.lines() {
        let mut parts = line.split_whitespace();
        added += parts.next().unwrap().parse::<u32>().unwrap();
        removed += parts.next().unwrap().parse::<u32>().unwrap();
    }
    let all = stat(&repo, "");
    // `new.txt` is untracked, so git diff HEAD does not see it; this counts it as added lines.
    assert_eq!((all.added, all.removed), (added + 2, removed));
    assert_eq!(all.files, 4);
    let dir = stat(&repo, "dir");
    assert_eq!((dir.files, dir.added, dir.removed), (1, 1, 0));
    let one = stat(&repo, "a.txt");
    assert_eq!((one.files, one.added, one.removed), (1, 2, 1));
    let clean = stat(&repo, "missing");
    assert_eq!(clean, Default::default());
}

#[test]
fn binary_files_are_counted_but_not_in_lines() {
    let Some(repo) = repo() else { return };
    std::fs::write(repo.path().join("pic.bin"), [0u8, 1, 2, 3]).unwrap();
    repo.commit_all("bin");
    std::fs::write(repo.path().join("pic.bin"), [0u8, 9, 9, 9, 9]).unwrap();
    let s = stat(&repo, "");
    assert_eq!((s.files, s.binary, s.added, s.removed), (1, 1, 0, 0));
}
