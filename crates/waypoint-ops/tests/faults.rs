// Fault injection and cancellation at every step of each simple operation: whichever call fails
// or is cancelled, the tree ends as the original or the complete result (never a half item), no
// partial file is left behind, and the job ends in a state that says what happened.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;

struct Scenario<P: Provider + 'static> {
    name: &'static str,
    /// The tree to start from, given whether the provider can make symlinks.
    tree: fn(bool) -> Tree,
    /// Runs before the fault is scripted (to put something in the Trash).
    prepare: fn(&mut Harness<P>),
    request: fn(&Harness<P>) -> JobRequest,
    /// A permanent delete cannot be atomic across a tree: it may stop with part removed.
    removal: bool,
    /// Also script a second failure on the very next call, which lands in the cleanup.
    double: bool,
    /// Trees that are also whole, because a request with several items does them one at a time.
    between: fn() -> Vec<Tree>,
}

fn no_between() -> Vec<Tree> {
    Vec::new()
}

fn nothing<P: Provider + 'static>(_: &mut Harness<P>) {}

fn with_links(links: bool, mut base: Tree) -> Tree {
    if links {
        base.insert("tree/link".to_owned(), Node::Link("a".to_owned()));
    }
    base
}

fn scenarios<P: Provider + 'static>() -> Vec<Scenario<P>> {
    vec![
        Scenario {
            name: "create folder",
            tree: |_| tree(&[("keep", "k")]),
            prepare: nothing,
            request: |h| h.request(JobKind::CreateFolder, &[], Some(""), Some("new")),
            removal: false,
            double: false,
            between: no_between,
        },
        Scenario {
            name: "create file, keep both",
            tree: |_| tree(&[("n", "x")]),
            prepare: nothing,
            request: |h| {
                let mut r = h.request(JobKind::CreateFile, &[], Some(""), Some("n"));
                r.options.conflict = Some(ConflictPolicy::KeepBoth);
                r
            },
            removal: false,
            double: false,
            between: no_between,
        },
        Scenario {
            name: "rename",
            tree: |_| tree(&[("a", "x"), ("b", "y")]),
            prepare: nothing,
            request: |h| h.request(JobKind::Rename, &["a"], None, Some("c")),
            removal: false,
            double: false,
            between: no_between,
        },
        Scenario {
            name: "case-only rename of a folder",
            tree: |_| tree(&[("Dir/", ""), ("Dir/f", "1")]),
            prepare: nothing,
            request: |h| h.request(JobKind::Rename, &["Dir"], None, Some("DIR")),
            removal: false,
            double: false,
            between: no_between,
        },
        Scenario {
            name: "duplicate a file of several chunks",
            tree: |_| {
                let mut t = tree(&[("keep", "k")]);
                t.insert("big.bin".to_owned(), Node::File(vec![3; 150_000]));
                t
            },
            prepare: nothing,
            request: |h| h.request(JobKind::Duplicate, &["big.bin"], None, None),
            removal: false,
            double: true,
            between: no_between,
        },
        Scenario {
            name: "duplicate a tree",
            tree: |links| {
                with_links(
                    links,
                    tree(&[
                        ("tree/", ""),
                        ("tree/a", "aa"),
                        ("tree/sub/", ""),
                        ("tree/sub/b", "bb"),
                        ("tree/sub/c", "cc"),
                    ]),
                )
            },
            prepare: nothing,
            request: |h| h.request(JobKind::Duplicate, &["tree"], None, None),
            removal: false,
            double: true,
            between: no_between,
        },
        Scenario {
            name: "trash two items",
            tree: |_| tree(&[("a", "1"), ("d/", ""), ("d/x", "2"), ("keep", "k")]),
            prepare: nothing,
            request: |h| h.request(JobKind::Trash, &["a", "d"], None, None),
            removal: false,
            double: false,
            between: || {
                vec![
                    tree(&[("d/", ""), ("d/x", "2"), ("keep", "k")]),
                    tree(&[("a", "1"), ("keep", "k")]),
                ]
            },
        },
        Scenario {
            name: "restore",
            tree: |_| tree(&[("a", "1"), ("keep", "k")]),
            prepare: |h| {
                let result = h.run(h.request(JobKind::Trash, &["a"], None, None));
                assert_eq!(result.state, JobState::Done);
            },
            request: |h| JobRequest {
                kind: JobKind::Restore,
                sources: Sources::Locations {
                    locations: h
                        .trash
                        .receipts()
                        .iter()
                        .map(|r| h.trash.trashed_location(r))
                        .collect(),
                },
                destination: None,
                name: None,
                options: JobOptions::default(),
                origin_window: "main-1".to_owned(),
            },
            removal: false,
            double: false,
            between: no_between,
        },
        Scenario {
            name: "delete a tree and a file",
            tree: |links| {
                with_links(
                    links,
                    tree(&[
                        ("tree/", ""),
                        ("tree/a", "aa"),
                        ("tree/sub/", ""),
                        ("tree/sub/b", "bb"),
                        ("single", "s"),
                        ("keep", "k"),
                    ]),
                )
            },
            prepare: nothing,
            request: |h| h.request(JobKind::Delete, &["tree", "single"], None, None),
            removal: true,
            double: false,
            between: no_between,
        },
    ]
}

