// The copy executor: files, folders and links come out as they went in, written atomically through
// partial names, with the speed, the progress and the free-space and refusal checks the plan makes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;
mod xfer;

use xfer::*;

fn is_file(tree: &Tree, key: &str, bytes: &[u8]) -> bool {
    tree.get(key) == Some(&Node::File(bytes.to_vec()))
}

#[test]
fn a_file_is_copied_with_its_bytes_time_and_mode_and_the_source_is_untouched() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        let bytes = pattern(5 * SMALL_CHUNK + 17, 3);
        build(&h, &tree(&[("src/", ""), ("dst/", "")]));
        put_bytes(&h, "src/f.bin", &bytes);
        set_mtime(&h, "src/f.bin", 1_000_000_123_456);
        if cfg!(unix) {
            set_mode(&h, "src/f.bin", 0o640);
        }
        h.provider.reset();
        let result = go(&mut h, JobKind::Copy, &["src/f.bin"], "dst", None);
        done(&result);
        assert_eq!(read_bytes(&h, "dst/f.bin"), bytes);
        assert_eq!(read_bytes(&h, "src/f.bin"), bytes);
        let (a, b) = (mtime_of(&h, "dst/f.bin").unwrap(), 1_000_000_123_456i64);
        assert!((a - b).abs() <= 1, "{a} vs {b}");
        if cfg!(unix) {
            assert_eq!(mode_of(&h, "dst/f.bin"), Some(0o640));
        }
        assert!(leftovers(&work_tree(&h)).is_empty());
        let report = result.report.unwrap();
        assert_eq!(report.created, vec![h.loc("dst/f.bin")]);
        assert_eq!(report.progress.bytes_done, bytes.len() as u64);
        assert_eq!(report.progress.items_done, 1);
    });
}

#[test]
fn a_folder_comes_out_whole_with_links_empty_folders_and_times() {
    each_provider!(|h, rule, links| {
        let _ = rule;
        let mut t = tree(&[
            ("src/", ""),
            ("src/top/", ""),
            ("src/top/a.txt", "alpha"),
            ("src/top/empty/", ""),
            ("src/top/sub/", ""),
            ("src/top/sub/zero", ""),
            ("src/top/sub/b.bin", "bravo"),
            ("dst/", ""),
        ]);
        if links {
            t.insert(
                "src/top/dangling".to_owned(),
                Node::Link("nowhere".to_owned()),
            );
            t.insert("src/top/dirlink".to_owned(), Node::Link("sub".to_owned()));
            t.insert(
                "src/top/abs".to_owned(),
                Node::Link("/no/such/place".to_owned()),
            );
        }
        build(&h, &t);
        set_mtime(&h, "src/top/a.txt", 900_000_000_000);
        set_mtime(&h, "src/top/sub", 910_000_000_000);
        set_mtime(&h, "src/top", 920_000_000_000);
        let before = work_tree(&h);
        let result = go(&mut h, JobKind::Copy, &["src/top"], "dst", None);
        done(&result);
        let after = work_tree(&h);
        // The copy equals the source, link for link, and the source is as it was.
        for (key, node) in &before {
            if let Some(rest) = key.strip_prefix("src/top") {
                assert_eq!(after.get(&format!("dst/top{rest}")), Some(node), "{key}");
            }
            assert_eq!(after.get(key), Some(node), "source {key}");
        }
        let copied = before
            .keys()
            .filter(|k| *k == "src/top" || k.starts_with("src/top/"))
            .count();
        assert_eq!(after.len(), before.len() + copied, "{after:#?}");
        for dir in ["top/a.txt", "top/sub", "top"] {
            let (a, b) = (
                mtime_of(&h, &format!("dst/{dir}")).unwrap(),
                mtime_of(&h, &format!("src/{dir}")).unwrap(),
            );
            assert!((a - b).abs() <= 1, "{dir}: {a} vs {b}");
        }
        assert!(leftovers(&after).is_empty());
        // One new entry was made, whatever is below it.
        assert_eq!(result.report.unwrap().created, vec![h.loc("dst/top")]);
    });
}

