// Seeded random sequences of undoable jobs: undoing everything in order restores the exact original
// tree, redo reproduces the result, and an undo either does what it should or refuses and leaves
// the tree alone. On the local provider and on the in-memory provider under both case rules.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;
mod journal_support;

use std::sync::atomic::{AtomicUsize, Ordering};

use common::*;
use journal_support::*;

static ENTRIES: AtomicUsize = AtomicUsize::new(0);
static UNDONE: AtomicUsize = AtomicUsize::new(0);
static REFUSED: AtomicUsize = AtomicUsize::new(0);

const NAMES: [&str; 8] = [
    "a", "B", "b", "c.txt", "C.TXT", "d (2).md", "e.tar.gz", ".hid",
];

fn without_links(tree: Tree, links: bool) -> Tree {
    if links {
        return tree;
    }
    tree.into_iter()
        .filter(|(_, node)| !matches!(node, Node::Link(_)))
        .collect()
}

fn folders(tree: &Tree) -> Vec<String> {
    let mut out = vec![String::new()];
    out.extend(
        tree.iter()
            .filter(|(_, n)| **n == Node::Dir)
            .map(|(k, _)| k.clone()),
    );
    out
}

fn nested(a: &str, b: &str) -> bool {
    a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}

/// A request over what the tree holds now, or none when there is nothing to act on.
fn random_request<P: Provider + 'static>(
    rng: &mut Rng,
    h: &JournalHarness<P>,
    tree: &Tree,
    kinds: usize,
) -> Option<JobRequest> {
    let keys: Vec<&String> = tree.keys().collect();
    let name = *rng.pick(&NAMES);
    match rng.below(kinds) {
        0 | 1 => {
            let kind = if rng.chance(2) {
                JobKind::CreateFolder
            } else {
                JobKind::CreateFile
            };
            let dest = rng.pick(&folders(tree)).clone();
            let explicit = rng.chance(2).then_some(name);
            Some(h.request(kind, &[], Some(&dest), explicit))
        }
        2 | 3 => {
            if keys.is_empty() {
                return None;
            }
            let source = (*rng.pick(&keys)).clone();
            Some(h.request(JobKind::Rename, &[&source], None, Some(name)))
        }
        4 => {
            if keys.is_empty() {
                return None;
            }
            let first = (*rng.pick(&keys)).clone();
            let mut sources = vec![first.clone()];
            let second = (*rng.pick(&keys)).clone();
            if !nested(&first, &second) && rng.chance(2) {
                sources.push(second);
            }
            let refs: Vec<&str> = sources.iter().map(String::as_str).collect();
            Some(h.request(JobKind::Duplicate, &refs, None, None))
        }
        5 | 6 => {
            if keys.is_empty() {
                return None;
            }
            let first = (*rng.pick(&keys)).clone();
            let mut sources = vec![first.clone()];
            let second = (*rng.pick(&keys)).clone();
            if !nested(&first, &second) && rng.chance(2) {
                sources.push(second);
            }
            let refs: Vec<&str> = sources.iter().map(String::as_str).collect();
            Some(h.request(JobKind::Trash, &refs, None, None))
        }
        _ => {
            // Permanent delete: not undoable, and it can make an older undo impossible.
            if keys.is_empty() {
                return None;
            }
            let source = (*rng.pick(&keys)).clone();
            Some(h.request(JobKind::Delete, &[&source], None, None))
        }
    }
}

struct Recorded {
    id: JournalId,
    before: Tree,
    after: Tree,
}

#[test]
fn undoing_everything_in_order_restores_the_exact_original_tree() {
    for seed in 0..30u64 {
        each_journal_provider!(|h, rule, links| {
            let mut rng = Rng::seeded(seed);
            let _ = rule;
            let start = without_links(random_tree(&mut rng, 10), links);
            jbuild(&h, &start);
            let mut history: Vec<Recorded> = Vec::new();
            for step in 0..16 {
                let tree = jwork(&h);
                h.provider.reset();
                let Some(request) = random_request(&mut rng, &h, &tree, 7) else {
                    continue;
                };
                let run = h.run_journalled(request);
                h.provider.reset();
                let after = jwork(&h);
                let context = format!("seed {seed} step {step}");
                assert!(partials(&after).is_empty(), "{context}");
                assert!(h.store.violations().is_empty(), "{context}");
                match (&run.state, run.entry) {
                    (JobState::Done, Some(id)) => {
                        ENTRIES.fetch_add(1, Ordering::Relaxed);
                        history.push(Recorded {
                            id,
                            before: tree,
                            after: after.clone(),
                        })
                    }
                    (JobState::Done, None) => {}
                    (_, entry) => {
                        assert_eq!(entry, None, "{context}: {:?}", run.state);
                        if run.plan.is_none() {
                            assert_eq!(after, tree, "{context}: a refusal wrote");
                        }
                    }
                }
                assert!(h.journal.pending().is_empty(), "{context}");

                // Now and then undo and redo the newest entry: the pre-state, then the result.
                if rng.chance(4) {
                    if let Some(last) = history.last() {
                        if h.journal.last_undoable() == Some(last.id) && after == last.after {
                            let id = last.id;
                            let undo = h.undo(id);
                            h.provider.reset();
                            assert_eq!(undo.state, JobState::Done, "{context}: {:?}", undo.undo);
                            assert_eq!(jwork(&h), last.before, "{context}: undo");
                            h.provider.reset();
                            let redo = h.redo(id);
                            h.provider.reset();
                            assert_eq!(redo.state, JobState::Done, "{context}: {:?}", redo.failure);
                            assert_eq!(jwork(&h), last.after, "{context}: redo");
                        }
                    }
                }
            }

            // A permanent delete cannot be undone and may stop an older undo; this property is
            // about sequences without one, so only check the entries since the last delete.
            // (The delete case is in `an_undo_does_its_work_or_refuses_and_changes_nothing`.)
            let deletes = h
                .events
                .iter()
                .filter(|e| {
                    matches!(
                        e,
                        OpsEvent::JobAdded { job, .. } if job.kind == JobKind::Delete
                    )
                })
                .count();
            if deletes == 0 {
                for recorded in history.iter().rev() {
                    h.provider.reset();
                    if h.journal.last_undoable() != Some(recorded.id) {
                        continue;
                    }
                    let undo = h.undo_last();
                    UNDONE.fetch_add(1, Ordering::Relaxed);
                    h.provider.reset();
                    assert_eq!(undo.state, JobState::Done, "seed {seed}: {:?}", undo.undo);
                    assert_eq!(
                        jwork(&h),
                        recorded.before,
                        "seed {seed}: entry {} did not restore its pre-state",
                        recorded.id.0
                    );
                }
                assert_eq!(
                    jwork(&h),
                    start,
                    "seed {seed}: undoing everything restores the original tree"
                );
                assert_eq!(h.journal.last_undoable(), None);
            }
        });
    }
}

