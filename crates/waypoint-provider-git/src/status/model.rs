// What the status of a working tree is made of: a change per entry, a badge per folder.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// One kind of difference. The order is how loud it is: a conflict outranks an ordinary change,
/// and an ignored entry is the quietest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Change {
    Ignored,
    Untracked,
    TypeChanged,
    Renamed,
    Added,
    Deleted,
    Modified,
    Conflicted,
}

impl Change {
    /// The letter `git status --short` uses for it (`?` untracked, `!` ignored, `U` conflicted).
    pub const fn letter(self) -> char {
        match self {
            Change::Modified => 'M',
            Change::Added => 'A',
            Change::Deleted => 'D',
            Change::Renamed => 'R',
            Change::TypeChanged => 'T',
            Change::Untracked => '?',
            Change::Ignored => '!',
            Change::Conflicted => 'U',
        }
    }

    /// Whether this is a change someone has made, which ignored files are not.
    pub const fn counts_as_change(self) -> bool {
        !matches!(self, Change::Ignored)
    }
}

/// The status of one path: what is staged (`HEAD` against the index) and what is not (the index
/// against the files). An untracked, ignored or conflicted path has only the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EntryStatus {
    pub staged: Option<Change>,
    pub unstaged: Option<Change>,
}

impl EntryStatus {
    pub const fn untracked() -> Self {
        Self {
            staged: None,
            unstaged: Some(Change::Untracked),
        }
    }

    pub const fn ignored() -> Self {
        Self {
            staged: None,
            unstaged: Some(Change::Ignored),
        }
    }

    pub fn is_conflicted(&self) -> bool {
        self.unstaged == Some(Change::Conflicted)
    }

    /// Whether anything but being ignored is the matter with the path.
    pub fn is_changed(&self) -> bool {
        self.staged.is_some_and(Change::counts_as_change)
            || self.unstaged.is_some_and(Change::counts_as_change)
    }

    /// The loudest of the two: what one letter in a column says.
    pub fn primary(&self) -> Option<Change> {
        self.staged.max(self.unstaged)
    }

    /// The two-letter form `git status --short` shows (`M ` staged, ` M` not, `MM` both), with
    /// `??`, `!!` and `UU` for the paths that have no staged side.
    pub fn short(&self) -> [char; 2] {
        match self.unstaged {
            Some(Change::Untracked) => ['?', '?'],
            Some(Change::Ignored) => ['!', '!'],
            Some(Change::Conflicted) => ['U', 'U'],
            other => [
                self.staged.map_or(' ', Change::letter),
                other.map_or(' ', Change::letter),
            ],
        }
    }

    /// Sorts changed paths by how much attention they want: conflicts first, then edits, then new
    /// files, with clean last (a `None` status sorts after everything).
    pub fn sort_rank(status: Option<&EntryStatus>) -> u8 {
        match status.and_then(EntryStatus::primary) {
            Some(Change::Conflicted) => 0,
            Some(Change::Modified) => 1,
            Some(Change::Deleted) => 2,
            Some(Change::Added) => 3,
            Some(Change::Renamed) => 4,
            Some(Change::TypeChanged) => 5,
            Some(Change::Untracked) => 6,
            Some(Change::Ignored) => 8,
            None => 7,
        }
    }
}

/// What a folder shows when something inside it has changed. Ignored files are not counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FolderBadge {
    /// How many changed paths are inside, at any depth (an untracked folder counts once).
    pub changed: u32,
    /// How many of them are conflicted.
    pub conflicted: u32,
}

/// What `RepoStatus::names_in` says about one name in a folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NameStatus {
    /// The path's own status, when it has one.
    pub status: Option<EntryStatus>,
    /// For a folder, what changed inside it.
    pub badge: Option<FolderBadge>,
}

/// Every path of a working tree that is not clean, found at one moment. Clean paths are not
/// stored, so it is as small as the changes are. An untracked or ignored folder is one entry for the
/// folder, which `status_of` applies to everything inside.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoStatus {
    root: PathBuf,
    /// Repository-relative paths with `/` separators, as bytes.
    entries: BTreeMap<Vec<u8>, EntryStatus>,
    folders: HashMap<Vec<u8>, FolderBadge>,
}

