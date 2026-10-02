// The journal's persistence and start-up recovery: the document and its generations, the
// write-ahead record, and a crash at every call of every kind of job.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;
mod journal_support;

use std::sync::Arc;

use common::*;
use journal_support::*;
use waypoint_ops::testing::journal_storage::CountingSaver;
use waypoint_protocol::Location;
use waypoint_vfs::CancelToken;

fn create_folder<P: Provider + 'static>(h: &JournalHarness<P>, name: &str) -> JobRequest {
    h.request(JobKind::CreateFolder, &[], Some(""), Some(name))
}

fn deps_over(
    storage: Arc<dyn JournalStorage>,
    h: &JournalHarness<impl Provider + 'static>,
) -> JournalDeps {
    JournalDeps {
        settings: h.settings.clone(),
        clock: h.clock.clone(),
        storage,
        saver: Arc::new(CountingSaver::default()),
    }
}

#[test]
fn the_write_ahead_record_is_stored_before_the_job_and_everything_else_waits_for_a_flush() {
    let (mut h, _g) = local_jh();
    jbuild(&h, &tree(&[("a", "1")]));
    assert_eq!(h.storage.saves(), 0);
    // Stop the job right after `begin`: a crash at the first write.
    let request = create_folder(&h, "x");
    let run = h.run_journalled_hooked(request, &mut |h, _| {
        // The planner reads twice, then the executor's first write is call 3 or later.
        h.provider.crash_at(3)
    });
    assert!(run.crashed);
    let stored = h
        .storage
        .current_document()
        .expect("stored before the job ran");
    assert_eq!(stored.body.pending.len(), 1);
    assert_eq!(stored.body.pending[0].job, run.id);
    assert_eq!(stored.body.pending[0].kind, JobKind::CreateFolder);
    assert_eq!(
        stored.body.pending[0].label,
        "Create folder \u{201c}x\u{201d}"
    );

    // A finished job asks for a save and does not write until it is flushed.
    let (mut h, _g) = local_jh();
    let saves = h.storage.saves();
    let asked = h.saver.count();
    let run = h.run_journalled(create_folder(&h, "x"));
    assert!(run.entry.is_some());
    assert_eq!(h.storage.saves(), saves + 1, "only the write-ahead record");
    assert!(h.saver.count() > asked);
    assert!(h.journal.is_dirty());
    assert!(h
        .storage
        .current_document()
        .unwrap()
        .body
        .entries
        .is_empty());
    h.journal.flush().unwrap();
    assert!(!h.journal.is_dirty());
    let stored = h.storage.current_document().unwrap();
    assert_eq!(stored.body.entries.len(), 1);
    assert!(stored.body.pending.is_empty());
    assert_eq!(stored.version, JOURNAL_VERSION);
}

#[test]
fn begin_reports_a_storage_that_cannot_write() {
    let (mut h, _g) = local_jh();
    h.storage.fail_saves(true);
    let record = PendingRecord {
        job: JobId(1),
        at_ms: 0,
        kind: JobKind::Trash,
        label: "x".to_owned(),
        items: vec![],
        folders: vec![],
        renames: vec![],
    };
    assert!(matches!(h.journal.begin(record), Err(StorageError::Io(_))));
}

#[test]
fn the_journal_survives_a_restart_and_keeps_counting() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        jbuild(&h, &tree(&[("a", "1"), ("b", "2")]));
        let one = h.run_journalled(create_folder(&h, "one")).entry.unwrap();
        let two = h
            .run_journalled(h.request(JobKind::Rename, &["a"], None, Some("c")))
            .entry
            .unwrap();
        let three = h
            .run_journalled(h.request(JobKind::Trash, &["b"], None, None))
            .entry
            .unwrap();
        assert!(h.undo(three).state == JobState::Done);
        let entries = h.journal.entries().to_vec();
        let revision = h.journal.revision();
        h.journal.flush().unwrap();

        let report = h.restart();
        assert!(!report.needs_notice(), "{report:?}");
        assert_eq!(h.journal.entries(), entries.as_slice());
        assert_eq!(h.journal.revision(), revision);
        assert_eq!(h.journal.last_redoable(), Some(three));

        // Undo and redo still work on what was loaded, and ids carry on.
        let tree_now = jwork(&h);
        done_ok(&h.undo(two));
        done_ok(&h.redo(two));
        assert_eq!(jwork(&h), tree_now);
        done_ok(&h.redo(three));
        let next = h.run_journalled(create_folder(&h, "four")).entry.unwrap();
        assert!(next > three && three > two && two > one);
        assert!(h.journal.revision() > revision);
    });
}

