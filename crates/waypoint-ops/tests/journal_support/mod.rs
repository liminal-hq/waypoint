// Fixtures for the journal tests: the journalled harness over each provider, and tree helpers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code, unused_imports)]

pub use waypoint_ops::testing::journal_harness::{JournalHarness, JournalRun};
pub use waypoint_ops::testing::journal_storage::MemoryJournalStorage;

use crate::common::*;

/// Runs the body once for each provider the journal is tested on: the local provider over a
/// temporary directory, and the in-memory provider under each case rule.
#[macro_export]
macro_rules! each_journal_provider {
    (|$h:ident, $rule:ident, $links:ident| $body:block) => {{
        {
            let dir = tempfile::tempdir().unwrap();
            let base = $crate::common::VfsPath::File(
                $crate::common::FilePath::from_path(dir.path()).unwrap(),
            );
            #[allow(unused_mut)]
            let mut $h = $crate::journal_support::JournalHarness::new(
                $crate::common::LocalProvider::new(),
                base,
            );
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
            let mut $h = $crate::journal_support::JournalHarness::new(
                $crate::common::MemoryProvider::new(root, $rule),
                base,
            );
            let $links = true;
            eprintln!("-- {name}");
            $body
        }
    }};
}

pub fn local_jh() -> (JournalHarness<LocalProvider>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let base = VfsPath::File(FilePath::from_path(dir.path()).unwrap());
    (JournalHarness::new(LocalProvider::new(), base), dir)
}

pub fn memory_jh(rule: CaseRule) -> (JournalHarness<MemoryProvider>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let root = FilePath::from_path(dir.path()).unwrap();
    let base = VfsPath::File(root.clone());
    (
        JournalHarness::new(MemoryProvider::new(root, rule), base),
        dir,
    )
}

pub fn jbuild<P: Provider + 'static>(h: &JournalHarness<P>, tree: &Tree) {
    build(&h.harness, tree);
}

pub fn jwork<P: Provider + 'static>(h: &JournalHarness<P>) -> Tree {
    work_tree(&h.harness)
}

/// The error a run failed with.
pub fn failed_with(run: &JournalRun) -> &OpsError {
    error_of(&run.state)
}

/// Writes `content` over the file at `relative` below the work folder, as another program would.
pub fn overwrite<P: Provider + 'static>(h: &JournalHarness<P>, relative: &str, content: &str) {
    use std::io::Write;
    let path = h.path(relative);
    let mut stream = h
        .provider
        .create_write(&path, waypoint_vfs::WriteOptions::truncate())
        .unwrap_or_else(|e| panic!("writing {relative}: {e:?}"));
    stream.write_all(content.as_bytes()).unwrap();
    stream.finish(false).unwrap();
    h.provider.reset();
}