#[test]
fn a_link_to_a_folder_is_copied_as_a_link_and_never_followed() {
    each_provider!(|h, rule, links| {
        let _ = rule;
        if !links {
            return;
        }
        let mut t = tree(&[
            ("src/", ""),
            ("src/real/", ""),
            ("src/real/x", "x"),
            ("dst/", ""),
        ]);
        t.insert("src/ln".to_owned(), Node::Link("real".to_owned()));
        build(&h, &t);
        let result = go(&mut h, JobKind::Copy, &["src/ln"], "dst", None);
        done(&result);
        assert_eq!(
            work_tree(&h).get("dst/ln"),
            Some(&Node::Link("real".to_owned()))
        );
        assert!(!work_tree(&h).contains_key("dst/real"));
        // The plan counted it as one entry with no bytes.
        assert_eq!(result.plan.unwrap().total_items, 1);
    });
}

#[test]
fn files_around_the_chunk_size_copy_exactly() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("src/", ""), ("dst/", "")]));
        let sizes = [
            0,
            1,
            SMALL_CHUNK - 1,
            SMALL_CHUNK,
            SMALL_CHUNK + 1,
            3 * SMALL_CHUNK,
            3 * SMALL_CHUNK + 1,
            70_000,
        ];
        let mut names = Vec::new();
        for (n, size) in sizes.iter().enumerate() {
            let name = format!("f{n}");
            put_bytes(&h, &format!("src/{name}"), &pattern(*size, n as u8));
            names.push(format!("src/{name}"));
        }
        h.provider.reset();
        let sources: Vec<&str> = names.iter().map(String::as_str).collect();
        let result = go(&mut h, JobKind::Copy, &sources, "dst", None);
        done(&result);
        for (n, size) in sizes.iter().enumerate() {
            assert_eq!(
                read_bytes(&h, &format!("dst/f{n}")),
                pattern(*size, n as u8),
                "size {size}"
            );
        }
        let total: usize = sizes.iter().sum();
        let progress = result.report.unwrap().progress;
        assert_eq!(progress.bytes_done, total as u64);
        assert_eq!(progress.bytes_total, total as u64);
        assert_eq!(progress.items_done, sizes.len() as u64);
    });
}

#[test]
fn a_file_is_written_under_a_partial_name_and_renamed_into_place() {
    // Looks at the folder from inside the copy, at every progress report.
    struct Peek<'a> {
        provider: &'a dyn Provider,
        work: VfsPath,
        seen: Vec<Vec<String>>,
    }
    impl ExecSink for Peek<'_> {
        fn progress(&mut self, _: &Progress, _: &Counts) {
            let inside = tree_of(self.provider, &self.work.join("dst").unwrap());
            self.seen.push(inside.keys().cloned().collect());
        }
    }
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &tree(&[("src/", ""), ("dst/", "")]));
        put_bytes(&h, "src/big.dat", &pattern(4 * SMALL_CHUNK, 1));
        h.provider.reset();
        let planned = h
            .plan(&req(&h, JobKind::Copy, &["src/big.dat"], "dst", None))
            .unwrap();
        let mut peek = Peek {
            provider: h.provider.as_ref(),
            work: h.work.clone(),
            seen: Vec::new(),
        };
        // Verifying keeps the provider's fast path out of it, so every chunk reports.
        let options = RunOptions {
            chunk_bytes: SMALL_CHUNK,
            verify: Some(VerifyAlgorithm::Blake3),
            ..RunOptions::default()
        };
        Executor::new(h.env.clone())
            .run_with(JobId(7), &planned, &CancelToken::new(), &mut peek, options)
            .unwrap();
        let during: Vec<&String> = peek.seen.iter().flatten().collect();
        assert!(!during.is_empty());
        for name in &during {
            // Never the final name until the end; always this job's partial name before.
            if name.as_str() != "big.dat" {
                assert!(
                    name.starts_with(".waypoint-partial-7-") && name.ends_with("-big.dat"),
                    "{name}"
                );
            }
        }
        // The partial is seen while the bytes go, and the final name only after it.
        let first_final = peek
            .seen
            .iter()
            .position(|names| names.iter().any(|n| n == "big.dat"))
            .expect("the file arrives");
        assert!(first_final > 2, "{:?}", peek.seen);
        assert!(peek.seen[..first_final]
            .iter()
            .all(|names| names.len() == 1 && names[0].starts_with(".waypoint-partial-")));
        assert!(peek.seen[first_final..]
            .iter()
            .all(|names| names == &["big.dat".to_owned()]));
        assert!(work_tree(&h).contains_key("dst/big.dat"));
    });
}

