// The undo journal: what each undoable job records, undo and redo through the queue, the refusals
// when something changed since, the cap, the redo chain, and the history the app lists.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;
mod journal_support;

use common::*;
use journal_support::*;
use waypoint_protocol::Location;

fn create_folder<P: Provider + 'static>(h: &JournalHarness<P>, at: &str, name: &str) -> JobRequest {
    h.request(JobKind::CreateFolder, &[], Some(at), Some(name))
}

fn trash<P: Provider + 'static>(h: &JournalHarness<P>, items: &[&str]) -> JobRequest {
    h.request(JobKind::Trash, items, None, None)
}

fn done<P: Provider + 'static>(run: &JournalRun, h: &JournalHarness<P>) {
    assert_eq!(
        run.state,
        JobState::Done,
        "{:?} {:?}",
        run.failure,
        run.undo
    );
    assert!(h.store.violations().is_empty());
}

#[test]
fn each_undoable_job_is_undone_and_redone() {
    each_journal_provider!(|h, rule, links| {
        let _ = links;
        let start = tree(&[
            ("a", "1"),
            ("Dir/", ""),
            ("Dir/f", "ff"),
            ("keep", "k"),
            ("t1", "x"),
            ("t2", "y"),
        ]);
        jbuild(&h, &start);
        let cases: Vec<JobRequest> = vec![
            create_folder(&h, "", "made"),
            h.request(JobKind::CreateFile, &[], Some(""), Some("note.txt")),
            h.request(JobKind::Rename, &["a"], None, Some("renamed")),
            h.request(JobKind::Rename, &["Dir"], None, Some("DIR")),
            h.request(JobKind::Duplicate, &["Dir", "keep"], None, None),
            trash(&h, &["t1", "Dir"]),
        ];
        for request in cases {
            let kind = request.kind;
            let before = jwork(&h);
            let run = h.run_journalled(request);
            done(&run, &h);
            let entry = run.entry.expect("an undoable job leaves an entry");
            let after = jwork(&h);
            assert_ne!(before, after, "{kind:?} does something");
            assert!(h.store.job(run.id).unwrap().undoable);

            let undo = h.undo(entry);
            done(&undo, &h);
            assert_eq!(jwork(&h), before, "{kind:?}: undo restores the tree");
            assert_eq!(
                h.journal.entry(entry).unwrap().state,
                EntryState::Undone,
                "{kind:?}"
            );
            assert!(h.journal.pending().is_empty());

            let redo = h.redo(entry);
            done(&redo, &h);
            assert_eq!(jwork(&h), after, "{kind:?}: redo reproduces the result");
            assert_eq!(h.journal.entry(entry).unwrap().state, EntryState::Applied);

            let undo = h.undo(entry);
            done(&undo, &h);
            assert_eq!(jwork(&h), before, "{kind:?}: undo after redo");
            let _ = rule;
            // Leave the tree as it was for the next case, and the journal tidy.
        }
        assert!(h.trash.is_empty());
    });
}

#[test]
fn undo_and_redo_run_as_queue_jobs() {
    let (mut h, _g) = local_jh();
    jbuild(&h, &tree(&[("a", "1")]));
    let run = h.run_journalled(h.request(JobKind::Rename, &["a"], None, Some("b")));
    let entry = run.entry.unwrap();
    let undo = h.undo(entry);
    assert_eq!(
        h.store.job(undo.id).unwrap().kind,
        JobKind::Undo { of: entry }
    );
    let redo = h.redo(entry);
    assert_eq!(
        h.store.job(redo.id).unwrap().kind,
        JobKind::Redo { of: entry }
    );
    assert_eq!(h.replayed().jobs, h.store.snapshot().jobs);
    assert!(h.store.violations().is_empty());
    // Undo and redo are not themselves entries.
    assert_eq!(h.journal.entries().len(), 1);
}

