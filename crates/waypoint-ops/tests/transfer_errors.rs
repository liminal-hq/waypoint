// What a copy or move does with an error on one item (A48): the job waits, the sink answers Retry,
// Skip, Skip all or Cancel, and each means exactly that. Without an answer the job fails at the item.
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

/// Records every error asked about and answers from a list (the last answer repeats).
type Asked = Rc<RefCell<Vec<(Location, OpsError)>>>;

fn script(decisions: Vec<Option<Decision>>) -> (Answers, Asked) {
    let asked: Asked = Rc::default();
    let seen = asked.clone();
    let mut calls = 0usize;
    let answers = Answers {
        conflicts: Box::new(|_| Vec::new()),
        errors: Box::new(move |item, error| {
            seen.borrow_mut().push((item.clone(), error.clone()));
            let answer = decisions[calls.min(decisions.len() - 1)];
            calls += 1;
            answer
        }),
    };
    (answers, asked)
}

fn four_files<P: Provider + 'static>(h: &Harness<P>) {
    build(
        h,
        &tree(&[
            ("src/", ""),
            ("src/a", "A"),
            ("src/b", "B"),
            ("src/c", "C"),
            ("src/d", "D"),
            ("dst/", ""),
        ]),
    );
}

/// The nth open of a source for reading fails, which is the nth file's copy (fast paths off by
/// verifying, which opens every source twice, so use `OpenRead` numbers with verification off by
/// making the copy go through the loop: the memory provider has no fast path unless enabled).
fn fail_read_of_nth_file<P: Provider + 'static>(h: &Harness<P>, n: usize, kind: FaultKind) {
    h.provider.fail_nth(Op::OpenRead, n, kind);
}

#[test]
fn an_unanswered_error_fails_the_job_at_the_item_after_the_ones_before_it() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    four_files(&h);
    fail_read_of_nth_file(&h, 3, FaultKind::PermissionDenied);
    let result = go(
        &mut h,
        JobKind::Copy,
        &["src/a", "src/b", "src/c", "src/d"],
        "dst",
        None,
    );
    match &result.state {
        JobState::Failed { error, item, done } => {
            assert!(matches!(error, OpsError::PermissionDenied { .. }));
            assert_eq!(item.as_ref(), Some(&h.loc("src/c")));
            assert_eq!(*done, 2);
        }
        other => panic!("{other:?}"),
    }
    h.provider.reset();
    assert_eq!(dst_tree(&h), tree(&[("a", "A"), ("b", "B")]));
}

#[test]
fn skip_records_the_item_as_failed_and_goes_on() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    four_files(&h);
    fail_read_of_nth_file(&h, 2, FaultKind::PermissionDenied);
    let (mut answers, asked) = script(vec![Some(Decision::Skip)]);
    let request = req(
        &h,
        JobKind::Copy,
        &["src/a", "src/b", "src/c", "src/d"],
        "dst",
        None,
    );
    let result = run(&mut h, request, &mut answers);
    done(&result);
    assert_eq!(asked.borrow().len(), 1);
    assert_eq!(asked.borrow()[0].0, h.loc("src/b"));
    let report = result.report.unwrap();
    assert_eq!(report.counts.failed, 1);
    assert_eq!(report.counts.skipped, 0);
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(report.skipped[0].0, h.loc("src/b"));
    assert!(matches!(
        report.skipped[0].1,
        OpsError::PermissionDenied { .. }
    ));
    // The progress still reaches the end: the skipped item is counted.
    assert_eq!(report.progress.items_done, 4);
    assert_eq!(report.progress.bytes_done, report.progress.bytes_total);
    h.provider.reset();
    assert_eq!(dst_tree(&h), tree(&[("a", "A"), ("c", "C"), ("d", "D")]));
    // The job went through Waiting with the error and the item.
    let waited = h.events.iter().any(|e| {
        matches!(e, OpsEvent::JobChanged { job, .. } if matches!(
            &job.state,
            JobState::Waiting { reason: WaitReason::Error { item, .. } } if *item == h.loc("src/b")
        ))
    });
    assert!(waited);
}

