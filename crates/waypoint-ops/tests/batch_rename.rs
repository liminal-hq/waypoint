// A batch rename through the whole engine, on the local provider and on the in-memory provider
// under both case rules: swaps, chains and rotations reach their names, a clash refuses the whole
// job before anything is written, a failure or a cancel at any step leaves the original tree,
// a crash loses nothing, and one undo restores every name.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;
mod journal_support;

use std::collections::BTreeMap;
use std::time::{Duration, UNIX_EPOCH};

use common::*;
use journal_support::*;
use waypoint_ops::testing::faulty::FaultKind;
use waypoint_vfs::FileTimes;

const NOW: i64 = 1_700_000_000_000;

fn spec(rules: Vec<RenameRule>) -> RenameSpec {
    RenameSpec {
        rules,
        utc_offset_minutes: 0,
        now_ms: Some(NOW),
    }
}

fn batch<P: Provider + 'static>(
    h: &Harness<P>,
    sources: &[&str],
    rules: Vec<RenameRule>,
) -> JobRequest {
    let mut request = h.request(JobKind::BatchRename, sources, None, None);
    request.rename = Some(spec(rules));
    request
}

/// Names the entries `start`, `start + 1`, … in the order they are selected, keeping extensions.
fn number(start: u32) -> RenameRule {
    RenameRule::Counter {
        start,
        step: 1,
        width: 0,
        position: RulePosition::ReplaceStem,
        separator: String::new(),
    }
}

fn replace(find: &str, with: &str) -> RenameRule {
    RenameRule::FindReplace {
        find: find.to_owned(),
        replace: with.to_owned(),
        regex: false,
        case_sensitive: true,
        scope: RenameScope::Stem,
        all: true,
    }
}

fn upper() -> RenameRule {
    RenameRule::Case {
        mode: CaseMode::Upper,
        scope: RenameScope::Stem,
    }
}

/// The tree with the given top-level entries moved to new names (all at once).
fn renamed(start: &Tree, moves: &[(&str, &str)]) -> Tree {
    let mut out = start.clone();
    let mut taken = Vec::new();
    for (from, _) in moves {
        let prefix = format!("{from}/");
        for key in start
            .keys()
            .filter(|k| *k == from || k.starts_with(&prefix))
        {
            taken.push((key.clone(), out.remove(key).unwrap()));
        }
    }
    for (key, node) in taken {
        let (from, to) = moves
            .iter()
            .find(|(from, _)| key == *from || key.starts_with(&format!("{from}/")))
            .unwrap();
        out.insert(format!("{to}{}", &key[from.len()..]), node);
    }
    out
}

fn ok<P: Provider + 'static>(request: JobRequest, h: &mut Harness<P>) -> ExecReport {
    let result = h.run(request);
    assert_eq!(result.state, JobState::Done, "{:?}", result.failure);
    assert!(h.store.violations().is_empty());
    assert!(partials(&work_tree(h)).is_empty());
    result.report.expect("a finished job reports")
}

#[test]
fn a_swap_a_chain_and_a_rotation_reach_their_names() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        let start = tree(&[("1", "one"), ("2", "two"), ("3", "three"), ("keep", "k")]);
        build(&h, &start);
        // The selection order drives the numbering, so each case below is a permutation.
        type Case<'a> = (&'a [&'a str], u32, Vec<(&'a str, &'a str)>, usize);
        let cases: [Case; 3] = [
            // Swap: 2 becomes 1 and 1 becomes 2, through a temporary name.
            (&["2", "1"], 1, vec![("2", "1"), ("1", "2")], 3),
            // Chain: 1 becomes 2, 2 becomes 3, 3 becomes 4; nothing waits on a cycle.
            (
                &["1", "2", "3"],
                2,
                vec![("1", "2"), ("2", "3"), ("3", "4")],
                3,
            ),
            // Rotation: 2 becomes 1, 3 becomes 2, 1 becomes 3.
            (
                &["2", "3", "1"],
                1,
                vec![("2", "1"), ("3", "2"), ("1", "3")],
                4,
            ),
        ];
        for (sources, first, moves, steps) in cases {
            reset_tree(&mut h, &start);
            let expected = renamed(&start, &moves);
            let report = ok(batch(&h, sources, vec![number(first)]), &mut h);
            assert_eq!(work_tree(&h), expected, "{sources:?}");
            assert_eq!(report.renamed.len(), sources.len());
            assert_eq!(report.inverse.len(), steps, "{sources:?}");
        }
    });
}

