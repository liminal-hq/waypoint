// What the Trash view's operations do: restore (with its clashes and its missing folders), delete
// from the Trash, empty it, and the "older than N days" sweep, over the fake Trash.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;
mod xfer;

use std::cell::RefCell;
use std::rc::Rc;

use xfer::*;

const DAY: i64 = 86_400_000;

fn trashed_request<P: Provider + 'static>(
    h: &Harness<P>,
    kind: JobKind,
    receipts: &[TrashReceipt],
) -> JobRequest {
    JobRequest {
        kind,
        sources: Sources::Locations {
            locations: receipts
                .iter()
                .map(|r| h.trash.trashed_location(r))
                .collect(),
        },
        destination: None,
        name: None,
        options: JobOptions::default(),
        origin_window: "main-1".to_owned(),
    }
}

fn empty_request(older_than_days: Option<u32>) -> JobRequest {
    JobRequest {
        kind: JobKind::EmptyTrash { older_than_days },
        sources: Sources::Locations { locations: vec![] },
        destination: None,
        name: None,
        options: JobOptions::default(),
        origin_window: "app".to_owned(),
    }
}

/// Trashes `names` (relative to the work folder) and returns the receipts, in order.
fn trash_away<P: Provider + 'static>(h: &mut Harness<P>, names: &[&str]) -> Vec<TrashReceipt> {
    let request = h.request(JobKind::Trash, names, None, None);
    let result = run_plain(h, request);
    done(&result);
    result.report.unwrap().trashed
}

#[test]
fn restore_with_a_free_name_puts_everything_back() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        let source = tree(&[("a.txt", "x"), ("d/", ""), ("d/in", "y")]);
        build(&h, &source);
        let receipts = trash_away(&mut h, &["a.txt", "d"]);
        let request = trashed_request(&h, JobKind::Restore, &receipts);
        let planned = h.plan(&request).unwrap();
        assert!(planned.conflicts.is_empty());
        let result = run_plain(&mut h, request);
        done(&result);
        assert_eq!(result.report.unwrap().restored.len(), 2);
        assert_eq!(work_tree(&h), source);
        assert!(h.trash.is_empty());
    });
}

#[test]
fn a_taken_name_is_a_conflict_the_job_waits_on_and_nothing_is_written() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "old")]));
        let receipts = trash_away(&mut h, &["a"]);
        build(&h, &tree(&[("a", "new")]));
        h.provider.reset();
        let request = trashed_request(&h, JobKind::Restore, &receipts);
        let result = run_plain(&mut h, request);
        let JobState::Waiting {
            reason: WaitReason::Conflicts { conflicts },
        } = &result.state
        else {
            panic!("{:?}", result.state);
        };
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].kind, ConflictKind::FileOverFile);
        assert_eq!(conflicts[0].source, h.trash.trashed_location(&receipts[0]));
        assert_eq!(conflicts[0].existing, h.loc("a"));
        assert_eq!(conflicts[0].name, "a");
        assert_eq!(h.provider.write_calls(), 0);
        assert_eq!(work_tree(&h), tree(&[("a", "new")]));
        assert_eq!(h.trash.len(), 1);
    });
}

#[test]
fn keep_both_restores_under_a_free_name() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a.txt", "old"), ("d/", "")]));
        let receipts = trash_away(&mut h, &["a.txt", "d"]);
        build(
            &h,
            &tree(&[("a.txt", "new"), ("a (2).txt", "other"), ("d/", "")]),
        );
        let mut answers = Answers::always(Some(ConflictPolicy::KeepBoth), None);
        let request = trashed_request(&h, JobKind::Restore, &receipts);
        let result = run(&mut h, request, &mut answers);
        done(&result);
        let restored = result.report.unwrap().restored;
        assert_eq!(restored, vec![h.loc("a (3).txt"), h.loc("d (2)")]);
        let t = work_tree(&h);
        assert_eq!(t.get("a.txt"), Some(&file("new")));
        assert_eq!(t.get("a (2).txt"), Some(&file("other")));
        assert_eq!(t.get("a (3).txt"), Some(&file("old")));
        assert_eq!(t.get("d (2)"), Some(&Node::Dir));
        assert!(h.trash.is_empty());
    });
}