#[test]
fn refusals_happen_before_any_write() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(
            &h,
            &tree(&[
                ("a/", ""),
                ("a/inner/", ""),
                ("a/inner/f", "x"),
                ("f", "x"),
                ("d/", ""),
                ("plain", "p"),
            ]),
        );
        let cases = [
            // Into itself.
            (
                h.request(JobKind::Copy, &["a"], Some("a/inner"), None),
                "intoItself",
            ),
            (
                h.request(JobKind::Move, &["a"], Some("a"), None),
                "intoItself",
            ),
            // Already in the folder.
            (
                h.request(JobKind::Copy, &["f"], Some(""), None),
                "sameFolder",
            ),
            // Not a folder, not there.
            (h.request(JobKind::Copy, &["f"], Some("plain"), None), "io"),
            (
                h.request(JobKind::Copy, &["gone"], Some("d"), None),
                "notFound",
            ),
            (
                h.request(JobKind::Copy, &["f"], Some("gone"), None),
                "notFound",
            ),
        ];
        for (request, expected) in cases {
            h.provider.reset();
            let result = run_plain(&mut h, request);
            let error = error_of(&result.state);
            let name = match error {
                OpsError::IntoItself => "intoItself",
                OpsError::SameFolder => "sameFolder",
                OpsError::NotFound { .. } => "notFound",
                OpsError::Io { .. } => "io",
                other => panic!("{other:?}"),
            };
            assert_eq!(name, expected);
            assert_eq!(h.provider.write_calls(), 0, "{expected} wrote");
        }
    });
}

#[test]
fn too_little_room_is_refused_before_any_write_and_says_how_much() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    build(&h, &tree(&[("src/", ""), ("dst/", "")]));
    put_bytes(&h, "src/big", &pattern(10_000, 0));
    put_bytes(&h, "src/big2", &pattern(10_000, 1));
    memory(&h).set_space(
        VolumeId(1),
        VolumeSpace {
            free_bytes: 15_000,
            total_bytes: 1_000_000,
        },
    );
    h.provider.reset();
    let result = go(&mut h, JobKind::Copy, &["src/big", "src/big2"], "dst", None);
    assert_eq!(
        error_of(&result.state),
        &OpsError::NotEnoughSpace {
            needed: 20_000,
            free: 15_000
        }
    );
    assert_eq!(h.provider.write_calls(), 0);
    assert!(work_tree(&h).keys().all(|k| !k.starts_with("dst/")));
    // One fits.
    let result = go(&mut h, JobKind::Copy, &["src/big"], "dst", None);
    done(&result);
}