#[test]
fn a_case_only_change_goes_through_a_temporary_name_where_it_must() {
    each_provider!(|h, rule, links| {
        let _ = links;
        let start = tree(&[("readme", "r"), ("Dir/", ""), ("Dir/f", "x"), ("keep", "k")]);
        build(&h, &start);
        let report = ok(batch(&h, &["readme", "Dir"], vec![upper()]), &mut h);
        let expected = renamed(&start, &[("readme", "README"), ("Dir", "DIR")]);
        assert_eq!(work_tree(&h), expected);
        // Under the insensitive rule the two spellings are one name: two steps each.
        let steps = if rule == CaseRule::Insensitive { 4 } else { 2 };
        assert_eq!(report.inverse.len(), steps);
    });
}

#[test]
fn a_clash_refuses_the_whole_job_and_writes_nothing() {
    each_provider!(|h, rule, links| {
        let _ = links;
        build(
            &h,
            &tree(&[
                ("a.txt", "1"),
                ("b.txt", "2"),
                ("keep.txt", "k"),
                ("c", "3"),
            ]),
        );
        let before = work_tree(&h);
        let refuse = |request: JobRequest, h: &mut Harness<_>| {
            let result = h.run(request);
            assert_eq!(h.provider.write_calls(), 0, "a refused job writes nothing");
            assert_eq!(work_tree(h), before);
            error_of(&result.state).clone()
        };
        // Takes the name of an entry that stays.
        let error = refuse(batch(&h, &["a.txt"], vec![replace("a", "keep")]), &mut h);
        assert!(matches!(error, OpsError::NameInUse { .. }), "{error:?}");
        // Two entries get one name.
        let error = refuse(
            batch(
                &h,
                &["a.txt", "b.txt"],
                vec![replace("a", "z"), replace("b", "z")],
            ),
            &mut h,
        );
        assert!(matches!(error, OpsError::NameInUse { .. }), "{error:?}");
        // Not a name at all.
        let error = refuse(batch(&h, &["a.txt"], vec![replace("a", "x/y")]), &mut h);
        assert!(matches!(error, OpsError::InvalidName { .. }), "{error:?}");
        // The rules change nothing.
        let error = refuse(batch(&h, &["a.txt"], vec![replace("zzz", "y")]), &mut h);
        assert!(matches!(error, OpsError::InvalidName { .. }), "{error:?}");
        // A pattern that does not compile.
        let broken = RenameRule::FindReplace {
            find: "(".to_owned(),
            replace: String::new(),
            regex: true,
            case_sensitive: true,
            scope: RenameScope::Stem,
            all: true,
        };
        let error = refuse(batch(&h, &["a.txt"], vec![broken]), &mut h);
        assert!(matches!(error, OpsError::InvalidName { .. }), "{error:?}");
        // No rules at all.
        let mut request = batch(&h, &["a.txt"], vec![]);
        let error = refuse(request.clone(), &mut h);
        assert!(matches!(error, OpsError::InvalidName { .. }), "{error:?}");
        request.rename = None;
        let error = refuse(request, &mut h);
        assert!(matches!(error, OpsError::Io { .. }), "{error:?}");
        // A name only Windows refuses is refused only where the provider is Windows-like.
        let reserved = h.run(batch(&h, &["c"], vec![replace("c", "CON")]));
        if rule == CaseRule::Insensitive {
            assert!(matches!(
                error_of(&reserved.state),
                OpsError::InvalidName { .. }
            ));
        } else {
            assert_eq!(reserved.state, JobState::Done);
        }
    });
}

