// The planner: what it refuses, what it finds, and that it only ever reads.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{CancelToken, ListingHandle, SelectionSpec, VolumeId, VolumeSpace};

fn plan_ok(h: &Harness<impl Provider + 'static>, request: &JobRequest) -> Plan {
    let planned = h.plan(request).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(h.provider.write_calls(), 0, "planning never writes");
    planned
}

fn plan_err(h: &Harness<impl Provider + 'static>, request: &JobRequest) -> OpsError {
    let error = h.plan(request).expect_err("the plan is refused");
    assert_eq!(
        h.provider.write_calls(),
        0,
        "a refusal comes before any write"
    );
    error
}

#[test]
fn a_destination_inside_a_source_is_refused() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[("a/", ""), ("a/b/", ""), ("a/b/c/", ""), ("f", "x")]),
        );
        for kind in [JobKind::Copy, JobKind::Move] {
            for dest in ["a", "a/b", "a/b/c"] {
                assert_eq!(
                    plan_err(&h, &h.request(kind, &["a"], Some(dest), None)),
                    OpsError::IntoItself,
                    "{kind:?} into {dest}"
                );
            }
            // A sibling with a name that merely starts the same way is not inside.
            h.provider.create_dir(&h.path("a2")).unwrap();
            h.provider.reset();
            plan_ok(&h, &h.request(kind, &["a"], Some("a2"), None));
            h.provider.remove_dir(&h.path("a2")).unwrap();
            h.provider.reset();
        }
    });
}

#[test]
fn into_itself_follows_the_providers_case_rule() {
    each_provider!(|h, rule, links| {
        let _ = links;
        build(&h, &tree(&[("Src/", ""), ("Src/sub/", "")]));
        let request = h.request(JobKind::Copy, &["Src"], Some("src/sub"), None);
        match rule {
            CaseRule::Insensitive => assert_eq!(plan_err(&h, &request), OpsError::IntoItself),
            // On a case-sensitive provider `src` is another (missing) folder.
            CaseRule::Sensitive => {
                assert!(matches!(plan_err(&h, &request), OpsError::NotFound { .. }))
            }
        }
    });
}

#[test]
fn moving_into_the_same_folder_is_refused_and_a_mix_skips_what_is_there() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "1"), ("d/", ""), ("d/b", "2")]));
        assert_eq!(
            plan_err(&h, &h.request(JobKind::Move, &["a"], Some(""), None)),
            OpsError::SameFolder
        );
        assert_eq!(
            plan_err(&h, &h.request(JobKind::Copy, &["a"], Some(""), None)),
            OpsError::SameFolder
        );
        let mixed = plan_ok(&h, &h.request(JobKind::Move, &["a", "d/b"], Some(""), None));
        assert_eq!(mixed.items.len(), 1);
        assert_eq!(mixed.items[0].source, Some(h.path("d/b")));
        assert_eq!(
            mixed.warnings,
            vec![PlanWarning::AlreadyThere {
                location: h.loc("a")
            }]
        );
    });
}

#[test]
fn copying_into_the_same_folder_with_keep_both_gets_a_free_name() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a.txt", "1"), ("a (2).txt", "2")]));
        let mut request = h.request(JobKind::Copy, &["a.txt"], Some(""), None);
        request.options.conflict = Some(ConflictPolicy::KeepBoth);
        let planned = plan_ok(&h, &request);
        assert_eq!(planned.items[0].target, Some(h.path("a (3).txt")));
        assert!(planned.conflicts.is_empty());
    });
}