fn done_ok(run: &JournalRun) {
    assert_eq!(
        run.state,
        JobState::Done,
        "{:?} {:?}",
        run.failure,
        run.undo
    );
}

#[test]
fn the_earlier_generation_is_kept_like_the_session_keeps_it() {
    let (mut h, _g) = local_jh();
    jbuild(&h, &tree(&[("a", "1")]));
    h.run_journalled(create_folder(&h, "one"));
    h.journal.flush().unwrap();
    let first = h.storage.current_text().unwrap();
    assert_eq!(h.storage.previous_text(), None);
    h.restart();
    h.run_journalled(create_folder(&h, "two"));
    h.journal.flush().unwrap();
    // The first save of the new run rotated what the old run left into `previous`.
    assert_eq!(h.storage.previous_text(), Some(first));
    assert_ne!(h.storage.current_text(), h.storage.previous_text());
}

#[test]
fn an_unreadable_latest_copy_falls_back_to_the_earlier_one_and_says_so() {
    let (mut h, _g) = local_jh();
    jbuild(&h, &tree(&[("a", "1")]));
    h.run_journalled(create_folder(&h, "one"));
    h.journal.flush().unwrap();
    h.restart();
    h.run_journalled(create_folder(&h, "two"));
    h.journal.flush().unwrap();
    h.storage.put_current_text("{ this is not a journal");
    let report = h.restart();
    assert!(report.from_previous);
    let discarded = report.discarded.clone().expect("the bad file is reported");
    assert!(matches!(discarded.reason, DiscardReason::Corrupt { .. }));
    assert_eq!(
        discarded.set_aside.as_deref(),
        Some("journal.json.corrupt-1")
    );
    assert!(report.needs_notice());
    // The earlier generation held the first entry only.
    assert_eq!(h.journal.entries().len(), 1);
    assert_eq!(
        h.journal.entries()[0].label,
        "Create folder \u{201c}one\u{201d}"
    );
}

#[test]
fn a_journal_that_cannot_be_read_at_all_starts_empty() {
    let (mut h, _g) = local_jh();
    h.storage.put_current_text("garbage");
    let report = h.restart();
    assert!(h.journal.entries().is_empty());
    assert!(matches!(
        report.discarded.as_ref().map(|d| &d.reason),
        Some(DiscardReason::Corrupt { .. })
    ));
    assert_eq!(
        report.discarded.unwrap().set_aside.as_deref(),
        Some("journal.json.corrupt-1")
    );
    // It still works, and the next save replaces the garbage.
    jbuild(&h, &tree(&[("a", "1")]));
    let entry = h.run_journalled(create_folder(&h, "x")).entry.unwrap();
    h.journal.flush().unwrap();
    assert!(h.storage.current_document().is_some());
    done_ok(&h.undo(entry));
}

#[test]
fn a_document_of_another_version_is_set_aside_and_not_used() {
    let (mut h, _g) = local_jh();
    jbuild(&h, &tree(&[("a", "1")]));
    h.run_journalled(create_folder(&h, "x"));
    h.journal.flush().unwrap();
    let mut doc = h.storage.current_document().unwrap();
    doc.version = JOURNAL_VERSION + 1;
    h.storage
        .put_current_text(&serde_json::to_string(&doc).unwrap());
    let report = h.restart();
    assert!(h.journal.entries().is_empty());
    let discarded = report.discarded.unwrap();
    assert_eq!(
        discarded.reason,
        DiscardReason::UnsupportedVersion {
            found: JOURNAL_VERSION + 1,
            supported: JOURNAL_VERSION
        }
    );
    assert!(discarded.set_aside.is_some());
}

