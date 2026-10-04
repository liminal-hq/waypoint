// Finding and opening repositories.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

/// The working folder of the repository that holds `dir`, found by looking for a `.git` folder or
/// file (a linked worktree and a submodule have a file) in `dir` and each folder above it, as Git
/// does. The nearest wins, so a submodule is its own repository. Nothing is opened: this is a few
/// `stat` calls, cheap enough to ask for every folder a listing shows.
pub fn find_repository(dir: &Path) -> Option<PathBuf> {
    let mut at = Some(dir);
    while let Some(folder) = at {
        if is_git_marker(&folder.join(".git")) {
            return Some(folder.to_path_buf());
        }
        at = folder.parent();
    }
    None
}

/// The most subfolders `repositories_in` looks at; a folder with more is not worth a stat each.
const REPOSITORY_SCAN_CAP: usize = 5_000;

/// The names of the subfolders of `folder` that are the top of a repository, for the badge a folder
/// icon may carry. One `stat` per subfolder, symlinked folders not followed, and nothing for a
/// folder with more than a few thousand.
pub fn repositories_in(folder: &Path) -> std::collections::HashSet<std::ffi::OsString> {
    let mut found = std::collections::HashSet::new();
    let Ok(read) = std::fs::read_dir(folder) else {
        return found;
    };
    for (seen, entry) in read.flatten().enumerate() {
        if seen >= REPOSITORY_SCAN_CAP {
            break;
        }
        if entry.file_type().is_ok_and(|kind| kind.is_dir())
            && is_git_marker(&entry.path().join(".git"))
        {
            found.insert(entry.file_name());
        }
    }
    found
}

/// A `.git` folder that looks like one, or a `.git` file (`gitdir: …`).
fn is_git_marker(candidate: &Path) -> bool {
    match std::fs::metadata(candidate) {
        Ok(meta) if meta.is_file() => true,
        Ok(meta) if meta.is_dir() => candidate.join("HEAD").is_file(),
        _ => false,
    }
}

/// Opens the repository at a working folder (or a bare repository's own folder) with the user's
/// and the system's configuration, which is where global excludes and `core.*` settings live.
pub(crate) fn open(path: &Path) -> Result<gix::Repository, String> {
    gix::open(path).map_err(|error| error.to_string())
}

/// Where the Git folder of a working folder is, for the watcher to find `HEAD`, the index and the
/// references. Reads the `gitdir:` of a `.git` file; `None` when it cannot be told.
pub(crate) fn git_dir_of(workdir: &Path) -> Option<PathBuf> {
    let dot_git = workdir.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    let text = std::fs::read_to_string(&dot_git).ok()?;
    let target = text.lines().find_map(|line| line.strip_prefix("gitdir:"))?;
    let target = Path::new(target.trim());
    Some(if target.is_absolute() {
        target.to_path_buf()
    } else {
        workdir.join(target)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_marker_wins() {
        let dir = tempfile::tempdir().unwrap();
        let outer = dir.path().join("outer");
        let inner = outer.join("sub");
        std::fs::create_dir_all(outer.join(".git")).unwrap();
        std::fs::write(outer.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::create_dir_all(inner.join("deeper")).unwrap();
        assert_eq!(find_repository(&inner.join("deeper")), Some(outer.clone()));
        // A submodule's `.git` is a file.
        std::fs::write(inner.join(".git"), "gitdir: ../.git/modules/sub\n").unwrap();
        assert_eq!(find_repository(&inner.join("deeper")), Some(inner.clone()));
        assert_eq!(git_dir_of(&inner), Some(inner.join("../.git/modules/sub")));
    }

    #[test]
    fn the_repositories_among_the_subfolders_are_found() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::create_dir_all(dir.path().join("plain/.git")).unwrap();
        std::fs::create_dir_all(dir.path().join("empty")).unwrap();
        std::fs::write(dir.path().join("file.txt"), "x").unwrap();
        let linked = dir.path().join("linked");
        std::fs::create_dir_all(&linked).unwrap();
        std::fs::write(linked.join(".git"), "gitdir: ../repo/.git\n").unwrap();
        let found = repositories_in(dir.path());
        let mut names: Vec<_> = found
            .iter()
            .map(|n| n.to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["linked", "repo"]);
        assert!(repositories_in(&dir.path().join("missing")).is_empty());
    }

    #[test]
    fn a_folder_called_git_without_a_head_is_not_a_repository() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        assert_eq!(find_repository(dir.path()), None);
    }
}