fn parent_of(rel: &[u8]) -> Option<&[u8]> {
    rel.iter()
        .rposition(|&b| b == b'/')
        .map(|at| &rel[..at])
        .or(if rel.is_empty() { None } else { Some(&[]) })
}

impl RepoStatus {
    pub fn new(root: PathBuf, entries: BTreeMap<Vec<u8>, EntryStatus>) -> Self {
        let mut folders: HashMap<Vec<u8>, FolderBadge> = HashMap::new();
        for (path, status) in &entries {
            if !status.is_changed() {
                continue;
            }
            let conflicted = u32::from(status.is_conflicted());
            let mut at = parent_of(path);
            while let Some(folder) = at {
                let badge = folders.entry(folder.to_vec()).or_default();
                badge.changed += 1;
                badge.conflicted += conflicted;
                at = parent_of(folder);
            }
        }
        Self {
            root,
            entries,
            folders,
        }
    }

    /// The working folder this status is of.
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many paths have a status (collapsed folders counted once).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&[u8], &EntryStatus)> {
        self.entries
            .iter()
            .map(|(path, status)| (path.as_slice(), status))
    }

    /// The status recorded for exactly this path.
    pub fn entry(&self, rel: &[u8]) -> Option<&EntryStatus> {
        self.entries.get(rel)
    }

    /// The status of a path, which is that of the untracked or ignored folder it is inside when it
    /// has none of its own.
    pub fn status_of(&self, rel: &[u8]) -> Option<EntryStatus> {
        if let Some(status) = self.entries.get(rel) {
            return Some(*status);
        }
        self.inherited(rel)
    }

    /// The untracked or ignored folder above a path, as the status the path takes.
    pub fn inherited(&self, rel: &[u8]) -> Option<EntryStatus> {
        let mut at = parent_of(rel);
        while let Some(folder) = at {
            if !folder.is_empty() {
                if let Some(status) = self.entries.get(folder) {
                    if matches!(status.unstaged, Some(Change::Untracked | Change::Ignored)) {
                        return Some(*status);
                    }
                }
            }
            at = parent_of(folder);
        }
        None
    }

    /// The untracked or ignored folder entry above `rel` (not `rel` itself), with its status.
    pub fn collapsed_ancestor(&self, rel: &[u8]) -> Option<(&[u8], EntryStatus)> {
        let mut at = parent_of(rel);
        while let Some(folder) = at {
            if !folder.is_empty() {
                if let Some((path, status)) = self.entries.get_key_value(folder) {
                    if matches!(status.unstaged, Some(Change::Untracked | Change::Ignored)) {
                        return Some((path.as_slice(), *status));
                    }
                }
            }
            at = parent_of(folder);
        }
        None
    }

    /// Whether any path has a staged rename, which a status of only some paths cannot tell from a
    /// delete and an add.
    pub fn has_staged_renames(&self) -> bool {
        self.entries
            .values()
            .any(|status| status.staged == Some(Change::Renamed))
    }

    /// What changed inside a folder, or `None` when nothing has (the badge a folder shows). The
    /// repository's own top is `b""`.
    pub fn badge(&self, folder: &[u8]) -> Option<FolderBadge> {
        self.folders.get(folder).copied()
    }

    /// The status and badge of every name in a folder that has either, by name: what a listing
    /// asks for once and then reads per row. A folder inside an untracked or ignored folder has
    /// no entries of its own, so its names take `inherited(folder)`; the caller applies that.
    pub fn names_in(&self, folder: &[u8]) -> HashMap<Vec<u8>, NameStatus> {
        let mut out: HashMap<Vec<u8>, NameStatus> = HashMap::new();
        let prefix: Vec<u8> = if folder.is_empty() {
            Vec::new()
        } else {
            let mut p = folder.to_vec();
            p.push(b'/');
            p
        };
        let range = self.entries.range(prefix.clone()..);
        for (path, status) in range {
            if !path.starts_with(&prefix) {
                break;
            }
            let rest = &path[prefix.len()..];
            if rest.is_empty() {
                continue;
            }
            match rest.iter().position(|&b| b == b'/') {
                None => out.entry(rest.to_vec()).or_default().status = Some(*status),
                Some(at) => {
                    let name = &rest[..at];
                    let mut child = prefix.clone();
                    child.extend_from_slice(name);
                    let entry = out.entry(name.to_vec()).or_default();
                    entry.badge = self.folders.get(&child).copied();
                }
            }
        }
        // A folder's own entry (untracked) has no deeper entries; a tracked folder has no entry,
        // only the badge, which the loop above set when something inside it changed.
        out
    }

    /// The counts a status bar or a summary wants.
    pub fn counts(&self) -> StatusCounts {
        let mut counts = StatusCounts::default();
        for status in self.entries.values() {
            if status.is_conflicted() {
                counts.conflicted += 1;
                continue;
            }
            if status.staged.is_some_and(Change::counts_as_change) {
                counts.staged += 1;
            }
            match status.unstaged {
                Some(Change::Untracked) => counts.untracked += 1,
                Some(Change::Ignored) | None => {}
                Some(_) => counts.unstaged += 1,
            }
        }
        counts
    }

    /// Takes `fresh` (a status computed for only some folders) in place of what this one had under
    /// each of `scopes`, which are repository-relative folders or paths.
    pub fn merged(&self, scopes: &[Vec<u8>], fresh: RepoStatus) -> RepoStatus {
        let covered = |path: &[u8]| {
            scopes.iter().any(|scope| {
                path == scope.as_slice()
                    || (path.len() > scope.len()
                        && path.starts_with(scope)
                        && path[scope.len()] == b'/')
            })
        };
        let mut entries: BTreeMap<Vec<u8>, EntryStatus> = self
            .entries
            .iter()
            .filter(|(path, _)| !covered(path))
            .map(|(path, status)| (path.clone(), *status))
            .collect();
        entries.extend(fresh.entries);
        RepoStatus::new(self.root.clone(), entries)
    }
}

