// Conflicts executed by policy (A48): nothing is overwritten without a decision, each policy does
// what it says, a clash below a merged folder is decided by the job's policy or asked about alone,
// and a replacement that fails puts the original back.
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

#[test]
fn a_clash_waits_for_an_answer_and_nothing_is_written_meanwhile() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[
                ("src/", ""),
                ("src/a", "new"),
                ("dst/", ""),
                ("dst/a", "old"),
            ]),
        );
        for kind in [JobKind::Copy, JobKind::Move] {
            h.provider.reset();
            let result = go(&mut h, kind, &["src/a"], "dst", None);
            // Parked, with the clash in its reason and no default chosen.
            let JobState::Waiting {
                reason: WaitReason::Conflicts { conflicts },
            } = &result.state
            else {
                panic!("{:?}", result.state);
            };
            assert_eq!(conflicts.len(), 1);
            assert_eq!(conflicts[0].kind, ConflictKind::FileOverFile);
            assert_eq!(conflicts[0].source, h.loc("src/a"));
            assert_eq!(conflicts[0].existing, h.loc("dst/a"));
            assert_eq!(h.provider.write_calls(), 0, "{kind:?} wrote");
            assert_eq!(dst_tree(&h), tree(&[("a", "old")]));
            assert_eq!(src_tree(&h), tree(&[("a", "new")]));
        }
    });
}

#[test]
fn replace_swaps_the_file_in_and_leaves_nothing_aside() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[
                ("src/", ""),
                ("src/a", "new"),
                ("dst/", ""),
                ("dst/a", "old"),
            ]),
        );
        let result = go(
            &mut h,
            JobKind::Copy,
            &["src/a"],
            "dst",
            Some(ConflictPolicy::Replace),
        );
        done(&result);
        assert_eq!(dst_tree(&h), tree(&[("a", "new")]));
        assert_eq!(src_tree(&h), tree(&[("a", "new")]));
        let report = result.report.unwrap();
        assert_eq!(report.transfer.replaced, vec![h.loc("dst/a")]);
        assert!(report.transfer.leftovers.is_empty());
        assert!(leftovers(&work_tree(&h)).is_empty());
    });
}

#[test]
fn skip_leaves_both_sides_and_counts_the_skip() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[
                ("src/", ""),
                ("src/a", "new"),
                ("src/b", "bee"),
                ("dst/", ""),
                ("dst/a", "old"),
            ]),
        );
        for kind in [JobKind::Copy, JobKind::Move] {
            let result = go(
                &mut h,
                kind,
                &["src/a", "src/b"],
                "dst",
                Some(ConflictPolicy::Skip),
            );
            done(&result);
            assert_eq!(dst_tree(&h), tree(&[("a", "old"), ("b", "bee")]));
            let report = result.report.unwrap();
            assert_eq!(report.counts.skipped, 1);
            assert_eq!(report.counts.failed, 0);
            assert_eq!(report.skipped[0].0, h.loc("src/a"));
            // A moved `b` is gone from the source; the skipped `a` stays.
            let expected = if kind == JobKind::Move {
                tree(&[("a", "new")])
            } else {
                tree(&[("a", "new"), ("b", "bee")])
            };
            assert_eq!(src_tree(&h), expected);
            // Put things back for the next kind.
            if kind == JobKind::Move {
                put_bytes(&h, "src/b", b"bee");
            }
            h.provider.remove_file(&h.path("dst/b")).unwrap();
        }
    });
}

#[test]
fn keep_both_never_collides() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[
                ("src/", ""),
                ("src/a.txt", "n1"),
                ("src/d/", ""),
                ("src/d/f", "inside"),
                ("dst/", ""),
                ("dst/a.txt", "o1"),
                ("dst/a (2).txt", "o2"),
                ("dst/d/", ""),
                ("dst/d (2)", "a file in the way of the next name"),
            ]),
        );
        let result = go(
            &mut h,
            JobKind::Copy,
            &["src/a.txt", "src/d"],
            "dst",
            Some(ConflictPolicy::KeepBoth),
        );
        done(&result);
        let now = dst_tree(&h);
        assert_eq!(now.get("a.txt"), Some(&file("o1")));
        assert_eq!(now.get("a (2).txt"), Some(&file("o2")));
        assert_eq!(now.get("a (3).txt"), Some(&file("n1")));
        assert_eq!(
            now.get("d (2)"),
            Some(&file("a file in the way of the next name"))
        );
        assert_eq!(now.get("d (3)"), Some(&Node::Dir));
        assert_eq!(now.get("d (3)/f"), Some(&file("inside")));
        assert_eq!(now.get("d"), Some(&Node::Dir));
        assert!(!now.contains_key("d/f"));
        assert_eq!(
            result.report.unwrap().created,
            vec![h.loc("dst/a (3).txt"), h.loc("dst/d (3)")]
        );
    });
}