#[test]
fn entries_in_several_folders_are_judged_each_in_its_own() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        let start = tree(&[
            ("d1/", ""),
            ("d2/", ""),
            ("d1/x", "x1"),
            ("d2/x", "x2"),
            ("d2/1", "taken"),
        ]);
        build(&h, &start);
        // The first selected entry is numbered 1, which d2 already holds.
        let error = {
            let result = h.run(batch(&h, &["d2/x", "d1/x"], vec![number(1)]));
            error_of(&result.state).clone()
        };
        assert!(matches!(error, OpsError::NameInUse { .. }), "{error:?}");
        assert_eq!(work_tree(&h), start);
        // Starting at 5 numbers 5 and 6, which are free in both folders.
        ok(batch(&h, &["d2/x", "d1/x"], vec![number(5)]), &mut h);
        assert_eq!(
            work_tree(&h),
            renamed_in(&start, &[("d2/x", "d2/5"), ("d1/x", "d1/6")])
        );
    });
}

#[test]
fn a_folder_and_something_inside_it_cannot_be_renamed_together() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        let start = tree(&[("d/", ""), ("d/x", "1"), ("keep", "k")]);
        build(&h, &start);
        let both = batch(
            &h,
            &["d", "d/x"],
            vec![replace("d", "e"), replace("x", "y")],
        );

        // The preview says so on the inner entry, and is not ready.
        let cancel = CancelToken::new();
        let preview = waypoint_ops::preview_batch(&both, &h.plan_ctx(&cancel)).unwrap();
        assert_eq!(preview.rows[0].problems, vec![]);
        assert_eq!(
            preview.rows[1].problems,
            vec![Problem::NestedSelection { with: 0 }]
        );
        assert_eq!(preview.problems, 1);
        assert!(!preview.ready());

        // The job is refused before anything is written, where it used to fail at its second step.
        let result = h.run(both);
        assert!(matches!(
            error_of(&result.state),
            OpsError::InvalidName { .. }
        ));
        assert_eq!(h.provider.write_calls(), 0);
        assert_eq!(work_tree(&h), start);

        // The folder staying as it is, the entry inside it can be renamed with it selected.
        ok(batch(&h, &["d", "d/x"], vec![replace("x", "y")]), &mut h);
        assert_eq!(work_tree(&h), renamed(&start, &[("d/x", "d/y")]));
    });
}

fn renamed_in(start: &Tree, moves: &[(&str, &str)]) -> Tree {
    let mut out = start.clone();
    for (from, to) in moves {
        let node = out.remove(*from).unwrap();
        out.insert((*to).to_owned(), node);
    }
    out
}

#[test]
fn dates_come_from_the_entries_and_from_the_time_given() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a.txt", "1"), ("b.txt", "2")]));
        // 2001-09-09 01:46:40 UTC and 2023-11-14 22:13:20 UTC.
        for (name, secs) in [("a.txt", 1_000_000_000u64), ("b.txt", 1_700_000_000)] {
            h.provider
                .set_times(
                    &h.path(name),
                    FileTimes {
                        accessed: None,
                        modified: Some(UNIX_EPOCH + Duration::from_secs(secs)),
                    },
                )
                .unwrap();
        }
        let modified = RenameRule::DateToken {
            source: DateSource::Modified,
            format: "%Y-%m-%d".to_owned(),
            position: RulePosition::Prefix,
            separator: "_".to_owned(),
        };
        ok(batch(&h, &["a.txt", "b.txt"], vec![modified]), &mut h);
        let names: Vec<String> = work_tree(&h).keys().cloned().collect();
        assert_eq!(names, ["2001-09-09_a.txt", "2023-11-14_b.txt"]);
        let today = RenameRule::DateToken {
            source: DateSource::Today,
            format: "%Y%m%d%H%M".to_owned(),
            position: RulePosition::Suffix,
            separator: "-".to_owned(),
        };
        let mut request = batch(&h, &["2001-09-09_a.txt"], vec![today]);
        request.rename.as_mut().unwrap().utc_offset_minutes = 120;
        ok(request, &mut h);
        assert!(work_tree(&h).contains_key("2001-09-09_a-202311150013.txt"));
    });
}

fn start_tree() -> Tree {
    tree(&[
        ("1", "one"),
        ("2", "two"),
        ("3", "three"),
        ("x.txt", "ex"),
        ("D/", ""),
        ("D/f", "f"),
    ])
}

