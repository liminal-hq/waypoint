// The move executor: a rename on one volume, and copy then remove, one item at a time, across
// volumes (which the in-memory provider simulates with `CrossesDevices`). Stopping at any step
// leaves each item either moved or untouched.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;
mod xfer;

use std::cell::RefCell;
use std::rc::Rc;

use xfer::*;

fn dst_tree<P: Provider + 'static>(h: &Harness<P>) -> Tree {
    tree_of(h.provider.as_ref(), &h.path("dst"))
}

fn src_tree<P: Provider + 'static>(h: &Harness<P>) -> Tree {
    tree_of(h.provider.as_ref(), &h.path("src"))
}

/// A memory harness with `src` on a volume of its own, so a move from `src` to `dst` crosses.
fn crossing(rule: CaseRule) -> (Harness<MemoryProvider>, tempfile::TempDir) {
    let (h, dir) = memory_harness(rule);
    h.provider.create_dir(&h.path("src")).unwrap();
    memory(&h).set_volume(&h.path("src"), VolumeId(2));
    h.provider.create_dir(&h.path("dst")).unwrap();
    (h, dir)
}

fn fill<P: Provider + 'static>(h: &Harness<P>, t: &Tree) {
    // `populate` wants every parent; the tree lists them.
    populate(h.provider.as_ref(), &h.path("src"), t);
    h.provider.reset();
}

fn sample() -> Tree {
    let mut t = tree(&[
        ("top/", ""),
        ("top/a.txt", "alpha"),
        ("top/sub/", ""),
        ("top/sub/b.bin", ""),
        ("top/sub/deep/", ""),
        ("top/sub/deep/c", "see"),
        ("top/empty/", ""),
        ("loose", "loose file"),
    ]);
    t.insert(
        "top/big".to_owned(),
        Node::File(pattern(5 * SMALL_CHUNK + 3, 4)),
    );
    t.insert("top/ln".to_owned(), Node::Link("sub".to_owned()));
    t.insert("top/dangling".to_owned(), Node::Link("nowhere".to_owned()));
    t
}

#[test]
fn on_one_volume_a_move_is_a_rename_and_reads_nothing() {
    each_provider!(|h, rule, links| {
        let _ = rule;
        let mut t = sample();
        if !links {
            t.retain(|_, n| !matches!(n, Node::Link(_)));
        }
        build(&h, &tree(&[("src/", ""), ("dst/", "")]));
        fill(&h, &t);
        set_mtime(&h, "src/top/a.txt", 700_000_000_000);
        let result = go(
            &mut h,
            JobKind::Move,
            &["src/top", "src/loose"],
            "dst",
            None,
        );
        done(&result);
        assert_eq!(h.provider.calls_of(Op::OpenRead), 0);
        assert_eq!(h.provider.calls_of(Op::CreateWrite), 0);
        assert_eq!(h.provider.calls_of(Op::Rename), 2);
        assert_eq!(dst_tree(&h), t);
        assert!(src_tree(&h).is_empty());
        assert_eq!(mtime_of(&h, "dst/top/a.txt"), Some(700_000_000_000));
        let report = result.report.unwrap();
        assert_eq!(
            report.renamed,
            vec![
                (h.loc("src/top"), h.loc("dst/top")),
                (h.loc("src/loose"), h.loc("dst/loose"))
            ]
        );
        // Progress still reaches the end, counting everything the rename carried.
        assert_eq!(report.progress.items_done, report.progress.items_total);
        assert_eq!(report.progress.bytes_done, report.progress.bytes_total);
    });
}