/// How many paths are in each state; a path with staged and unstaged changes counts in both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatusCounts {
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicted: u32,
}

impl StatusCounts {
    /// Whether anything is the matter, by the rule `git status` uses for a clean tree (untracked
    /// files count).
    pub fn is_dirty(&self) -> bool {
        self.staged + self.unstaged + self.untracked + self.conflicted > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(entries: &[(&str, EntryStatus)]) -> RepoStatus {
        RepoStatus::new(
            PathBuf::from("/repo"),
            entries
                .iter()
                .map(|(path, status)| (path.as_bytes().to_vec(), *status))
                .collect(),
        )
    }

    const MODIFIED: EntryStatus = EntryStatus {
        staged: None,
        unstaged: Some(Change::Modified),
    };

    #[test]
    fn a_change_badges_every_folder_above_it() {
        let s = status(&[
            ("a/b/c.txt", MODIFIED),
            ("a/d.txt", EntryStatus::untracked()),
        ]);
        assert_eq!(s.badge(b"a/b").unwrap().changed, 1);
        assert_eq!(s.badge(b"a").unwrap().changed, 2);
        assert_eq!(s.badge(b"").unwrap().changed, 2);
        assert_eq!(s.badge(b"other"), None);
    }

    #[test]
    fn ignored_paths_badge_nothing() {
        let s = status(&[("target", EntryStatus::ignored())]);
        assert_eq!(s.badge(b""), None);
        assert!(!s.counts().is_dirty());
    }

    #[test]
    fn a_conflict_is_counted_in_the_badge_and_outranks_everything() {
        let conflict = EntryStatus {
            staged: None,
            unstaged: Some(Change::Conflicted),
        };
        let s = status(&[("x/y", conflict), ("x/z", MODIFIED)]);
        assert_eq!(s.badge(b"x").unwrap().conflicted, 1);
        assert_eq!(s.counts().conflicted, 1);
        assert_eq!(
            EntryStatus::sort_rank(Some(&conflict)),
            0,
            "conflicts sort first"
        );
        assert_eq!(conflict.short(), ['U', 'U']);
    }

    #[test]
    fn a_path_inside_an_untracked_or_ignored_folder_takes_its_status() {
        let s = status(&[
            ("build", EntryStatus::ignored()),
            ("new", EntryStatus::untracked()),
        ]);
        assert_eq!(s.status_of(b"build/out/a.o"), Some(EntryStatus::ignored()));
        assert_eq!(s.status_of(b"new/a.txt"), Some(EntryStatus::untracked()));
        assert_eq!(s.status_of(b"elsewhere/a.txt"), None);
        assert_eq!(s.entry(b"build/out/a.o"), None);
    }

    #[test]
    fn names_in_a_folder_carry_their_status_or_their_badge() {
        let s = status(&[
            ("src/lib.rs", MODIFIED),
            ("src/deep/x.rs", MODIFIED),
            ("src/deep/y.rs", EntryStatus::untracked()),
            ("README.md", MODIFIED),
        ]);
        let src = s.names_in(b"src");
        assert_eq!(src[&b"lib.rs".to_vec()].status, Some(MODIFIED));
        assert_eq!(src[&b"deep".to_vec()].badge.unwrap().changed, 2);
        let root = s.names_in(b"");
        assert_eq!(root[&b"README.md".to_vec()].status, Some(MODIFIED));
        assert_eq!(root[&b"src".to_vec()].badge.unwrap().changed, 3);
        assert!(s.names_in(b"empty").is_empty());
    }

    #[test]
    fn the_short_form_shows_staged_and_unstaged_sides() {
        let both = EntryStatus {
            staged: Some(Change::Modified),
            unstaged: Some(Change::Modified),
        };
        assert_eq!(both.short(), ['M', 'M']);
        let staged_add = EntryStatus {
            staged: Some(Change::Added),
            unstaged: None,
        };
        assert_eq!(staged_add.short(), ['A', ' ']);
        assert_eq!(EntryStatus::untracked().short(), ['?', '?']);
        assert_eq!(MODIFIED.short(), [' ', 'M']);
    }

    #[test]
    fn counts_separate_staged_from_unstaged() {
        let both = EntryStatus {
            staged: Some(Change::Modified),
            unstaged: Some(Change::Modified),
        };
        let s = status(&[
            ("a", both),
            ("b", MODIFIED),
            ("c", EntryStatus::untracked()),
            ("d", EntryStatus::ignored()),
        ]);
        assert_eq!(
            s.counts(),
            StatusCounts {
                staged: 1,
                unstaged: 2,
                untracked: 1,
                conflicted: 0
            }
        );
    }

    #[test]
    fn merging_replaces_only_what_a_scope_covers() {
        let before = status(&[
            ("a/one", MODIFIED),
            ("a/two", MODIFIED),
            ("b/three", MODIFIED),
            ("ab", MODIFIED),
        ]);
        let fresh = status(&[("a/two", EntryStatus::untracked())]);
        let merged = before.merged(&[b"a".to_vec()], fresh);
        assert_eq!(merged.entry(b"a/one"), None);
        assert_eq!(merged.entry(b"a/two"), Some(&EntryStatus::untracked()));
        assert_eq!(merged.entry(b"b/three"), Some(&MODIFIED));
        assert_eq!(
            merged.entry(b"ab"),
            Some(&MODIFIED),
            "a sibling of the same prefix stays"
        );
        assert_eq!(merged.badge(b"a").unwrap().changed, 1);
    }

    #[test]
    fn sorting_by_status_puts_the_loudest_first_and_clean_before_ignored() {
        let ranks: Vec<u8> = [
            Some(EntryStatus {
                staged: None,
                unstaged: Some(Change::Conflicted),
            }),
            Some(MODIFIED),
            Some(EntryStatus::untracked()),
            None,
            Some(EntryStatus::ignored()),
        ]
        .iter()
        .map(|s| EntryStatus::sort_rank(s.as_ref()))
        .collect();
        let mut sorted = ranks.clone();
        sorted.sort_unstable();
        assert_eq!(ranks, sorted);
    }
}