/// The requests the fault and cancel tests run: a rotation, a case-only change and a chain.
fn scenarios<P: Provider + 'static>(h: &Harness<P>) -> Vec<(&'static str, JobRequest)> {
    vec![
        ("rotation", batch(h, &["2", "3", "1"], vec![number(1)])),
        ("chain", batch(h, &["1", "2", "3"], vec![number(2)])),
        ("case-only", batch(h, &["x.txt", "D"], vec![upper()])),
    ]
}

#[test]
fn a_failure_at_any_step_leaves_the_original_tree() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &start_tree());
        let names: Vec<&str> = scenarios(&h).iter().map(|(n, _)| *n).collect();
        for (at, name) in names.iter().enumerate() {
            // How many calls the job makes from the first call of planning to the end.
            reset_tree(&mut h, &start_tree());
            let request = scenarios(&h).remove(at).1;
            let clean = h.run(request);
            assert_eq!(clean.state, JobState::Done, "{name}");
            let total = h.provider.calls();
            let finished = work_tree(&h);
            for call in 1..=total {
                for kind in [
                    FaultKind::PermissionDenied,
                    FaultKind::Interrupted,
                    FaultKind::NotFound,
                ] {
                    reset_tree(&mut h, &start_tree());
                    let request = scenarios(&h).remove(at).1;
                    h.provider.fail_at(call, kind);
                    let result = h.run(request);
                    let tree_now = work_tree(&h);
                    assert!(
                        partials(&tree_now).is_empty(),
                        "{name} call {call} {kind:?}: a temporary name is left"
                    );
                    match result.state {
                        JobState::Done => {
                            assert_eq!(tree_now, finished, "{name} call {call} {kind:?}")
                        }
                        JobState::Failed { .. } => {
                            assert_eq!(
                                tree_now,
                                start_tree(),
                                "{name} call {call} {kind:?}: rolled back"
                            );
                            if let Some(failure) = result.failure {
                                assert!(failure.report.inverse.is_empty());
                                assert!(failure.report.renamed.is_empty());
                            }
                        }
                        other => panic!("{name} call {call} {kind:?}: {other:?}"),
                    }
                }
            }
        }
    });
}

/// Empties the work folder and builds `start` again.
fn reset_tree<P: Provider + 'static>(h: &mut Harness<P>, start: &Tree) {
    h.provider.reset();
    let keys: Vec<String> = work_tree(h).keys().cloned().collect();
    for key in keys.iter().rev() {
        let path = h.path(key);
        if let Ok(entry) = h.provider.stat(&path) {
            if entry.kind == waypoint_vfs::EntryKind::Directory {
                h.provider.remove_dir(&path).unwrap();
            } else {
                h.provider.remove_file(&path).unwrap();
            }
        }
    }
    build(h, start);
}

#[test]
fn a_failure_while_rolling_back_reports_exactly_what_is_left_and_undo_finishes_it() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        let start = start_tree();
        build(&h, &start);
        let probe = h.run(batch(&h, &["2", "3", "1"], vec![number(1)]));
        assert_eq!(probe.state, JobState::Done);
        let total = h.provider.calls();
        let mut leftovers = 0;
        for call in 1..=total {
            reset_tree(&mut h, &start);
            let request = batch(&h, &["2", "3", "1"], vec![number(1)]);
            // The step fails, and so does the first undo step after it.
            h.provider.fail_at(call, FaultKind::PermissionDenied);
            h.provider.fail_at(call + 1, FaultKind::PermissionDenied);
            let result = h.run(request);
            h.provider.reset();
            let Some(failure) = result.failure else {
                continue;
            };
            if failure.report.inverse.is_empty() {
                assert_eq!(work_tree(&h), start, "call {call}: rolled back");
                continue;
            }
            leftovers += 1;
            h.provider.reset();
            // What the report says is done is what the tree shows, and undoing it restores all.
            for (from, to) in &failure.report.renamed {
                let from = VfsPath::from_location(from).unwrap();
                let to = VfsPath::from_location(to).unwrap();
                assert!(h.provider.stat(&to).is_ok(), "call {call}: {to:?}");
                let _ = from;
            }
            let mut steps = failure.report.inverse.clone();
            steps.reverse();
            Executor::new(h.env.clone())
                .run_undo(JobId(99), &steps, &CancelToken::new(), &mut NullSink)
                .unwrap_or_else(|f| panic!("call {call}: {f:?}"));
            assert_eq!(
                work_tree(&h),
                start,
                "call {call}: undo finishes the rollback"
            );
        }
        assert!(leftovers > 0, "some fault pair lands in the rollback");
    });
}