struct BrokenStorage;

impl JournalStorage for BrokenStorage {
    fn load(&self) -> Result<Loaded, StorageError> {
        Err(StorageError::Io("the store is locked".to_owned()))
    }
    fn save(&self, _: &JournalDocument) -> Result<(), StorageError> {
        Err(StorageError::Io("the store is locked".to_owned()))
    }
}

#[test]
fn a_storage_that_fails_to_load_starts_the_journal_empty_with_a_report() {
    let (h, _g) = local_jh();
    let (journal, report) = Journal::open(deps_over(Arc::new(BrokenStorage), &h), &h.env.providers);
    assert!(journal.entries().is_empty());
    assert!(matches!(
        report.discarded.map(|d| d.reason),
        Some(DiscardReason::StorageFailed { .. })
    ));
}

#[test]
fn recovery_looks_only_in_the_folders_the_record_names_and_only_for_its_job() {
    let (mut h, _g) = local_jh();
    jbuild(
        &h,
        &tree(&[
            ("dst/", ""),
            ("elsewhere/", ""),
            ("dst/.waypoint-partial-9-1-other job", "x"),
            ("dst/keep", "k"),
            ("elsewhere/.waypoint-partial-1-1-mine but not named", "x"),
            ("src", "s"),
        ]),
    );
    // Job 1 had a partial in `dst`; the stray partials are a different job's and another folder's.
    jbuild(&h, &tree(&[("dst/.waypoint-partial-1-5-src", "half")]));
    let record = PendingRecord {
        job: JobId(1),
        at_ms: 5,
        kind: JobKind::Copy,
        label: "Copy \u{201c}src\u{201d}".to_owned(),
        items: vec![h.loc("src")],
        folders: vec![h.loc("dst"), h.loc("missing-folder")],
        renames: vec![],
    };
    h.journal.begin(record).unwrap();
    let report = h.restart();
    assert_eq!(report.interrupted.len(), 1);
    let job = &report.interrupted[0];
    assert_eq!(job.job, JobId(1));
    assert_eq!(job.planned, vec![h.loc("src")]);
    assert_eq!(job.removed, vec![h.loc("dst/.waypoint-partial-1-5-src")]);
    assert!(job.left.is_empty() && job.restored.is_empty());
    let after = jwork(&h);
    assert!(after.contains_key("dst/.waypoint-partial-9-1-other job"));
    assert!(after.contains_key("elsewhere/.waypoint-partial-1-1-mine but not named"));
    assert!(!after.contains_key("dst/.waypoint-partial-1-5-src"));
    assert!(after.contains_key("dst/keep") && after.contains_key("src"));
    // Recovery never resumes: the journal has no entry, no pending record and no job.
    assert!(h.journal.entries().is_empty() && h.journal.pending().is_empty());
    assert!(h.store.snapshot().jobs.is_empty());
    // And a second restart has nothing left to report.
    h.journal.flush().unwrap();
    assert!(!h.restart().needs_notice());
}

struct Scenario<P: Provider + 'static> {
    name: &'static str,
    tree: fn(bool) -> Tree,
    /// Runs journalled jobs to set up what the request acts on.
    prepare: fn(&mut JournalHarness<P>),
    request: fn(&mut JournalHarness<P>) -> JobRequest,
    /// Whole trees other than the first and the last that a job of several items passes through.
    between: fn() -> Vec<Tree>,
}

fn no_prep<P: Provider + 'static>(_: &mut JournalHarness<P>) {}

fn no_between() -> Vec<Tree> {
    Vec::new()
}