#[test]
fn keep_both_for_a_move_gives_the_new_name_and_removes_the_source() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[("src/", ""), ("src/a", "n"), ("dst/", ""), ("dst/a", "o")]),
        );
        let result = go(
            &mut h,
            JobKind::Move,
            &["src/a"],
            "dst",
            Some(ConflictPolicy::KeepBoth),
        );
        done(&result);
        assert_eq!(dst_tree(&h), tree(&[("a", "o"), ("a (2)", "n")]));
        assert!(src_tree(&h).is_empty());
        assert_eq!(
            result.report.unwrap().renamed,
            vec![(h.loc("src/a"), h.loc("dst/a (2)"))]
        );
    });
}

#[test]
fn merge_folds_a_folder_into_the_existing_one_and_asks_about_the_files_inside() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[
                ("src/", ""),
                ("src/p/", ""),
                ("src/p/new", "n"),
                ("src/p/clash", "from-src"),
                ("src/p/sub/", ""),
                ("src/p/sub/deep", "d"),
                ("dst/", ""),
                ("dst/p/", ""),
                ("dst/p/old", "o"),
                ("dst/p/clash", "from-dst"),
                ("dst/p/sub/", ""),
            ]),
        );
        set_mtime(&h, "dst/p", 800_000_000_000);
        set_mtime(&h, "src/p", 700_000_000_000);
        // Merge cannot settle a file against a file, so the one clash is asked about alone.
        let asked: Rc<RefCell<Vec<Vec<Conflict>>>> = Rc::default();
        let seen = asked.clone();
        let mut answers = Answers {
            conflicts: Box::new(move |conflicts| {
                seen.borrow_mut().push(conflicts.to_vec());
                vec![Resolution {
                    source: None,
                    policy: ConflictPolicy::Replace,
                }]
            }),
            errors: Box::new(|_, _| None),
        };
        let request = req(
            &h,
            JobKind::Copy,
            &["src/p"],
            "dst",
            Some(ConflictPolicy::MergeFolders),
        );
        let result = run(&mut h, request, &mut answers);
        done(&result);
        let asked = asked.borrow();
        assert_eq!(asked.len(), 1, "{asked:?}");
        assert_eq!(asked[0].len(), 1, "the job waits on just that conflict");
        assert_eq!(asked[0][0].kind, ConflictKind::FileOverFile);
        assert_eq!(asked[0][0].existing, h.loc("dst/p/clash"));
        assert_eq!(
            dst_tree(&h),
            tree(&[
                ("p/", ""),
                ("p/old", "o"),
                ("p/new", "n"),
                ("p/clash", "from-src"),
                ("p/sub/", ""),
                ("p/sub/deep", "d"),
            ])
        );
        let report = result.report.unwrap();
        assert_eq!(
            report.transfer.merged,
            vec![h.loc("dst/p"), h.loc("dst/p/sub")]
        );
        assert_eq!(report.transfer.replaced, vec![h.loc("dst/p/clash")]);
        // The new files are the entries created; the merged folders are not.
        assert!(report.created.contains(&h.loc("dst/p/new")));
        assert!(!report.created.contains(&h.loc("dst/p")));
        // The existing folder did not take the source folder's time (the new entries touched it).
        assert_ne!(mtime_of(&h, "dst/p"), Some(700_000_000_000));
        assert!(leftovers(&work_tree(&h)).is_empty());
    });
}