fn is_subset(smaller: &Tree, larger: &Tree) -> bool {
    smaller.iter().all(|(k, v)| larger.get(k) == Some(v))
}

fn check_engine<P: Provider + 'static>(h: &Harness<P>, context: &str) {
    assert!(h.store.violations().is_empty(), "{context}");
    assert_eq!(
        h.replayed(),
        h.store.snapshot(),
        "{context}: replay differs"
    );
}

/// Runs a scenario once to learn how many calls it makes, then once more for every call with each
/// kind of fault at that call.
fn fault_sweep<P: Provider + 'static>(
    make: &dyn Fn() -> (Harness<P>, tempfile::TempDir),
    links: bool,
    scenario: &Scenario<P>,
) {
    let start = |_: ()| {
        let (mut h, guard) = make();
        build(&h, &(scenario.tree)(links));
        (scenario.prepare)(&mut h);
        h.provider.reset();
        (h, guard)
    };
    let (mut baseline, _g) = start(());
    let before = work_tree(&baseline);
    baseline.provider.reset();
    let request = (scenario.request)(&baseline);
    let result = baseline.run(request.clone());
    assert_eq!(
        result.state,
        JobState::Done,
        "{}: {:?}",
        scenario.name,
        result.failure
    );
    let calls = baseline.provider.calls();
    baseline.provider.reset();
    let after = work_tree(&baseline);
    assert_ne!(
        before, after,
        "{}: the operation does something",
        scenario.name
    );
    assert!(calls >= 1, "{}", scenario.name);

    for kind in FaultKind::ALL {
        for call in 1..=calls {
            let doubles: &[bool] = if scenario.double {
                &[false, true]
            } else {
                &[false]
            };
            for &double in doubles {
                let (mut h, _g) = start(());
                h.provider.fail_at(call, kind);
                if double {
                    h.provider.fail_at(call + 1, FaultKind::PermissionDenied);
                }
                let context = format!(
                    "{} / {kind:?} at call {call}{}",
                    scenario.name,
                    if double { " and the next" } else { "" }
                );
                let result = h.run((scenario.request)(&h));
                h.provider.reset();
                let tree = work_tree(&h);
                let clean = without_partials(&tree);
                if !double {
                    assert!(
                        partials(&tree).is_empty(),
                        "{context}: partials {:?}",
                        partials(&tree)
                    );
                }
                match &result.state {
                    JobState::Done => assert_eq!(clean, after, "{context}: done but different"),
                    JobState::Failed { error, item, .. } => {
                        assert_ne!(*error, OpsError::Cancelled, "{context}");
                        // The planner's refusals name no item; a failed step names its item.
                        if result.plan.is_some() {
                            assert!(item.is_some(), "{context}: no item recorded");
                        }
                        if scenario.removal {
                            assert!(is_subset(&clean, &before), "{context}: {clean:?}");
                        } else {
                            assert!(
                                clean == before
                                    || clean == after
                                    || (scenario.between)().contains(&clean),
                                "{context}: a half result {clean:?}"
                            );
                        }
                        if result.plan.is_none() {
                            assert_eq!(clean, before, "{context}: a refusal wrote");
                            assert_eq!(h.provider.write_calls(), 0);
                        }
                    }
                    other => panic!("{context}: {other:?}"),
                }
                check_engine(&h, &context);
            }
        }
    }

    // A removal that stopped part way finishes when run again.
    if scenario.removal {
        for call in 1..=calls {
            let (mut h, _g) = start(());
            h.provider.fail_at(call, FaultKind::Interrupted);
            let first = h.run((scenario.request)(&h));
            h.provider.reset();
            if matches!(first.state, JobState::Failed { .. }) {
                let mut again = (scenario.request)(&h);
                // Only the sources that are still there.
                if let Sources::Locations { locations } = &mut again.sources {
                    locations.retain(|l| {
                        let path = VfsPath::from_location(l).unwrap();
                        h.provider.stat(&path).is_ok()
                    });
                }
                h.provider.reset();
                let second = h.run(again);
                assert_eq!(second.state, JobState::Done, "retry after call {call}");
                assert_eq!(work_tree(&h), after, "retry after call {call}");
            }
        }
    }
}