fn scenarios<P: Provider + 'static>() -> Vec<Scenario<P>> {
    vec![
        Scenario {
            name: "create folder",
            tree: |_| tree(&[("keep", "k")]),
            prepare: no_prep,
            request: |h| create_folder(h, "new"),
            between: no_between,
        },
        Scenario {
            name: "rename",
            tree: |_| tree(&[("a", "x"), ("b", "y")]),
            prepare: no_prep,
            request: |h| h.request(JobKind::Rename, &["a"], None, Some("c")),
            between: no_between,
        },
        Scenario {
            name: "case-only rename of a folder",
            tree: |_| tree(&[("Dir/", ""), ("Dir/f", "1")]),
            prepare: no_prep,
            request: |h| h.request(JobKind::Rename, &["Dir"], None, Some("DIR")),
            between: no_between,
        },
        Scenario {
            name: "duplicate a file of several chunks",
            tree: |_| {
                let mut t = tree(&[("keep", "k")]);
                t.insert("big.bin".to_owned(), Node::File(vec![3; 150_000]));
                t
            },
            prepare: no_prep,
            request: |h| h.request(JobKind::Duplicate, &["big.bin"], None, None),
            between: no_between,
        },
        Scenario {
            name: "duplicate a tree",
            tree: |_| {
                tree(&[
                    ("tree/", ""),
                    ("tree/a", "aa"),
                    ("tree/sub/", ""),
                    ("tree/sub/b", "bb"),
                ])
            },
            prepare: no_prep,
            request: |h| h.request(JobKind::Duplicate, &["tree"], None, None),
            between: no_between,
        },
        Scenario {
            name: "trash two items",
            tree: |_| tree(&[("a", "1"), ("d/", ""), ("d/x", "2"), ("keep", "k")]),
            prepare: no_prep,
            request: |h| h.request(JobKind::Trash, &["a", "d"], None, None),
            between: || {
                vec![
                    tree(&[("d/", ""), ("d/x", "2"), ("keep", "k")]),
                    tree(&[("a", "1"), ("keep", "k")]),
                ]
            },
        },
        Scenario {
            name: "undo a duplicated tree",
            tree: |_| {
                tree(&[
                    ("tree/", ""),
                    ("tree/a", "aa"),
                    ("tree/sub/", ""),
                    ("tree/sub/b", "bb"),
                ])
            },
            prepare: |h| {
                done_ok(&h.run_journalled(h.request(JobKind::Duplicate, &["tree"], None, None)));
            },
            request: |h| h.journal.undo_last_request("main-1").unwrap(),
            between: no_between,
        },
        Scenario {
            name: "undo a case-only rename",
            tree: |_| tree(&[("Dir/", ""), ("Dir/f", "1")]),
            prepare: |h| {
                done_ok(&h.run_journalled(h.request(JobKind::Rename, &["Dir"], None, Some("DIR"))));
            },
            request: |h| h.journal.undo_last_request("main-1").unwrap(),
            between: no_between,
        },
        Scenario {
            name: "undo a trash of two items",
            tree: |_| tree(&[("a", "1"), ("d/", ""), ("d/x", "2"), ("keep", "k")]),
            prepare: |h| {
                done_ok(&h.run_journalled(h.request(JobKind::Trash, &["a", "d"], None, None)));
            },
            request: |h| h.journal.undo_last_request("main-1").unwrap(),
            between: || {
                vec![
                    tree(&[("d/", ""), ("d/x", "2"), ("keep", "k")]),
                    tree(&[("a", "1"), ("keep", "k")]),
                ]
            },
        },
    ]
}

fn is_listed(tree: &Tree, candidates: &[&Tree]) -> bool {
    candidates.contains(&tree)
}