#[test]
fn apply_to_all_is_kept_on_the_job_and_covers_every_later_clash() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[
                ("src/", ""),
                ("src/a", "A"),
                ("src/b", "B"),
                ("src/c", "C"),
                ("dst/", ""),
                ("dst/a", "a"),
                ("dst/b", "b"),
                ("dst/c", "c"),
            ]),
        );
        let rounds = Rc::new(RefCell::new(0));
        let counted = rounds.clone();
        let mut answers = Answers {
            conflicts: Box::new(move |conflicts| {
                *counted.borrow_mut() += 1;
                assert_eq!(conflicts.len(), 3, "the batch is asked once");
                vec![Resolution {
                    source: None,
                    policy: ConflictPolicy::Replace,
                }]
            }),
            errors: Box::new(|_, _| None),
        };
        let request = req(&h, JobKind::Copy, &["src/a", "src/b", "src/c"], "dst", None);
        let result = run(&mut h, request, &mut answers);
        done(&result);
        assert_eq!(*rounds.borrow(), 1);
        assert_eq!(dst_tree(&h), tree(&[("a", "A"), ("b", "B"), ("c", "C")]));
        // Stored on the job, where a retry or a later clash finds it.
        let job = h.store.job(result.id).unwrap();
        assert_eq!(job.options.conflict, Some(ConflictPolicy::Replace));
        assert_eq!(
            h.store.resolutions(result.id).unwrap().all(),
            Some(ConflictPolicy::Replace)
        );
    });
}

#[test]
fn answers_for_single_sources_cover_only_those_sources() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[
                ("src/", ""),
                ("src/a", "A"),
                ("src/b", "B"),
                ("dst/", ""),
                ("dst/a", "a"),
                ("dst/b", "b"),
            ]),
        );
        let loc_a = h.loc("src/a");
        let loc_b = h.loc("src/b");
        let mut answers = Answers {
            conflicts: Box::new(move |_| {
                vec![
                    Resolution {
                        source: Some(loc_a.clone()),
                        policy: ConflictPolicy::Skip,
                    },
                    Resolution {
                        source: Some(loc_b.clone()),
                        policy: ConflictPolicy::KeepBoth,
                    },
                ]
            }),
            errors: Box::new(|_, _| None),
        };
        let request = req(&h, JobKind::Copy, &["src/a", "src/b"], "dst", None);
        let result = run(&mut h, request, &mut answers);
        done(&result);
        assert_eq!(
            dst_tree(&h),
            tree(&[("a", "a"), ("b", "b"), ("b (2)", "B")])
        );
        // Nothing was said for all.
        assert_eq!(h.store.job(result.id).unwrap().options.conflict, None);
    });
}

#[test]
fn a_partial_answer_keeps_the_job_waiting_for_the_rest() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[
                ("src/", ""),
                ("src/a", "A"),
                ("src/b", "B"),
                ("dst/", ""),
                ("dst/a", "a"),
                ("dst/b", "b"),
            ]),
        );
        let loc_a = h.loc("src/a");
        let rounds = Rc::new(RefCell::new(Vec::<usize>::new()));
        let seen = rounds.clone();
        let mut answers = Answers {
            conflicts: Box::new(move |conflicts| {
                seen.borrow_mut().push(conflicts.len());
                if conflicts.len() == 2 {
                    vec![Resolution {
                        source: Some(loc_a.clone()),
                        policy: ConflictPolicy::Replace,
                    }]
                } else {
                    vec![Resolution {
                        source: None,
                        policy: ConflictPolicy::Skip,
                    }]
                }
            }),
            errors: Box::new(|_, _| None),
        };
        let request = req(&h, JobKind::Copy, &["src/a", "src/b"], "dst", None);
        let result = run(&mut h, request, &mut answers);
        done(&result);
        assert_eq!(*rounds.borrow(), vec![2, 1]);
        assert_eq!(dst_tree(&h), tree(&[("a", "A"), ("b", "b")]));
    });
}