#[test]
fn retry_attempts_the_item_again_and_a_transient_error_passes() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    four_files(&h);
    fail_read_of_nth_file(&h, 2, FaultKind::Interrupted);
    let (mut answers, asked) = script(vec![Some(Decision::Retry)]);
    let request = req(
        &h,
        JobKind::Copy,
        &["src/a", "src/b", "src/c", "src/d"],
        "dst",
        None,
    );
    let result = run(&mut h, request, &mut answers);
    done(&result);
    assert_eq!(asked.borrow().len(), 1);
    let report = result.report.unwrap();
    assert_eq!(report.counts, Counts::default());
    assert!(report.skipped.is_empty());
    h.provider.reset();
    assert_eq!(
        dst_tree(&h),
        tree(&[("a", "A"), ("b", "B"), ("c", "C"), ("d", "D")])
    );
}

#[test]
fn retry_asks_again_when_the_item_keeps_failing() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    four_files(&h);
    // The item's open fails every time it is tried.
    memory(&h).fail_always(
        MemOp::OpenRead,
        waypoint_protocol::VfsError::PermissionDenied {
            location: h.loc("src/b"),
        },
    );
    let (mut answers, asked) = script(vec![
        Some(Decision::Retry),
        Some(Decision::Retry),
        Some(Decision::Skip),
    ]);
    let request = req(&h, JobKind::Copy, &["src/b"], "dst", None);
    let result = run(&mut h, request, &mut answers);
    done(&result);
    assert_eq!(
        asked.borrow().len(),
        3,
        "tried three times, skipped at the third"
    );
    assert_eq!(result.report.unwrap().counts.failed, 1);
}

#[test]
fn skip_all_skips_the_rest_of_that_kind_and_asks_again_for_another_kind() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    four_files(&h);
    // `a`: permission denied (skip all); `b`: the same kind again, skipped without asking;
    // `c`: another kind, which asks.
    h.provider
        .fail_nth(Op::OpenRead, 1, FaultKind::PermissionDenied);
    h.provider
        .fail_nth(Op::OpenRead, 2, FaultKind::PermissionDenied);
    h.provider.fail_nth(Op::OpenRead, 3, FaultKind::NotFound);
    let (mut answers, asked) = script(vec![Some(Decision::SkipAll), Some(Decision::Skip)]);
    let request = req(
        &h,
        JobKind::Copy,
        &["src/a", "src/b", "src/c", "src/d"],
        "dst",
        None,
    );
    let result = run(&mut h, request, &mut answers);
    done(&result);
    let asked = asked.borrow();
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert_eq!(asked[0].0, h.loc("src/a"));
    assert_eq!(asked[1].0, h.loc("src/c"));
    assert!(matches!(asked[1].1, OpsError::NotFound { .. }));
    let report = result.report.unwrap();
    assert_eq!(report.counts.failed, 3);
    assert_eq!(report.skipped.len(), 3);
    h.provider.reset();
    assert_eq!(dst_tree(&h), tree(&[("d", "D")]));
}

#[test]
fn cancel_stops_cleanly_and_removes_the_partial() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    four_files(&h);
    put_bytes(&h, "src/e", &pattern(5 * SMALL_CHUNK, 0));
    h.provider.reset();
    // The third write of the big file fails; the answer is Cancel.
    h.provider.fail_nth(Op::Write, 3, FaultKind::StorageFull);
    let (mut answers, asked) = script(vec![Some(Decision::Cancel)]);
    let request = req(&h, JobKind::Copy, &["src/a", "src/e", "src/b"], "dst", None);
    let result = run(&mut h, request, &mut answers);
    assert_eq!(result.state, JobState::Cancelled);
    assert_eq!(asked.borrow().len(), 1);
    assert_eq!(
        asked.borrow()[0].1,
        OpsError::NotEnoughSpace { needed: 0, free: 0 }
    );
    h.provider.reset();
    // `a` was done; the big file left no partial and no half file; `b` was never started.
    assert_eq!(dst_tree(&h), tree(&[("a", "A")]));
}

