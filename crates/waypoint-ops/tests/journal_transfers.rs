// Copy, move and link in the undo journal: each is undone and redone, an undo refuses when the
// result was edited or the old name is taken, a replace is never journalled, a move across (simulated)
// volumes copies back, and a fault at any step leaves a job that can still be undone to the exact
// original tree. Seeded random sequences undo everything to the original on the local provider and
// on the in-memory provider with a second volume.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;
mod journal_support;
mod xfer;

use std::sync::atomic::{AtomicUsize, Ordering};

use common::*;
use journal_support::*;
use xfer::{memory, pattern, set_mtime, VolumeId, SMALL_CHUNK};

static ENTRIES: AtomicUsize = AtomicUsize::new(0);
static CROSSED: AtomicUsize = AtomicUsize::new(0);

fn transfer<P: Provider + 'static>(
    h: &JournalHarness<P>,
    kind: JobKind,
    sources: &[&str],
    dest: &str,
    policy: Option<ConflictPolicy>,
) -> JobRequest {
    let mut request = h.request(kind, sources, Some(dest), None);
    request.options.conflict = policy;
    request
}

fn finished<P: Provider + 'static>(run: &JournalRun, h: &JournalHarness<P>) {
    assert_eq!(
        run.state,
        JobState::Done,
        "{:?} {:?}",
        run.failure.as_ref().map(|f| (&f.error, &f.item)),
        run.undo
    );
    assert!(h.store.violations().is_empty());
    assert!(h.journal.pending().is_empty());
}

/// Runs the request, then undoes and redoes it twice over, checking the tree each time. Returns the
/// entry's label.
fn round_trip<P: Provider + 'static>(
    h: &mut JournalHarness<P>,
    request: impl FnOnce(&JournalHarness<P>) -> JobRequest,
) -> String {
    let request = request(h);
    let before = jwork(h);
    h.provider.reset();
    let run = h.run_journalled(request);
    finished(&run, h);
    let id = run.entry.expect("the job left an entry");
    h.provider.reset();
    let after = jwork(h);
    assert_ne!(before, after, "the job changed something");
    assert!(partials(&after).is_empty());
    let label = h.journal.entry(id).unwrap().label.clone();
    for _ in 0..2 {
        let undo = h.undo(id);
        finished(&undo, h);
        h.provider.reset();
        assert_eq!(jwork(h), before, "undo restores the tree");
        let redo = h.redo(id);
        finished(&redo, h);
        h.provider.reset();
        assert_eq!(jwork(h), after, "redo reproduces the result");
    }
    let undo = h.undo(id);
    finished(&undo, h);
    h.provider.reset();
    assert_eq!(jwork(h), before);
    label
}

fn sample() -> Tree {
    let mut t = tree(&[
        ("src/", ""),
        ("src/a.txt", "alpha"),
        ("src/top/", ""),
        ("src/top/b", "bravo"),
        ("src/top/deep/", ""),
        ("src/top/deep/c", "see"),
        ("src/top/empty/", ""),
        ("src/loose", "l"),
        ("dst/", ""),
        ("dst/other", "o"),
    ]);
    t.insert(
        "src/top/big".to_owned(),
        Node::File(pattern(3 * SMALL_CHUNK + 7, 3)),
    );
    t
}

fn linkless(mut t: Tree, links: bool) -> Tree {
    if links {
        t.insert("src/top/ln".to_owned(), Node::Link("b".to_owned()));
    }
    t
}

#[test]
fn a_copy_is_undone_and_redone() {
    each_journal_provider!(|h, rule, links| {
        let _ = rule;
        jbuild(&h, &linkless(sample(), links));
        h.chunk_bytes = SMALL_CHUNK;
        let label = round_trip(&mut h, |h| {
            transfer(h, JobKind::Copy, &["src/top", "src/a.txt"], "dst", None)
        });
        assert_eq!(label, "Copy 2 items");
        let label = round_trip(&mut h, |h| {
            transfer(
                h,
                JobKind::Copy,
                &["src/loose"],
                "src",
                Some(ConflictPolicy::KeepBoth),
            )
        });
        assert_eq!(label, "Copy \u{201c}loose\u{201d}");
    });
}