#[test]
fn replace_if_newer_replaces_only_a_newer_source() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        let table = [
            ("newer", 2_000_000_000_000i64, 1_000_000_000_000i64, "new"),
            ("older", 1_000_000_000_000, 2_000_000_000_000, "old"),
            ("equal", 1_500_000_000_000, 1_500_000_000_000, "old"),
        ];
        for (name, src_ms, dst_ms, expected) in table {
            build_files(&h, name);
            set_mtime(&h, &format!("src/{name}"), src_ms);
            set_mtime(&h, &format!("dst/{name}"), dst_ms);
            let result = go(
                &mut h,
                JobKind::Copy,
                &[&format!("src/{name}")],
                "dst",
                Some(ConflictPolicy::ReplaceIfNewer),
            );
            done(&result);
            assert_eq!(
                read_bytes(&h, &format!("dst/{name}")),
                expected.as_bytes(),
                "{name}"
            );
            // A replaced file takes the source's time; a skipped one keeps its own.
            let at = mtime_of(&h, &format!("dst/{name}")).unwrap();
            let want = if expected == "new" { src_ms } else { dst_ms };
            assert!((at - want).abs() <= 1, "{name}: {at} vs {want}");
        }
    });

    fn build_files<P: Provider + 'static>(h: &Harness<P>, name: &str) {
        for (folder, content) in [("src", "new"), ("dst", "old")] {
            if h.provider.stat(&h.path(folder)).is_err() {
                h.provider.create_dir(&h.path(folder)).unwrap();
            }
            put_bytes(h, &format!("{folder}/{name}"), content.as_bytes());
        }
        h.provider.reset();
    }
}

#[test]
fn replace_if_newer_merges_folders_and_decides_each_file_inside() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[
                ("src/", ""),
                ("src/p/", ""),
                ("src/p/newer", "s-newer"),
                ("src/p/older", "s-older"),
                ("src/p/only", "s-only"),
                ("dst/", ""),
                ("dst/p/", ""),
                ("dst/p/newer", "d-newer"),
                ("dst/p/older", "d-older"),
            ]),
        );
        set_mtime(&h, "src/p/newer", 2_000_000_000_000);
        set_mtime(&h, "dst/p/newer", 1_000_000_000_000);
        set_mtime(&h, "src/p/older", 1_000_000_000_000);
        set_mtime(&h, "dst/p/older", 2_000_000_000_000);
        let result = go(
            &mut h,
            JobKind::Copy,
            &["src/p"],
            "dst",
            Some(ConflictPolicy::ReplaceIfNewer),
        );
        done(&result);
        assert_eq!(
            dst_tree(&h),
            tree(&[
                ("p/", ""),
                ("p/newer", "s-newer"),
                ("p/older", "d-older"),
                ("p/only", "s-only"),
            ])
        );
    });
}

#[test]
fn a_file_and_a_folder_with_one_name_can_only_be_skipped_or_kept_both() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        let start = tree(&[
            ("src/", ""),
            ("src/f", "file"),
            ("src/d/", ""),
            ("src/d/x", "x"),
            ("dst/", ""),
            ("dst/f/", ""),
            ("dst/f/keep", "k"),
            ("dst/d", "a file called d"),
        ]);
        build(&h, &start);
        // Replace refuses with a typed error and deletes nothing.
        h.provider.reset();
        let result = go(
            &mut h,
            JobKind::Copy,
            &["src/f"],
            "dst",
            Some(ConflictPolicy::Replace),
        );
        assert_eq!(
            error_of(&result.state),
            &OpsError::CannotReplace {
                location: h.loc("dst/f")
            }
        );
        let result = go(
            &mut h,
            JobKind::Move,
            &["src/d"],
            "dst",
            Some(ConflictPolicy::Replace),
        );
        assert_eq!(
            error_of(&result.state),
            &OpsError::CannotReplace {
                location: h.loc("dst/d")
            }
        );
        assert_eq!(h.provider.write_calls(), 0);
        assert_eq!(work_tree(&h), without_trash(&start));

        // The folder-sized choices cannot settle it either: the job asks, here being told to skip.
        let asked = Rc::new(RefCell::new(0));
        let counted = asked.clone();
        let mut answers = Answers {
            conflicts: Box::new(move |_| {
                *counted.borrow_mut() += 1;
                vec![Resolution {
                    source: None,
                    policy: ConflictPolicy::Skip,
                }]
            }),
            errors: Box::new(|_, _| None),
        };
        let request = req(
            &h,
            JobKind::Copy,
            &["src/f", "src/d"],
            "dst",
            Some(ConflictPolicy::MergeFolders),
        );
        let result = run(&mut h, request, &mut answers);
        done(&result);
        assert_eq!(result.report.unwrap().counts.skipped, 2);
        assert_eq!(work_tree(&h), without_trash(&start));

        // Keep both puts each beside what is in the way.
        let result = go(
            &mut h,
            JobKind::Copy,
            &["src/f", "src/d"],
            "dst",
            Some(ConflictPolicy::KeepBoth),
        );
        done(&result);
        let now = dst_tree(&h);
        assert_eq!(now.get("f"), Some(&Node::Dir));
        assert_eq!(now.get("f (2)"), Some(&file("file")));
        assert_eq!(now.get("d"), Some(&file("a file called d")));
        assert_eq!(now.get("d (2)/x"), Some(&file("x")));
    });

    fn without_trash(t: &Tree) -> Tree {
        t.clone()
    }
}