#[test]
fn permanent_delete_and_restore_leave_no_entry() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        jbuild(&h, &tree(&[("a", "1"), ("b", "2")]));
        let run = h.run_journalled(h.request(JobKind::Delete, &["a"], None, None));
        done(&run, &h);
        assert_eq!(run.entry, None);
        assert!(h.journal.entries().is_empty());
        assert!(h.journal.pending().is_empty());
        // A restore from the Trash is not journalled either.
        let trashed = h.run_journalled(trash(&h, &["b"]));
        let entry = trashed.entry.unwrap();
        let receipt = h.trash.receipts().remove(0);
        let request = JobRequest {
            kind: JobKind::Restore,
            sources: Sources::Locations {
                locations: vec![h.trash.trashed_location(&receipt)],
            },
            destination: None,
            name: None,
            options: JobOptions::default(),
            origin_window: "main-1".to_owned(),
            rename: None,
            archive: None,
        };
        let run = h.run_journalled(request);
        done(&run, &h);
        assert_eq!(run.entry, None);
        assert_eq!(h.journal.entries().len(), 1);
        // The restore took the item out of the Trash, so the trash entry's undo is refused.
        let refused = h.run_journalled(h.journal.undo_request(entry, "main-1").unwrap());
        assert!(matches!(
            failed_with(&refused),
            OpsError::UndoStale {
                reason: StaleReason::TrashEmptied,
                ..
            }
        ));
    });
}

#[test]
fn a_job_that_did_nothing_leaves_no_entry_and_no_record() {
    let (mut h, _g) = local_jh();
    jbuild(&h, &tree(&[("a", "1")]));
    // The planner refuses: nothing was begun.
    let run = h.run_journalled(h.request(JobKind::Rename, &["missing"], None, Some("x")));
    assert!(matches!(run.state, JobState::Failed { .. }));
    assert!(h.journal.entries().is_empty());
    assert!(h.journal.pending().is_empty());
    assert_eq!(h.storage.saves(), 0);
    // A first-item failure after the record is stored aborts it.
    h.provider.reset();
    let request = h.request(JobKind::Rename, &["a"], None, Some("b"));
    let run = h.run_journalled_hooked(request, &mut |h, _| {
        h.provider
            .fail_nth(Op::Rename, 1, FaultKind::PermissionDenied)
    });
    assert!(matches!(run.state, JobState::Failed { .. }));
    assert!(h.journal.entries().is_empty());
    assert!(h.journal.pending().is_empty());
}

#[test]
fn undo_refuses_when_a_created_entry_changed() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        jbuild(&h, &tree(&[("f", "one")]));
        // A folder that someone has since put a file in.
        let run = h.run_journalled(create_folder(&h, "", "made"));
        let folder = run.entry.unwrap();
        jbuild(&h, &tree(&[("made/x", "x")]));
        let before = jwork(&h);
        let refused = h.run_journalled(h.journal.undo_request(folder, "main-1").unwrap());
        assert!(
            matches!(
                failed_with(&refused),
                OpsError::UndoStale {
                    reason: StaleReason::Changed,
                    ..
                }
            ),
            "{:?}",
            refused.state
        );
        assert_eq!(jwork(&h), before, "a refused undo writes nothing");
        assert_eq!(h.journal.entry(folder).unwrap().state, EntryState::Applied);
        assert!(h.journal.pending().is_empty(), "nothing was begun");

        // A duplicate whose copy was edited.
        let run = h.run_journalled(h.request(JobKind::Duplicate, &["f"], None, None));
        let dup = run.entry.unwrap();
        let copy = h.journal.entry(dup).unwrap().inverse[0].subject();
        let copy_name = copy.display.rsplit(['/', '\\']).next().unwrap().to_owned();
        overwrite(&h, &copy_name, "edited and longer");
        let before = jwork(&h);
        let refused = h.run_journalled(h.journal.undo_request(dup, "main-1").unwrap());
        assert!(matches!(
            failed_with(&refused),
            OpsError::UndoStale {
                reason: StaleReason::Changed,
                ..
            }
        ));
        assert_eq!(jwork(&h), before);
    });
}

#[test]
fn undo_of_a_duplicated_tree_refuses_a_change_deep_inside() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        jbuild(&h, &tree(&[("d/", ""), ("d/s/", ""), ("d/s/f", "deep")]));
        let run = h.run_journalled(h.request(JobKind::Duplicate, &["d"], None, None));
        let entry = run.entry.unwrap();
        let copy = h.journal.entry(entry).unwrap().inverse[0].subject();
        let name = copy.display.rsplit(['/', '\\']).next().unwrap().to_owned();
        overwrite(&h, &format!("{name}/s/f"), "deeper still");
        let before = jwork(&h);
        let refused = h.run_journalled(h.journal.undo_request(entry, "main-1").unwrap());
        assert!(matches!(failed_with(&refused), OpsError::UndoStale { .. }));
        assert_eq!(jwork(&h), before);
    });
}

