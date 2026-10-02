// The simple operations end to end: each runs through the queue and the executor over every
// provider, and the tree that is left is compared with what the operation promises.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use waypoint_protocol::Location;
use waypoint_vfs::{CancelToken, FileTimes};

fn done(request: JobRequest, h: &mut Harness<impl Provider + 'static>) -> ExecReport {
    let result = h.run(request);
    assert_eq!(result.state, JobState::Done, "{:?}", result.failure);
    result.report.expect("a finished job has a report")
}

fn refused(request: JobRequest, h: &mut Harness<impl Provider + 'static>) -> OpsError {
    let before = work_tree(h);
    let result = h.run(request);
    assert_eq!(h.provider.write_calls(), 0, "a refused job writes nothing");
    assert_eq!(work_tree(h), before);
    error_of(&result.state).clone()
}

#[test]
fn create_makes_a_folder_and_a_file() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        let report = done(
            h.request(JobKind::CreateFolder, &[], Some(""), Some("docs")),
            &mut h,
        );
        assert_eq!(report.created, vec![h.loc("docs")]);
        done(
            h.request(JobKind::CreateFile, &[], Some("docs"), Some("notes.txt")),
            &mut h,
        );
        let t = work_tree(&h);
        assert_eq!(t.get("docs"), Some(&Node::Dir));
        assert_eq!(t.get("docs/notes.txt"), Some(&file("")));
    });
}

#[test]
fn create_refuses_a_taken_name_and_keeps_both_when_told_to() {
    each_provider!(|h, rule, links| {
        let _ = links;
        build(&h, &tree(&[("a.txt", "x")]));
        let request = h.request(JobKind::CreateFile, &[], Some(""), Some("a.txt"));
        assert!(matches!(
            refused(request.clone(), &mut h),
            OpsError::NameInUse { .. }
        ));
        // A name differing only in case is taken exactly where the provider folds case.
        let upper = h.request(JobKind::CreateFile, &[], Some(""), Some("A.TXT"));
        match rule {
            CaseRule::Insensitive => {
                assert!(matches!(refused(upper, &mut h), OpsError::NameInUse { .. }))
            }
            CaseRule::Sensitive => {
                done(upper, &mut h);
                assert!(work_tree(&h).contains_key("A.TXT"));
            }
        }
        let mut keep = request;
        keep.options.conflict = Some(ConflictPolicy::KeepBoth);
        let report = done(keep, &mut h);
        assert_eq!(report.created, vec![h.loc("a (2).txt")]);
        assert_eq!(work_tree(&h).get("a.txt"), Some(&file("x")));
    });
}

#[test]
fn create_without_a_name_picks_a_free_default() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        done(
            h.request(JobKind::CreateFolder, &[], Some(""), None),
            &mut h,
        );
        done(
            h.request(JobKind::CreateFolder, &[], Some(""), None),
            &mut h,
        );
        let t = work_tree(&h);
        assert!(t.contains_key("New folder") && t.contains_key("New folder (2)"));
    });
}

#[test]
fn create_checks_the_destination_and_the_name() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("file", "x")]));
        assert!(matches!(
            refused(
                h.request(JobKind::CreateFolder, &[], Some("missing"), Some("a")),
                &mut h
            ),
            OpsError::NotFound { .. }
        ));
        assert!(matches!(
            refused(
                h.request(JobKind::CreateFolder, &[], Some("file"), Some("a")),
                &mut h
            ),
            OpsError::Io { .. }
        ));
        for bad in ["", ".", "..", "a/b", "../escape", "a\0b"] {
            assert!(
                matches!(
                    refused(
                        h.request(JobKind::CreateFile, &[], Some(""), Some(bad)),
                        &mut h
                    ),
                    OpsError::InvalidName { .. }
                ),
                "{bad:?}"
            );
        }
    });
}

