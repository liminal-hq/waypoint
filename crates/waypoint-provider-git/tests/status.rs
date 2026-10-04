// The status of a working tree, checked against what `git status` says about the same files.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::collections::BTreeMap;

use support::*;
use waypoint_provider_git::{compute, Change, EntryStatus, RepoStatus, StatusOptions, UntrackedMode};
use waypoint_vfs::CancelToken;

fn status_of(repo: &Repo, options: &StatusOptions) -> RepoStatus {
    compute(repo.path(), options, &CancelToken::new(), None).unwrap()
}

/// `git status --porcelain` as path to the two letters, ignored included, untracked folders
/// collapsed.
fn oracle(repo: &Repo, untracked: &str) -> BTreeMap<String, String> {
    // Not `Repo::git`, which trims the output and so the first entry's leading space.
    let out = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["-c", "core.fsmonitor=false", "status", "--porcelain=v1", "-z"])
        .args(["--ignored=traditional", &format!("-u{untracked}")])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", if cfg!(windows) { "NUL" } else { "/dev/null" })
        .output()
        .unwrap();
    let out = String::from_utf8(out.stdout).unwrap();
    let mut map = BTreeMap::new();
    let mut parts = out.split('\0').filter(|p| !p.is_empty());
    while let Some(line) = parts.next() {
        let (xy, path) = line.split_at(2);
        let path = path.trim_start().trim_end_matches('/').to_owned();
        if xy.starts_with('R') || xy.starts_with('C') {
            parts.next(); // the source of the rename
        }
        map.insert(path, xy.to_owned());
    }
    map
}

fn ours(status: &RepoStatus) -> BTreeMap<String, String> {
    status
        .iter()
        .map(|(path, entry)| {
            (
                String::from_utf8_lossy(path).into_owned(),
                entry.short().iter().collect::<String>(),
            )
        })
        .collect()
}

fn busy_repo() -> Option<Repo> {
    let repo = repo()?;
    repo.write(".gitignore", "*.log\nout/\n");
    for name in ["a", "b", "c", "d", "e", "g_staged_later", "dir/in", "move_me"] {
        repo.write(name, &format!("{name}\n"));
    }
    repo.commit_all("base");
    repo.write("a", "a changed\n");
    repo.write("b", "b changed\n");
    repo.git(&["add", "b"]);
    repo.write("c", "c staged\n");
    repo.git(&["add", "c"]);
    repo.write("c", "c staged and more\n");
    repo.remove("d");
    repo.git(&["rm", "-q", "e"]);
    repo.write("f_added", "new\n");
    repo.git(&["add", "f_added"]);
    repo.write("untracked.txt", "u\n");
    repo.write("new_dir/one.txt", "1\n");
    repo.write("new_dir/two.txt", "2\n");
    repo.write("noise.log", "ignored\n");
    repo.write("out/build.o", "ignored dir\n");
    repo.git(&["mv", "move_me", "moved"]);
    Some(repo)
}

#[test]
fn every_state_matches_git_status() {
    let Some(repo) = busy_repo() else { return };
    let status = status_of(&repo, &StatusOptions::default());
    assert_eq!(ours(&status), oracle(&repo, "normal"));
    // And the cases spelled out, so a change in Git's own output is not the only thing checked.
    let entry = |p: &str| *status.entry(p.as_bytes()).unwrap();
    assert_eq!(entry("a").unstaged, Some(Change::Modified));
    assert_eq!(entry("b").staged, Some(Change::Modified));
    assert_eq!(entry("b").unstaged, None);
    assert_eq!(entry("c").staged, Some(Change::Modified));
    assert_eq!(entry("c").unstaged, Some(Change::Modified));
    assert_eq!(entry("d").unstaged, Some(Change::Deleted));
    assert_eq!(entry("e").staged, Some(Change::Deleted));
    assert_eq!(entry("f_added").staged, Some(Change::Added));
    assert_eq!(entry("untracked.txt"), EntryStatus::untracked());
    assert_eq!(entry("new_dir"), EntryStatus::untracked());
    assert_eq!(entry("noise.log"), EntryStatus::ignored());
    assert_eq!(entry("out"), EntryStatus::ignored());
    assert_eq!(entry("moved").staged, Some(Change::Renamed));
    assert_eq!(status.entry(b"clean"), None);
}

#[test]
fn untracked_files_can_be_listed_one_by_one_or_left_out() {
    let Some(repo) = busy_repo() else { return };
    let files = status_of(
        &repo,
        &StatusOptions {
            untracked: UntrackedMode::Files,
            ..StatusOptions::default()
        },
    );
    assert!(files.entry(b"new_dir/one.txt").is_some());
    assert!(files.entry(b"new_dir").is_none());
    // Git lists the files inside an ignored folder under `-uall`; this lists the folder once.
    let not_ignored = |map: BTreeMap<String, String>| -> BTreeMap<String, String> {
        map.into_iter().filter(|(_, xy)| xy != "!!").collect()
    };
    assert_eq!(not_ignored(ours(&files)), not_ignored(oracle(&repo, "all")));
    let hidden = status_of(
        &repo,
        &StatusOptions {
            untracked: UntrackedMode::Hidden,
            ..StatusOptions::default()
        },
    );
    assert!(hidden.entry(b"untracked.txt").is_none());
    assert!(hidden.entry(b"a").is_some());
}