#[test]
fn undo_refuses_when_a_renamed_entry_is_gone_or_its_name_is_taken() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        jbuild(&h, &tree(&[("a", "1"), ("b", "2")]));
        let run = h.run_journalled(h.request(JobKind::Rename, &["a"], None, Some("c")));
        let entry = run.entry.unwrap();

        // The old name has been taken.
        jbuild(&h, &tree(&[("a", "other")]));
        let before = jwork(&h);
        let refused = h.run_journalled(h.journal.undo_request(entry, "main-1").unwrap());
        assert!(matches!(
            failed_with(&refused),
            OpsError::UndoStale {
                reason: StaleReason::NameTaken,
                ..
            }
        ));
        assert_eq!(jwork(&h), before);

        // The renamed entry has been deleted.
        let run = h.run_journalled(h.request(JobKind::Delete, &["c"], None, None));
        done(&run, &h);
        let refused = h.run_journalled(h.journal.undo_request(entry, "main-1").unwrap());
        assert!(matches!(
            failed_with(&refused),
            OpsError::UndoStale {
                reason: StaleReason::Missing,
                ..
            }
        ));
    });
}

#[test]
fn undoing_a_trash_restores_from_the_receipt_and_refuses_when_it_was_emptied() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        jbuild(
            &h,
            &tree(&[("a", "1"), ("d/", ""), ("d/x", "2"), ("keep", "k")]),
        );
        let before = jwork(&h);
        let run = h.run_journalled(trash(&h, &["a", "d"]));
        let entry = run.entry.unwrap();
        assert_eq!(
            h.journal.entry(entry).unwrap().label,
            "Move 2 items to Trash"
        );
        assert_eq!(h.trash.len(), 2);
        done(&h.undo(entry), &h);
        assert_eq!(jwork(&h), before);
        assert!(h.trash.is_empty());
        done(&h.redo(entry), &h);
        assert_eq!(h.trash.len(), 2);

        // Someone empties the Trash: the undo is refused cleanly and nothing moves.
        let after = jwork(&h);
        h.trash.empty(None).unwrap();
        h.provider.reset();
        let refused = h.run_journalled(h.journal.undo_request(entry, "main-1").unwrap());
        assert!(matches!(
            failed_with(&refused),
            OpsError::UndoStale {
                reason: StaleReason::TrashEmptied,
                ..
            }
        ));
        assert_eq!(jwork(&h), after);
        assert_eq!(h.journal.entry(entry).unwrap().state, EntryState::Applied);
    });
}

#[test]
fn undoing_a_trash_refuses_when_the_old_name_is_taken_and_replaces_nothing() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        jbuild(&h, &tree(&[("a", "1")]));
        let run = h.run_journalled(trash(&h, &["a"]));
        let entry = run.entry.unwrap();
        jbuild(&h, &tree(&[("a", "newer")]));
        let refused = h.run_journalled(h.journal.undo_request(entry, "main-1").unwrap());
        assert!(matches!(
            failed_with(&refused),
            OpsError::UndoStale {
                reason: StaleReason::NameTaken,
                ..
            }
        ));
        assert_eq!(jwork(&h), tree(&[("a", "newer")]));
        assert_eq!(h.trash.len(), 1);
    });
}

#[test]
fn a_new_action_clears_the_redo_chain() {
    let (mut h, _g) = local_jh();
    jbuild(&h, &tree(&[("a", "1")]));
    let first = h
        .run_journalled(create_folder(&h, "", "one"))
        .entry
        .unwrap();
    let second = h
        .run_journalled(create_folder(&h, "", "two"))
        .entry
        .unwrap();
    done(&h.undo(second), &h);
    done(&h.undo(first), &h);
    assert_eq!(
        h.journal.last_redoable(),
        Some(first),
        "the one undone last"
    );
    assert_eq!(h.journal.last_undoable(), None);
    let snapshot = h.journal.snapshot();
    assert!(snapshot.undo.is_none());
    assert_eq!(snapshot.redo.as_ref().unwrap().id, first);
    // Redo order is the reverse of undo order.
    done(&h.redo_last(), &h);
    assert_eq!(h.journal.last_redoable(), Some(second));
    assert_eq!(h.journal.last_undoable(), Some(first));
    // A new action drops the entries that were undone.
    let third = h
        .run_journalled(create_folder(&h, "", "three"))
        .entry
        .unwrap();
    let ids: Vec<JournalId> = h.journal.entries().iter().map(|e| e.id).collect();
    assert_eq!(ids, vec![first, third]);
    assert_eq!(h.journal.last_redoable(), None);
    assert!(h.journal.redo_request(second, "main-1").is_err());
}