#[test]
fn windows_names_are_refused_only_under_the_case_insensitive_rules() {
    each_provider!(|h, rule, links| {
        let _ = links;
        for name in ["CON", "nul.txt", "a:b", "trailing.", "q?"] {
            let request = h.request(JobKind::CreateFile, &[], Some(""), Some(name));
            match rule {
                CaseRule::Insensitive => assert!(
                    matches!(refused(request, &mut h), OpsError::InvalidName { .. }),
                    "{name}"
                ),
                // On Windows a path cannot hold some of these names at all, whatever the case
                // rule, so a sensitive provider's own answer is not the one under test there.
                CaseRule::Sensitive if cfg!(windows) => {}
                CaseRule::Sensitive => {
                    // Linux accepts them; on a real Windows host the local provider is the
                    // insensitive one and this arm is not reached.
                    done(request, &mut h);
                    assert!(work_tree(&h).contains_key(name));
                }
            }
        }
    });
}

#[test]
fn rename_changes_the_name_and_keeps_the_content() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a.txt", "x"), ("d/", ""), ("d/in", "y")]));
        let report = done(
            h.request(JobKind::Rename, &["a.txt"], None, Some("b.txt")),
            &mut h,
        );
        assert_eq!(report.renamed, vec![(h.loc("a.txt"), h.loc("b.txt"))]);
        done(h.request(JobKind::Rename, &["d"], None, Some("e")), &mut h);
        let t = work_tree(&h);
        assert_eq!(t.get("b.txt"), Some(&file("x")));
        assert_eq!(t.get("e/in"), Some(&file("y")));
        assert!(!t.contains_key("a.txt") && !t.contains_key("d"));
    });
}

#[test]
fn rename_refuses_what_is_taken_or_meaningless() {
    each_provider!(|h, rule, links| {
        let _ = links;
        build(&h, &tree(&[("a", "1"), ("b", "2")]));
        assert!(matches!(
            refused(h.request(JobKind::Rename, &["a"], None, Some("b")), &mut h),
            OpsError::NameInUse { .. }
        ));
        assert_eq!(
            refused(h.request(JobKind::Rename, &["a"], None, Some("a")), &mut h),
            OpsError::SameFolder
        );
        assert!(matches!(
            refused(
                h.request(JobKind::Rename, &["a"], None, Some("x/y")),
                &mut h
            ),
            OpsError::InvalidName { .. }
        ));
        assert!(matches!(
            refused(
                h.request(JobKind::Rename, &["missing"], None, Some("z")),
                &mut h
            ),
            OpsError::NotFound { .. }
        ));
        // Another entry that only differs in case is taken where case folds.
        let clash = h.request(JobKind::Rename, &["a"], None, Some("B"));
        match rule {
            CaseRule::Insensitive => {
                assert!(matches!(refused(clash, &mut h), OpsError::NameInUse { .. }))
            }
            CaseRule::Sensitive => {
                done(clash, &mut h);
            }
        }
    });
}

#[test]
fn a_case_only_rename_goes_through_a_third_name_where_case_folds() {
    each_provider!(|h, rule, links| {
        let _ = links;
        build(&h, &tree(&[("readme.md", "x")]));
        let renames_before = h.provider.calls_of(Op::Rename);
        done(
            h.request(JobKind::Rename, &["readme.md"], None, Some("README.MD")),
            &mut h,
        );
        let t = work_tree(&h);
        assert_eq!(t.get("README.MD"), Some(&file("x")));
        assert!(!t.contains_key("readme.md"));
        let steps = h.provider.calls_of(Op::Rename) - renames_before;
        assert_eq!(steps, if rule == CaseRule::Insensitive { 2 } else { 1 });
    });
}

#[test]
fn a_failed_second_step_of_a_case_only_rename_restores_the_name() {
    each_provider!(|h, rule, links| {
        let _ = links;
        if rule == CaseRule::Sensitive {
            continue_next();
        } else {
            build(&h, &tree(&[("readme.md", "x")]));
            h.provider
                .fail_nth(Op::Rename, 2, FaultKind::PermissionDenied);
            let result = h.run(h.request(JobKind::Rename, &["readme.md"], None, Some("README.MD")));
            assert!(matches!(
                error_of(&result.state),
                OpsError::PermissionDenied { .. }
            ));
            assert_eq!(work_tree(&h), tree(&[("readme.md", "x")]));
        }
    });
}