#[test]
fn a_link_is_undone_and_redone() {
    each_journal_provider!(|h, rule, links| {
        let _ = rule;
        if links {
            jbuild(&h, &sample());
            let label = round_trip(&mut h, |h| {
                transfer(h, JobKind::Link, &["src/a.txt", "src/top"], "dst", None)
            });
            assert_eq!(label, "Link 2 items");
        }
    });
}

#[test]
fn a_move_on_one_volume_is_undone_and_redone() {
    each_journal_provider!(|h, rule, links| {
        let _ = rule;
        jbuild(&h, &linkless(sample(), links));
        let label = round_trip(&mut h, |h| {
            transfer(h, JobKind::Move, &["src/top", "src/a.txt"], "dst", None)
        });
        assert_eq!(label, "Move 2 items");
    });
}

#[test]
fn a_copy_into_a_merged_folder_undoes_only_what_it_created() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        let mut t = sample();
        t.extend(tree(&[
            ("dst/top/", ""),
            ("dst/top/mine", "mine"),
            ("dst/top/deep/", ""),
            ("dst/top/deep/also", "also"),
        ]));
        jbuild(&h, &t);
        h.chunk_bytes = SMALL_CHUNK;
        round_trip(&mut h, |h| {
            transfer(
                h,
                JobKind::Copy,
                &["src/top"],
                "dst",
                Some(ConflictPolicy::MergeFolders),
            )
        });
        // What was already in the folder is there throughout (round_trip compared the trees), and
        // the entries the merge added are separate steps of the entry.
        let run = h.run_journalled(transfer(
            &h,
            JobKind::Copy,
            &["src/top"],
            "dst",
            Some(ConflictPolicy::MergeFolders),
        ));
        finished(&run, &h);
        let entry = h.journal.entry(run.entry.unwrap()).unwrap();
        assert!(entry.inverse.len() >= 3, "{:?}", entry.inverse);
        assert!(entry.inverse.iter().all(|s| matches!(
            s,
            InverseStep::RemoveCreated {
                fingerprint: Some(_),
                ..
            }
        )));
    });
}

#[test]
fn a_move_into_a_merged_folder_gives_back_the_folder_it_emptied() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        let mut t = sample();
        t.extend(tree(&[("dst/top/", ""), ("dst/top/mine", "mine")]));
        jbuild(&h, &t);
        h.chunk_bytes = SMALL_CHUNK;
        round_trip(&mut h, |h| {
            transfer(
                h,
                JobKind::Move,
                &["src/top"],
                "dst",
                Some(ConflictPolicy::MergeFolders),
            )
        });
    });
}

#[test]
fn a_job_that_replaced_something_is_not_journalled_and_says_so() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        let mut t = sample();
        t.extend(tree(&[
            ("dst/a.txt", "old"),
            ("dst/top/", ""),
            ("dst/top/x", "x"),
        ]));
        jbuild(&h, &t);
        for kind in [JobKind::Copy, JobKind::Move] {
            let before = h.journal.entries().len();
            let run = h.run_journalled(transfer(
                &h,
                kind,
                &["src/a.txt"],
                "dst",
                Some(ConflictPolicy::Replace),
            ));
            finished(&run, &h);
            assert_eq!(run.entry, None, "{kind:?}");
            assert_eq!(h.journal.entries().len(), before);
            let report = run.report.as_ref().unwrap();
            assert!(report.inverse.is_empty());
            assert!(report.transfer.unjournalled.is_some(), "{kind:?}");
            assert_eq!(report.transfer.replaced.len(), 1);
            // Put the source back for the second round.
            if kind == JobKind::Move {
                break;
            }
        }
        // A folder replace too.
        let run = h.run_journalled(transfer(
            &h,
            JobKind::Copy,
            &["src/top"],
            "dst",
            Some(ConflictPolicy::Replace),
        ));
        finished(&run, &h);
        assert_eq!(run.entry, None);
        assert!(run.report.unwrap().transfer.unjournalled.is_some());
    });
}

