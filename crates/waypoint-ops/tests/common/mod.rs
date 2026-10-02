// Fixtures shared by the integration tests: the harness over each provider the engine is tested
// on, and helpers for comparing trees.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code, unused_imports)]

pub use waypoint_ops::testing::faulty::{FaultKind, Op};
pub use waypoint_ops::testing::harness::{Harness, RunResult};
pub use waypoint_ops::testing::tree::{
    partials, populate, random_tree, tree_of, without_partials, Node, Rng, Tree,
};
pub use waypoint_ops::*;
pub use waypoint_path::{CaseRule, FilePath, VfsPath};
pub use waypoint_vfs::{LocalProvider, MemoryProvider, Provider};

/// Runs the body once for each provider the engine is tested on: the local provider over a
/// temporary directory, and the in-memory provider under each case rule. `$links` says whether the
/// provider can make symlinks on this platform.
#[macro_export]
macro_rules! each_provider {
    (|$h:ident, $rule:ident, $links:ident| $body:block) => {{
        {
            let dir = tempfile::tempdir().unwrap();
            let base = $crate::common::VfsPath::File(
                $crate::common::FilePath::from_path(dir.path()).unwrap(),
            );
            #[allow(unused_mut)]
            let mut $h = $crate::common::Harness::new($crate::common::LocalProvider::new(), base);
            let $rule = $crate::common::CaseRule::NATIVE;
            let $links = cfg!(unix);
            eprintln!("-- local");
            $body
        }
        for (name, $rule) in [
            ("memory/sensitive", $crate::common::CaseRule::Sensitive),
            ("memory/insensitive", $crate::common::CaseRule::Insensitive),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let root = $crate::common::FilePath::from_path(dir.path()).unwrap();
            let base = $crate::common::VfsPath::File(root.clone());
            #[allow(unused_mut)]
            let mut $h = $crate::common::Harness::new(
                $crate::common::MemoryProvider::new(root, $rule),
                base,
            );
            let $links = true;
            eprintln!("-- {name}");
            $body
        }
    }};
}

/// A tree written as `(path, content)`; a path ending in `/` is a folder.
pub fn tree(entries: &[(&str, &str)]) -> Tree {
    entries
        .iter()
        .map(|(path, content)| match path.strip_suffix('/') {
            Some(dir) => (dir.to_owned(), Node::Dir),
            None => ((*path).to_owned(), Node::File(content.as_bytes().to_vec())),
        })
        .collect()
}

pub fn file(content: &str) -> Node {
    Node::File(content.as_bytes().to_vec())
}

/// Builds `tree` in the harness's work folder.
pub fn build<P: Provider + 'static>(h: &Harness<P>, tree: &Tree) {
    populate(h.provider.as_ref(), &h.work, tree);
    h.provider.reset();
}

pub fn work_tree<P: Provider + 'static>(h: &Harness<P>) -> Tree {
    tree_of(h.provider.as_ref(), &h.work)
}

/// The error a failed job ended with.
pub fn error_of(state: &JobState) -> &OpsError {
    match state {
        JobState::Failed { error, .. } => error,
        other => panic!("expected a failed job, found {other:?}"),
    }
}

/// The harness over the local provider in a temporary directory, with the directory's guard.
pub fn local_harness() -> (Harness<LocalProvider>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let base = VfsPath::File(FilePath::from_path(dir.path()).unwrap());
    (Harness::new(LocalProvider::new(), base), dir)
}

/// The harness over the in-memory provider under `rule`.
pub fn memory_harness(rule: CaseRule) -> (Harness<MemoryProvider>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let root = FilePath::from_path(dir.path()).unwrap();
    let base = VfsPath::File(root.clone());
    (Harness::new(MemoryProvider::new(root, rule), base), dir)
}