#[test]
fn top_level_name_clashes_are_found_by_the_case_rule() {
    each_provider!(|h, rule, links| {
        let _ = links;
        build(
            &h,
            &tree(&[
                ("src/", ""),
                ("src/Same", "s"),
                ("src/Folder/", ""),
                ("src/Folder/deep", "x"),
                ("src/lower", "l"),
                ("dst/", ""),
                ("dst/same", "dd"),
                ("dst/Folder", "i am a file"),
                ("dst/LOWER", "u"),
            ]),
        );
        let request = h.request(
            JobKind::Copy,
            &["src/Same", "src/Folder", "src/lower"],
            Some("dst"),
            None,
        );
        let planned = plan_ok(&h, &request);
        let found: Vec<(&str, ConflictKind)> = planned
            .conflicts
            .iter()
            .map(|c| (c.name.as_str(), c.kind))
            .collect();
        match rule {
            CaseRule::Insensitive => assert_eq!(
                found,
                [
                    ("same", ConflictKind::FileOverFile),
                    ("Folder", ConflictKind::FolderOverFile),
                    ("LOWER", ConflictKind::FileOverFile),
                ]
            ),
            CaseRule::Sensitive => {
                assert_eq!(found, [("Folder", ConflictKind::FolderOverFile)])
            }
        }
        assert!(planned.conflicts.iter().all(|c| !c.within_batch));
    });
}

#[test]
fn a_conflict_records_what_the_dialog_compares() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[("s/", ""), ("s/f", "12345"), ("d/", ""), ("d/f", "123")]),
        );
        let planned = plan_ok(&h, &h.request(JobKind::Move, &["s/f"], Some("d"), None));
        let c = &planned.conflicts[0];
        assert_eq!(c.source, h.loc("s/f"));
        assert_eq!(c.existing, h.loc("d/f"));
        assert_eq!((c.source_size, c.existing_size), (Some(5), Some(3)));
        assert!(c.source_modified_ms.is_some() && c.existing_modified_ms.is_some());
    });
}

#[test]
fn two_sources_wanting_one_name_clash_inside_the_batch() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[
                ("x/", ""),
                ("x/n", "1"),
                ("y/", ""),
                ("y/n", "2"),
                ("dst/", ""),
            ]),
        );
        let planned = plan_ok(
            &h,
            &h.request(JobKind::Copy, &["x/n", "y/n"], Some("dst"), None),
        );
        assert_eq!(planned.conflicts.len(), 1);
        assert!(planned.conflicts[0].within_batch);
        assert_eq!(planned.conflicts[0].source, h.loc("y/n"));
        assert_eq!(planned.conflicts[0].existing, h.loc("dst/n"));
    });
}

#[test]
fn folders_are_walked_for_counts_and_bytes_without_following_links() {
    each_provider!(|h, rule, links| {
        let _ = rule;
        let mut source = tree(&[
            ("t/", ""),
            ("t/a", "123"),
            ("t/sub/", ""),
            ("t/sub/b", "12345"),
            ("t/sub/deeper/", ""),
            ("t/sub/deeper/c", "1"),
            ("huge/", ""),
            ("huge/big", &"x".repeat(1000)),
            ("dst/", ""),
        ]);
        if links {
            // A link to a heavy folder, and a link back up the tree: neither is entered.
            source.insert("t/to-huge".to_owned(), Node::Link("../huge".to_owned()));
            source.insert("t/sub/up".to_owned(), Node::Link("..".to_owned()));
        }
        build(&h, &source);
        let planned = plan_ok(&h, &h.request(JobKind::Copy, &["t"], Some("dst"), None));
        let links_inside = if links { 2 } else { 0 };
        // t, a, sub, b, deeper, c and the links.
        assert_eq!(planned.total_items, 6 + links_inside);
        assert_eq!(planned.total_bytes, 9);
        assert_eq!(planned.items[0].entries, 6 + links_inside);
        // Counts for a delete are the same walk.
        let doomed = plan_ok(&h, &h.request(JobKind::Delete, &["t"], None, None));
        assert_eq!(doomed.total_items, 6 + links_inside);
    });
}