#[test]
fn progress_reports_bytes_items_and_a_speed_that_the_clock_decides() {
    // A clock that moves a tenth of a second at every report.
    struct Tick(std::sync::atomic::AtomicI64);
    impl Clock for Tick {
        fn now_ms(&self) -> i64 {
            self.0.fetch_add(100, std::sync::atomic::Ordering::Relaxed)
        }
    }
    struct Collect(Vec<Progress>);
    impl ExecSink for Collect {
        fn progress(&mut self, p: &Progress, _: &Counts) {
            self.0.push(p.clone());
        }
    }
    let (h, _dir) = memory_harness(CaseRule::Sensitive);
    build(&h, &tree(&[("src/", ""), ("dst/", "")]));
    put_bytes(&h, "src/big", &pattern(100 * SMALL_CHUNK, 0));
    h.provider.reset();
    let planned = h
        .plan(&req(&h, JobKind::Copy, &["src/big"], "dst", None))
        .unwrap();
    let mut sink = Collect(Vec::new());
    let options = RunOptions {
        chunk_bytes: SMALL_CHUNK,
        clock: std::sync::Arc::new(Tick(Default::default())),
        ..RunOptions::default()
    };
    Executor::new(h.env.clone())
        .run_with(JobId(1), &planned, &CancelToken::new(), &mut sink, options)
        .unwrap();
    let reports = sink.0;
    assert!(reports.len() > 100);
    // Bytes only go up; a steady 1 KiB per 100 ms is about 10 KiB/s once the first sample is in.
    assert!(reports
        .windows(2)
        .all(|w| w[0].bytes_done <= w[1].bytes_done));
    let mid = &reports[reports.len() / 2];
    assert!(
        (9_000..=11_000).contains(&mid.speed_bps),
        "{}",
        mid.speed_bps
    );
    let eta = mid.eta_ms.unwrap();
    let left = (mid.bytes_total - mid.bytes_done) as f64 / mid.speed_bps as f64 * 1000.0;
    assert!((eta as f64 - left).abs() < 1000.0, "{eta} vs {left}");
    let last = reports.last().unwrap();
    assert_eq!(last.bytes_done, last.bytes_total);
    assert_eq!(last.eta_ms, Some(0));
}

#[test]
fn memory_fast_copy_is_used_on_one_volume_and_skipped_when_verifying() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    memory(&h).enable_fast_copy(true);
    build(&h, &tree(&[("src/", ""), ("dst/", "")]));
    put_bytes(&h, "src/f", &pattern(5_000, 2));
    h.provider.reset();
    let result = go(&mut h, JobKind::Copy, &["src/f"], "dst", None);
    done(&result);
    assert_eq!(h.provider.calls_of(Op::CopyWithin), 1);
    assert_eq!(
        h.provider.calls_of(Op::Read),
        0,
        "the fast path read nothing"
    );
    assert_eq!(read_bytes(&h, "dst/f"), pattern(5_000, 2));
    assert_eq!(mtime_of(&h, "dst/f"), mtime_of(&h, "src/f"));

    // With verification the bytes are read and hashed, so the fast path is not used.
    h.provider.reset();
    let mut request = req(
        &h,
        JobKind::Copy,
        &["src/f"],
        "dst",
        Some(ConflictPolicy::KeepBoth),
    );
    request.options.verify = Some(true);
    let result = run_plain(&mut h, request);
    done(&result);
    assert_eq!(h.provider.calls_of(Op::CopyWithin), 0);
    assert!(h.provider.calls_of(Op::Read) > 0);
    assert_eq!(read_bytes(&h, "dst/f (2)"), pattern(5_000, 2));
}

#[test]
fn a_fast_path_that_cannot_cross_falls_back_to_the_loop() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    memory(&h).enable_fast_copy(true);
    build(&h, &tree(&[("src/", ""), ("dst/", "")]));
    put_bytes(&h, "src/f", &pattern(3_000, 2));
    h.provider.reset();
    h.provider
        .fail_nth(Op::CopyWithin, 1, FaultKind::CrossesDevices);
    let result = go(&mut h, JobKind::Copy, &["src/f"], "dst", None);
    done(&result);
    assert_eq!(read_bytes(&h, "dst/f"), pattern(3_000, 2));
    assert!(leftovers(&work_tree(&h)).is_empty());
}