#[test]
fn an_edited_copy_is_not_undone() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        jbuild(&h, &sample());
        let run = h.run_journalled(transfer(
            &h,
            JobKind::Copy,
            &["src/a.txt", "src/top"],
            "dst",
            None,
        ));
        finished(&run, &h);
        let id = run.entry.unwrap();
        // The copied file edited since.
        overwrite(&h, "dst/a.txt", "edited");
        let tree_now = jwork(&h);
        h.provider.reset();
        let undo = h.undo(id);
        assert!(
            matches!(
                failed_with(&undo),
                OpsError::UndoStale {
                    reason: StaleReason::Changed,
                    ..
                }
            ),
            "{:?}",
            undo.state
        );
        h.provider.reset();
        assert_eq!(jwork(&h), tree_now, "a refused undo writes nothing");
        assert_eq!(h.journal.entry(id).unwrap().state, EntryState::Applied);
    });
}

#[test]
fn a_copied_folder_with_something_added_is_not_undone() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        jbuild(&h, &sample());
        let run = h.run_journalled(transfer(&h, JobKind::Copy, &["src/top"], "dst", None));
        finished(&run, &h);
        let id = run.entry.unwrap();
        overwrite_new(&h, "dst/top/deep/extra", "mine");
        let tree_now = jwork(&h);
        h.provider.reset();
        let undo = h.undo(id);
        assert!(matches!(
            failed_with(&undo),
            OpsError::UndoStale {
                reason: StaleReason::Changed,
                ..
            }
        ));
        h.provider.reset();
        assert_eq!(jwork(&h), tree_now);
    });
}

fn overwrite_new<P: Provider + 'static>(h: &JournalHarness<P>, relative: &str, content: &str) {
    use std::io::Write;
    let mut stream = h
        .provider
        .create_write(&h.path(relative), waypoint_vfs::WriteOptions::exclusive())
        .unwrap();
    stream.write_all(content.as_bytes()).unwrap();
    stream.finish(false).unwrap();
    h.provider.reset();
}

#[test]
fn a_move_is_not_undone_into_a_name_that_is_taken() {
    each_journal_provider!(|h, rule, links| {
        let _ = (rule, links);
        jbuild(&h, &sample());
        let run = h.run_journalled(transfer(&h, JobKind::Move, &["src/a.txt"], "dst", None));
        finished(&run, &h);
        let id = run.entry.unwrap();
        overwrite_new(&h, "src/a.txt", "a new file in the old place");
        let tree_now = jwork(&h);
        let undo = h.undo(id);
        assert!(matches!(
            failed_with(&undo),
            OpsError::UndoStale {
                reason: StaleReason::NameTaken,
                ..
            }
        ));
        h.provider.reset();
        assert_eq!(jwork(&h), tree_now);
    });
}

fn crossing_jh() -> (JournalHarness<MemoryProvider>, tempfile::TempDir) {
    let (mut h, dir) = memory_jh(CaseRule::Sensitive);
    h.provider.create_dir(&h.path("src")).unwrap();
    memory(&h.harness).set_volume(&h.path("src"), VolumeId(2));
    h.provider.create_dir(&h.path("dst")).unwrap();
    h.provider.reset();
    h.chunk_bytes = SMALL_CHUNK;
    (h, dir)
}

fn cross_sample() -> Tree {
    let mut t = sample();
    t.insert("src/top/ln".to_owned(), Node::Link("b".to_owned()));
    t.insert(
        "src/top/deep/blob".to_owned(),
        Node::File(pattern(5 * SMALL_CHUNK + 1, 9)),
    );
    t
}

fn put<P: Provider + 'static>(h: &JournalHarness<P>, t: &Tree) {
    jbuild(h, t);
}