/// Runs the job once to learn its calls, then once for every call with a crash at it: reload the
/// journal from what was stored and recover.
fn crash_sweep<P: Provider + 'static>(
    make: &dyn Fn() -> (JournalHarness<P>, tempfile::TempDir),
    links: bool,
    scenario: &Scenario<P>,
) -> (usize, usize) {
    let (mut removed, mut restored) = (0, 0);
    let start = || {
        let (mut h, guard) = make();
        jbuild(&h, &(scenario.tree)(links));
        (scenario.prepare)(&mut h);
        h.journal.flush().unwrap();
        h.provider.reset();
        (h, guard)
    };
    let (mut baseline, _g) = start();
    let before = jwork(&baseline);
    baseline.provider.reset();
    let entries_before = baseline.journal.entries().len();
    let request = (scenario.request)(&mut baseline);
    let run = baseline.run_journalled(request);
    assert_eq!(
        run.state,
        JobState::Done,
        "{}: {:?}",
        scenario.name,
        run.failure
    );
    let calls = baseline.provider.calls();
    baseline.provider.reset();
    let after = jwork(&baseline);
    assert_ne!(before, after, "{}", scenario.name);
    let between = (scenario.between)();
    let mut crashed_after_begin = 0;

    for call in 1..=calls {
        let (mut h, _g) = start();
        let request = (scenario.request)(&mut h);
        let context = format!("{} / crash at call {call}", scenario.name);
        let run = h.run_journalled_hooked(request, &mut |h, _| h.provider.crash_at(call));
        assert!(run.crashed, "{context}");
        let pending_stored = h
            .storage
            .current_document()
            .is_some_and(|d| !d.body.pending.is_empty());

        let report = h.restart();
        let tree_now = jwork(&h);
        assert!(
            partials(&tree_now).is_empty(),
            "{context}: {:?}",
            partials(&tree_now)
        );
        let clean = without_partials(&tree_now);
        let mut candidates: Vec<&Tree> = vec![&before, &after];
        candidates.extend(between.iter());
        assert!(
            is_listed(&clean, &candidates),
            "{context}: a half result {clean:?}"
        );

        if pending_stored {
            crashed_after_begin += 1;
            assert_eq!(report.interrupted.len(), 1, "{context}: {report:?}");
            assert_eq!(report.interrupted[0].job, run.id, "{context}");
            assert!(!report.interrupted[0].label.is_empty());
            assert!(report.needs_notice());
            assert!(report.interrupted[0].left.is_empty(), "{context}");
            removed += report.interrupted[0].removed.len();
            restored += report.interrupted[0].restored.len();
        } else {
            assert!(report.interrupted.is_empty(), "{context}: {report:?}");
        }
        assert!(report.discarded.is_none(), "{context}");
        assert!(h.journal.pending().is_empty(), "{context}");
        // The crashed job made no entry; an undo that crashed left its entry as it was.
        assert_eq!(h.journal.entries().len(), entries_before, "{context}");
        assert!(
            h.journal
                .entries()
                .iter()
                .all(|e| e.state == EntryState::Applied),
            "{context}"
        );
        // Recovery is saved, and a second start has nothing to report.
        h.journal.flush().unwrap();
        assert!(!h.restart().needs_notice(), "{context}");
        // The engine and the world are still usable: the restored journal's last entry still
        // refuses or undoes cleanly, never panics.
        if let Ok(request) = h.journal.undo_last_request("main-1") {
            let _ = h.run_journalled(request);
        }
    }
    assert!(
        crashed_after_begin > 0,
        "{}: no crash landed after the record",
        scenario.name
    );
    (removed, restored)
}

macro_rules! sweep_over_providers {
    ($name:ident, $sweep:ident) => {
        #[test]
        fn $name() {
            for scenario in scenarios::<LocalProvider>() {
                eprintln!("local: {}", scenario.name);
                let (removed, _) = $sweep(&local_jh, cfg!(unix), &scenario);
                if scenario.name.starts_with("duplicate") {
                    assert!(
                        removed > 0,
                        "{}: recovery removed no partial",
                        scenario.name
                    );
                }
            }
            for rule in [CaseRule::Sensitive, CaseRule::Insensitive] {
                for scenario in scenarios::<MemoryProvider>() {
                    eprintln!("memory {rule:?}: {}", scenario.name);
                    let (removed, restored) = $sweep(&|| memory_jh(rule), true, &scenario);
                    if scenario.name.starts_with("duplicate")
                        || scenario.name.contains("undo a dup")
                    {
                        assert!(
                            removed > 0,
                            "{}: recovery removed no partial",
                            scenario.name
                        );
                    }
                    // A forward case-only rename is direct where the provider allows it; only the undo goes
                    // through a set-aside name that a crash can leave behind.
                    if rule == CaseRule::Insensitive
                        && scenario.name.contains("case-only")
                        && scenario.name.starts_with("undo")
                    {
                        assert!(restored > 0, "{}: recovery put nothing back", scenario.name);
                    }
                }
            }
        }
    };
}