#[test]
fn across_volumes_a_move_copies_then_removes_and_ends_with_the_same_tree() {
    for rule in [CaseRule::Sensitive, CaseRule::Insensitive] {
        let (mut h, _dir) = crossing(rule);
        let t = sample();
        fill(&h, &t);
        set_mtime(&h, "src/top/a.txt", 700_000_000_000);
        set_mtime(&h, "src/top/sub", 710_000_000_000);
        set_mtime(&h, "src/top", 720_000_000_000);
        set_mode(&h, "src/top/a.txt", 0o600);
        let result = go(
            &mut h,
            JobKind::Move,
            &["src/top", "src/loose"],
            "dst",
            None,
        );
        done(&result);
        assert_eq!(dst_tree(&h), t, "{rule:?}");
        assert!(src_tree(&h).is_empty(), "{:#?}", src_tree(&h));
        for (path, ms) in [
            ("dst/top/a.txt", 700_000_000_000),
            ("dst/top/sub", 710_000_000_000),
            ("dst/top", 720_000_000_000),
        ] {
            assert_eq!(mtime_of(&h, path), Some(ms), "{path}");
        }
        assert_eq!(mode_of(&h, "dst/top/a.txt"), Some(0o600));
        assert!(leftovers(&work_tree(&h)).is_empty());
        // The folder moved as a whole, as far as the journal is concerned.
        let report = result.report.unwrap();
        assert_eq!(
            report.renamed,
            vec![
                (h.loc("src/top"), h.loc("dst/top")),
                (h.loc("src/loose"), h.loc("dst/loose"))
            ]
        );
        assert_eq!(report.progress.items_done, report.progress.items_total);
        assert_eq!(report.progress.bytes_done, report.progress.bytes_total);
    }
}

#[test]
fn a_cross_volume_move_removes_each_source_item_after_its_copy_not_all_at_the_end() {
    // Looks at both sides at every progress report.
    struct Watch<'a> {
        provider: &'a dyn Provider,
        src: VfsPath,
        dst: VfsPath,
        seen: Vec<(usize, usize)>,
    }
    impl ExecSink for Watch<'_> {
        fn progress(&mut self, _: &Progress, _: &Counts) {
            let src = tree_of(self.provider, &self.src);
            let dst = tree_of(self.provider, &self.dst);
            let files = |t: &Tree| t.values().filter(|n| matches!(n, Node::File(_))).count();
            self.seen.push((files(&src), files(&dst)));
        }
    }
    let (h, _dir) = crossing(CaseRule::Sensitive);
    let mut t = tree(&[("top/", "")]);
    for n in 0..6 {
        t.insert(format!("top/f{n}"), Node::File(pattern(3 * SMALL_CHUNK, n)));
    }
    fill(&h, &t);
    let planned = h
        .plan(&req(&h, JobKind::Move, &["src/top"], "dst", None))
        .unwrap();
    let mut watch = Watch {
        provider: h.provider.as_ref(),
        src: h.path("src"),
        dst: h.path("dst"),
        seen: Vec::new(),
    };
    let options = RunOptions {
        chunk_bytes: SMALL_CHUNK,
        ..RunOptions::default()
    };
    Executor::new(h.env.clone())
        .run_with(JobId(1), &planned, &CancelToken::new(), &mut watch, options)
        .unwrap();
    // At every report no file is on neither side or on both, apart from the one being copied (its
    // partial is a file on the destination side, so it may be on both for a moment); and the
    // source falls one file at a time while the destination grows.
    let mut intermediate = 0;
    for (src, dst) in &watch.seen {
        assert!(*src + *dst >= 6, "a file is nowhere: {src} + {dst}");
        assert!(*src + *dst <= 7, "{src} + {dst}");
        if *src > 0 && *src < 6 && *dst > 0 {
            intermediate += 1;
        }
    }
    assert!(intermediate > 5, "{:?}", watch.seen);
    assert_eq!(
        dst_tree(&h),
        t.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Tree>()
    );
}

/// What a source and a destination may look like after a stop: every top-level file is whole in one
/// of them, and nowhere half. `names` are the files of the moved folder.
fn assert_each_item_moved_or_untouched(
    src: &Tree,
    dst: &Tree,
    original: &Tree,
    at: &str,
    gone_means_gone: bool,
) {
    assert!(
        leftovers(src).is_empty() && leftovers(dst).is_empty(),
        "{at}: {src:?} {dst:?}"
    );
    for (key, node) in original {
        let in_src = src.get(key) == Some(node);
        let in_dst = dst.get(key) == Some(node);
        match node {
            Node::Dir => {}
            // A provider that says a source is not found (when it is) is believed: it is gone, so
            // the copy is kept, and the file shows on both sides.
            _ if gone_means_gone => assert!(in_src || in_dst, "{at}: {key} is nowhere"),
            _ => assert!(
                in_src ^ in_dst,
                "{at}: {key} is in the source: {in_src}, in the destination: {in_dst}\nsrc {src:?}\ndst {dst:?}"
            ),
        }
        // Nothing under this name but the whole item.
        for side in [src, dst] {
            if let Some(found) = side.get(key) {
                assert_eq!(found, node, "{at}: {key} is not whole");
            }
        }
    }
    // Nothing is anywhere that was not in the original.
    for key in src.keys().chain(dst.keys()) {
        assert!(original.contains_key(key), "{at}: unexpected {key}");
    }
}