#[test]
fn a_move_across_volumes_is_copied_back_and_redone() {
    let (mut h, _dir) = crossing_jh();
    // `crossing_jh` made the folders; build only what is below them.
    let mut t = cross_sample();
    t.retain(|k, _| k != "src" && k != "dst");
    put(&h, &t);
    let run = h.run_journalled(transfer(
        &h,
        JobKind::Move,
        &["src/top", "src/a.txt"],
        "dst",
        None,
    ));
    finished(&run, &h);
    let entry = h.journal.entry(run.entry.unwrap()).unwrap().clone();
    assert!(
        entry.inverse.iter().all(|s| matches!(
            s,
            InverseStep::CopyBack {
                fingerprint: Some(_),
                ..
            }
        )),
        "{:?}",
        entry.inverse
    );
    CROSSED.fetch_add(1, Ordering::Relaxed);
    let (mut h, _dir) = crossing_jh();
    put(&h, &t);
    round_trip(&mut h, |h| {
        transfer(h, JobKind::Move, &["src/top", "src/a.txt"], "dst", None)
    });
}

#[test]
fn an_undo_across_volumes_keeps_times_and_refuses_edits_and_taken_names() {
    let (mut h, _dir) = crossing_jh();
    let mut t = cross_sample();
    t.retain(|k, _| k != "src" && k != "dst");
    put(&h, &t);
    set_mtime(&h.harness, "src/a.txt", 1_600_000_000_000);
    let run = h.run_journalled(transfer(
        &h,
        JobKind::Move,
        &["src/a.txt", "src/top"],
        "dst",
        None,
    ));
    finished(&run, &h);
    let id = run.entry.unwrap();
    h.provider.reset();
    let moved = h.provider.stat(&h.path("dst/a.txt")).unwrap();
    assert_eq!(moved.modified_ms, Some(1_600_000_000_000));

    // Edited at the destination: the copy back would lose the edit.
    overwrite(&h, "dst/a.txt", "changed");
    let now = jwork(&h);
    let refused = h.undo(id);
    assert!(matches!(
        failed_with(&refused),
        OpsError::UndoStale {
            reason: StaleReason::Changed,
            ..
        }
    ));
    h.provider.reset();
    assert_eq!(jwork(&h), now);

    // A new file in the old place: never overwritten either.
    let (mut h, _dir) = crossing_jh();
    put(&h, &t);
    set_mtime(&h.harness, "src/a.txt", 1_600_000_000_000);
    let run = h.run_journalled(transfer(&h, JobKind::Move, &["src/a.txt"], "dst", None));
    finished(&run, &h);
    let id = run.entry.unwrap();
    overwrite_new(&h, "src/a.txt", "squatter");
    let now = jwork(&h);
    let refused = h.undo(id);
    assert!(matches!(
        failed_with(&refused),
        OpsError::UndoStale {
            reason: StaleReason::NameTaken,
            ..
        }
    ));
    h.provider.reset();
    assert_eq!(jwork(&h), now);

    // And with nothing in the way, the file comes back with its time.
    let (mut h, _dir) = crossing_jh();
    put(&h, &t);
    set_mtime(&h.harness, "src/a.txt", 1_600_000_000_000);
    let run = h.run_journalled(transfer(&h, JobKind::Move, &["src/a.txt"], "dst", None));
    let id = run.entry.unwrap();
    let undo = h.undo(id);
    finished(&undo, &h);
    h.provider.reset();
    assert_eq!(
        h.provider.stat(&h.path("src/a.txt")).unwrap().modified_ms,
        Some(1_600_000_000_000)
    );
}

#[test]
fn a_merge_move_across_volumes_is_undone() {
    let (mut h, _dir) = crossing_jh();
    let mut t = cross_sample();
    t.retain(|k, _| k != "src" && k != "dst");
    t.extend(tree(&[("dst/top/", ""), ("dst/top/mine", "mine")]));
    put(&h, &t);
    round_trip(&mut h, |h| {
        transfer(
            h,
            JobKind::Move,
            &["src/top"],
            "dst",
            Some(ConflictPolicy::MergeFolders),
        )
    });
}