sweep_over_providers!(
    a_crash_at_any_call_is_recovered_to_a_consistent_tree_and_one_report,
    crash_sweep
);

struct Quiet;

impl ExecSink for Quiet {
    fn progress(&mut self, _: &Progress, _: &Counts) {}
    fn on_error(&mut self, _: &Location, _: &OpsError) -> Option<Decision> {
        None
    }
}

/// Runs a copy of `src/a` over `dst/a` with the write-ahead record in place, as the plugin's worker
/// does, and crashes at `crash` (a call counted from the first of the run) if given.
fn replacing_copy<P: Provider + 'static>(h: &mut JournalHarness<P>, crash: Option<usize>) {
    let request = h.request(JobKind::Copy, &["src/a"], Some("dst"), None);
    let plan = h.plan(&request).unwrap();
    let id = JobId(7);
    h.journal
        .begin(PendingRecord::for_plan(id, 0, &request, &plan))
        .unwrap();
    h.provider.reset();
    if let Some(call) = crash {
        h.provider.crash_at(call);
    }
    let options = RunOptions {
        resolutions: Resolutions::new(Some(ConflictPolicy::Replace)),
        ..RunOptions::default()
    };
    // A crash at a call whose failure the job swallows (the last removal) still ends Ok.
    let _ =
        Executor::new(h.env.clone()).run_with(id, &plan, &CancelToken::new(), &mut Quiet, options);
}

/// A displayed path with `/` separators, which is how the trees name their entries (Windows
/// displays `\`).
fn slashed(display: &str) -> String {
    display.replace('\\', "/")
}

#[test]
fn a_crash_at_any_call_of_a_replace_never_loses_the_original_it_set_aside() {
    fn sweep<P: Provider + 'static>(make: &dyn Fn() -> (JournalHarness<P>, tempfile::TempDir)) {
        let start = || {
            let (h, guard) = make();
            jbuild(
                &h,
                &tree(&[
                    ("src/", ""),
                    ("src/a", "new"),
                    ("dst/", ""),
                    ("dst/a", "old"),
                ]),
            );
            (h, guard)
        };
        let (mut baseline, _g) = start();
        replacing_copy(&mut baseline, None);
        let calls = baseline.provider.calls();
        assert!(calls > 3);
        let (mut restored_any, mut left_any) = (false, false);
        for call in 1..=calls {
            let (mut h, _g) = start();
            replacing_copy(&mut h, Some(call));
            let report = h.restart();
            let tree_now = jwork(&h);
            let at = format!("crash at call {call}/{calls}: {tree_now:?} {report:?}");
            assert!(partials(&tree_now).is_empty(), "{at}");
            // Whatever the crash caught, the name holds the old file or the new one.
            let held = tree_now.get("dst/a");
            assert!(
                held == Some(&Node::File(b"old".to_vec()))
                    || held == Some(&Node::File(b"new".to_vec())),
                "{at}"
            );
            // What is still under a waiting name is reported and is never deleted.
            let interrupted = report.interrupted.first();
            for key in tree_now
                .keys()
                .filter(|k| k.split('/').any(|n| n.starts_with(".waypoint-replaced-")))
            {
                let listed = interrupted.is_some_and(|j| {
                    j.left
                        .iter()
                        .any(|l| slashed(&l.display).ends_with(key.as_str()))
                });
                assert!(listed, "{at}: {key} is not reported");
            }
            if let Some(job) = interrupted {
                restored_any |= !job.restored.is_empty();
                left_any |= !job.left.is_empty();
                assert!(
                    job.restored
                        .iter()
                        .all(|r| slashed(&r.display).ends_with("dst/a")),
                    "{at}"
                );
            }
        }
        assert!(
            restored_any,
            "no crash landed between the set-aside and the rename-in"
        );
        assert!(
            left_any,
            "no crash landed between the rename-in and the removal"
        );
    }
    sweep(&local_jh);
    sweep(&|| memory_jh(CaseRule::Sensitive));
    sweep(&|| memory_jh(CaseRule::Insensitive));
}