#[test]
fn the_walk_reports_progress() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        let mut source = tree(&[("t/", ""), ("dst/", "")]);
        for i in 0..700 {
            source.insert(format!("t/f{i}"), Node::File(vec![1; 2]));
        }
        build(&h, &source);
        let request = h.request(JobKind::Copy, &["t"], Some("dst"), None);
        let cancel = CancelToken::new();
        let mut reports = Vec::new();
        let planned = plan_with_progress(&request, &h.plan_ctx(&cancel), &mut |p| {
            reports.push(p.clone())
        })
        .unwrap();
        assert_eq!(planned.total_items, 701);
        assert!(reports.len() >= 3, "{}", reports.len());
        assert!(reports.windows(2).all(|w| w[0].items <= w[1].items));
        assert_eq!(reports.last().unwrap().bytes, 1400);
    });
}

#[test]
fn planning_can_be_cancelled_before_and_during_the_walk() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        let mut source = tree(&[("t/", ""), ("dst/", "")]);
        for i in 0..20 {
            source.insert(format!("t/d{i}"), Node::Dir);
        }
        build(&h, &source);
        let request = h.request(JobKind::Copy, &["t"], Some("dst"), None);
        let cancel = CancelToken::new();
        cancel.cancel();
        assert_eq!(
            plan(&request, &h.plan_ctx(&cancel)),
            Err(OpsError::Cancelled)
        );
        // Cancel at every call the planner makes: it either finishes or reports Cancelled, and
        // never writes.
        let mut calls = None;
        for n in 1.. {
            h.provider.reset();
            let cancel = CancelToken::new();
            h.provider.cancel_at(n, &cancel);
            let outcome = plan(&request, &h.plan_ctx(&cancel));
            assert_eq!(h.provider.write_calls(), 0);
            match outcome {
                Err(OpsError::Cancelled) => {}
                Ok(_) => {
                    calls = Some(n);
                    break;
                }
                Err(other) => panic!("{other:?}"),
            }
        }
        assert!(calls.unwrap() > 5);
    });
}

#[test]
fn the_same_volume_is_known_not_assumed() {
    let dir = tempfile::tempdir().unwrap();
    let root = FilePath::from_path(dir.path()).unwrap();
    let memory = MemoryProvider::new(root.clone(), CaseRule::Sensitive);
    let h = Harness::new(memory.clone(), VfsPath::File(root));
    build(&h, &tree(&[("a", "1"), ("d/", ""), ("other/", "")]));
    let request = h.request(JobKind::Move, &["a"], Some("d"), None);
    assert!(plan_ok(&h, &request).same_volume);
    memory.set_volume(&h.path("other"), VolumeId(9));
    let across = h.request(JobKind::Move, &["a"], Some("other"), None);
    assert!(!plan_ok(&h, &across).same_volume);
    // A provider that cannot tell is treated as a different volume, the safe answer.
    let local = tempfile::tempdir().unwrap();
    let local_root = FilePath::from_path(local.path()).unwrap();
    let lh = Harness::new(LocalProvider::new(), VfsPath::File(local_root));
    build(&lh, &tree(&[("a", "1"), ("d/", "")]));
    assert!(plan_ok(&lh, &lh.request(JobKind::Move, &["a"], Some("d"), None)).same_volume);
}

// Only the local provider walks a path through a symlink; the in-memory one resolves links in the
// last component only, so its `canonicalize` is covered by the provider conformance tests instead.
#[cfg(unix)]
#[test]
fn a_destination_reached_through_a_symlinked_ancestor_is_still_inside_the_source() {
    {
        let (h, _dir) = local_harness();
        let mut t = tree(&[("a/", ""), ("a/b/", ""), ("a/b/c/", ""), ("other/", "")]);
        t.insert("link".to_owned(), Node::Link("a".to_owned()));
        t.insert("deep".to_owned(), Node::Link("a/b".to_owned()));
        build(&h, &t);
        for kind in [JobKind::Copy, JobKind::Move] {
            // `link` is `a`, so `link/b/c` is `a/b/c`; `deep/c` is too. (A link itself is not a
            // folder to put things in, so only the ancestor case reaches the check.)
            for dest in ["link/b", "link/b/c", "deep/c"] {
                assert_eq!(
                    plan_err(&h, &h.request(kind, &["a"], Some(dest), None)),
                    OpsError::IntoItself,
                    "{kind:?} into {dest}"
                );
            }
            // A link to somewhere else is no problem.
            plan_ok(&h, &h.request(kind, &["a"], Some("other"), None));
        }
    }
}