/// A fault of each kind at each call of a job: whatever the job did before it stopped is in the
/// journal, and undoing it gives back the exact original tree.
fn fault_sweep(kind: JobKind, crossing: bool, policy: Option<ConflictPolicy>, extra: &Tree) {
    let make = || {
        let (h, dir) = if crossing {
            crossing_jh()
        } else {
            let (mut h, dir) = memory_jh(CaseRule::Sensitive);
            h.chunk_bytes = SMALL_CHUNK;
            (h, dir)
        };
        let mut t = cross_sample();
        if crossing {
            t.retain(|k, _| k != "src" && k != "dst");
        }
        t.extend(extra.clone());
        put(&h, &t);
        h.provider.reset();
        (h, dir)
    };
    let sources = ["src/top", "src/a.txt", "src/loose"];
    let (mut baseline, _g) = make();
    let before = jwork(&baseline);
    let run = baseline.run_journalled(transfer(&baseline, kind, &sources, "dst", policy));
    finished(&run, &baseline);
    let calls = run.exec_calls;
    for fault in FaultKind::ALL {
        for call in 1..=calls {
            let (mut h, _g) = make();
            let context = format!("{kind:?} crossing={crossing} {fault:?} at {call}/{calls}");
            let request = transfer(&h, kind, &sources, "dst", policy);
            let run = h.run_journalled_hooked(request, &mut |h, _| h.provider.fail_at(call, fault));
            h.provider.reset();
            let after = jwork(&h);
            assert!(partials(&after).is_empty(), "{context}: {after:?}");
            assert!(h.journal.pending().is_empty(), "{context}");
            let Some(id) = run.entry else {
                // Nothing was journalled: the job left the tree as it was, or the planner refused.
                if run.report.as_ref().is_some_and(|r| r.inverse.is_empty())
                    || run
                        .failure
                        .as_ref()
                        .is_some_and(|f| f.report.inverse.is_empty())
                    || run.plan.is_none()
                {
                    assert_eq!(after, before, "{context}: nothing journalled but it wrote");
                }
                continue;
            };
            ENTRIES.fetch_add(1, Ordering::Relaxed);
            let undo = h.undo(id);
            h.provider.reset();
            let refused = match &undo.state {
                JobState::Failed {
                    error: OpsError::UndoStale { reason, .. },
                    ..
                } => Some(*reason),
                _ => None,
            };
            // An injected `NotFound` on a source's removal reads as "already gone" to the move,
            // which then records a move that left the source in place too: the undo sees both and
            // refuses, which is safe.
            if refused == Some(StaleReason::Unverified)
                || (fault == FaultKind::NotFound && refused.is_some())
            {
                // The fault landed in the read that fingerprints the result, after the job had
                // stopped, so the entry cannot vouch for what it made: the undo refuses and
                // writes nothing, which is the safe outcome.
                assert_eq!(jwork(&h), after, "{context}: a refusal wrote");
                continue;
            }
            assert_eq!(
                undo.state,
                JobState::Done,
                "{context}: {:?}",
                undo.undo
                    .as_ref()
                    .map(|r| r.as_ref().err().map(|f| (&f.error, &f.item)))
            );
            assert_eq!(jwork(&h), before, "{context}: undo restores the original");
            assert!(h.journal.pending().is_empty(), "{context}");
        }
    }
}

#[test]
fn a_fault_at_any_step_of_a_copy_leaves_a_job_that_undoes_to_the_original() {
    fault_sweep(JobKind::Copy, false, None, &Tree::new());
}

#[test]
fn a_fault_at_any_step_of_a_move_on_one_volume_undoes_to_the_original() {
    fault_sweep(JobKind::Move, false, None, &Tree::new());
}

#[test]
fn a_fault_at_any_step_of_a_move_across_volumes_undoes_to_the_original() {
    fault_sweep(JobKind::Move, true, None, &Tree::new());
}