#[test]
fn a_failure_at_any_step_of_a_cross_volume_move_leaves_each_item_moved_or_untouched() {
    for kind in FaultKind::ALL {
        let (mut h, _dir) = crossing(CaseRule::Sensitive);
        let mut t = tree(&[
            ("top/", ""),
            ("top/a", "alpha"),
            ("top/sub/", ""),
            ("top/sub/b", "bravo"),
            ("loose", "l"),
        ]);
        t.insert(
            "top/big".to_owned(),
            Node::File(pattern(3 * SMALL_CHUNK + 5, 1)),
        );
        t.insert("top/ln".to_owned(), Node::Link("a".to_owned()));
        // The first run is clean and counts the calls.
        fill(&h, &t);
        let result = go(
            &mut h,
            JobKind::Move,
            &["src/top", "src/loose"],
            "dst",
            None,
        );
        done(&result);
        let calls = h.provider.calls();
        for step in 1..=calls {
            let (mut h, _dir) = crossing(CaseRule::Sensitive);
            fill(&h, &t);
            h.provider.fail_at(step, kind);
            let result = go(
                &mut h,
                JobKind::Move,
                &["src/top", "src/loose"],
                "dst",
                None,
            );
            h.provider.reset();
            let at = format!("{kind:?} at {step}/{calls}");
            assert_each_item_moved_or_untouched(
                &src_tree(&h),
                &dst_tree(&h),
                &expected_union(&t),
                &at,
                kind == FaultKind::NotFound,
            );
            match &result.state {
                JobState::Done => {}
                JobState::Failed { .. } | JobState::Cancelled => {}
                other => panic!("{at}: {other:?}"),
            }
        }
        let _ = &mut h;
    }
}

/// Every entry a moved tree can show, wherever it is: the original tree under the names the two
/// sides use (`top/...` and `loose` on either side).
fn expected_union(original: &Tree) -> Tree {
    original.clone()
}

#[test]
fn a_cancel_at_any_step_of_a_cross_volume_move_leaves_each_item_moved_or_untouched() {
    let t = {
        let mut t = tree(&[
            ("top/", ""),
            ("top/a", "alpha"),
            ("top/sub/", ""),
            ("top/sub/b", "bravo"),
            ("loose", "l"),
        ]);
        t.insert(
            "top/big".to_owned(),
            Node::File(pattern(3 * SMALL_CHUNK + 5, 1)),
        );
        t
    };
    let (mut h, _dir) = crossing(CaseRule::Sensitive);
    fill(&h, &t);
    let calls = {
        let r = go(
            &mut h,
            JobKind::Move,
            &["src/top", "src/loose"],
            "dst",
            None,
        );
        done(&r);
        h.provider.calls()
    };
    let mut cancelled = 0;
    for step in 1..=calls {
        let (mut h, _dir) = crossing(CaseRule::Sensitive);
        fill(&h, &t);
        let request = req(&h, JobKind::Move, &["src/top", "src/loose"], "dst", None);
        let result = run_transfer(
            &mut h,
            request,
            &mut Answers::default(),
            &small(),
            &mut |h, token| h.provider.cancel_at(step, token),
        );
        h.provider.reset();
        let at = format!("cancel at {step}/{calls}");
        assert_each_item_moved_or_untouched(&src_tree(&h), &dst_tree(&h), &t, &at, false);
        if result.state == JobState::Cancelled {
            cancelled += 1;
            // A cancel that lands while the job is still being planned has no executor failure.
            if let Some(failure) = result.failure {
                assert_eq!(failure.error, OpsError::Cancelled);
            }
        } else {
            assert_eq!(result.state, JobState::Done, "{at}");
        }
    }
    assert!(cancelled > calls / 2, "{cancelled} of {calls}");
}

