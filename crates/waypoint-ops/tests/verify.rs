// Verified copies (A51): the digest of what was read is compared with the written file read back and
// with the source read again; a mismatch removes the partial and fails the item; the job records
// what it verified.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;
mod xfer;

use xfer::*;

fn dst_tree<P: Provider + 'static>(h: &Harness<P>) -> Tree {
    tree_of(h.provider.as_ref(), &h.path("dst"))
}

fn src_tree<P: Provider + 'static>(h: &Harness<P>) -> Tree {
    tree_of(h.provider.as_ref(), &h.path("src"))
}

fn verified_request<P: Provider + 'static>(
    h: &Harness<P>,
    kind: JobKind,
    sources: &[&str],
) -> JobRequest {
    let mut request = req(h, kind, sources, "dst", None);
    request.options.verify = Some(true);
    request
}

fn sample(links: bool) -> Tree {
    let mut t = tree(&[
        ("src/", ""),
        ("src/top/", ""),
        ("src/top/a", "alpha"),
        ("src/top/empty", ""),
        ("src/top/sub/", ""),
        ("dst/", ""),
    ]);
    t.insert(
        "src/top/big".to_owned(),
        Node::File(pattern(5 * SMALL_CHUNK + 9, 2)),
    );
    if links {
        t.insert("src/top/ln".to_owned(), Node::Link("a".to_owned()));
    }
    t
}

#[test]
fn a_verified_copy_records_the_algorithm_the_file_count_and_one_digest() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        build(&h, &sample(links));
        let request = verified_request(&h, JobKind::Copy, &["src/top"]);
        let result = run(&mut h, request, &mut Answers::default());
        done(&result);
        let verified = result
            .report
            .as_ref()
            .unwrap()
            .transfer
            .verified
            .clone()
            .unwrap();
        assert_eq!(verified.algorithm, VerifyAlgorithm::Blake3);
        // Three files: `a`, `empty` and `big` (a link and folders are not hashed).
        assert_eq!(verified.files, 3);
        assert_eq!(verified.digest.len(), 64);
        // The job's snapshot carries it.
        assert_eq!(
            h.store.job(result.id).unwrap().verified,
            Some(verified.clone())
        );
        assert_eq!(h.replayed().jobs[0].verified, Some(verified.clone()));
        // It is the digest of the file digests in the order the files were copied (by name).
        let mut manifest = Manifest::new(VerifyAlgorithm::Blake3);
        for bytes in [
            b"alpha".to_vec(),
            pattern(5 * SMALL_CHUNK + 9, 2),
            Vec::new(),
        ] {
            manifest.add(&digest_of(VerifyAlgorithm::Blake3, &bytes));
        }
        assert_eq!(manifest.snapshot().unwrap().digest, verified.digest);
    });
}

#[test]
fn the_digest_does_not_depend_on_the_chunk_size_or_the_provider() {
    let mut seen = Vec::new();
    for chunk in [100, SMALL_CHUNK, 1 << 20] {
        for rule in [CaseRule::Sensitive, CaseRule::Insensitive] {
            let (mut h, _dir) = memory_harness(rule);
            build(&h, &sample(true));
            let request = verified_request(&h, JobKind::Copy, &["src/top"]);
            let options = TransferOptions {
                chunk_bytes: chunk,
                ..TransferOptions::default()
            };
            let result = run_transfer(
                &mut h,
                request,
                &mut Answers::default(),
                &options,
                &mut |_, _| {},
            );
            done(&result);
            seen.push(result.report.unwrap().transfer.verified.unwrap().digest);
        }
    }
    let (mut h, _dir) = local_harness();
    build(&h, &sample(cfg!(unix)));
    let request = verified_request(&h, JobKind::Copy, &["src/top"]);
    let result = run(&mut h, request, &mut Answers::default());
    done(&result);
    seen.push(result.report.unwrap().transfer.verified.unwrap().digest);
    assert!(seen.windows(2).all(|w| w[0] == w[1]), "{seen:?}");
}