use waypoint_vfs::CancelToken;

#[test]
fn a_cancel_at_any_step_leaves_the_original_tree() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &start_tree());
        let probe = h.run(batch(&h, &["2", "3", "1"], vec![number(1)]));
        assert_eq!(probe.state, JobState::Done);
        let total = h.provider.calls();
        for call in 1..=total {
            reset_tree(&mut h, &start_tree());
            let request = batch(&h, &["2", "3", "1"], vec![number(1)]);
            let result = h.run_hooked(request, &mut |h, token| {
                h.provider.cancel_at(call, token);
            });
            let now = work_tree(&h);
            assert!(partials(&now).is_empty(), "call {call}");
            match result.state {
                JobState::Cancelled => assert_eq!(now, start_tree(), "call {call}"),
                JobState::Done => assert_ne!(now, start_tree()),
                other => panic!("call {call}: {other:?}"),
            }
        }
    });
}

#[test]
fn a_crash_at_any_call_loses_no_entry_and_recovery_reports_the_one_set_aside() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        let start = start_tree();
        jbuild(&h, &start);
        let probe = h.run_journalled(batch(&h, &["2", "3", "1"], vec![number(1)]));
        assert_eq!(probe.state, JobState::Done);
        let total = h.provider.calls();
        let contents = |t: &Tree| -> Vec<Vec<u8>> {
            let mut all: Vec<Vec<u8>> = t
                .values()
                .filter_map(|n| match n {
                    Node::File(bytes) => Some(bytes.clone()),
                    _ => None,
                })
                .collect();
            all.sort();
            all
        };
        let mut set_aside = 0;
        for call in 1..=total {
            reset_names(&mut h, &start);
            let request = batch(&h, &["2", "3", "1"], vec![number(1)]);
            let run = h.run_journalled_hooked(request, &mut |h, _| h.provider.crash_at(call));
            h.provider.reset();
            let after = jwork(&h);
            assert_eq!(
                contents(&after),
                contents(&start),
                "call {call}: every file is somewhere"
            );
            for key in after.keys() {
                let top = key.split('/').next().unwrap();
                assert!(
                    ["1", "2", "3", "x.txt", "D"].contains(&top) || top.starts_with(TEMP_PREFIX),
                    "call {call}: unexpected {key}"
                );
            }
            let report = h.restart();
            if !run.crashed {
                continue;
            }
            let aside: Vec<&String> = after
                .keys()
                .filter(|k| k.starts_with(TEMP_PREFIX))
                .collect();
            if !aside.is_empty() {
                set_aside += 1;
                let left: Vec<String> = report
                    .interrupted
                    .iter()
                    .flat_map(|j| j.left.iter().map(|l| l.display.clone()))
                    .collect();
                assert_eq!(left.len(), aside.len(), "call {call}: {left:?}");
                assert!(report.interrupted.iter().all(|j| j.removed.is_empty()));
                assert_eq!(jwork(&h), after, "recovery does not touch the entry");
            }
        }
        assert!(
            set_aside > 0,
            "some crash lands while an entry is set aside"
        );
    });
}

use waypoint_ops::exec::TEMP_PREFIX;

/// Puts the work folder back to `start`, whatever state a crash left it in.
fn reset_names<P: Provider + 'static>(h: &mut JournalHarness<P>, start: &Tree) {
    h.provider.reset();
    let keys: Vec<String> = jwork(h).keys().cloned().collect();
    for key in keys.iter().rev() {
        let path = h.path(key);
        if let Ok(entry) = h.provider.stat(&path) {
            if entry.kind == waypoint_vfs::EntryKind::Directory {
                h.provider.remove_dir(&path).unwrap();
            } else {
                h.provider.remove_file(&path).unwrap();
            }
        }
    }
    jbuild(h, start);
}

