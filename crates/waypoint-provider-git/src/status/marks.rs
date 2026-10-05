// From a repository's status to the marks a folder's listing shows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;

use waypoint_vfs::{FolderMarks, GitChange, GitMark};

use super::model::{Change, EntryStatus, FolderBadge, RepoStatus};
use super::tracker::rel_bytes;

fn change(change: Change) -> GitChange {
    match change {
        Change::Modified => GitChange::Modified,
        Change::Added => GitChange::Added,
        Change::Deleted => GitChange::Deleted,
        Change::Renamed => GitChange::Renamed,
        Change::TypeChanged => GitChange::TypeChanged,
        Change::Untracked => GitChange::Untracked,
        Change::Ignored => GitChange::Ignored,
        Change::Conflicted => GitChange::Conflicted,
    }
}

/// The mark of a path with this status and, if it is a folder, this badge.
pub fn mark(status: Option<&EntryStatus>, badge: Option<FolderBadge>) -> GitMark {
    GitMark {
        staged: status.and_then(|s| s.staged).map(change),
        unstaged: status.and_then(|s| s.unstaged).map(change),
        inside: badge.map_or(0, |b| b.changed),
        conflicted_inside: badge.map_or(0, |b| b.conflicted),
        repository: false,
    }
}

/// The marks for the listing of `folder` (an absolute path inside the working tree `status` is
/// of): one for each entry that has a status or something changed inside it, and a default for a
/// folder that is itself untracked or ignored (everything in it is).
pub fn folder_marks(status: &RepoStatus, folder: &Path) -> FolderMarks {
    let Ok(rel) = folder.strip_prefix(status.root()) else {
        return FolderMarks::default();
    };
    let rel = rel_bytes(rel);
    let default = status
        .status_of(&rel)
        .filter(|s| matches!(s.unstaged, Some(Change::Untracked | Change::Ignored)))
        .map(|s| mark(Some(&s), None));
    let names = if default.is_some() {
        Default::default()
    } else {
        status
            .names_in(&rel)
            .into_iter()
            .map(|(name, found)| {
                (
                    crate::names::os_string(&name),
                    mark(found.status.as_ref(), found.badge),
                )
            })
            .collect()
    };
    FolderMarks { default, names }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::*;

    const MODIFIED: EntryStatus = EntryStatus {
        staged: None,
        unstaged: Some(Change::Modified),
    };

    fn status() -> RepoStatus {
        let entries: BTreeMap<Vec<u8>, EntryStatus> = [
            ("README.md", MODIFIED),
            ("src/lib.rs", MODIFIED),
            ("src/deep/x.rs", MODIFIED),
            ("new_dir", EntryStatus::untracked()),
            ("target", EntryStatus::ignored()),
        ]
        .into_iter()
        .map(|(p, s)| (p.as_bytes().to_vec(), s))
        .collect();
        RepoStatus::new(PathBuf::from("/repo"), entries)
    }

    #[test]
    fn a_folder_lists_its_changed_names_and_the_badges_of_folders_with_changes() {
        let marks = folder_marks(&status(), Path::new("/repo"));
        assert_eq!(marks.default, None);
        let readme = marks.names[&OsString::from("README.md")];
        assert_eq!(readme.unstaged, Some(GitChange::Modified));
        let src = marks.names[&OsString::from("src")];
        assert_eq!(src.inside, 2);
        assert_eq!(src.unstaged, None);
        assert_eq!(
            marks.names[&OsString::from("new_dir")].unstaged,
            Some(GitChange::Untracked)
        );
        assert_eq!(
            marks.names[&OsString::from("target")].unstaged,
            Some(GitChange::Ignored)
        );
    }

    #[test]
    fn a_subfolder_lists_only_what_is_in_it() {
        let marks = folder_marks(&status(), Path::new("/repo/src"));
        assert_eq!(marks.names.len(), 2);
        assert_eq!(marks.names[&OsString::from("deep")].inside, 1);
        assert_eq!(
            marks.names[&OsString::from("lib.rs")].unstaged,
            Some(GitChange::Modified)
        );
    }

    #[test]
    fn everything_in_an_untracked_or_ignored_folder_takes_its_mark() {
        let marks = folder_marks(&status(), Path::new("/repo/target/debug"));
        assert_eq!(
            marks.default.unwrap().unstaged,
            Some(GitChange::Ignored),
            "a folder inside an ignored folder is ignored"
        );
        let marks = folder_marks(&status(), Path::new("/repo/new_dir"));
        assert_eq!(marks.default.unwrap().unstaged, Some(GitChange::Untracked));
        assert!(marks.names.is_empty());
    }

    #[test]
    fn a_clean_folder_and_a_folder_outside_the_tree_have_no_marks() {
        assert!(folder_marks(&status(), Path::new("/repo/clean")).is_empty());
        assert!(folder_marks(&status(), Path::new("/elsewhere")).is_empty());
    }
}