#[test]
fn the_setting_chooses_the_algorithm_and_the_request_can_override_it() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    build(&h, &tree(&[("src/", ""), ("src/f", "data"), ("dst/", "")]));
    let options = TransferOptions {
        settings: OpsSettings {
            verify_after_copy: true,
            verify_algorithm: VerifyAlgorithm::Sha256,
            ..OpsSettings::default()
        },
        ..small()
    };
    // The setting is on and the request says nothing: verified, by SHA-256.
    let request = req(
        &h,
        JobKind::Copy,
        &["src/f"],
        "dst",
        Some(ConflictPolicy::KeepBoth),
    );
    let result = run_transfer(
        &mut h,
        request,
        &mut Answers::default(),
        &options,
        &mut |_, _| {},
    );
    done(&result);
    let verified = result.report.unwrap().transfer.verified.unwrap();
    assert_eq!(verified.algorithm, VerifyAlgorithm::Sha256);
    assert_eq!(
        verified.digest,
        hex(&digest_of(
            VerifyAlgorithm::Sha256,
            &digest_of(VerifyAlgorithm::Sha256, b"data")
        ))
    );
    // The request turns it off.
    let mut request = req(
        &h,
        JobKind::Copy,
        &["src/f"],
        "dst",
        Some(ConflictPolicy::KeepBoth),
    );
    request.options.verify = Some(false);
    let result = run_transfer(
        &mut h,
        request,
        &mut Answers::default(),
        &options,
        &mut |_, _| {},
    );
    done(&result);
    assert_eq!(result.report.unwrap().transfer.verified, None);
    // And the default setting is off.
    let request = req(
        &h,
        JobKind::Copy,
        &["src/f"],
        "dst",
        Some(ConflictPolicy::KeepBoth),
    );
    let result = run(&mut h, request, &mut Answers::default());
    done(&result);
    assert_eq!(result.report.unwrap().transfer.verified, None);
    assert_eq!(h.store.job(result.id).unwrap().verified, None);
}

#[test]
fn a_corrupt_read_at_any_point_of_a_verified_copy_is_caught_and_leaves_nothing() {
    for (kind, mover) in [(JobKind::Copy, false), (JobKind::Move, true)] {
        let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
        let t = tree(&[("src/", ""), ("dst/", "")]);
        build(&h, &t);
        let content = pattern(3 * SMALL_CHUNK + 100, 7);
        put_bytes(&h, "src/f", &content);
        h.provider.reset();
        let request = verified_request(&h, kind, &["src/f"]);
        let result = run_transfer(
            &mut h,
            request,
            &mut Answers::default(),
            &small(),
            &mut |_, _| {},
        );
        // A move on one volume is a rename: nothing to verify, nothing read.
        assert_eq!(h.provider.calls_of(Op::Read) > 0, !mover, "{kind:?}");
        done(&result);
        if mover {
            continue;
        }
        let reads = h.provider.calls_of(Op::Read);
        let mut caught = 0;
        let mut quiet = 0;
        for n in 1..=reads {
            let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
            build(&h, &t);
            put_bytes(&h, "src/f", &content);
            h.provider.reset();
            h.provider.corrupt_read_nth(n);
            let request = verified_request(&h, kind, &["src/f"]);
            let result = run_transfer(
                &mut h,
                request,
                &mut Answers::default(),
                &small(),
                &mut |_, _| {},
            );
            h.provider.reset();
            let now = dst_tree(&h);
            assert_eq!(read_bytes(&h, "src/f"), content);
            match &result.state {
                JobState::Failed {
                    error:
                        OpsError::VerifyFailed {
                            location,
                            expected,
                            actual,
                        },
                    ..
                } => {
                    caught += 1;
                    // Nothing under a final name and no partial, whatever read was corrupted.
                    assert!(
                        now.is_empty(),
                        "read {n}: {:?}",
                        now.keys().collect::<Vec<_>>()
                    );
                    assert_eq!(expected.len(), 64);
                    assert_eq!(actual.len(), 64);
                    assert_ne!(expected, actual);
                    // It names the destination or the source, never the partial.
                    assert!(
                        *location == h.loc("dst/f") || *location == h.loc("src/f"),
                        "{location:?}"
                    );
                }
                // A read that returned no bytes (the end of the file) has nothing to corrupt.
                JobState::Done => {
                    quiet += 1;
                    assert!(
                        now.get("f") == Some(&Node::File(content.clone())),
                        "read {n}"
                    );
                }
                other => panic!("read {n}: {other:?}"),
            }
        }
        assert!(caught >= 3 * 4, "{caught} of {reads} reads caught");
        assert_eq!(caught + quiet, reads);
        // Each of the three passes ends with two reads that return no bytes (one inside the last
        // chunk's fill and one after it), which have nothing to corrupt.
        assert_eq!(quiet, 6, "only end-of-file reads are unharmed");
    }
}