#[test]
fn summaries_list_the_history_newest_first() {
    let (mut h, _g) = local_jh();
    jbuild(&h, &tree(&[("a", "1")]));
    let one = h
        .run_journalled(create_folder(&h, "", "one"))
        .entry
        .unwrap();
    let two = h
        .run_journalled(h.request(JobKind::Rename, &["a"], None, Some("b")))
        .entry
        .unwrap();
    done(&h.undo(two), &h);
    let list = h.journal.summaries();
    assert_eq!(
        list.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![two, one]
    );
    assert_eq!(
        list[0].label,
        "Rename \u{201c}a\u{201d} to \u{201c}b\u{201d}"
    );
    assert!(!list[0].undoable && list[0].redoable);
    assert!(list[1].undoable && !list[1].redoable);
    assert_eq!(list[1].label, "Create folder \u{201c}one\u{201d}");
    // Any entry can be undone from the list, not only the newest.
    done(&h.undo(one), &h);
}

#[test]
fn journal_events_mirror_the_history_on_their_own_gate() {
    let (mut h, _g) = local_jh();
    jbuild(&h, &tree(&[("a", "1")]));
    let one = h
        .run_journalled(create_folder(&h, "", "one"))
        .entry
        .unwrap();
    done(&h.undo(one), &h);
    done(&h.redo(one), &h);
    let revisions: Vec<u64> = h.journal_events.iter().map(OpsEvent::revision).collect();
    assert!(revisions.windows(2).all(|w| w[0] < w[1]), "{revisions:?}");
    let mut mirror = h.replayed();
    for event in &h.journal_events {
        mirror.apply(event);
    }
    assert_eq!(mirror.journal, h.journal.snapshot());
    assert_eq!(mirror.revision, h.store.revision());
    // A stale event is ignored.
    let before = mirror.clone();
    mirror.apply(&h.journal_events[0]);
    assert_eq!(mirror, before);
}

#[test]
fn the_cap_trims_the_oldest_and_never_splits_an_entry() {
    let settings = OpsSettings {
        undo_depth: 3,
        ..OpsSettings::default()
    };
    let dir = tempfile::tempdir().unwrap();
    let base = VfsPath::File(FilePath::from_path(dir.path()).unwrap());
    let mut h = JournalHarness::with_settings(LocalProvider::new(), base, settings);
    let names = ["t1", "t2", "t3", "t4", "t5", "t6"];
    jbuild(
        &h,
        &names.iter().map(|n| (n.to_string(), file("x"))).collect(),
    );
    let mut ids = Vec::new();
    for pair in names.chunks(2) {
        // Entries of one step and of two.
        let run = h.run_journalled(trash(&h, pair));
        ids.push((run.entry.unwrap(), pair.len()));
        let run = h.run_journalled(create_folder(&h, "", &format!("f{}", ids.len())));
        ids.push((run.entry.unwrap(), 1));
        assert!(h.journal.entries().len() <= 3);
    }
    let kept: Vec<JournalId> = h.journal.entries().iter().map(|e| e.id).collect();
    let newest: Vec<JournalId> = ids.iter().rev().take(3).rev().map(|(id, _)| *id).collect();
    assert_eq!(kept, newest, "the three newest remain");
    for entry in h.journal.entries() {
        let steps = ids.iter().find(|(id, _)| *id == entry.id).unwrap().1;
        assert_eq!(
            entry.inverse.len(),
            steps,
            "an entry keeps all of its steps"
        );
    }
    // What is kept still undoes.
    done(&h.undo_last(), &h);
}