#[test]
fn an_error_inside_a_folder_skips_that_file_and_the_folder_still_arrives() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    build(
        &h,
        &tree(&[
            ("src/", ""),
            ("src/top/", ""),
            ("src/top/a", "A"),
            ("src/top/bad", "BAD"),
            ("src/top/z", "Z"),
            ("dst/", ""),
        ]),
    );
    let (mut answers, asked) = script(vec![Some(Decision::Skip)]);
    // Names are visited in order: a, bad, z.
    h.provider
        .fail_nth(Op::OpenRead, 2, FaultKind::PermissionDenied);
    let request = req(&h, JobKind::Copy, &["src/top"], "dst", None);
    let result = run(&mut h, request, &mut answers);
    done(&result);
    assert_eq!(asked.borrow()[0].0, h.loc("src/top/bad"));
    h.provider.reset();
    assert_eq!(
        dst_tree(&h),
        tree(&[("top/", ""), ("top/a", "A"), ("top/z", "Z")])
    );
    // The folder arrived, though not whole, so it is not counted as done.
    assert_eq!(result.report.unwrap().counts.failed, 1);
    assert_eq!(result.plan.unwrap().items.len(), 1);
}

#[test]
fn a_moved_folder_keeps_what_was_skipped_in_the_source() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    h.provider.create_dir(&h.path("src")).unwrap();
    h.provider.create_dir(&h.path("dst")).unwrap();
    memory(&h).set_volume(&h.path("src"), VolumeId(2));
    populate(
        h.provider.as_ref(),
        &h.path("src"),
        &tree(&[
            ("top/", ""),
            ("top/a", "A"),
            ("top/bad", "BAD"),
            ("top/z", "Z"),
        ]),
    );
    h.provider.reset();
    h.provider
        .fail_nth(Op::OpenRead, 2, FaultKind::PermissionDenied);
    let (mut answers, _) = script(vec![Some(Decision::Skip)]);
    let request = req(&h, JobKind::Move, &["src/top"], "dst", None);
    let result = run(&mut h, request, &mut answers);
    done(&result);
    h.provider.reset();
    assert_eq!(
        dst_tree(&h),
        tree(&[("top/", ""), ("top/a", "A"), ("top/z", "Z")])
    );
    assert_eq!(src_tree(&h), tree(&[("top/", ""), ("top/bad", "BAD")]));
}

#[test]
fn a_folder_that_could_not_be_created_is_one_error_and_its_contents_are_skipped_with_it() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    build(
        &h,
        &tree(&[
            ("src/", ""),
            ("src/top/", ""),
            ("src/top/a", "A"),
            ("src/top/b", "B"),
            ("src/other", "O"),
            ("dst/", ""),
        ]),
    );
    h.provider
        .fail_nth(Op::CreateDir, 1, FaultKind::PermissionDenied);
    let (mut answers, asked) = script(vec![Some(Decision::Skip)]);
    let request = req(&h, JobKind::Copy, &["src/top", "src/other"], "dst", None);
    let result = run(&mut h, request, &mut answers);
    done(&result);
    assert_eq!(asked.borrow().len(), 1);
    h.provider.reset();
    assert_eq!(dst_tree(&h), tree(&[("other", "O")]));
    // Progress counts the folder and what was in it as passed over.
    let report = result.report.unwrap();
    assert_eq!(report.progress.items_done, report.progress.items_total);
}