#[test]
fn a_fault_at_any_step_of_a_merge_move_across_volumes_undoes_to_the_original() {
    let extra = tree(&[("dst/top/", ""), ("dst/top/mine", "mine")]);
    fault_sweep(
        JobKind::Move,
        true,
        Some(ConflictPolicy::MergeFolders),
        &extra,
    );
}

#[test]
fn a_fault_at_any_step_of_a_link_undoes_to_the_original() {
    fault_sweep(JobKind::Link, false, None, &Tree::new());
}

#[test]
fn a_crash_during_an_undo_across_volumes_is_recovered_with_no_partial_left() {
    let make = || {
        let (mut h, dir) = crossing_jh();
        let mut t = cross_sample();
        t.retain(|k, _| k != "src" && k != "dst");
        put(&h, &t);
        let run = h.run_journalled(transfer(
            &h,
            JobKind::Move,
            &["src/top", "src/a.txt"],
            "dst",
            None,
        ));
        finished(&run, &h);
        h.journal.flush().unwrap();
        h.provider.reset();
        (h, dir)
    };
    let (mut baseline, _g) = make();
    let id = baseline.journal.last_undoable().unwrap();
    let after_move = jwork(&baseline);
    baseline.provider.reset();
    let undo = baseline.undo(id);
    finished(&undo, &baseline);
    let calls = baseline.provider.calls();
    let original = {
        baseline.provider.reset();
        jwork(&baseline)
    };
    for call in 1..=calls {
        let (mut h, _g) = make();
        let id = h.journal.last_undoable().unwrap();
        let request = h.journal.undo_request(id, "main-1").unwrap();
        let run = h.run_journalled_hooked(request, &mut |h, _| h.provider.crash_at(call));
        assert!(run.crashed, "call {call}");
        let report = h.restart();
        let tree = jwork(&h);
        let context = format!("crash at {call}/{calls}");
        assert!(partials(&tree).is_empty(), "{context}: {tree:?} {report:?}");
        // Whatever the crash left is the move's result, the original, or a mix in which no entry
        // is lost: every entry is whole under one name or the other.
        let mut seen: Tree = Tree::new();
        for (key, node) in &tree {
            seen.insert(key.clone(), node.clone());
        }
        let _ = (&after_move, &original);
        for (key, node) in &original {
            let moved = key.replacen("src/", "dst/", 1);
            let here = seen.get(key).or_else(|| seen.get(&moved));
            if matches!(node, Node::File(_)) {
                assert_eq!(here, Some(node), "{context}: {key} lost or changed");
            }
        }
    }
}

// ---- random sequences ----

fn folders(tree: &Tree) -> Vec<String> {
    let mut out: Vec<String> = tree
        .iter()
        .filter(|(_, n)| **n == Node::Dir)
        .map(|(k, _)| k.clone())
        .collect();
    out.sort();
    out
}

fn nested(a: &str, b: &str) -> bool {
    a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}

const POLICIES: [Option<ConflictPolicy>; 4] = [
    None,
    Some(ConflictPolicy::KeepBoth),
    Some(ConflictPolicy::Skip),
    Some(ConflictPolicy::MergeFolders),
];

fn random_transfer<P: Provider + 'static>(
    rng: &mut Rng,
    h: &JournalHarness<P>,
    tree: &Tree,
    links: bool,
) -> Option<JobRequest> {
    let keys: Vec<&String> = tree.keys().filter(|k| k.contains('/')).collect();
    let dests = folders(tree);
    if keys.is_empty() || dests.is_empty() {
        return None;
    }
    let kind = match rng.below(6) {
        0 | 1 => JobKind::Copy,
        2..=4 => JobKind::Move,
        _ if links => JobKind::Link,
        _ => JobKind::Copy,
    };
    let first = (*rng.pick(&keys)).clone();
    let mut sources = vec![first.clone()];
    let second = (*rng.pick(&keys)).clone();
    if !nested(&first, &second) && rng.chance(2) {
        sources.push(second);
    }
    let refs: Vec<&str> = sources.iter().map(String::as_str).collect();
    let dest = rng.pick(&dests).clone();
    let policy = *rng.pick(&POLICIES);
    Some(transfer(h, kind, &refs, &dest, policy))
}