/// The `each_provider!` body is not a loop, so a body that applies to some providers only says so
/// with an empty `else`.
fn continue_next() {}

#[test]
fn duplicate_copies_a_file_beside_itself_with_a_free_name() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a.txt", "hello"), ("a (2).txt", "other")]));
        let report = done(
            h.request(JobKind::Duplicate, &["a.txt"], None, None),
            &mut h,
        );
        assert_eq!(report.created, vec![h.loc("a (3).txt")]);
        let t = work_tree(&h);
        assert_eq!(t.get("a (3).txt"), Some(&file("hello")));
        assert_eq!(t.get("a.txt"), Some(&file("hello")));
        assert_eq!(t.get("a (2).txt"), Some(&file("other")));
        assert!(partials(&t).is_empty());
    });
}

#[test]
fn duplicate_copies_a_tree_with_its_links_and_its_times() {
    each_provider!(|h, rule, links| {
        let _ = rule;
        let mut source = tree(&[
            ("d/", ""),
            ("d/inner/", ""),
            ("d/inner/big", ""),
            ("d/x", "xx"),
        ]);
        source.insert("d/inner/big".to_owned(), Node::File(vec![7; 200_000]));
        if links {
            source.insert("d/link".to_owned(), Node::Link("x".to_owned()));
            source.insert("d/dangling".to_owned(), Node::Link("../nowhere".to_owned()));
        }
        build(&h, &source);
        let stamp = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_500_000_000);
        h.provider
            .set_times(
                &h.path("d/x"),
                FileTimes {
                    accessed: None,
                    modified: Some(stamp),
                },
            )
            .unwrap();
        h.provider.reset();
        let report = done(h.request(JobKind::Duplicate, &["d"], None, None), &mut h);
        assert_eq!(report.created, vec![h.loc("d (2)")]);
        let t = work_tree(&h);
        for (key, node) in &source {
            assert_eq!(t.get(key), Some(node));
            let copy = format!("d (2){}", &key[1..]);
            assert_eq!(t.get(&copy), Some(node), "{copy}");
        }
        assert!(partials(&t).is_empty());
        let original = h.provider.stat(&h.path("d/x")).unwrap();
        let copy = h.provider.stat(&h.path("d (2)/x")).unwrap();
        assert_eq!(copy.modified_ms, original.modified_ms);
        assert!(report.progress.bytes_done >= 200_000);
    });
}

#[test]
fn several_duplicates_in_one_job_get_different_names() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "1"), ("d/", ""), ("d/a", "2")]));
        let report = done(
            h.request(JobKind::Duplicate, &["a", "d/a", "d"], None, None),
            &mut h,
        );
        assert_eq!(
            report.created,
            vec![h.loc("a (2)"), h.loc("d/a (2)"), h.loc("d (2)")]
        );
    });
}

#[test]
fn trash_and_restore_round_trip() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        let source = tree(&[("a.txt", "x"), ("d/", ""), ("d/in", "y")]);
        build(&h, &source);
        let report = done(
            h.request(JobKind::Trash, &["a.txt", "d"], None, None),
            &mut h,
        );
        assert_eq!(report.trashed.len(), 2);
        assert!(work_tree(&h).is_empty());
        assert_eq!(h.trash.len(), 2);
        // Restore by the locations the Trash view shows.
        let trashed: Vec<Location> = report
            .trashed
            .iter()
            .map(|r| h.trash.trashed_location(r))
            .collect();
        let request = JobRequest {
            kind: JobKind::Restore,
            sources: Sources::Locations { locations: trashed },
            destination: None,
            name: None,
            options: JobOptions::default(),
            origin_window: "main-1".to_owned(),
        };
        let restored = done(request, &mut h);
        assert_eq!(restored.restored.len(), 2);
        assert_eq!(work_tree(&h), source);
        assert!(h.trash.is_empty());
    });
}