#[test]
fn two_sources_with_one_name_clash_with_each_other() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[
                ("x/", ""),
                ("x/n", "from-x"),
                ("y/", ""),
                ("y/n", "from-y"),
                ("dst/", ""),
            ]),
        );
        let request = req(&h, JobKind::Copy, &["x/n", "y/n"], "dst", None);
        let result = run_plain(&mut h, request);
        let JobState::Waiting {
            reason: WaitReason::Conflicts { conflicts },
        } = &result.state
        else {
            panic!("{:?}", result.state);
        };
        assert!(conflicts[0].within_batch);
        assert_eq!(h.provider.write_calls(), 0);
        // Kept both: the second becomes `n (2)`.
        let result = go(
            &mut h,
            JobKind::Copy,
            &["x/n", "y/n"],
            "dst",
            Some(ConflictPolicy::KeepBoth),
        );
        done(&result);
        assert_eq!(dst_tree(&h), tree(&[("n", "from-x"), ("n (2)", "from-y")]));
    });
}

#[test]
fn names_that_differ_only_by_case_clash_where_the_provider_folds_case() {
    for (rule, clashes) in [(CaseRule::Sensitive, false), (CaseRule::Insensitive, true)] {
        let (mut h, _dir) = memory_harness(rule);
        build(
            &h,
            &tree(&[
                ("src/", ""),
                ("src/b", "lower"),
                ("dst/", ""),
                ("dst/B", "UPPER"),
            ]),
        );
        let result = go(&mut h, JobKind::Copy, &["src/b"], "dst", None);
        assert_eq!(
            matches!(result.state, JobState::Waiting { .. }),
            clashes,
            "{rule:?}"
        );
        if !clashes {
            done(&result);
            assert_eq!(dst_tree(&h), tree(&[("B", "UPPER"), ("b", "lower")]));
            continue;
        }
        let result = go(
            &mut h,
            JobKind::Copy,
            &["src/b"],
            "dst",
            Some(ConflictPolicy::KeepBoth),
        );
        done(&result);
        assert_eq!(dst_tree(&h), tree(&[("B", "UPPER"), ("b (2)", "lower")]));
        let result = go(
            &mut h,
            JobKind::Copy,
            &["src/b"],
            "dst",
            Some(ConflictPolicy::Replace),
        );
        done(&result);
        // The name takes the spelling of what replaced it.
        let now = dst_tree(&h);
        assert_eq!(now.get("b"), Some(&file("lower")));
        assert!(!now.contains_key("B"));
    }
}