#[test]
fn a_move_whose_source_will_not_go_leaves_the_item_untouched() {
    let (mut h, _dir) = crossing(CaseRule::Sensitive);
    fill(&h, &tree(&[("a", "alpha"), ("b", "bravo")]));
    // The first removal of a source file is refused.
    h.provider
        .fail_nth(Op::RemoveFile, 1, FaultKind::PermissionDenied);
    let result = go(&mut h, JobKind::Move, &["src/a", "src/b"], "dst", None);
    assert!(matches!(
        error_of(&result.state),
        OpsError::PermissionDenied { .. }
    ));
    h.provider.reset();
    // `a` was copied, then the source would not go, so the copy was taken back: untouched.
    assert_eq!(src_tree(&h), tree(&[("a", "alpha"), ("b", "bravo")]));
    assert!(dst_tree(&h).is_empty());
}

#[test]
fn a_replacing_move_whose_source_will_not_go_puts_the_old_file_back() {
    let (mut h, _dir) = crossing(CaseRule::Sensitive);
    fill(&h, &tree(&[("a", "new")]));
    put_bytes(&h, "dst/a", b"old");
    h.provider.reset();
    h.provider
        .fail_nth(Op::RemoveFile, 1, FaultKind::PermissionDenied);
    let result = go(
        &mut h,
        JobKind::Move,
        &["src/a"],
        "dst",
        Some(ConflictPolicy::Replace),
    );
    assert!(matches!(result.state, JobState::Failed { .. }));
    h.provider.reset();
    assert_eq!(src_tree(&h), tree(&[("a", "new")]));
    assert_eq!(dst_tree(&h), tree(&[("a", "old")]));
}

#[test]
fn a_rename_that_crosses_devices_falls_back_to_copy_and_remove_on_any_provider() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[
                ("src/", ""),
                ("dst/", ""),
                ("src/f", "content"),
                ("src/d/", ""),
                ("src/d/x", "x"),
            ]),
        );
        // The first rename of each item is the move's own.
        h.provider
            .fail_nth(Op::Rename, 1, FaultKind::CrossesDevices);
        let result = go(&mut h, JobKind::Move, &["src/f"], "dst", None);
        done(&result);
        let copied = h.provider.calls_of(Op::OpenRead) + h.provider.calls_of(Op::CopyWithin);
        assert!(copied >= 1, "it copied");
        assert_eq!(dst_tree(&h).get("f"), Some(&file("content")));
        assert!(!src_tree(&h).contains_key("f"));
        h.provider.reset();
        h.provider
            .fail_nth(Op::Rename, 1, FaultKind::CrossesDevices);
        let result = go(&mut h, JobKind::Move, &["src/d"], "dst", None);
        done(&result);
        assert_eq!(dst_tree(&h).get("d/x"), Some(&file("x")));
        assert!(src_tree(&h).is_empty());
    });
}

#[test]
fn a_folder_merged_by_a_move_is_removed_only_when_everything_in_it_went() {
    for crossing_volumes in [false, true] {
        let (mut h, _dir) = if crossing_volumes {
            crossing(CaseRule::Sensitive)
        } else {
            let (h, d) = memory_harness(CaseRule::Sensitive);
            h.provider.create_dir(&h.path("src")).unwrap();
            h.provider.create_dir(&h.path("dst")).unwrap();
            (h, d)
        };
        fill(
            &h,
            &tree(&[
                ("p/", ""),
                ("p/new", "n"),
                ("p/same", "from-src"),
                ("p/sub/", ""),
                ("p/sub/x", "x"),
            ]),
        );
        populate(
            h.provider.as_ref(),
            &h.path("dst"),
            &tree(&[
                ("p/", ""),
                ("p/same", "from-dst"),
                ("p/sub/", ""),
                ("p/sub/y", "y"),
            ]),
        );
        h.provider.reset();
        // Skip the one clash: `p/same` stays in the source, so the source folders stay.
        let result = go(
            &mut h,
            JobKind::Move,
            &["src/p"],
            "dst",
            Some(ConflictPolicy::Skip),
        );
        // Skip, as the policy for all, also skips the folder clash: the merge asks about folders
        // only when told to merge, so nothing moved at all.
        done(&result);
        assert_eq!(result.report.as_ref().unwrap().counts.skipped, 1);
        assert_eq!(src_tree(&h).len(), 5);

        let asked = Rc::new(RefCell::new(0));
        let counted = asked.clone();
        let mut answers = Answers {
            conflicts: Box::new(move |conflicts| {
                *counted.borrow_mut() += 1;
                // For this file only.
                vec![Resolution {
                    source: Some(conflicts[0].source.clone()),
                    policy: ConflictPolicy::Skip,
                }]
            }),
            errors: Box::new(|_, _| None),
        };
        // Merge folders; the file clash is asked about and answered with Skip.
        let request = req(
            &h,
            JobKind::Move,
            &["src/p"],
            "dst",
            Some(ConflictPolicy::MergeFolders),
        );
        let result = run(&mut h, request, &mut answers);
        done(&result);
        assert_eq!(*asked.borrow(), 1);
        assert_eq!(
            dst_tree(&h),
            tree(&[
                ("p/", ""),
                ("p/same", "from-dst"),
                ("p/new", "n"),
                ("p/sub/", ""),
                ("p/sub/y", "y"),
                ("p/sub/x", "x"),
            ])
        );
        // What was skipped is still in the source, in its folder; what moved is gone.
        assert_eq!(src_tree(&h), tree(&[("p/", ""), ("p/same", "from-src")]));
        assert_eq!(result.report.unwrap().counts.skipped, 1);

        // Replace the one left: now the folder is empty and is removed too.
        let request = req(
            &h,
            JobKind::Move,
            &["src/p"],
            "dst",
            Some(ConflictPolicy::MergeFolders),
        );
        let mut replace = Answers::always(Some(ConflictPolicy::Replace), None);
        let result = run(&mut h, request, &mut replace);
        done(&result);
        assert_eq!(dst_tree(&h).get("p/same"), Some(&file("from-src")));
        assert!(src_tree(&h).is_empty(), "{:?}", src_tree(&h));
    }
}