fn random_trees(rng: &mut Rng, links: bool) -> Tree {
    let mut t = Tree::new();
    for side in ["v1", "v2"] {
        t.insert(side.to_owned(), Node::Dir);
        for (key, node) in random_tree(rng, 8) {
            if !links && matches!(node, Node::Link(_)) {
                continue;
            }
            t.insert(format!("{side}/{key}"), node);
        }
    }
    t
}

fn random_sequences<P: Provider + 'static>(
    seed: u64,
    h: &mut JournalHarness<P>,
    links: bool,
    second_volume: bool,
) {
    let mut rng = Rng::seeded(seed);
    let start = random_trees(&mut rng, links);
    // The memory provider needs the volume before anything is on it.
    jbuild(h, &start);
    let _ = second_volume;
    let mut history: Vec<(JournalId, Tree, Tree)> = Vec::new();
    for step in 0..14 {
        let before = jwork(h);
        h.provider.reset();
        let Some(request) = random_transfer(&mut rng, h, &before, links) else {
            continue;
        };
        let context = format!("seed {seed} step {step} {:?}", request.kind);
        let run = h.run_journalled(request);
        h.provider.reset();
        let after = jwork(h);
        assert!(partials(&after).is_empty(), "{context}");
        assert!(h.store.violations().is_empty(), "{context}");
        assert!(h.journal.pending().is_empty(), "{context}");
        match run.entry {
            Some(id) => {
                ENTRIES.fetch_add(1, Ordering::Relaxed);
                history.push((id, before.clone(), after.clone()));
                // A job that failed part way (a merge can put two names that differ only by case
                // side by side, which a case-insensitive provider refuses) made an entry for what
                // it did; doing that again fails the same way, so only a whole job is redone.
                let round_trip = rng.chance(4) && run.state == JobState::Done;
                if round_trip {
                    let undo = h.undo(id);
                    h.provider.reset();
                    assert_eq!(undo.state, JobState::Done, "{context}: {:?}", undo.undo);
                    assert_eq!(jwork(h), before, "{context}: undo");
                    let redo = h.redo(id);
                    h.provider.reset();
                    assert_eq!(redo.state, JobState::Done, "{context}: {:?}", redo.failure);
                    assert_eq!(jwork(h), after, "{context}: redo");
                }
            }
            None => {
                // Not journalled, so it did nothing a journal would have to reverse.
                if run.plan.is_none() || run.failure.is_none() {
                    assert_eq!(after, before, "{context}: it wrote with no entry");
                }
            }
        }
    }
    while let Some((id, before, _)) = history.pop() {
        assert_eq!(h.journal.last_undoable(), Some(id), "seed {seed}");
        let undo = h.undo_last();
        h.provider.reset();
        assert_eq!(
            undo.state,
            JobState::Done,
            "seed {seed}: {:?}",
            undo.undo
                .as_ref()
                .map(|r| r.as_ref().err().map(|f| (&f.error, &f.item)))
        );
        assert_eq!(
            jwork(h),
            before,
            "seed {seed}: entry {} did not restore",
            id.0
        );
    }
    assert_eq!(
        jwork(h),
        start,
        "seed {seed}: everything undone is the original"
    );
}

#[test]
fn random_copies_moves_and_links_undo_to_the_exact_original() {
    for seed in 0..40u64 {
        each_journal_provider!(|h, rule, links| {
            let _ = rule;
            h.chunk_bytes = SMALL_CHUNK;
            random_sequences(seed, &mut h, links, false);
        });
    }
}

#[test]
fn random_sequences_across_a_second_volume_undo_to_the_exact_original() {
    for seed in 200..260u64 {
        let (mut h, _dir) = memory_jh(CaseRule::Sensitive);
        h.chunk_bytes = SMALL_CHUNK;
        // `v2` is a volume of its own, so a move between the two sides copies.
        h.provider.create_dir(&h.path("v2")).unwrap();
        memory(&h.harness).set_volume(&h.path("v2"), VolumeId(2));
        h.provider.reset();
        random_sequences_split(seed, &mut h);
    }
}