#[test]
fn a_move_that_fails_verification_keeps_its_source() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    h.provider.create_dir(&h.path("src")).unwrap();
    h.provider.create_dir(&h.path("dst")).unwrap();
    memory(&h).set_volume(&h.path("src"), VolumeId(2));
    let content = pattern(4 * SMALL_CHUNK, 1);
    put_bytes(&h, "src/f", &content);
    put_bytes(&h, "src/g", b"good");
    h.provider.reset();
    // Corrupt the read-back of the first file: reads 1..=5 are its copy (4 chunks and the end),
    // 6 onward its read-back.
    h.provider.corrupt_read_nth(6);
    let request = verified_request(&h, JobKind::Move, &["src/f", "src/g"]);
    let result = run_transfer(
        &mut h,
        request,
        &mut Answers::default(),
        &small(),
        &mut |_, _| {},
    );
    assert!(matches!(
        error_of(&result.state),
        OpsError::VerifyFailed { .. }
    ));
    h.provider.reset();
    assert_eq!(
        src_tree(&h),
        tree(&[("f", "")])
            .into_iter()
            .chain([
                ("f".to_owned(), Node::File(content.clone())),
                ("g".to_owned(), file("good")),
            ])
            .collect::<Tree>()
    );
    assert!(dst_tree(&h).is_empty());
    // Nothing was recorded as verified: the failing file was the first.
    assert_eq!(h.store.job(result.id).unwrap().verified, None);
}

#[test]
fn a_verification_failure_can_be_skipped_and_the_rest_is_still_recorded() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    build(
        &h,
        &tree(&[
            ("src/", ""),
            ("dst/", ""),
            ("src/a", "alpha"),
            ("src/b", "bravo"),
        ]),
    );
    // `a`: one read of data, one of the end; then its read-back (two reads); corrupt the first of
    // those, read 3.
    h.provider.corrupt_read_nth(3);
    let mut answers = Answers::always(None, Some(Decision::Skip));
    let request = verified_request(&h, JobKind::Copy, &["src/a", "src/b"]);
    let result = run_transfer(&mut h, request, &mut answers, &small(), &mut |_, _| {});
    done(&result);
    let report = result.report.unwrap();
    assert_eq!(report.counts.failed, 1);
    assert!(matches!(report.skipped[0].1, OpsError::VerifyFailed { .. }));
    assert_eq!(report.transfer.verified.as_ref().unwrap().files, 1);
    h.provider.reset();
    assert_eq!(dst_tree(&h), tree(&[("b", "bravo")]));
}

#[test]
fn a_verified_replace_that_fails_keeps_the_original() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    build(
        &h,
        &tree(&[
            ("src/", ""),
            ("dst/", ""),
            ("src/a", "new"),
            ("dst/a", "old"),
        ]),
    );
    h.provider.corrupt_read_nth(3);
    let mut request = verified_request(&h, JobKind::Copy, &["src/a"]);
    request.options.conflict = Some(ConflictPolicy::Replace);
    let result = run_transfer(
        &mut h,
        request,
        &mut Answers::default(),
        &small(),
        &mut |_, _| {},
    );
    assert!(matches!(
        error_of(&result.state),
        OpsError::VerifyFailed { .. }
    ));
    h.provider.reset();
    assert_eq!(dst_tree(&h), tree(&[("a", "old")]));
}