#[test]
fn a_moved_link_stays_a_link_across_volumes() {
    let (mut h, _dir) = crossing(CaseRule::Sensitive);
    let mut t = tree(&[("real/", ""), ("real/x", "x")]);
    t.insert("ln".to_owned(), Node::Link("real".to_owned()));
    t.insert("dangling".to_owned(), Node::Link("../nowhere".to_owned()));
    fill(&h, &t);
    let result = go(
        &mut h,
        JobKind::Move,
        &["src/ln", "src/dangling"],
        "dst",
        None,
    );
    done(&result);
    assert_eq!(dst_tree(&h), {
        let mut e = Tree::new();
        e.insert("ln".to_owned(), Node::Link("real".to_owned()));
        e.insert("dangling".to_owned(), Node::Link("../nowhere".to_owned()));
        e
    });
    // The folder the link pointed at is untouched.
    assert_eq!(src_tree(&h), tree(&[("real/", ""), ("real/x", "x")]));
}

#[test]
fn a_folder_moved_by_copy_keeps_the_time_it_had_though_emptying_it_changes_it() {
    // On a real file system taking entries out of a folder changes its time, so the time to give
    // the copy is the one the folder had when the job met it.
    let (mut h, _dir) = local_harness();
    build(
        &h,
        &tree(&[
            ("src/", ""),
            ("src/d/", ""),
            ("src/d/a", "a"),
            ("src/d/sub/", ""),
            ("src/d/sub/b", "b"),
            ("dst/", ""),
        ]),
    );
    set_mtime(&h, "src/d/sub", 600_000_000_000);
    set_mtime(&h, "src/d", 610_000_000_000);
    // The move's own rename of the folder is refused as crossing volumes, so it copies.
    h.provider
        .fail_nth(Op::Rename, 1, FaultKind::CrossesDevices);
    let result = go(&mut h, JobKind::Move, &["src/d"], "dst", None);
    done(&result);
    assert!(src_tree(&h).is_empty());
    assert_eq!(mtime_of(&h, "dst/d"), Some(610_000_000_000));
    assert_eq!(mtime_of(&h, "dst/d/sub"), Some(600_000_000_000));
}