#[test]
fn an_undo_does_its_work_or_refuses_and_changes_nothing() {
    for seed in 100..130u64 {
        each_journal_provider!(|h, rule, links| {
            let _ = rule;
            let mut rng = Rng::seeded(seed);
            let start = without_links(random_tree(&mut rng, 10), links);
            jbuild(&h, &start);
            for step in 0..24 {
                let tree = jwork(&h);
                h.provider.reset();
                let context = format!("seed {seed} step {step}");
                let ids: Vec<JournalId> = h.journal.entries().iter().map(|e| e.id).collect();
                if !ids.is_empty() && rng.chance(3) {
                    // Undo or redo any entry from the history, as the palette would.
                    let id = *rng.pick(&ids);
                    let entry = h.journal.entry(id).unwrap().clone();
                    let request = if rng.chance(2) {
                        h.journal.undo_request(id, "main-1")
                    } else {
                        h.journal.redo_request(id, "main-1")
                    };
                    let Ok(request) = request else {
                        continue;
                    };
                    let run = h.run_journalled(request);
                    h.provider.reset();
                    let after = jwork(&h);
                    assert!(partials(&after).is_empty(), "{context}");
                    assert!(h.journal.pending().is_empty(), "{context}");
                    assert!(h.store.violations().is_empty(), "{context}");
                    match &run.state {
                        JobState::Done => {}
                        JobState::Failed { error, .. } => {
                            if matches!(error, OpsError::UndoStale { .. }) {
                                REFUSED.fetch_add(1, Ordering::Relaxed);
                            }
                            assert!(
                                matches!(
                                    error,
                                    OpsError::UndoStale { .. }
                                        | OpsError::NameInUse { .. }
                                        | OpsError::NotFound { .. }
                                        | OpsError::InvalidName { .. }
                                        | OpsError::SameFolder
                                        | OpsError::Protected { .. }
                                ),
                                "{context}: {error:?}"
                            );
                            if run.plan.is_none() {
                                assert_eq!(after, tree, "{context}: a refusal changed the tree");
                                assert_eq!(
                                    h.journal.entry(id).map(|e| e.state),
                                    Some(entry.state),
                                    "{context}"
                                );
                            }
                        }
                        other => panic!("{context}: {other:?}"),
                    }
                    continue;
                }
                let Some(request) = random_request(&mut rng, &h, &tree, 8) else {
                    continue;
                };
                let run = h.run_journalled(request);
                h.provider.reset();
                let after = jwork(&h);
                assert!(partials(&after).is_empty(), "{context}");
                assert!(h.journal.pending().is_empty(), "{context}");
                if run.plan.is_none() {
                    assert_eq!(after, tree, "{context}: a refusal wrote");
                }
            }
            // The history never exceeds the cap, and ids only go up.
            let ids: Vec<u64> = h.journal.entries().iter().map(|e| e.id.0).collect();
            assert!(ids.len() <= 50);
            assert!(ids.windows(2).all(|w| w[0] < w[1]));
            let revisions: Vec<u64> = h.journal_events.iter().map(OpsEvent::revision).collect();
            assert!(revisions.windows(2).all(|w| w[0] < w[1]), "{revisions:?}");
        });
    }
}

#[test]
fn the_properties_exercise_enough() {
    // Runs after the others by name order only by luck, so do the sampling itself.
    undoing_everything_in_order_restores_the_exact_original_tree();
    an_undo_does_its_work_or_refuses_and_changes_nothing();
    assert!(ENTRIES.load(Ordering::Relaxed) > 300, "entries");
    assert!(UNDONE.load(Ordering::Relaxed) > 100, "undone");
    assert!(REFUSED.load(Ordering::Relaxed) > 5, "refusals");
}