#[test]
fn skip_leaves_the_item_in_the_trash() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "old"), ("b", "bee")]));
        let receipts = trash_away(&mut h, &["a", "b"]);
        build(&h, &tree(&[("a", "new")]));
        let mut answers = Answers::always(Some(ConflictPolicy::Skip), None);
        let request = trashed_request(&h, JobKind::Restore, &receipts);
        let result = run(&mut h, request, &mut answers);
        done(&result);
        let report = result.report.unwrap();
        assert_eq!(report.restored, vec![h.loc("b")]);
        assert_eq!(report.skipped.len(), 1);
        let t = work_tree(&h);
        assert_eq!(t.get("a"), Some(&file("new")));
        assert_eq!(t.get("b"), Some(&file("bee")));
        assert_eq!(h.trash.len(), 1);
    });
}

#[test]
fn replace_swaps_a_file_in_and_leaves_nothing_aside() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "old")]));
        let receipts = trash_away(&mut h, &["a"]);
        build(&h, &tree(&[("a", "new")]));
        let mut answers = Answers::always(Some(ConflictPolicy::Replace), None);
        let request = trashed_request(&h, JobKind::Restore, &receipts);
        let result = run(&mut h, request, &mut answers);
        done(&result);
        let t = work_tree(&h);
        assert_eq!(t, tree(&[("a", "old")]));
        assert!(leftovers(&t).is_empty());
        assert!(h.trash.is_empty());
    });
}

#[test]
fn replace_refuses_a_folder_and_a_mixed_clash_and_changes_nothing() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("d/", ""), ("d/in", "x"), ("f", "file")]));
        let receipts = trash_away(&mut h, &["d", "f"]);
        // A folder where a folder was, and a folder where the file was.
        build(
            &h,
            &tree(&[("d/", ""), ("d/mine", "m"), ("f/", ""), ("f/mine", "m")]),
        );
        let before = work_tree(&h);
        let mut answers = Answers::always(Some(ConflictPolicy::Replace), None);
        let request = trashed_request(&h, JobKind::Restore, &receipts);
        let result = run(&mut h, request, &mut answers);
        assert!(
            matches!(error_of(&result.state), OpsError::CannotReplace { .. }),
            "{:?}",
            result.state
        );
        assert_eq!(work_tree(&h), before);
        assert_eq!(h.trash.len(), 2);
    });
}

#[test]
fn a_failed_replace_puts_the_original_back() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "old")]));
        let receipts = trash_away(&mut h, &["a"]);
        build(&h, &tree(&[("a", "new")]));
        // The first rename sets the file aside; the second is the restore, which fails.
        h.provider.reset();
        h.provider
            .fail_nth(Op::Rename, 2, FaultKind::PermissionDenied);
        let mut answers = Answers::always(Some(ConflictPolicy::Replace), None);
        let request = trashed_request(&h, JobKind::Restore, &receipts);
        let result = run(&mut h, request, &mut answers);
        assert!(matches!(
            error_of(&result.state),
            OpsError::PermissionDenied { .. }
        ));
        let t = work_tree(&h);
        assert_eq!(t, tree(&[("a", "new")]), "the file is back, nothing aside");
        assert_eq!(h.trash.len(), 1);
    });
}

#[test]
fn an_item_that_left_the_trash_during_a_replace_is_not_found() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "old")]));
        let receipts = trash_away(&mut h, &["a"]);
        build(&h, &tree(&[("a", "new")]));
        let request = trashed_request(&h, JobKind::Restore, &receipts);
        h.trash.forget(&receipts[0]);
        let result = run_plain(&mut h, request);
        assert!(matches!(error_of(&result.state), OpsError::NotFound { .. }));
        assert_eq!(work_tree(&h), tree(&[("a", "new")]));
    });
}