#[test]
fn one_undo_restores_every_name_and_a_redo_does_it_again() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        let start = start_tree();
        jbuild(&h, &start);
        let run = h.run_journalled(batch(&h, &["2", "3", "1"], vec![number(1)]));
        assert_eq!(run.state, JobState::Done, "{:?}", run.failure);
        let entry = run.entry.expect("a batch is one journal entry");
        let after = jwork(&h);
        assert_ne!(after, start);
        assert_eq!(h.journal.entries().len(), 1);
        let recorded = h.journal.entry(entry).unwrap();
        assert_eq!(recorded.label, "Rename 3 items");
        assert_eq!(recorded.kind, JobKind::BatchRename);

        let undo = h.undo(entry);
        assert_eq!(undo.state, JobState::Done, "{:?}", undo.undo);
        assert_eq!(jwork(&h), start, "undo restores the exact names");
        assert!(partials(&jwork(&h)).is_empty());

        let redo = h.redo(entry);
        assert_eq!(redo.state, JobState::Done, "{:?}", redo.failure);
        assert_eq!(jwork(&h), after, "redo plans from the stored rules");
        let undo = h.undo(entry);
        assert_eq!(undo.state, JobState::Done, "{:?}", undo.undo);
        assert_eq!(jwork(&h), start);
        assert!(h.journal.pending().is_empty());
    });
}

#[test]
fn undo_is_refused_cleanly_when_an_entry_changed_since() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        let start = start_tree();
        jbuild(&h, &start);
        let run = h.run_journalled(batch(&h, &["2", "3", "1"], vec![number(1)]));
        let entry = run.entry.unwrap();
        // Someone moves an entry the batch made.
        h.provider
            .rename(&h.path("3"), &h.path("moved"), false)
            .unwrap();
        h.provider.reset();
        let request = h.journal.undo_request(entry, "main-1").unwrap();
        let before = jwork(&h);
        let undo = h.run_journalled(request);
        assert!(
            matches!(failed_with(&undo), OpsError::UndoStale { .. }),
            "{:?}",
            undo.state
        );
        assert_eq!(jwork(&h), before, "nothing is touched");
    });
}

#[test]
fn a_redo_carries_the_time_the_first_run_used() {
    let (mut h, _guard) = local_jh();
    jbuild(&h, &tree(&[("a.txt", "1")]));
    let today = RenameRule::DateToken {
        source: DateSource::Today,
        format: "%Y-%m-%d".to_owned(),
        position: RulePosition::Prefix,
        separator: "_".to_owned(),
    };
    let mut request = batch(&h, &["a.txt"], vec![today]);
    request.rename.as_mut().unwrap().now_ms = None;
    let run = h.run_journalled(request);
    let entry = run.entry.unwrap();
    let forward = &h.journal.entry(entry).unwrap().forward.request;
    let frozen = forward.rename.as_ref().unwrap().now_ms;
    assert!(
        frozen.is_some(),
        "the clock is read once, at the first plan"
    );
    let new_name = jwork(&h).keys().next().unwrap().clone();
    assert_eq!(
        h.journal.entry(entry).unwrap().label,
        format!("Rename \u{201c}a.txt\u{201d} to \u{201c}{new_name}\u{201d}")
    );
    let names: BTreeMap<_, _> = jwork(&h).into_iter().collect();
    h.undo(entry);
    h.redo(entry);
    assert_eq!(jwork(&h).into_iter().collect::<BTreeMap<_, _>>(), names);
}

#[test]
fn a_partial_selection_counts_from_the_whole_selection() {
    // The unchanged entry still takes a number, so a redo (which runs over every source) agrees.
    let (mut h, _guard) = local_jh();
    jbuild(&h, &tree(&[("1", "a"), ("b", "b"), ("c", "c")]));
    let run = h.run_journalled(batch(&h, &["1", "b", "c"], vec![number(1)]));
    assert_eq!(run.state, JobState::Done);
    let names: Vec<String> = jwork(&h).keys().cloned().collect();
    assert_eq!(names, ["1", "2", "3"], "`1` stays, the others take 2 and 3");
    assert_eq!(
        h.journal.entry(run.entry.unwrap()).unwrap().label,
        "Rename 2 items"
    );
    let entry = run.entry.unwrap();
    h.undo(entry);
    h.redo(entry);
    assert_eq!(jwork(&h).keys().cloned().collect::<Vec<_>>(), names);
}
