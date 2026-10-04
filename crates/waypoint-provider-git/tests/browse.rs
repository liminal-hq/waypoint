// Browsing a revision: the folders, files, links and submodules of a commit, read-only.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::io::Read;

use support::*;
use waypoint_provider_git::{GitProvider, ObjectKind};
use waypoint_vfs::conformance::{self, Subject};
use waypoint_vfs::{CancelToken, EntryKind, IconGroup, Provider, ScannedEntry};

fn list(provider: &GitProvider, path: &waypoint_path::VfsPath) -> Vec<ScannedEntry> {
    let mut entries = provider
        .list(path, &CancelToken::new(), usize::MAX, &mut |_| {})
        .unwrap();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn names(entries: &[ScannedEntry]) -> Vec<String> {
    entries
        .iter()
        .map(|e| e.name.to_string_lossy().into_owned())
        .collect()
}

fn read(provider: &GitProvider, path: &waypoint_path::VfsPath) -> String {
    let mut text = String::new();
    provider
        .open_read(path)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn kind(result: &Result<impl std::fmt::Debug, waypoint_protocol::VfsError>) -> String {
    conformance::kind(result)
}

#[test]
fn a_repository_whose_commit_is_empty_passes_the_conformance_suite() {
    let Some(repo) = repo() else { return };
    repo.git(&["commit", "-q", "--allow-empty", "-m", "empty"]);
    let provider = GitProvider::new();
    conformance::run(&Subject {
        name: "git+file",
        provider: &provider,
        root: repo.at(None),
    });
}

#[test]
fn a_revision_lists_its_folders_and_files() {
    let Some(repo) = repo() else { return };
    repo.write("README.md", "# Hello\n");
    repo.write("src/main.rs", "fn main() {}\n");
    repo.write("src/deep/leaf.txt", "leaf");
    repo.write(".hidden", "x");
    repo.commit_at("first", "2026-01-02T03:04:05Z");
    let provider = GitProvider::new();

    let root = list(&provider, &repo.at(None));
    assert_eq!(names(&root), [".hidden", "README.md", "src"]);
    let src = root.iter().find(|e| e.name == "src").unwrap();
    assert_eq!(src.kind, EntryKind::Directory);
    assert_eq!(src.group, IconGroup::Folder);
    let readme = root.iter().find(|e| e.name == "README.md").unwrap();
    assert_eq!(readme.kind, EntryKind::File);
    assert_eq!(readme.size, Some(8));
    assert!(root.iter().find(|e| e.name == ".hidden").unwrap().hidden);
    assert!(!readme.hidden);
    // A commit has one time, which every entry reports as when it was modified.
    assert_eq!(readme.modified_ms, Some(1_767_323_045_000));

    let deep = list(&provider, &repo.at_inner(None, "src"));
    assert_eq!(names(&deep), ["deep", "main.rs"]);
    assert_eq!(
        read(&provider, &repo.at_inner(None, "src/deep/leaf.txt")),
        "leaf"
    );
    let stat = provider.stat(&repo.at_inner(None, "src/main.rs")).unwrap();
    assert_eq!(stat.size, Some(13));
    assert_eq!(stat.name, "main.rs");
    assert!(provider.read_only());
    assert_eq!(provider.scheme(), "git+file");
}

#[test]
fn a_branch_a_tag_and_a_commit_each_show_their_own_tree() {
    let Some(repo) = repo() else { return };
    repo.write("a.txt", "one");
    let first = repo.commit_all("one");
    repo.git(&["tag", "v1"]);
    repo.write("a.txt", "two");
    repo.write("b.txt", "bee");
    repo.commit_all("two");
    repo.git(&["branch", "side", &first]);
    let provider = GitProvider::new();

    assert_eq!(read(&provider, &repo.at_inner(None, "a.txt")), "two");
    assert_eq!(
        read(&provider, &repo.at_inner(Some("main"), "a.txt")),
        "two"
    );
    assert_eq!(read(&provider, &repo.at_inner(Some("v1"), "a.txt")), "one");
    assert_eq!(
        read(&provider, &repo.at_inner(Some("side"), "a.txt")),
        "one"
    );
    assert_eq!(
        read(&provider, &repo.at_inner(Some(&first), "a.txt")),
        "one"
    );
    assert_eq!(
        read(&provider, &repo.at_inner(Some(&first[..8]), "a.txt")),
        "one"
    );
    assert_eq!(names(&list(&provider, &repo.at(Some("v1")))), ["a.txt"]);
    assert_eq!(
        names(&list(&provider, &repo.at(Some("main~1")))),
        ["a.txt"],
        "a revision expression works too"
    );
    assert_eq!(
        names(&list(&provider, &repo.at(Some("main")))),
        ["a.txt", "b.txt"]
    );
}

#[test]
fn what_is_not_there_is_not_found() {
    let Some(repo) = repo() else { return };
    repo.write("a.txt", "x");
    repo.write("dir/b.txt", "y");
    repo.commit_all("one");
    let provider = GitProvider::new();
    let cancel = CancelToken::new();

    assert_eq!(
        kind(&provider.stat(&repo.at_inner(None, "missing"))),
        "notFound"
    );
    assert_eq!(
        kind(&provider.stat(&repo.at_inner(None, "a.txt/under-a-file"))),
        "notFound"
    );
    assert_eq!(
        kind(&provider.stat(&repo.at_inner(Some("no-such-branch"), ""))),
        "notFound"
    );
    assert_eq!(
        kind(&provider.list(&repo.at_inner(None, "a.txt"), &cancel, 0, &mut |_| {})),
        "notADirectory"
    );
    assert_eq!(
        kind(&provider.open_read(&repo.at_inner(None, "dir")).map(|_| ())),
        "isADirectory"
    );
    let elsewhere = tempfile::tempdir().unwrap();
    let not_a_repo = waypoint_path::VfsPath::Git(
        waypoint_path::GitPath::new(
            waypoint_path::FilePath::from_path(elsewhere.path()).unwrap(),
            None,
        )
        .unwrap(),
    );
    assert_eq!(kind(&provider.stat(&not_a_repo)), "notFound");
}

#[test]
fn a_repository_with_no_commits_is_an_empty_folder() {
    let Some(repo) = repo() else { return };
    let provider = GitProvider::new();
    assert!(list(&provider, &repo.at(None)).is_empty());
    assert_eq!(
        provider.stat(&repo.at(None)).unwrap().kind,
        EntryKind::Directory
    );
    assert_eq!(kind(&provider.stat(&repo.at_inner(None, "x"))), "notFound");
    assert_eq!(provider.describe(&repo.at(None)).unwrap().commit, None);
}

#[test]
fn names_that_are_awkward_survive() {
    let Some(repo) = repo() else { return };
    for name in [
        "with space.txt",
        "100%.txt",
        "café.txt",
        "日本語.txt",
        "#hash",
    ] {
        repo.write(name, name);
    }
    repo.commit_all("names");
    let provider = GitProvider::new();
    assert_eq!(list(&provider, &repo.at(None)).len(), 5);
    for name in [
        "with space.txt",
        "100%.txt",
        "café.txt",
        "日本語.txt",
        "#hash",
    ] {
        assert_eq!(read(&provider, &repo.at_inner(None, name)), name);
    }
}

#[test]
fn an_executable_is_reported_as_one() {
    let Some(repo) = repo() else { return };
    repo.write("run", "#!/bin/sh\n");
    repo.write("plain", "x");
    repo.git(&["add", "-A"]);
    repo.git(&["update-index", "--chmod=+x", "run"]);
    repo.git(&["commit", "-q", "-m", "x"]);
    let provider = GitProvider::new();
    assert_eq!(
        provider.describe(&repo.at_inner(None, "run")).unwrap().kind,
        ObjectKind::Executable
    );
    assert_eq!(
        provider
            .describe(&repo.at_inner(None, "plain"))
            .unwrap()
            .kind,
        ObjectKind::Blob
    );
    let run = provider.stat(&repo.at_inner(None, "run")).unwrap();
    assert_eq!(run.group, IconGroup::Executable);
}

#[test]
fn links_are_reported_as_links_and_resolved_inside_the_tree() {
    let Some(repo) = repo() else { return };
    repo.write("target.txt", "payload");
    repo.write("dir/inner.txt", "inner");
    repo.git(&["add", "-A"]);
    repo.index_link("to-file", "target.txt");
    repo.index_link("to-dir", "dir");
    repo.index_link("dir/up", "../target.txt");
    repo.index_link("broken", "nowhere");
    repo.index_link("outside", "/etc/passwd");
    repo.index_link("chain", "to-file");
    repo.commit_staged("links");
    let provider = GitProvider::new();
    let entries = list(&provider, &repo.at(None));
    let by = |name: &str| entries.iter().find(|e| e.name == name).unwrap().clone();

    assert_eq!(by("to-file").kind, EntryKind::Symlink);
    assert_eq!(by("to-file").link_target, Some(EntryKind::File));
    assert_eq!(by("to-dir").link_target, Some(EntryKind::Directory));
    assert_eq!(by("chain").link_target, Some(EntryKind::File));
    assert_eq!(by("broken").link_target, None);
    assert_eq!(by("broken").group, IconGroup::Symlink);
    assert_eq!(by("outside").link_target, None);
    assert!(!by("to-file").link_pending);
    assert_eq!(by("to-file").size, Some("target.txt".len() as u64));
    assert_eq!(
        provider.read_link(&repo.at_inner(None, "to-file")).unwrap(),
        "target.txt"
    );
    // A link in a folder resolves against that folder, and a file link reads as its target.
    let nested = list(&provider, &repo.at_inner(None, "dir"));
    assert_eq!(
        nested.iter().find(|e| e.name == "up").unwrap().link_target,
        Some(EntryKind::File)
    );
    assert_eq!(read(&provider, &repo.at_inner(None, "to-file")), "payload");
    assert_eq!(read(&provider, &repo.at_inner(None, "dir/up")), "payload");
    assert_eq!(
        provider
            .describe(&repo.at_inner(None, "to-dir"))
            .unwrap()
            .kind,
        ObjectKind::Symlink
    );
}

#[test]
fn a_link_over_the_budget_is_left_pending_and_resolves_later() {
    let Some(repo) = repo() else { return };
    repo.write("t.txt", "x");
    repo.git(&["add", "-A"]);
    repo.index_link("l1", "t.txt");
    repo.index_link("l2", "t.txt");
    repo.commit_staged("links");
    let provider = GitProvider::new();
    let entries = provider
        .list(&repo.at(None), &CancelToken::new(), 0, &mut |_| {})
        .unwrap();
    let pending = entries.iter().find(|e| e.name == "l1").unwrap();
    assert!(pending.link_pending);
    assert_eq!(pending.link_target, None);
    let resolved = provider.resolve_link(&repo.at(None), pending).unwrap();
    assert!(!resolved.link_pending);
    assert_eq!(resolved.link_target, Some(EntryKind::File));
    // A link that is gone from the tree is `notFound`, and the caller drops the update.
    let mut gone = pending.clone();
    gone.name = "vanished".into();
    assert_eq!(
        kind(&provider.resolve_link(&repo.at(None), &gone)),
        "notFound"
    );
}

#[test]
fn a_submodule_is_listed_as_one_and_cannot_be_opened_here() {
    let Some(repo) = repo() else { return };
    repo.write("a.txt", "x");
    let sub = repo.commit_all("first");
    repo.index_submodule("vendor/lib", &sub);
    repo.commit_staged("add the submodule");
    let provider = GitProvider::new();
    let vendor = list(&provider, &repo.at_inner(None, "vendor"));
    let lib = vendor.iter().find(|e| e.name == "lib").unwrap();
    assert_eq!(lib.kind, EntryKind::Other);
    assert_eq!(lib.group, IconGroup::Folder);
    assert_eq!(lib.size, None);
    let info = provider
        .describe(&repo.at_inner(None, "vendor/lib"))
        .unwrap();
    assert_eq!(info.kind, ObjectKind::Submodule);
    assert_eq!(info.id, sub, "the id is the commit it is pinned at");
    assert_eq!(
        kind(&provider.list(
            &repo.at_inner(None, "vendor/lib"),
            &CancelToken::new(),
            0,
            &mut |_| {}
        )),
        "unsupported"
    );
    assert_eq!(
        kind(
            &provider
                .open_read(&repo.at_inner(None, "vendor/lib"))
                .map(|_| ())
        ),
        "unsupported"
    );
}

#[test]
fn a_read_can_start_part_way_in() {
    let Some(repo) = repo() else { return };
    repo.write("n.txt", "0123456789");
    repo.commit_all("n");
    let provider = GitProvider::new();
    let mut tail = String::new();
    provider
        .open_read_at(&repo.at_inner(None, "n.txt"), 6)
        .unwrap()
        .read_to_string(&mut tail)
        .unwrap();
    assert_eq!(tail, "6789");
    let mut past = String::new();
    provider
        .open_read_at(&repo.at_inner(None, "n.txt"), 99)
        .unwrap()
        .read_to_string(&mut past)
        .unwrap();
    assert_eq!(past, "");
    assert!(provider.capabilities().range_read);
}

#[test]
fn a_cancelled_listing_stops() {
    let Some(repo) = repo() else { return };
    for n in 0..20 {
        repo.write(&format!("f{n}"), "x");
    }
    repo.commit_all("many");
    let provider = GitProvider::new();
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(
        kind(&provider.list(&repo.at(None), &cancel, 0, &mut |_| {})),
        "cancelled"
    );
}

#[test]
fn describe_names_the_commit_a_revision_resolved_to() {
    let Some(repo) = repo() else { return };
    repo.write("a", "x");
    let head = repo.commit_all("one");
    let provider = GitProvider::new();
    let info = provider
        .describe(&repo.at_inner(Some("main"), "a"))
        .unwrap();
    assert_eq!(info.commit.as_deref(), Some(head.as_str()));
    assert_eq!(info.kind, ObjectKind::Blob);
    assert_eq!(info.id, repo.git(&["rev-parse", "HEAD:a"]));
}

#[test]
fn one_provider_serves_many_repositories_and_notices_a_new_commit() {
    let Some(first) = repo() else { return };
    let Some(second) = repo() else { return };
    first.write("one", "1");
    first.commit_all("c");
    second.write("two", "2");
    second.commit_all("c");
    let provider = GitProvider::new();
    assert_eq!(names(&list(&provider, &first.at(None))), ["one"]);
    assert_eq!(names(&list(&provider, &second.at(None))), ["two"]);
    first.write("three", "3");
    first.commit_all("d");
    assert_eq!(names(&list(&provider, &first.at(None))), ["one", "three"]);
}