#[test]
fn a_clash_that_appears_after_planning_is_asked_about_one_by_one() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "old")]));
        let receipts = trash_away(&mut h, &["a"]);
        let request = trashed_request(&h, JobKind::Restore, &receipts);
        let asked = Rc::new(RefCell::new(0));
        let counter = asked.clone();
        let mut answers = Answers {
            conflicts: Box::new(move |_| {
                *counter.borrow_mut() += 1;
                vec![Resolution {
                    source: None,
                    policy: ConflictPolicy::KeepBoth,
                }]
            }),
            errors: Box::new(|_, _| None),
        };
        // The name is taken after the plan found it free, before the job runs.
        let planned = h.plan(&request).unwrap();
        assert!(planned.conflicts.is_empty());
        build(&h, &tree(&[("a", "late")]));
        let result = run(&mut h, request, &mut answers);
        done(&result);
        assert_eq!(*asked.borrow(), 1);
        let t = work_tree(&h);
        assert_eq!(t.get("a"), Some(&file("late")));
        assert_eq!(t.get("a (2)"), Some(&file("old")));
    });
}

#[test]
fn two_items_from_one_place_restore_one_then_ask_about_the_other() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "first")]));
        let mut receipts = trash_away(&mut h, &["a"]);
        build(&h, &tree(&[("a", "second")]));
        receipts.extend(trash_away(&mut h, &["a"]));
        let mut answers = Answers::always(Some(ConflictPolicy::KeepBoth), None);
        let request = trashed_request(&h, JobKind::Restore, &receipts);
        let result = run(&mut h, request, &mut answers);
        done(&result);
        let t = work_tree(&h);
        assert_eq!(t.get("a"), Some(&file("first")));
        assert_eq!(t.get("a (2)"), Some(&file("second")));
    });
}

#[test]
fn a_missing_folder_is_a_typed_error_the_job_can_be_told_to_make() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[("deep/", ""), ("deep/er/", ""), ("deep/er/f", "x")]),
        );
        let receipts = trash_away(&mut h, &["deep/er/f"]);
        // Nothing asked: the job stops with the typed error naming the folder that is gone.
        let remove = |h: &Harness<_>| {
            h.provider.remove_dir(&h.path("deep/er")).unwrap();
            h.provider.remove_dir(&h.path("deep")).unwrap();
        };
        remove(&h);
        let request = trashed_request(&h, JobKind::Restore, &receipts);
        let result = run_plain(&mut h, request.clone());
        assert_eq!(
            error_of(&result.state),
            &OpsError::OriginMissingParent {
                location: h.loc("deep/er")
            }
        );
        assert_eq!(h.trash.len(), 1);

        // Told to make the folders, it makes every missing one and restores.
        let mut answers = Answers::always(None, Some(Decision::CreateParents));
        let result = run(&mut h, request, &mut answers);
        done(&result);
        assert_eq!(
            work_tree(&h),
            tree(&[("deep/", ""), ("deep/er/", ""), ("deep/er/f", "x")])
        );
        assert!(h.trash.is_empty());
    });
}

#[test]
fn skipping_a_restore_that_needs_a_missing_folder_leaves_it_in_the_trash() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("d/", ""), ("d/f", "x"), ("g", "y")]));
        let receipts = trash_away(&mut h, &["d/f", "g"]);
        h.provider.remove_dir(&h.path("d")).unwrap();
        let mut answers = Answers::always(None, Some(Decision::Skip));
        let request = trashed_request(&h, JobKind::Restore, &receipts);
        let result = run(&mut h, request, &mut answers);
        done(&result);
        assert_eq!(work_tree(&h), tree(&[("g", "y")]));
        assert_eq!(h.trash.len(), 1);
    });
}

#[test]
fn a_restore_of_an_item_that_left_the_trash_is_not_found_at_planning() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "x")]));
        let receipts = trash_away(&mut h, &["a"]);
        h.trash.forget(&receipts[0]);
        let request = trashed_request(&h, JobKind::Restore, &receipts);
        let result = run_plain(&mut h, request);
        assert!(matches!(error_of(&result.state), OpsError::NotFound { .. }));
    });
}

#[test]
fn delete_in_the_trash_removes_the_item_for_good() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[("a", "x"), ("d/", ""), ("d/in", "y"), ("keep", "k")]),
        );
        let receipts = trash_away(&mut h, &["a", "d", "keep"]);
        let request = trashed_request(&h, JobKind::Delete, &receipts[..2]);
        let planned = h.plan(&request).unwrap();
        assert_eq!(planned.total_items, 2);
        let result = run_plain(&mut h, request);
        done(&result);
        let report = result.report.unwrap();
        assert_eq!(report.deleted.len(), 2);
        assert!(report.restored.is_empty());
        assert_eq!(h.trash.len(), 1);
        assert_eq!(h.trash.receipts(), vec![receipts[2].clone()]);
        // Nothing came back, and nothing can be undone from it.
        assert!(work_tree(&h).is_empty());
        assert!(report.inverse.is_empty());
    });
}