#[test]
fn a_depth_of_zero_keeps_no_history() {
    let settings = OpsSettings {
        undo_depth: 0,
        ..OpsSettings::default()
    };
    let dir = tempfile::tempdir().unwrap();
    let base = VfsPath::File(FilePath::from_path(dir.path()).unwrap());
    let mut h = JournalHarness::with_settings(LocalProvider::new(), base, settings);
    let run = h.run_journalled(create_folder(&h, "", "x"));
    assert_eq!(run.entry, None);
    assert!(h.journal.entries().is_empty());
    assert!(h.journal.pending().is_empty());
}

#[test]
fn a_job_that_stopped_part_way_is_recorded_for_what_it_did() {
    let (mut h, _g) = local_jh();
    jbuild(&h, &tree(&[("a", "1"), ("b", "2")]));
    // The second item's trash fails, so only the first is done and recorded.
    let request = trash(&h, &["a", "b"]);
    let run = h.run_journalled_hooked(request, &mut |h, _| {
        h.provider
            .fail_nth(Op::Rename, 2, FaultKind::PermissionDenied)
    });
    assert!(matches!(run.state, JobState::Failed { .. }));
    let entry = run.entry.expect("the first item is undoable");
    let e = h.journal.entry(entry).unwrap();
    assert_eq!(e.inverse.len(), 1);
    assert_eq!(e.label, "Move \u{201c}a\u{201d} to Trash");
    h.provider.reset();
    done(&h.undo(entry), &h);
    assert_eq!(jwork(&h), tree(&[("a", "1"), ("b", "2")]));
}

#[test]
fn an_undo_that_fails_part_way_stops_and_can_be_finished() {
    // Sweep a fault over every call of the undo of a three-step entry, with each kind of fault.
    // How many calls an undo of this entry makes.
    let calls = {
        let (mut h, _g) = local_jh();
        jbuild(
            &h,
            &tree(&[("a", "1"), ("b", "2"), ("c", "3"), ("keep", "k")]),
        );
        let entry = h.run_journalled(trash(&h, &["a", "b", "c"])).entry.unwrap();
        h.provider.reset();
        done(&h.undo(entry), &h);
        h.provider.calls()
    };
    assert!(calls > 6, "{calls}");
    for kind in FaultKind::ALL {
        let mut call = 1;
        while call <= calls {
            let (mut h, _g) = local_jh();
            let start = tree(&[("a", "1"), ("b", "2"), ("c", "3"), ("keep", "k")]);
            jbuild(&h, &start);
            let entry = h.run_journalled(trash(&h, &["a", "b", "c"])).entry.unwrap();
            let trashed = jwork(&h);
            h.provider.reset();
            let request = h.journal.undo_request(entry, "main-1").unwrap();
            let run = h.run_journalled_hooked(request, &mut |h, _| h.provider.fail_at(call, kind));
            h.provider.reset();
            let context = format!("{kind:?} at call {call}");
            if run.state == JobState::Done {
                // A fault the checks absorb (a missing name is a free name): the undo completed.
                assert_eq!(jwork(&h), start, "{context}");
                call += 1;
                continue;
            }
            let tree_now = jwork(&h);
            let e = h.journal.entry(entry).unwrap().clone();
            match &run.undo {
                None => {
                    // Refused in planning: nothing moved, nothing changed.
                    assert_eq!(tree_now, trashed, "{context}");
                    assert_eq!(e.state, EntryState::Applied);
                    assert!(!e.partly_undone);
                }
                Some(Err(failure)) => {
                    assert_eq!(failure.applied + failure.remaining.len(), 3, "{context}");
                    assert_eq!(e.inverse.len(), failure.remaining.len(), "{context}");
                    assert_eq!(e.state, EntryState::Applied, "{context}");
                    assert_eq!(e.partly_undone, failure.applied > 0, "{context}");
                    // The steps that ran are the last ones recorded, in reverse order: c, b, a.
                    let restored = ["c", "b", "a"][..failure.applied].to_vec();
                    for name in &restored {
                        assert!(tree_now.contains_key(*name), "{context}: {name}");
                    }
                    for name in &["c", "b", "a"][failure.applied..] {
                        assert!(!tree_now.contains_key(*name), "{context}: {name}");
                    }
                    if failure.applied > 0 {
                        assert!(h.journal.redo_request(entry, "main-1").is_err());
                    }
                }
                Some(Ok(_)) => panic!("{context}: done but not done"),
            }
            assert!(h.journal.pending().is_empty(), "{context}");
            // Undoing again finishes it, and nothing is done twice.
            h.provider.reset();
            done(&h.undo(entry), &h);
            assert_eq!(jwork(&h), start, "{context}: finished");
            assert_eq!(h.journal.entry(entry).unwrap().state, EntryState::Undone);
            done(&h.redo(entry), &h);
            assert_eq!(jwork(&h), trashed, "{context}: redone");
            call += 1;
        }
    }
}