/// Cancels at every call. The job is cancelled or had already finished, and leaves nothing but the
/// original or the result (removals: a smaller original), and no partial file.
fn cancel_sweep<P: Provider + 'static>(
    make: &dyn Fn() -> (Harness<P>, tempfile::TempDir),
    links: bool,
    scenario: &Scenario<P>,
) {
    let start = || {
        let (mut h, guard) = make();
        build(&h, &(scenario.tree)(links));
        (scenario.prepare)(&mut h);
        h.provider.reset();
        (h, guard)
    };
    let (mut baseline, _g) = start();
    let before = work_tree(&baseline);
    baseline.provider.reset();
    let request = (scenario.request)(&baseline);
    assert_eq!(baseline.run(request).state, JobState::Done);
    let calls = baseline.provider.calls();
    baseline.provider.reset();
    let after = work_tree(&baseline);

    let mut cancelled_somewhere = false;
    for call in 1..=calls + 1 {
        let (mut h, _g) = start();
        let request = (scenario.request)(&h);
        let result = h.run_hooked(request, &mut |h, token| h.provider.cancel_at(call, token));
        h.provider.reset();
        let tree = work_tree(&h);
        let context = format!("{} / cancel at call {call}", scenario.name);
        assert!(partials(&tree).is_empty(), "{context}: partials");
        match &result.state {
            JobState::Done => assert_eq!(tree, after, "{context}"),
            JobState::Cancelled => {
                cancelled_somewhere = true;
                if scenario.removal {
                    assert!(is_subset(&tree, &before), "{context}");
                } else {
                    assert!(
                        tree == before || tree == after || (scenario.between)().contains(&tree),
                        "{context}: {tree:?}"
                    );
                }
                if let Some(failure) = &result.failure {
                    assert_eq!(failure.error, OpsError::Cancelled);
                }
            }
            other => panic!("{context}: {other:?}"),
        }
        check_engine(&h, &context);
    }
    // A job of one call has no step left to notice a cancel at.
    assert!(cancelled_somewhere || calls < 3, "{}", scenario.name);
}

macro_rules! sweep_over_providers {
    ($name:ident, $sweep:ident) => {
        #[test]
        fn $name() {
            for scenario in scenarios::<LocalProvider>() {
                eprintln!("local: {}", scenario.name);
                $sweep(&local_harness, cfg!(unix), &scenario);
            }
            for rule in [CaseRule::Sensitive, CaseRule::Insensitive] {
                for scenario in scenarios::<MemoryProvider>() {
                    eprintln!("memory {rule:?}: {}", scenario.name);
                    $sweep(&|| memory_harness(rule), true, &scenario);
                }
            }
        }
    };
}

sweep_over_providers!(
    a_fault_at_any_step_leaves_the_original_or_the_result,
    fault_sweep
);
sweep_over_providers!(
    a_cancel_at_any_step_leaves_the_original_or_the_result,
    cancel_sweep
);