#[test]
fn a_delete_never_enters_a_folder_on_another_volume() {
    let dir = tempfile::tempdir().unwrap();
    let root = FilePath::from_path(dir.path()).unwrap();
    let memory = MemoryProvider::new(root.clone(), CaseRule::Sensitive);
    let h = Harness::new(memory.clone(), VfsPath::File(root));
    build(
        &h,
        &tree(&[
            ("tree/", ""),
            ("tree/a", "1"),
            ("tree/sub/", ""),
            ("tree/sub/mnt/", ""),
            ("tree/sub/mnt/data", "d"),
            ("dst/", ""),
        ]),
    );
    memory.set_volume(&h.path("tree/sub/mnt"), VolumeId(9));

    // The plan refuses before anything is removed.
    let delete = h.request(JobKind::Delete, &["tree"], None, None);
    assert_eq!(
        plan_err(&h, &delete),
        OpsError::Protected {
            location: h.path("tree/sub/mnt").to_location()
        }
    );
    // A copy may still cross volumes, so only a delete refuses.
    plan_ok(&h, &h.request(JobKind::Copy, &["tree"], Some("dst"), None));

    // The executor guards on its own, in case the mount appeared after the plan: it fails with
    // `InUse` without touching what the other volume holds.
    let error = waypoint_ops::remove_all(&memory, &h.path("tree")).unwrap_err();
    assert!(matches!(error, VfsError::InUse { .. }), "{error:?}");
    assert!(memory.stat(&h.path("tree/sub/mnt/data")).is_ok());
    // Removing only the part on the same volume as its root works.
    waypoint_ops::remove_all(&memory, &h.path("tree/a")).unwrap();
    // The mount point itself is a folder of its own volume, so removing it as a root is allowed to
    // proceed into its own contents.
    waypoint_ops::remove_all(&memory, &h.path("tree/sub/mnt")).unwrap();
}

#[test]
fn a_copy_that_does_not_fit_is_refused_but_a_same_volume_move_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let root = FilePath::from_path(dir.path()).unwrap();
    let memory = MemoryProvider::new(root.clone(), CaseRule::Sensitive);
    let h = Harness::new(memory.clone(), VfsPath::File(root));
    build(&h, &tree(&[("big", &"x".repeat(500)), ("d/", "")]));
    memory.set_space(
        VolumeId(1),
        VolumeSpace {
            free_bytes: 100,
            total_bytes: 1000,
        },
    );
    assert_eq!(
        plan_err(&h, &h.request(JobKind::Copy, &["big"], Some("d"), None)),
        OpsError::NotEnoughSpace {
            needed: 500,
            free: 100
        }
    );
    assert!(
        plan_err(&h, &h.request(JobKind::Duplicate, &["big"], None, None)).eq(
            &OpsError::NotEnoughSpace {
                needed: 500,
                free: 100
            }
        )
    );
    plan_ok(&h, &h.request(JobKind::Move, &["big"], Some("d"), None));
}

#[test]
fn a_selection_is_resolved_through_the_resolver() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "1"), ("b", "2"), ("dst/", "")]));
        *h.resolver.0.lock().unwrap() = vec![h.loc("b"), h.loc("a"), h.loc("b")];
        let request = JobRequest {
            kind: JobKind::Copy,
            sources: Sources::Selection {
                handle: ListingHandle(4),
                spec: SelectionSpec::AllExcept { ids: vec![] },
            },
            destination: Some(h.loc("dst")),
            name: None,
            options: JobOptions::default(),
            origin_window: "main-1".to_owned(),
        };
        let planned = plan_ok(&h, &request);
        let order: Vec<_> = planned.items.iter().map(|i| i.source.clone()).collect();
        assert_eq!(order, [Some(h.path("b")), Some(h.path("a"))]);
        assert_eq!(
            planned.warnings,
            vec![PlanWarning::DuplicateSource {
                location: h.loc("b")
            }]
        );
    });
}