#[test]
fn ignored_paths_can_be_left_out() {
    let Some(repo) = busy_repo() else { return };
    let quiet = status_of(
        &repo,
        &StatusOptions {
            ignored: false,
            ..StatusOptions::default()
        },
    );
    assert!(quiet.entry(b"noise.log").is_none());
    assert!(quiet.entry(b"out").is_none());
    assert!(quiet.entry(b"untracked.txt").is_some());
}

#[test]
fn a_path_inside_an_untracked_or_ignored_folder_takes_its_status() {
    let Some(repo) = busy_repo() else { return };
    let status = status_of(&repo, &StatusOptions::default());
    assert_eq!(
        status.status_of(b"new_dir/one.txt"),
        Some(EntryStatus::untracked())
    );
    assert_eq!(
        status.status_of(b"out/deeper/x.o"),
        Some(EntryStatus::ignored())
    );
    assert_eq!(status.status_of(b"dir/in"), None);
}

#[test]
fn folders_get_a_badge_when_anything_inside_changed() {
    let Some(repo) = repo() else { return };
    repo.write("clean/x", "x");
    repo.write("dirty/deep/y", "y");
    repo.commit_all("base");
    repo.write("dirty/deep/y", "changed");
    let status = status_of(&repo, &StatusOptions::default());
    assert_eq!(status.badge(b"dirty/deep").unwrap().changed, 1);
    assert_eq!(status.badge(b"dirty").unwrap().changed, 1);
    assert_eq!(status.badge(b""), status.badge(b"dirty"));
    assert_eq!(status.badge(b"clean"), None);
}

#[test]
fn a_merge_conflict_is_conflicted_and_counted() {
    let Some(repo) = repo() else { return };
    repo.write("f", "base\n");
    repo.commit_all("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("f", "other\n");
    repo.commit_all("other");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("f", "main\n");
    repo.commit_all("main");
    let merge = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["merge", "other"])
        .env("GIT_CONFIG_GLOBAL", if cfg!(windows) { "NUL" } else { "/dev/null" })
        .env("GIT_AUTHOR_NAME", "a")
        .env("GIT_AUTHOR_EMAIL", "a@example.test")
        .env("GIT_COMMITTER_NAME", "a")
        .env("GIT_COMMITTER_EMAIL", "a@example.test")
        .output()
        .unwrap();
    assert!(!merge.status.success(), "the merge should conflict");
    let status = status_of(&repo, &StatusOptions::default());
    let entry = status.entry(b"f").unwrap();
    assert!(entry.is_conflicted());
    assert_eq!(entry.primary(), Some(Change::Conflicted));
    assert_eq!(status.counts().conflicted, 1);
    assert_eq!(status.badge(b"").unwrap().conflicted, 1);
    assert_eq!(ours(&status), oracle(&repo, "normal"));
}

#[test]
fn gitignore_the_exclude_file_and_the_configured_excludes_file_all_apply() {
    let Some(repo) = repo() else { return };
    repo.write(".gitignore", "from_gitignore\n");
    repo.write("kept", "k");
    repo.commit_all("base");
    std::fs::write(repo.path().join(".git/info/exclude"), "from_exclude\n").unwrap();
    let global = repo.path().join(".git/global-excludes");
    std::fs::write(&global, "from_global\n").unwrap();
    repo.git(&["config", "core.excludesFile", global.to_str().unwrap()]);
    for name in ["from_gitignore", "from_exclude", "from_global", "plain"] {
        repo.write(name, name);
    }
    let status = status_of(&repo, &StatusOptions::default());
    for ignored in ["from_gitignore", "from_exclude", "from_global"] {
        assert_eq!(
            status.entry(ignored.as_bytes()),
            Some(&EntryStatus::ignored()),
            "{ignored} is ignored"
        );
    }
    assert_eq!(status.entry(b"plain"), Some(&EntryStatus::untracked()));
    assert_eq!(ours(&status), oracle(&repo, "normal"));
}

#[test]
fn a_clean_tree_has_no_entries_and_an_empty_repository_works() {
    let Some(repo) = repo() else { return };
    assert!(status_of(&repo, &StatusOptions::default()).is_empty());
    repo.write("a", "a");
    repo.commit_all("a");
    assert!(status_of(&repo, &StatusOptions::default()).is_empty());
}

#[test]
fn a_scope_computes_only_what_is_under_it() {
    let Some(repo) = busy_repo() else { return };
    let scoped = compute(
        repo.path(),
        &StatusOptions::default(),
        &CancelToken::new(),
        Some(&[b"a".to_vec(), b"dir".to_vec()]),
    )
    .unwrap();
    assert_eq!(scoped.len(), 1);
    assert!(scoped.entry(b"a").is_some());
    // A name with glob characters is matched literally.
    repo.write("star*.txt", "s");
    let literal = compute(
        repo.path(),
        &StatusOptions::default(),
        &CancelToken::new(),
        Some(&[b"star*.txt".to_vec()]),
    )
    .unwrap();
    assert_eq!(literal.len(), 1);
}

#[test]
fn a_cancelled_status_says_so() {
    let Some(repo) = busy_repo() else { return };
    let cancel = CancelToken::new();
    cancel.cancel();
    let result = compute(repo.path(), &StatusOptions::default(), &cancel, None);
    assert!(matches!(
        result,
        Err(waypoint_provider_git::StatusError::Cancelled)
    ));
}

#[test]
fn a_folder_that_is_not_a_repository_is_an_error_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let result = compute(dir.path(), &StatusOptions::default(), &CancelToken::new(), None);
    assert!(result.is_err());
}