#[test]
fn every_fault_with_skip_all_leaves_a_tree_with_no_half_files() {
    for kind in FaultKind::ALL {
        let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
        let t = {
            let mut t = tree(&[
                ("src/", ""),
                ("src/top/", ""),
                ("src/top/a", "alpha"),
                ("src/top/sub/", ""),
                ("src/top/sub/b", "bravo"),
                ("dst/", ""),
            ]);
            t.insert(
                "src/top/big".to_owned(),
                Node::File(pattern(4 * SMALL_CHUNK + 1, 3)),
            );
            t.insert("src/top/ln".to_owned(), Node::Link("a".to_owned()));
            t
        };
        build(&h, &t);
        let r = go(&mut h, JobKind::Copy, &["src/top"], "dst", None);
        done(&r);
        let calls = h.provider.calls();
        for step in 1..=calls {
            let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
            build(&h, &t);
            h.provider.fail_at(step, kind);
            let (mut answers, _) = script(vec![Some(Decision::SkipAll)]);
            let request = req(&h, JobKind::Copy, &["src/top"], "dst", None);
            let result = run(&mut h, request, &mut answers);
            h.provider.reset();
            let dst = dst_tree(&h);
            assert!(leftovers(&dst).is_empty(), "{kind:?} {step}: {dst:?}");
            let source = tree_of(h.provider.as_ref(), &h.path("src/top"));
            let original: Tree = t
                .iter()
                .filter_map(|(k, v)| {
                    k.strip_prefix("src/top/")
                        .map(|r| (r.to_owned(), v.clone()))
                })
                .collect();
            assert_eq!(
                source, original,
                "{kind:?} {step}: a copy never touches the source"
            );
            // Whatever arrived is whole.
            for (key, node) in tree_in(&dst, "top/") {
                assert_eq!(original.get(&key), Some(&node), "{kind:?} {step}: {key}");
            }
            assert!(
                matches!(result.state, JobState::Done | JobState::Failed { .. }),
                "{kind:?} {step}: {:?}",
                result.state
            );
        }
    }

    fn tree_in(t: &Tree, prefix: &str) -> Tree {
        t.iter()
            .filter_map(|(k, v)| k.strip_prefix(prefix).map(|r| (r.to_owned(), v.clone())))
            .collect()
    }
}

#[test]
fn a_source_that_vanishes_at_any_step_is_an_error_for_that_item_only() {
    let setup = || {
        let (h, dir) = memory_harness(CaseRule::Sensitive);
        four_files(&h);
        (h, dir)
    };
    let (mut h, _dir) = setup();
    done(&go(
        &mut h,
        JobKind::Copy,
        &["src/a", "src/b", "src/c", "src/d"],
        "dst",
        None,
    ));
    let calls = h.provider.calls();
    let mut errors = 0;
    for step in 1..=calls {
        let (mut h, _dir) = setup();
        h.provider.vanish_at(step, &h.path("src/b"));
        let (mut answers, asked) = script(vec![Some(Decision::Skip)]);
        let request = req(
            &h,
            JobKind::Copy,
            &["src/a", "src/b", "src/c", "src/d"],
            "dst",
            None,
        );
        let result = run(&mut h, request, &mut answers);
        if result.plan.is_none() {
            // It vanished while the job was being planned: the plan refuses, before any write.
            assert!(matches!(error_of(&result.state), OpsError::NotFound { .. }));
            continue;
        }
        done(&result);
        h.provider.reset();
        let now = dst_tree(&h);
        assert!(leftovers(&now).is_empty(), "{step}: {now:?}");
        for (name, content) in [("a", "A"), ("c", "C"), ("d", "D")] {
            assert_eq!(now.get(name), Some(&file(content)), "{step}");
        }
        // `b` either arrived whole (it was read before it vanished) or is an error.
        match now.get("b") {
            Some(node) => assert_eq!(node, &file("B"), "{step}"),
            None => assert_eq!(asked.borrow().len(), 1, "{step}"),
        }
        errors += asked.borrow().len();
        assert!(asked
            .borrow()
            .iter()
            .all(|(item, _)| *item == h.loc("src/b")));
    }
    assert!(errors > 0);
}
