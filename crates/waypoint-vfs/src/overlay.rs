// What a listing can be decorated with from outside: the marks of a folder, and who supplies them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::sync::Arc;

use waypoint_path::VfsPath;

use crate::model::GitMark;

/// The Git marks of one folder's entries, by name: the ones that have a mark, and the mark every
/// other entry takes (a folder inside an untracked or ignored folder is wholly that).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderMarks {
    pub default: Option<GitMark>,
    pub names: HashMap<OsString, GitMark>,
}

impl FolderMarks {
    /// The mark of the entry called `name`, if it has one.
    pub fn mark_for(&self, name: &OsStr) -> Option<GitMark> {
        self.names.get(name).copied().or(self.default)
    }

    pub fn is_empty(&self) -> bool {
        self.default.is_none() && self.names.is_empty()
    }
}

/// Where an overlay sends the marks of a folder: now, and again whenever they change. It may be
/// called from any thread.
pub type MarkSink = Arc<dyn Fn(FolderMarks) + Send + Sync>;

/// Keeps an overlay running for one listing; dropping it stops it.
pub trait OverlayGuard: Send {}

/// Decorates the listings of folders with marks computed elsewhere (the Git status of a working
/// tree, A102). The vfs plugin holds one, which the app gives it, so the plugin knows nothing of
/// Git and the Git plugin nothing of listings.
pub trait FolderOverlay: Send + Sync {
    /// Starts decorating the listing of `folder`. `None` means there is nothing to decorate (not in
    /// a repository, or the overlay is off); otherwise the guard keeps it going for as long as the
    /// listing is open.
    fn attach(&self, folder: &VfsPath, sink: MarkSink) -> Option<Box<dyn OverlayGuard>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GitChange;

    fn mark(change: GitChange) -> GitMark {
        GitMark {
            unstaged: Some(change),
            ..GitMark::default()
        }
    }

    #[test]
    fn a_named_mark_wins_over_the_default() {
        let marks = FolderMarks {
            default: Some(mark(GitChange::Ignored)),
            names: HashMap::from([(OsString::from("a"), mark(GitChange::Modified))]),
        };
        assert_eq!(
            marks.mark_for(OsStr::new("a")),
            Some(mark(GitChange::Modified))
        );
        assert_eq!(
            marks.mark_for(OsStr::new("b")),
            Some(mark(GitChange::Ignored))
        );
        assert!(FolderMarks::default().is_empty());
        assert_eq!(FolderMarks::default().mark_for(OsStr::new("a")), None);
    }

    #[test]
    fn marks_sort_loudest_first_and_clean_before_ignored() {
        let conflicted = mark(GitChange::Conflicted);
        let modified = mark(GitChange::Modified);
        let untracked = mark(GitChange::Untracked);
        let ignored = mark(GitChange::Ignored);
        let inside = GitMark {
            inside: 3,
            ..GitMark::default()
        };
        let conflict_inside = GitMark {
            inside: 3,
            conflicted_inside: 1,
            ..GitMark::default()
        };
        let ranks: Vec<u8> = [
            Some(&conflicted),
            Some(&conflict_inside),
            Some(&modified),
            Some(&untracked),
            Some(&inside),
            None,
            Some(&ignored),
        ]
        .into_iter()
        .map(GitMark::sort_rank)
        .collect();
        assert_eq!(ranks, [0, 0, 1, 6, 6, 7, 8]);
        assert!(GitMark::default().primary().is_none());
        assert!(modified.is_changed() && inside.is_changed());
        assert!(!ignored.is_changed() && !GitMark::default().is_changed());
    }
}
