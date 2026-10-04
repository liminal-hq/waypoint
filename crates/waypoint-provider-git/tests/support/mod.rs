// Fixture repositories for the tests, made with the real `git` so the provider is checked against
// what Git itself writes. The provider never runs `git`; only these fixtures do.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use waypoint_path::{FilePath, GitPath, VfsPath};

/// Whether this run must have `git` (CI sets it); otherwise a missing `git` skips the test.
pub fn required() -> bool {
    std::env::var_os("WAYPOINT_GIT_REQUIRE").is_some()
}

pub fn have_git() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// A repository in a temporary folder, with helpers to change it.
pub struct Repo {
    pub dir: tempfile::TempDir,
}

/// Starts a test: `None` (after saying why) when `git` is missing and not required.
pub fn repo() -> Option<Repo> {
    if !have_git() {
        assert!(!required(), "WAYPOINT_GIT_REQUIRE is set but `git` is missing");
        eprintln!("skipped: `git` is not installed (set WAYPOINT_GIT_REQUIRE to fail instead)");
        return None;
    }
    let repo = Repo {
        dir: tempfile::tempdir().expect("a temporary folder"),
    };
    repo.git(&["init", "-q", "-b", "main"]);
    Some(repo)
}

impl Repo {
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn git(&self, args: &[&str]) -> String {
        self.git_in(self.path(), args)
    }

    pub fn git_in(&self, dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(dir)
            .args(["-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
            .args(["-c", "core.fsmonitor=false", "-c", "protocol.file.allow=always"])
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", null_device())
            .env("GIT_AUTHOR_NAME", "Ada Tester")
            .env("GIT_AUTHOR_EMAIL", "ada@example.test")
            .env("GIT_COMMITTER_NAME", "Ada Tester")
            .env("GIT_COMMITTER_EMAIL", "ada@example.test")
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    pub fn write(&self, rel: &str, text: &str) {
        let at = self.path().join(rel);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }

    pub fn remove(&self, rel: &str) {
        std::fs::remove_file(self.path().join(rel)).unwrap();
    }

    pub fn commit_all(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    /// Commits what is in the index as it is (for entries `index_link` and `index_submodule`
    /// added, which `commit_all` would drop because nothing on disk backs them).
    pub fn commit_staged(&self, message: &str) -> String {
        self.git(&["commit", "-q", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    pub fn commit_at(&self, message: &str, date: &str) -> String {
        self.git(&["add", "-A"]);
        let out = Command::new("git")
            .current_dir(self.path())
            .args(["-c", "commit.gpgsign=false", "commit", "-q", "-m", message])
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", null_device())
            .env("GIT_AUTHOR_NAME", "Ada Tester")
            .env("GIT_AUTHOR_EMAIL", "ada@example.test")
            .env("GIT_COMMITTER_NAME", "Ada Tester")
            .env("GIT_COMMITTER_EMAIL", "ada@example.test")
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        self.git(&["rev-parse", "HEAD"])
    }

    pub fn file_path(&self) -> FilePath {
        FilePath::from_path(self.path()).unwrap()
    }

    /// The top of the repository at `rev` (`None` follows `HEAD`).
    pub fn at(&self, rev: Option<&str>) -> VfsPath {
        VfsPath::Git(GitPath::new(self.file_path(), rev.map(str::to_owned)).unwrap())
    }

    pub fn at_inner(&self, rev: Option<&str>, inner: &str) -> VfsPath {
        let mut path = self.at(rev);
        for part in inner.split('/').filter(|part| !part.is_empty()) {
            path = path.join(part).unwrap();
        }
        path
    }

    /// Replaces a path with a symlink to `target` (Unix only: skipped on Windows, where making one
    /// needs a privilege).
    #[cfg(unix)]
    pub fn symlink(&self, rel: &str, target: &str) {
        let at = self.path().join(rel);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, at).unwrap();
    }

    /// Records a link in the index without needing the privilege to create one on disk.
    pub fn index_link(&self, rel: &str, target: &str) {
        let blob = {
            let at = self.path().join(".link-blob");
            std::fs::write(&at, target).unwrap();
            let id = self.git(&["hash-object", "-w", ".link-blob"]);
            std::fs::remove_file(at).unwrap();
            id
        };
        self.git(&["update-index", "--add", "--cacheinfo", &format!("120000,{blob},{rel}")]);
    }

    /// Records a submodule (a commit of another repository) in the index.
    pub fn index_submodule(&self, rel: &str, commit: &str) {
        self.git(&["update-index", "--add", "--cacheinfo", &format!("160000,{commit},{rel}")]);
    }
}

fn null_device() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from("NUL")
    } else {
        PathBuf::from("/dev/null")
    }
}