#[test]
fn restore_never_replaces_what_took_the_name() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "old")]));
        let report = done(h.request(JobKind::Trash, &["a"], None, None), &mut h);
        build(&h, &tree(&[("a", "new")]));
        let request = JobRequest {
            kind: JobKind::Restore,
            sources: Sources::Locations {
                locations: vec![h.trash.trashed_location(&report.trashed[0])],
            },
            destination: None,
            name: None,
            options: JobOptions::default(),
            origin_window: "main-1".to_owned(),
        };
        let result = h.run(request);
        assert!(matches!(
            error_of(&result.state),
            OpsError::NameInUse { .. }
        ));
        assert_eq!(work_tree(&h).get("a"), Some(&file("new")));
        assert_eq!(h.trash.len(), 1);
    });
}

#[test]
fn trash_needs_a_trash_and_refuses_protected_places() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "x")]));
        h.trash.set_available(Err("no trash folder".to_owned()));
        assert!(matches!(
            refused(h.request(JobKind::Trash, &["a"], None, None), &mut h),
            OpsError::TrashUnavailable { .. }
        ));
        h.trash.set_available(Ok(()));
        let mut root = h.base.clone();
        while let Some(parent) = root.parent() {
            root = parent;
        }
        for protected in [h.base.clone(), root] {
            let request = JobRequest {
                kind: JobKind::Trash,
                sources: Sources::Locations {
                    locations: vec![protected.to_location()],
                },
                destination: None,
                name: None,
                options: JobOptions::default(),
                origin_window: "main-1".to_owned(),
            };
            assert!(matches!(
                refused(request, &mut h),
                OpsError::Protected { .. }
            ));
        }
    });
}

#[test]
fn delete_removes_files_and_trees_and_never_follows_a_link() {
    each_provider!(|h, rule, links| {
        let _ = rule;
        let mut source = tree(&[
            ("keep/", ""),
            ("keep/precious", "p"),
            ("gone/", ""),
            ("gone/sub/", ""),
            ("gone/sub/x", "x"),
            ("gone/y", "y"),
            ("single", "s"),
        ]);
        if links {
            source.insert("gone/to-keep".to_owned(), Node::Link("../keep".to_owned()));
        }
        build(&h, &source);
        let report = done(
            h.request(JobKind::Delete, &["gone", "single"], None, None),
            &mut h,
        );
        assert_eq!(report.deleted, vec![h.loc("gone"), h.loc("single")]);
        let t = work_tree(&h);
        assert_eq!(
            t.keys().cloned().collect::<Vec<_>>(),
            ["keep", "keep/precious"]
        );
        assert_eq!(report.progress.items_done, if links { 6 } else { 5 });
    });
}

#[test]
fn delete_refuses_roots_the_home_folder_and_mount_points() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[("mnt/", ""), ("mnt/data", "d"), ("other", "o")]),
        );
        h.protected = Protected::new(vec![h.base.clone(), h.path("mnt")]);
        h.env.protected = h.protected.clone();
        let mut root = h.base.clone();
        while let Some(parent) = root.parent() {
            root = parent;
        }
        for protected in [h.path("mnt"), h.base.clone(), root] {
            let request = JobRequest {
                kind: JobKind::Delete,
                sources: Sources::Locations {
                    locations: vec![protected.to_location()],
                },
                destination: None,
                name: None,
                options: JobOptions::default(),
                origin_window: "main-1".to_owned(),
            };
            assert!(matches!(
                refused(request, &mut h),
                OpsError::Protected { .. }
            ));
        }
        assert!(matches!(
            refused(
                h.request(JobKind::Delete, &["other", "mnt"], None, None),
                &mut h
            ),
            OpsError::Protected { .. }
        ));
        assert!(work_tree(&h).contains_key("other"));
    });
}