#[test]
fn a_source_that_is_not_a_location_or_has_no_provider_is_refused() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        let nonsense = JobRequest {
            kind: JobKind::Delete,
            sources: Sources::Locations {
                locations: vec![Location::new("x", "not a uri")],
            },
            destination: None,
            name: None,
            options: JobOptions::default(),
            origin_window: "main-1".to_owned(),
        };
        assert!(matches!(plan_err(&h, &nonsense), OpsError::Io { .. }));
        let mut other = nonsense.clone();
        other.sources = Sources::Locations {
            locations: vec![Location::new("x", "sftp://host/x")],
        };
        assert!(matches!(plan_err(&h, &other), OpsError::Io { .. }));
    });
}

#[test]
fn totals_name_what_the_close_guard_needs() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("s/", ""), ("s/f", "1"), ("d/", "")]));
        let planned = plan_ok(&h, &h.request(JobKind::Move, &["s/f"], Some("d"), None));
        let totals = planned.totals();
        assert_eq!(totals.sources.count, Some(1));
        assert_eq!(totals.sources.first.as_deref(), Some("f"));
        assert!(totals.touches.contains(&h.loc("s")));
        assert!(totals.touches.contains(&h.loc("d")));
        assert_eq!(totals.trees, vec![h.loc("s/f")]);
    });
}

/// Random requests over random trees, most of them nonsense: planning never calls a write
/// primitive, whatever it decides.
#[test]
fn planning_never_writes_whatever_the_request() {
    let kinds = [
        JobKind::CreateFolder,
        JobKind::CreateFile,
        JobKind::Rename,
        JobKind::Duplicate,
        JobKind::Trash,
        JobKind::Restore,
        JobKind::Delete,
        JobKind::Copy,
        JobKind::Move,
        JobKind::BatchRename,
        JobKind::Undo { of: JournalId(1) },
    ];
    let names = [
        "a", "B", "b", "c.txt", "..", "", "x/y", "CON", "d (2).md", "nul.",
    ];
    for seed in 0..40u64 {
        let mut rng = Rng::seeded(seed);
        let dir = tempfile::tempdir().unwrap();
        let root = FilePath::from_path(dir.path()).unwrap();
        let rule = if seed % 2 == 0 {
            CaseRule::Sensitive
        } else {
            CaseRule::Insensitive
        };
        let h = Harness::new(MemoryProvider::new(root.clone(), rule), VfsPath::File(root));
        let mut source = random_tree(&mut rng, 25);
        source.retain(|_, n| !matches!(n, Node::Link(_)));
        build(&h, &source);
        let mut paths: Vec<String> = source.keys().cloned().collect();
        paths.extend(["".to_owned(), "missing".to_owned(), "a/b/c".to_owned()]);
        let before = work_tree(&h);
        for _ in 0..40 {
            let kind = *rng.pick(&kinds);
            let count = rng.below(3);
            let sources: Vec<&str> = (0..count).map(|_| rng.pick(&paths).as_str()).collect();
            let dest = rng.chance(2).then(|| rng.pick(&paths).as_str());
            let name = rng.chance(2).then(|| *rng.pick(&names));
            let mut request = h.request(kind, &sources, dest, name);
            if rng.chance(4) {
                request.options.conflict = Some(ConflictPolicy::KeepBoth);
            }
            h.provider.reset();
            let _ = h.plan(&request);
            assert_eq!(h.provider.write_calls(), 0, "seed {seed}: {request:?}");
        }
        assert_eq!(work_tree(&h), before);
    }
}