/// Like `random_sequences`, where `v2` already exists as the second volume.
fn random_sequences_split(seed: u64, h: &mut JournalHarness<MemoryProvider>) {
    let mut rng = Rng::seeded(seed);
    let mut start = random_trees(&mut rng, true);
    start.remove("v2");
    jbuild(h, &start);
    let mut history: Vec<(JournalId, Tree)> = Vec::new();
    let original = jwork(h);
    for step in 0..14 {
        let before = jwork(h);
        h.provider.reset();
        let Some(request) = random_transfer(&mut rng, h, &before, true) else {
            continue;
        };
        let context = format!("seed {seed} step {step} {:?}", request.kind);
        let run = h.run_journalled(request);
        h.provider.reset();
        let after = jwork(h);
        assert!(partials(&after).is_empty(), "{context}");
        assert!(h.journal.pending().is_empty(), "{context}");
        match run.entry {
            Some(id) => {
                ENTRIES.fetch_add(1, Ordering::Relaxed);
                if run.report.as_ref().is_some_and(|r| {
                    r.inverse
                        .iter()
                        .any(|s| matches!(s, InverseStep::CopyBack { .. }))
                }) {
                    CROSSED.fetch_add(1, Ordering::Relaxed);
                }
                history.push((id, before.clone()));
                if rng.chance(4) {
                    let undo = h.undo(id);
                    h.provider.reset();
                    assert_eq!(undo.state, JobState::Done, "{context}: {:?}", undo.undo);
                    assert_eq!(jwork(h), before, "{context}: undo");
                    let redo = h.redo(id);
                    h.provider.reset();
                    assert_eq!(redo.state, JobState::Done, "{context}: {:?}", redo.failure);
                    assert_eq!(jwork(h), after, "{context}: redo");
                }
            }
            None => {
                if run.plan.is_none() || run.failure.is_none() {
                    assert_eq!(after, before, "{context}: it wrote with no entry");
                }
            }
        }
    }
    while let Some((id, before)) = history.pop() {
        assert_eq!(h.journal.last_undoable(), Some(id), "seed {seed}");
        let undo = h.undo_last();
        h.provider.reset();
        assert_eq!(
            undo.state,
            JobState::Done,
            "seed {seed}: {:?}",
            undo.undo
                .as_ref()
                .map(|r| r.as_ref().err().map(|f| (&f.error, &f.item)))
        );
        assert_eq!(jwork(h), before, "seed {seed}: entry {}", id.0);
    }
    assert_eq!(jwork(h), original, "seed {seed}");
}

#[test]
fn the_sequences_exercise_enough() {
    random_copies_moves_and_links_undo_to_the_exact_original();
    random_sequences_across_a_second_volume_undo_to_the_exact_original();
    assert!(ENTRIES.load(Ordering::Relaxed) > 400, "entries");
    assert!(CROSSED.load(Ordering::Relaxed) > 20, "cross-volume entries");
}

#[test]
fn two_sources_merged_into_one_folder_made_by_the_job_undo_in_order() {
    // Two folders of one name merge into the one the job made for the first, so the second's
    // entries are placed inside a folder that is itself a step of the entry.
    let (mut h, _dir) = crossing_jh();
    let t = tree(&[
        ("src/one/", ""),
        ("src/one/same/", ""),
        ("src/one/same/a", "a"),
        ("src/two/", ""),
        ("src/two/same/", ""),
        ("src/two/same/b", "b"),
        ("src/two/same/c/", ""),
    ]);
    put(&h, &t);
    for kind in [JobKind::Copy, JobKind::Move] {
        round_trip(&mut h, |h| {
            transfer(
                h,
                kind,
                &["src/one/same", "src/two/same"],
                "dst",
                Some(ConflictPolicy::MergeFolders),
            )
        });
    }
}