#[test]
fn a_cancelled_undo_keeps_the_steps_it_did_not_reach() {
    let (mut h, _g) = local_jh();
    let start = tree(&[("a", "1"), ("b", "2"), ("c", "3")]);
    jbuild(&h, &start);
    let entry = h.run_journalled(trash(&h, &["a", "b", "c"])).entry.unwrap();
    h.provider.reset();
    let request = h.journal.undo_request(entry, "main-1").unwrap();
    // The checks make a fixed number of calls; cancel after the first step's own work.
    let mut stopped = None;
    for call in 1..40 {
        let (mut g, _gd) = local_jh();
        jbuild(&g, &start);
        let e = g.run_journalled(trash(&g, &["a", "b", "c"])).entry.unwrap();
        g.provider.reset();
        let request = g.journal.undo_request(e, "main-1").unwrap();
        let run =
            g.run_journalled_hooked(request, &mut |g, token| g.provider.cancel_at(call, token));
        if run.state == JobState::Cancelled {
            let entry = g.journal.entry(e).unwrap();
            assert_eq!(entry.state, EntryState::Applied);
            let done_steps = 3 - entry.inverse.len();
            assert_eq!(entry.partly_undone, done_steps > 0);
            stopped = Some(done_steps);
            g.provider.reset();
            done(&g.undo(e), &g);
            assert_eq!(jwork(&g), start);
        }
    }
    assert!(stopped.is_some(), "some call index cancelled the undo");
    drop(request);
}

#[test]
fn the_wire_shapes_are_plain() {
    let step = InverseStep::RemoveCreated {
        location: Location::new("/a", "file:///a"),
        fingerprint: Some(Fingerprint {
            is_dir: false,
            size: Some(3),
            modified_ms: Some(5),
            entry_count: None,
            digest: None,
        }),
    };
    let json = serde_json::to_string(&step).unwrap();
    assert!(
        json.starts_with(r#"{"kind":"removeCreated","location":{"#),
        "{json}"
    );
    assert_eq!(serde_json::from_str::<InverseStep>(&json).unwrap(), step);
    assert_eq!(serde_json::to_string(&JournalId(7)).unwrap(), "7");
    assert_eq!(
        serde_json::to_string(&StaleReason::TrashEmptied).unwrap(),
        r#""trashEmptied""#
    );
    let error = OpsError::UndoStale {
        location: Location::new("/a", "file:///a"),
        reason: StaleReason::Changed,
    };
    let json = serde_json::to_string(&error).unwrap();
    assert!(
        json.starts_with(r#"{"kind":"undoStale","location":"#),
        "{json}"
    );
    assert!(error.to_string().contains("cannot be undone"));
}

#[test]
fn a_folders_fingerprint_notices_a_child_link_that_was_retargeted() {
    each_journal_provider!(|h, rule, links| {
        let _ = rule;
        if !links {
            return;
        }
        let mut t = tree(&[("d/", ""), ("d/f", "x")]);
        t.insert("d/ln".to_owned(), Node::Link("f".to_owned()));
        jbuild(&h, &t);
        let at = h.path("d");
        let before = fingerprint(h.provider.as_ref(), &at).unwrap();
        assert!(same_fingerprint(
            &before,
            &fingerprint(h.provider.as_ref(), &at).unwrap()
        ));
        // The link is replaced by one that points elsewhere; nothing else changes.
        let link = h.path("d/ln");
        h.provider.remove_file(&link).unwrap();
        h.provider
            .symlink(&link, std::ffi::OsStr::new("elsewhere"))
            .unwrap();
        let after = fingerprint(h.provider.as_ref(), &at).unwrap();
        assert!(!same_fingerprint(&before, &after));
    });
}