#[test]
fn a_failed_replace_puts_the_original_back_at_every_step() {
    for folder in [false, true] {
        each_provider!(|h, rule, links| {
            let _ = (rule, links);
            let start = if folder {
                tree(&[
                    ("src/", ""),
                    ("src/p/", ""),
                    ("src/p/a", "new-a"),
                    ("src/p/b", "new-b"),
                    ("dst/", ""),
                    ("dst/p/", ""),
                    ("dst/p/old", "old"),
                ])
            } else {
                tree(&[
                    ("src/", ""),
                    ("src/p", "new"),
                    ("dst/", ""),
                    ("dst/p", "old"),
                ])
            };
            let after_ok = if folder {
                tree(&[("p/", ""), ("p/a", "new-a"), ("p/b", "new-b")])
            } else {
                tree(&[("p", "new")])
            };
            let before = tree_in(&start, "dst/");
            let src_before = tree_in(&start, "src/");
            for kind in [JobKind::Copy, JobKind::Move] {
                // A clean run, to learn how many calls it makes.
                rebuild(&mut h, &start);
                h.provider.reset();
                let result = replace_p(&mut h, kind, folder);
                done(&result);
                let calls = h.provider.calls();
                assert_eq!(dst_tree(&h), after_ok, "{kind:?} clean");
                for step in 1..=calls {
                    rebuild(&mut h, &start);
                    h.provider.reset();
                    h.provider.fail_at(step, FaultKind::PermissionDenied);
                    let result = replace_p(&mut h, kind, folder);
                    assert!(
                        h.provider.calls() >= step,
                        "the fault at {step} never fired"
                    );
                    h.provider.reset();
                    let dst = dst_tree(&h);
                    let src = src_tree(&h);
                    let at = format!("{folder} {kind:?} step {step}/{calls}");
                    assert!(
                        leftovers(&dst).is_empty() && leftovers(&src).is_empty(),
                        "{at}: {dst:#?}"
                    );
                    // The destination is the original or the finished replacement, never a mixture.
                    assert!(dst == before || dst == after_ok, "{at}: {dst:#?}");
                    if kind == JobKind::Copy {
                        assert_eq!(src, src_before, "{at}");
                    } else if dst == before {
                        // Nothing moved: the source is whole.
                        assert_eq!(src, src_before, "{at}");
                    } else {
                        // Moved: the source is gone, or (a folder, removed child by child after
                        // the swap) holds only what had not been removed yet.
                        assert!(
                            src.is_empty()
                                || (folder && src.keys().all(|k| src_before.contains_key(k))),
                            "{at}: {src:#?}"
                        );
                    }
                    if result.state == JobState::Done {
                        assert_eq!(dst, after_ok, "{at}");
                    }
                }
            }
        });
    }

    /// Replaces `dst/p` with `src/p`. A folder is replaced only on an answer given for it, so that
    /// is what the job is given; a file takes the policy for all.
    fn replace_p<P: Provider + 'static>(
        h: &mut Harness<P>,
        kind: JobKind,
        folder: bool,
    ) -> RunResult {
        if !folder {
            return go(h, kind, &["src/p"], "dst", Some(ConflictPolicy::Replace));
        }
        let request = req(h, kind, &["src/p"], "dst", None);
        let mut answers = Answers {
            conflicts: Box::new(|conflicts| {
                conflicts
                    .iter()
                    .map(|c| Resolution {
                        source: Some(c.source.clone()),
                        policy: ConflictPolicy::Replace,
                    })
                    .collect()
            }),
            errors: Box::new(|_, _| None),
        };
        run(h, request, &mut answers)
    }

    /// The subtree of `start` below `prefix`, with the prefix removed.
    fn tree_in(start: &Tree, prefix: &str) -> Tree {
        start
            .iter()
            .filter_map(|(k, v)| k.strip_prefix(prefix).map(|r| (r.to_owned(), v.clone())))
            .collect()
    }

    /// Removes everything in the work folder and builds `start` again.
    fn rebuild<P: Provider + 'static>(h: &mut Harness<P>, start: &Tree) {
        let current = work_tree(h);
        let mut keys: Vec<&String> = current.keys().collect();
        keys.sort_by_key(|k| std::cmp::Reverse(k.matches('/').count()));
        for key in keys {
            let path = h.path(key);
            match current.get(key) {
                Some(Node::Dir) => h.provider.remove_dir(&path).unwrap(),
                _ => h.provider.remove_file(&path).unwrap(),
            }
        }
        populate(h.provider.as_ref(), &h.work, start);
    }
}