#[test]
fn delete_in_the_trash_of_a_missing_item_fails_with_not_found() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "x")]));
        let receipts = trash_away(&mut h, &["a"]);
        let request = trashed_request(&h, JobKind::Delete, &receipts);
        h.trash.forget(&receipts[0]);
        let result = run_plain(&mut h, request);
        assert!(matches!(error_of(&result.state), OpsError::NotFound { .. }));
    });
}

#[test]
fn delete_still_removes_ordinary_files_for_good() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "x"), ("t", "y")]));
        let receipts = trash_away(&mut h, &["t"]);
        let mut request = h.request(JobKind::Delete, &["a"], None, None);
        if let Sources::Locations { locations } = &mut request.sources {
            locations.push(h.trash.trashed_location(&receipts[0]));
        }
        let result = run_plain(&mut h, request);
        done(&result);
        assert!(work_tree(&h).is_empty());
        assert!(h.trash.is_empty());
    });
}

#[test]
fn emptying_the_trash_removes_everything_and_says_how_many() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "x"), ("b", "y"), ("c/", "")]));
        trash_away(&mut h, &["a", "b", "c"]);
        let planned = h.plan(&empty_request(None)).unwrap();
        assert!(planned.items.is_empty());
        let result = run_plain(&mut h, empty_request(None));
        done(&result);
        let report = result.report.unwrap();
        assert_eq!(report.emptied, 3);
        assert!(h.trash.is_empty());
        assert!(report.inverse.is_empty());
        // An empty Trash is a job that did nothing, not a failure.
        let again = run_plain(&mut h, empty_request(None));
        done(&again);
        assert_eq!(again.report.unwrap().emptied, 0);
    });
}

#[test]
fn the_sweep_removes_only_items_that_have_been_there_long_enough() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("old", "x"), ("middle", "y"), ("new", "z")]));
        trash_away(&mut h, &["old"]);
        h.clock.advance(20 * DAY);
        trash_away(&mut h, &["middle"]);
        h.clock.advance(5 * DAY);
        let kept = trash_away(&mut h, &["new"]);
        h.clock.advance(DAY);
        // "old" is 26 days in, "middle" 6, "new" 1.
        let result = run_plain(&mut h, empty_request(Some(30)));
        done(&result);
        assert_eq!(result.report.unwrap().emptied, 0);
        let result = run_plain(&mut h, empty_request(Some(26)));
        done(&result);
        assert_eq!(result.report.unwrap().emptied, 1, "exactly 26 days counts");
        let result = run_plain(&mut h, empty_request(Some(2)));
        done(&result);
        assert_eq!(result.report.unwrap().emptied, 1);
        assert_eq!(h.trash.receipts(), kept);
    });
}

#[test]
fn emptying_needs_a_trash() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        h.trash
            .set_available(Err("the Trash is not browsable here".to_owned()));
        let result = run_plain(&mut h, empty_request(None));
        assert!(matches!(
            error_of(&result.state),
            OpsError::TrashUnavailable { .. }
        ));
    });
}

#[test]
fn the_new_kinds_and_settings_serialise_for_the_wire() {
    assert_eq!(
        serde_json::to_string(&JobKind::EmptyTrash {
            older_than_days: Some(30)
        })
        .unwrap(),
        r#"{"kind":"emptyTrash","olderThanDays":30}"#
    );
    assert_eq!(
        serde_json::to_string(&Decision::CreateParents).unwrap(),
        r#""createParents""#
    );
    // Settings saved before the sweep existed still load, with the sweep off.
    let old = r#"{"concurrency":2,"verifyAfterCopy":false,"verifyAlgorithm":"blake3","confirmTrash":false,"undoDepth":50}"#;
    let settings: OpsSettings = serde_json::from_str(old).unwrap();
    assert_eq!(settings.trash_expiry_days, None);
}