#[test]
fn a_file_changed_while_it_was_copied_is_not_removed() {
    // Edits the source in the middle of its own copy.
    struct Editor<'a> {
        provider: &'a dyn Provider,
        src: VfsPath,
        edited: bool,
    }
    impl ExecSink for Editor<'_> {
        fn progress(&mut self, _: &Progress, _: &Counts) {
            if !self.edited {
                self.edited = true;
                let mut w = self
                    .provider
                    .create_write(&self.src, WriteOptions::truncate())
                    .unwrap();
                std::io::Write::write_all(&mut w, b"edited meanwhile").unwrap();
                w.finish(false).unwrap();
            }
        }
    }
    let (h, _dir) = crossing(CaseRule::Sensitive);
    put_bytes(&h, "src/f", &pattern(4 * SMALL_CHUNK, 3));
    h.provider.reset();
    let planned = h
        .plan(&req(&h, JobKind::Move, &["src/f"], "dst", None))
        .unwrap();
    let mut editor = Editor {
        provider: h.provider.as_ref(),
        src: h.path("src/f"),
        edited: false,
    };
    let options = RunOptions {
        chunk_bytes: SMALL_CHUNK,
        ..RunOptions::default()
    };
    let failure = Executor::new(h.env.clone())
        .run_with(
            JobId(1),
            &planned,
            &CancelToken::new(),
            &mut editor,
            options,
        )
        .unwrap_err();
    assert_eq!(
        failure.error,
        OpsError::ChangedSince {
            location: h.loc("src/f")
        }
    );
    h.provider.reset();
    // The newer content is where it was, and the stale copy is gone.
    assert_eq!(src_tree(&h), tree(&[("f", "edited meanwhile")]));
    assert!(dst_tree(&h).is_empty());
}

#[test]
fn a_source_that_is_gone_when_its_copy_is_done_leaves_the_copy() {
    // Another program deletes the source after it was read: the copy is all that is left, so it
    // stays and the move counts as done.
    let (mut h, _dir) = crossing(CaseRule::Sensitive);
    put_bytes(&h, "src/f", b"precious");
    h.provider.reset();
    let src = h.path("src/f");
    // The removal of the source finds it already gone.
    h.provider
        .fail_always_where(Op::RemoveFile, FaultKind::NotFound, move |p| *p == src);
    let result = go(&mut h, JobKind::Move, &["src/f"], "dst", None);
    done(&result);
    h.provider.reset();
    assert_eq!(dst_tree(&h), tree(&[("f", "precious")]));
}

#[test]
fn a_cross_volume_move_syncs_each_copy_before_its_source_goes_and_a_copy_does_not() {
    let (mut h, _dir) = crossing(CaseRule::Sensitive);
    fill(&h, &tree(&[("a", "alpha"), ("b", "bravo")]));
    let result = go(&mut h, JobKind::Move, &["src/a", "src/b"], "dst", None);
    done(&result);
    // No fast path (it never syncs), and every finished write was synced.
    assert_eq!(h.provider.calls_of(Op::CopyWithin), 0);
    assert_eq!(h.provider.finish_syncs(), vec![true, true]);
    assert_eq!(dst_tree(&h), tree(&[("a", "alpha"), ("b", "bravo")]));

    let (mut h, _dir) = crossing(CaseRule::Sensitive);
    fill(&h, &tree(&[("a", "alpha")]));
    let result = go(&mut h, JobKind::Copy, &["src/a"], "dst", None);
    done(&result);
    assert_eq!(h.provider.finish_syncs(), vec![false]);
}

#[test]
fn a_destination_that_swallowed_bytes_keeps_the_source_of_a_cross_volume_move() {
    let (mut h, _dir) = crossing(CaseRule::Sensitive);
    let original = tree(&[("a", "alpha"), ("b", "bravo")]);
    fill(&h, &original);
    // The first write reports every byte written but keeps half of them.
    h.provider.short_write_nth(1);
    let result = go(&mut h, JobKind::Move, &["src/a", "src/b"], "dst", None);
    assert!(
        matches!(result.state, JobState::Failed { .. }),
        "{:?}",
        result.state
    );
    h.provider.reset();
    // Nothing short was put in place and nothing was removed.
    assert_eq!(src_tree(&h), original);
    assert!(dst_tree(&h).is_empty(), "{:?}", dst_tree(&h));
}

#[test]
fn a_failed_sync_keeps_the_source_of_a_cross_volume_move() {
    let (mut h, _dir) = crossing(CaseRule::Sensitive);
    let original = tree(&[("a", "alpha")]);
    fill(&h, &original);
    h.provider.fail_nth(Op::Finish, 1, FaultKind::StorageFull);
    let result = go(&mut h, JobKind::Move, &["src/a"], "dst", None);
    assert!(matches!(
        error_of(&result.state),
        OpsError::NotEnoughSpace { .. }
    ));
    h.provider.reset();
    assert_eq!(src_tree(&h), original);
    assert!(dst_tree(&h).is_empty());
}