#[test]
fn resolve_is_only_for_a_job_waiting_on_conflicts() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    build(
        &h,
        &tree(&[("src/", ""), ("src/a", "A"), ("dst/", ""), ("dst/a", "a")]),
    );
    // A finished job refuses answers and is unchanged.
    let result = go(
        &mut h,
        JobKind::Copy,
        &["src/a"],
        "dst",
        Some(ConflictPolicy::Skip),
    );
    done(&result);
    let before = h.store.snapshot();
    let answers = [Resolution {
        source: None,
        policy: ConflictPolicy::Replace,
    }];
    assert!(matches!(
        h.store.resolve(result.id, &answers),
        Err(QueueError::Illegal { .. })
    ));
    assert_eq!(h.store.snapshot(), before);
    assert!(matches!(
        h.store.resolve(JobId(99), &answers),
        Err(QueueError::UnknownJob(_))
    ));

    // A waiting one takes them, and an answer that settles nothing leaves it waiting as it was.
    let result = go(&mut h, JobKind::Copy, &["src/a"], "dst", None);
    assert!(matches!(result.state, JobState::Waiting { .. }));
    let revision = h.store.revision();
    let unrelated = [Resolution {
        source: Some(h.loc("src/other")),
        policy: ConflictPolicy::Skip,
    }];
    assert!(h.store.resolve(result.id, &unrelated).unwrap().is_empty());
    assert_eq!(h.store.revision(), revision);
    assert!(matches!(
        h.store.job(result.id).unwrap().state,
        JobState::Waiting { .. }
    ));
    let events = h.store.resolve(result.id, &answers).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(h.store.job(result.id).unwrap().state, JobState::Running);
    assert_eq!(
        h.store.job(result.id).unwrap().options.conflict,
        Some(ConflictPolicy::Replace)
    );
}

#[test]
fn an_original_that_cannot_be_put_back_is_reported_not_lost() {
    // The move's rename is refused as crossing volumes after the old file was set aside, and then
    // the old file cannot be renamed back either. It stays under its waiting name, the job stops
    // there, and the report says where it is.
    let (mut h, _dir) = local_harness();
    build(
        &h,
        &tree(&[
            ("src/", ""),
            ("src/a", "new"),
            ("dst/", ""),
            ("dst/a", "old"),
        ]),
    );
    let src = h.path("src").display();
    h.provider
        .fail_always_where(Op::Rename, FaultKind::CrossesDevices, move |p| {
            p.display().starts_with(&src)
        });
    h.provider
        .fail_always_where(Op::Rename, FaultKind::PermissionDenied, |p| {
            p.display().contains(".waypoint-replaced-")
        });
    let result = go(
        &mut h,
        JobKind::Move,
        &["src/a"],
        "dst",
        Some(ConflictPolicy::Replace),
    );
    let failure = result.failure.expect("the job stopped");
    assert!(matches!(failure.error, OpsError::Io { .. }));
    let stranded = failure.report.transfer.leftovers.clone();
    assert_eq!(stranded.len(), 1);
    h.provider.reset();
    let now = dst_tree(&h);
    assert_eq!(now.len(), 1, "{now:?}");
    let (name, node) = now.iter().next().unwrap();
    assert!(name.starts_with(".waypoint-replaced-"), "{name}");
    assert_eq!(node, &file("old"));
    assert!(stranded[0].display.ends_with(name.as_str()));
    // The source was not touched.
    assert_eq!(src_tree(&h), tree(&[("a", "new")]));
}

#[test]
fn a_policy_for_all_merges_a_folder_clash_and_only_an_answer_for_it_replaces_the_folder() {
    let mut start = tree(&[
        ("src/", ""),
        ("src/p/", ""),
        ("src/p/a", "new-a"),
        ("dst/", ""),
        ("dst/p/", ""),
        ("dst/p/old", "old"),
    ]);
    // Replace for all, as the request carried it: the folder is merged into, not deleted.
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    populate(h.provider.as_ref(), &h.work, &start);
    h.provider.reset();
    let result = go(
        &mut h,
        JobKind::Copy,
        &["src/p"],
        "dst",
        Some(ConflictPolicy::Replace),
    );
    done(&result);
    assert_eq!(
        dst_tree(&h),
        tree(&[("p/", ""), ("p/a", "new-a"), ("p/old", "old")])
    );
    assert!(result.report.unwrap().transfer.replaced.is_empty());

    // The same answer given for that folder replaces it.
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    populate(h.provider.as_ref(), &h.work, &std::mem::take(&mut start));
    h.provider.reset();
    let request = req(&h, JobKind::Copy, &["src/p"], "dst", None);
    let mut answers = Answers {
        conflicts: Box::new(|conflicts| {
            conflicts
                .iter()
                .map(|c| Resolution {
                    source: Some(c.source.clone()),
                    policy: ConflictPolicy::Replace,
                })
                .collect()
        }),
        errors: Box::new(|_, _| None),
    };
    done(&run(&mut h, request, &mut answers));
    assert_eq!(dst_tree(&h), tree(&[("p/", ""), ("p/a", "new-a")]));
}