#[test]
fn delete_and_trash_refuse_a_folder_that_holds_a_protected_path() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[
                ("outer/", ""),
                ("outer/mnt/", ""),
                ("outer/mnt/data", "d"),
                ("outer/other", "o"),
            ]),
        );
        h.protected = Protected::new(vec![h.path("outer/mnt")]);
        h.env.protected = h.protected.clone();
        for kind in [JobKind::Delete, JobKind::Trash] {
            assert!(matches!(
                refused(h.request(kind, &["outer"], None, None), &mut h),
                OpsError::Protected { .. }
            ));
        }
        assert_eq!(work_tree(&h).len(), 4, "nothing was removed");
        // Below the protected path and beside it, removal is fine.
        let report = done(
            h.request(JobKind::Delete, &["outer/other"], None, None),
            &mut h,
        );
        assert_eq!(report.deleted.len(), 1);
    });
}

#[test]
fn delete_of_something_missing_is_refused_before_any_removal() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "x")]));
        assert!(matches!(
            refused(
                h.request(JobKind::Delete, &["a", "missing"], None, None),
                &mut h
            ),
            OpsError::NotFound { .. }
        ));
    });
}

#[test]
fn a_source_that_changed_since_planning_is_reported() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "x")]));
        let planned = h
            .plan(&h.request(JobKind::Delete, &["a"], None, None))
            .unwrap();
        // The file is replaced by a folder after the plan was made.
        h.provider.remove_file(&h.path("a")).unwrap();
        h.provider.create_dir(&h.path("a")).unwrap();
        let executor = Executor::new(h.env.clone());
        let failure = executor
            .run(JobId(1), &planned, &CancelToken::new(), &mut NullSink)
            .unwrap_err();
        assert!(matches!(failure.error, OpsError::ChangedSince { .. }));
        assert_eq!(failure.done, 0);
        assert_eq!(work_tree(&h).get("a"), Some(&Node::Dir));
    });
}

#[test]
fn kinds_without_an_executor_report_unsupported_and_write_nothing() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "x"), ("d/", "")]));
        for request in [
            h.request(JobKind::Copy, &["a"], Some("d"), None),
            h.request(JobKind::Move, &["a"], Some("d"), None),
            h.request(JobKind::BatchRename, &["a"], None, None),
            h.request(JobKind::Undo { of: JobId(1) }, &[], None, None),
            h.request(JobKind::Redo { of: JobId(1) }, &[], None, None),
        ] {
            assert!(matches!(
                refused(request, &mut h),
                OpsError::Unsupported { .. }
            ));
        }
    });
}

#[test]
fn skipping_an_item_keeps_the_rest_going() {
    // A sink that answers every error with Skip.
    struct Skipper;
    impl ExecSink for Skipper {
        fn on_error(&mut self, _: &Location, _: &OpsError) -> Option<Decision> {
            Some(Decision::Skip)
        }
    }
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "1"), ("b", "2"), ("c", "3")]));
        let planned = h
            .plan(&h.request(JobKind::Delete, &["a", "b", "c"], None, None))
            .unwrap();
        h.provider
            .fail_nth(Op::RemoveFile, 2, FaultKind::PermissionDenied);
        let executor = Executor::new(h.env.clone());
        let report = executor
            .run(JobId(1), &planned, &CancelToken::new(), &mut Skipper)
            .unwrap();
        assert_eq!(report.counts.skipped, 1);
        assert_eq!(report.skipped[0].0, h.loc("b"));
        assert_eq!(work_tree(&h), tree(&[("b", "2")]));
    });
}

#[test]
fn retrying_an_item_runs_it_again() {
    struct Retrier(u32);
    impl ExecSink for Retrier {
        fn on_error(&mut self, _: &Location, _: &OpsError) -> Option<Decision> {
            self.0 += 1;
            Some(Decision::Retry)
        }
    }
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("a", "1")]));
        let planned = h
            .plan(&h.request(JobKind::Delete, &["a"], None, None))
            .unwrap();
        h.provider
            .fail_nth(Op::RemoveFile, 1, FaultKind::Interrupted);
        let executor = Executor::new(h.env.clone());
        let mut sink = Retrier(0);
        executor
            .run(JobId(1), &planned, &CancelToken::new(), &mut sink)
            .unwrap();
        assert_eq!(sink.0, 1);
        assert!(work_tree(&h).is_empty());
    });
}