#[test]
fn any_other_failure_of_the_fast_path_fails_the_item_and_leaves_nothing() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    memory(&h).enable_fast_copy(true);
    build(&h, &tree(&[("src/", ""), ("dst/", "")]));
    put_bytes(&h, "src/f", &pattern(3_000, 2));
    h.provider.reset();
    h.provider
        .fail_nth(Op::CopyWithin, 1, FaultKind::PermissionDenied);
    let result = go(&mut h, JobKind::Copy, &["src/f"], "dst", None);
    assert!(matches!(
        error_of(&result.state),
        OpsError::PermissionDenied { .. }
    ));
    assert!(tree_of(h.provider.as_ref(), &h.path("dst")).is_empty());
}

#[test]
fn local_provider_fast_path_copies_real_files() {
    let (mut h, _dir) = local_harness();
    build(&h, &tree(&[("src/", ""), ("dst/", "")]));
    put_bytes(&h, "src/f", &pattern(300_000, 9));
    h.provider.reset();
    let result = go(&mut h, JobKind::Copy, &["src/f"], "dst", None);
    done(&result);
    assert_eq!(read_bytes(&h, "dst/f"), pattern(300_000, 9));
    let _ = is_file;
}

#[test]
fn names_windows_cannot_hold_are_refused_before_any_write_under_the_insensitive_rule() {
    // The fixtures bypass the provider's own name checks, as a folder from another system would.
    for (name, bad) in [
        ("aux.txt", true),
        ("CON", true),
        ("what?.txt", true),
        ("trailing.", true),
        ("fine.txt", false),
    ] {
        for rule in [CaseRule::Sensitive, CaseRule::Insensitive] {
            let (mut h, _dir) = memory_harness(rule);
            h.provider.create_dir(&h.path("src")).unwrap();
            h.provider.create_dir(&h.path("dst")).unwrap();
            memory(&h).put_file(&h.path("src").join(name).unwrap(), b"x");
            h.provider.reset();
            let refused = bad && rule == CaseRule::Insensitive;
            for kind in [JobKind::Copy, JobKind::Move] {
                let result = go(&mut h, kind, &[&format!("src/{name}")], "dst", None);
                if refused {
                    assert!(
                        matches!(error_of(&result.state), OpsError::InvalidName { .. }),
                        "{name} {kind:?}: {:?}",
                        result.state
                    );
                    assert_eq!(h.provider.write_calls(), 0, "{name} {kind:?}");
                } else {
                    done(&result);
                    // Put the file back for the next kind.
                    if kind == JobKind::Move {
                        memory(&h).put_file(&h.path("src").join(name).unwrap(), b"x");
                    }
                    h.provider
                        .remove_file(&h.path("dst").join(name).unwrap())
                        .unwrap();
                    h.provider.reset();
                }
            }
        }
    }
}

#[test]
fn a_name_inside_a_folder_that_the_destination_cannot_hold_fails_that_entry_only() {
    let (mut h, _dir) = memory_harness(CaseRule::Insensitive);
    h.provider.create_dir(&h.path("dst")).unwrap();
    for name in ["top/nul", "top/ok", "top/sub/also?bad", "top/sub/fine"] {
        memory(&h).put_file(&h.path(&format!("src/{name}")), b"x");
    }
    h.provider.reset();
    // No answer: the job fails at the entry, and nothing is left under a name.
    let result = go(&mut h, JobKind::Copy, &["src/top"], "dst", None);
    assert!(matches!(
        error_of(&result.state),
        OpsError::InvalidName { .. }
    ));
    assert!(tree_of(h.provider.as_ref(), &h.path("dst")).is_empty());
    // Skipped: the rest of the folder arrives.
    let mut answers = Answers::always(None, Some(Decision::SkipAll));
    let request = req(&h, JobKind::Copy, &["src/top"], "dst", None);
    let result = run(&mut h, request, &mut answers);
    done(&result);
    assert_eq!(result.report.unwrap().counts.failed, 2);
    h.provider.reset();
    let now = tree_of(h.provider.as_ref(), &h.path("dst"));
    let keys: Vec<&String> = now.keys().collect();
    assert_eq!(keys, vec!["top", "top/ok", "top/sub", "top/sub/fine"]);
}
