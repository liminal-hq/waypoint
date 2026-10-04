// Extract and Compress end to end: archives made and unpacked through the queue, the planner and the
// executor, with the journal, over the local provider and the real archive provider.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;
mod journal_support;

use std::sync::Arc;

use common::*;
use journal_support::*;
use waypoint_protocol::VfsError;
use waypoint_provider_archive::{ArchiveOptions, ArchiveProvider};

type Jh = JournalHarness<LocalProvider>;

/// A harness whose engine also serves archives held in its sandbox.
fn archives() -> (Jh, tempfile::TempDir) {
    let (h, dir, _) = archives_with();
    (h, dir)
}

fn archives_with() -> (Jh, tempfile::TempDir, Arc<ArchiveProvider>) {
    let (mut h, dir) = local_jh();
    let provider = h.provider.clone();
    let served = ArchiveProvider::new(
        Arc::new(
            move |path: &VfsPath| -> Result<Arc<dyn Provider>, VfsError> {
                match path {
                    VfsPath::File(_) => Ok(provider.clone()),
                    other => Err(VfsError::Unsupported {
                        what: other.scheme().to_owned(),
                    }),
                }
            },
        ),
        ArchiveOptions::default(),
    );
    h.harness.env.providers.register_archives(served.clone());
    (h, dir, served)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// A stored zip whose entries say what `claimed` says about their size: `(name, data, claimed size)`.
fn raw_zip(entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data, claimed) in entries {
        let offset = out.len() as u32;
        let crc = crc32(data);
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&[20, 0, 0, 8, 0, 0, 0, 0, 0x21, 0x58]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&claimed.to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&[20, 3, 20, 0, 0, 8, 0, 0, 0, 0, 0x21, 0x58]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&claimed.to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0u8; 8]);
        central.extend_from_slice(&(0o100_644u32 << 16).to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let at = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&at.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn put(h: &Jh, relative: &str, bytes: &[u8]) {
    use std::io::Write;
    let mut stream = h
        .provider
        .create_write(&h.path(relative), waypoint_vfs::WriteOptions::exclusive())
        .unwrap();
    stream.write_all(bytes).unwrap();
    stream.finish(false).unwrap();
    h.provider.reset();
}

fn compress(h: &Jh, sources: &[&str], dest: &str, name: &str, format: ArchiveFormat) -> JobRequest {
    let mut request = h.request(JobKind::Compress, sources, Some(dest), Some(name));
    request.archive = Some(ArchiveSpec::Compress { format });
    request
}

fn extract(h: &Jh, archives: &[&str], dest: Option<&str>, layout: ExtractLayout) -> JobRequest {
    let mut request = h.request(JobKind::Extract, archives, dest, None);
    request.archive = Some(ArchiveSpec::Extract {
        layout,
        allow_large: false,
    });
    request
}

fn ok(run: &JournalRun) {
    assert_eq!(
        run.state,
        JobState::Done,
        "{:?} {:?}",
        run.failure,
        run.undo
    );
}

fn sample() -> Tree {
    let mut tree = tree(&[
        ("out/", ""),
        ("src/", ""),
        ("src/a.txt", "alpha"),
        ("src/deep/", ""),
        ("src/deep/b.txt", "beta"),
        ("src/empty/", ""),
        ("src/zero", ""),
    ]);
    tree.insert(
        "src/big.bin".into(),
        Node::File((0..200_000u32).map(|n| (n % 251) as u8).collect()),
    );
    tree
}

/// The entries of `tree` below `prefix`, as they would be below `new`.
fn moved(tree: &Tree, prefix: &str, new: &str) -> Tree {
    tree.iter()
        .filter_map(|(path, node)| {
            path.strip_prefix(prefix)
                .filter(|rest| rest.starts_with('/'))
                .map(|rest| (format!("{new}{rest}"), node.clone()))
        })
        .collect()
}

#[test]
fn every_format_compresses_and_extracts_back_and_both_undo() {
    for format in ArchiveFormat::ALL {
        let (mut h, _g) = archives();
        let name = format!("pack{}", format.extension());
        jbuild(&h, &sample());
        let before = jwork(&h);
        let made = h.run_journalled(compress(&h, &["src"], "", "pack", format));
        ok(&made);
        let entry = made.entry.expect("a compression is journalled");
        assert_eq!(made.report.as_ref().unwrap().created, vec![h.loc(&name)]);
        let with_archive = jwork(&h);
        assert!(
            partials(&with_archive).is_empty(),
            "no partial file is left"
        );
        assert!(matches!(with_archive.get(&name), Some(Node::File(bytes)) if !bytes.is_empty()));

        // Extracting it into a folder named after it gives back what was packed.
        let out = h.run_journalled(extract(&h, &[&name], Some("out"), ExtractLayout::Folder));
        ok(&out);
        let after = jwork(&h);
        assert!(partials(&after).is_empty());
        for (path, node) in moved(&before, "src", "out/pack/src") {
            assert_eq!(after.get(&path), Some(&node), "{format:?}: {path}");
        }
        assert_eq!(after.get("out/pack"), Some(&Node::Dir));

        // Undo takes away exactly what was extracted, then exactly the archive.
        ok(&h.undo(out.entry.expect("an extraction is journalled")));
        assert_eq!(
            jwork(&h),
            with_archive,
            "{format:?}: the extraction is undone"
        );
        ok(&h.undo(entry));
        assert_eq!(jwork(&h), before, "{format:?}: the archive is undone");
        // And both can be done again.
        ok(&h.redo(entry));
        assert_eq!(jwork(&h), with_archive, "{format:?}: redo");
    }
}

#[test]
fn the_auto_layout_never_scatters_an_archive_among_what_is_there() {
    let (mut h, _g) = archives();
    jbuild(
        &h,
        &tree(&[
            ("out/", ""),
            ("out2/", ""),
            ("two/", ""),
            ("two/a", "1"),
            ("two/b", "2"),
            ("one/", ""),
            ("one/only", "x"),
        ]),
    );
    ok(&h.run_journalled(compress(
        &h,
        &["two/a", "two/b"],
        "",
        "pair",
        ArchiveFormat::Zip,
    )));
    ok(&h.run_journalled(compress(&h, &["one"], "", "single", ArchiveFormat::TarGz)));
    ok(&h.run_journalled(extract(
        &h,
        &["pair.zip", "single.tar.gz"],
        Some("out"),
        ExtractLayout::Auto,
    )));
    let tree = jwork(&h);
    // Two things at its top: a folder named after the archive. One thing: put straight in.
    assert_eq!(tree.get("out/pair/a"), Some(&file("1")));
    assert_eq!(tree.get("out/pair/b"), Some(&file("2")));
    assert_eq!(tree.get("out/one/only"), Some(&file("x")));
    assert!(!tree.contains_key("out/a") && !tree.contains_key("out/single"));
    // Contents puts the top-level entries in as they are.
    ok(&h.run_journalled(extract(
        &h,
        &["pair.zip"],
        Some("out2"),
        ExtractLayout::Contents,
    )));
    let tree = jwork(&h);
    assert_eq!(tree.get("out2/a"), Some(&file("1")));
    // Without a destination an archive is extracted beside itself.
    ok(&h.run_journalled(extract(&h, &["pair.zip"], None, ExtractLayout::Auto)));
    assert_eq!(jwork(&h).get("pair/a"), Some(&file("1")));
}

#[test]
fn a_name_that_is_taken_is_a_conflict_extraction_answers_like_a_copy() {
    let (mut h, _g) = archives();
    jbuild(&h, &tree(&[("out/", ""), ("src/", ""), ("src/a", "new")]));
    ok(&h.run_journalled(compress(&h, &["src/a"], "", "pack", ArchiveFormat::Zip)));
    put(&h, "out/a", b"old");
    // Nobody has said what to do, so nothing is overwritten.
    let asked = h
        .plan(&extract(
            &h,
            &["pack.zip"],
            Some("out"),
            ExtractLayout::Contents,
        ))
        .unwrap();
    assert_eq!(asked.conflicts.len(), 1);
    let refused = h.run_journalled(extract(
        &h,
        &["pack.zip"],
        Some("out"),
        ExtractLayout::Contents,
    ));
    assert!(
        matches!(refused.state, JobState::Failed { .. }),
        "{:?}",
        refused.state
    );
    assert_eq!(jwork(&h).get("out/a"), Some(&file("old")));
    let mut keep = extract(&h, &["pack.zip"], Some("out"), ExtractLayout::Contents);
    keep.options.conflict = Some(ConflictPolicy::KeepBoth);
    ok(&h.run_journalled(keep));
    let tree = jwork(&h);
    assert_eq!(tree.get("out/a"), Some(&file("old")));
    assert_eq!(tree.get("out/a (2)"), Some(&file("new")));
    let mut skip = extract(&h, &["pack.zip"], Some("out"), ExtractLayout::Contents);
    skip.options.conflict = Some(ConflictPolicy::Skip);
    ok(&h.run_journalled(skip));
    assert_eq!(jwork(&h).len(), tree.len());
}

#[test]
fn a_compression_answers_a_taken_archive_name() {
    let (mut h, _g) = archives();
    jbuild(&h, &tree(&[("src/", ""), ("src/a", "1")]));
    put(&h, "pack.zip", b"not really a zip");
    let refused = h.run_journalled(compress(&h, &["src"], "", "pack", ArchiveFormat::Zip));
    assert!(matches!(refused.state, JobState::Failed { .. }));
    assert_eq!(jwork(&h).get("pack.zip"), Some(&file("not really a zip")));
    let mut keep = compress(&h, &["src"], "", "pack", ArchiveFormat::Zip);
    keep.options.conflict = Some(ConflictPolicy::KeepBoth);
    let kept = h.run_journalled(keep);
    ok(&kept);
    assert_eq!(kept.report.unwrap().created, vec![h.loc("pack (2).zip")]);
    let mut skip = compress(&h, &["src"], "", "pack", ArchiveFormat::Zip);
    skip.options.conflict = Some(ConflictPolicy::Skip);
    let skipped = h.run_journalled(skip);
    ok(&skipped);
    assert_eq!(
        skipped.entry, None,
        "nothing was done, so nothing is journalled"
    );
    let mut replace = compress(&h, &["src"], "", "pack", ArchiveFormat::Zip);
    replace.options.conflict = Some(ConflictPolicy::Replace);
    let replaced = h.run_journalled(replace);
    ok(&replaced);
    assert!(matches!(jwork(&h).get("pack.zip"), Some(Node::File(b)) if b.len() > 20));
    assert_eq!(
        replaced.entry, None,
        "a replaced archive is gone for good, so it cannot be undone"
    );
    assert!(partials(&jwork(&h)).is_empty());
}

#[test]
fn an_archive_never_contains_itself() {
    let (mut h, _g) = archives();
    jbuild(&h, &tree(&[("src/", ""), ("src/a", "1")]));
    // The archive is made inside the folder it packs.
    ok(&h.run_journalled(compress(&h, &["src"], "src", "pack", ArchiveFormat::Zip)));
    ok(&h.run_journalled(extract(
        &h,
        &["src/pack.zip"],
        Some("src"),
        ExtractLayout::Folder,
    )));
    let tree = jwork(&h);
    assert_eq!(tree.get("src/pack/src/a"), Some(&file("1")));
    assert!(!tree.contains_key("src/pack/src/pack.zip"));
}

#[test]
fn a_cancel_at_any_step_leaves_nothing_half_written() {
    // How many provider calls an uninterrupted run makes, for each job.
    let small = || {
        tree(&[
            ("out/", ""),
            ("src/", ""),
            ("src/a", "alpha"),
            ("src/d/", ""),
            ("src/d/b", "beta"),
            ("src/empty/", ""),
        ])
    };
    for format in [
        ArchiveFormat::Zip,
        ArchiveFormat::TarGz,
        ArchiveFormat::SevenZ,
    ] {
        let name = format!("pack{}", format.extension());
        let calls = {
            let (mut h, _g) = archives();
            jbuild(&h, &small());
            ok(&h.run_journalled(compress(&h, &["src"], "", "pack", format)));
            let before = h.provider.calls();
            ok(&h.run_journalled(extract(&h, &[&name], Some("out"), ExtractLayout::Folder)));
            (before, h.provider.calls())
        };
        // A cancel during the compression.
        for step in 1..=calls.0 {
            let (mut h, _g) = archives();
            jbuild(&h, &small());
            let before = jwork(&h);
            let run = h.run_journalled_hooked(
                compress(&h, &["src"], "", "pack", format),
                &mut |h, token| h.provider.cancel_at(step, token),
            );
            let tree = jwork(&h);
            assert!(
                partials(&tree).is_empty(),
                "{format:?} step {step}: {:?}",
                partials(&tree)
            );
            match run.state {
                JobState::Done => assert!(tree.contains_key(&name)),
                _ => assert_eq!(
                    tree, before,
                    "{format:?} step {step}: a cancelled compression leaves nothing"
                ),
            }
        }
        // A cancel during the extraction.
        for step in 1..=(calls.1 - calls.0) {
            let (mut h, _g) = archives();
            jbuild(&h, &small());
            ok(&h.run_journalled(compress(&h, &["src"], "", "pack", format)));
            let before = jwork(&h);
            h.provider.reset();
            let run = h.run_journalled_hooked(
                extract(&h, &[&name], Some("out"), ExtractLayout::Folder),
                &mut |h, token| h.provider.cancel_at(step, token),
            );
            let tree = jwork(&h);
            assert!(
                partials(&tree).is_empty(),
                "{format:?} step {step}: {:?}",
                partials(&tree)
            );
            match run.state {
                JobState::Done => assert_eq!(tree.get("out/pack/src/d/b"), Some(&file("beta"))),
                _ => assert!(
                    !tree.keys().any(|k| k.starts_with("out/pack"))
                        || tree.get("out/pack/src/d/b") == Some(&file("beta")),
                    "{format:?} step {step}: only whole entries stay: {tree:?} (before {before:?})"
                ),
            }
        }
    }
}

#[test]
fn entries_that_try_to_escape_are_left_out_and_nothing_lands_outside() {
    let (mut h, _g) = archives();
    jbuild(&h, &tree(&[("out/", "")]));
    put(
        &h,
        "evil.zip",
        &raw_zip(&[
            ("fine.txt", b"ok", 2),
            ("../../outside.txt", b"slip", 4),
            ("/abs/path.txt", b"abs", 3),
            ("dir/../up.txt", b"mid", 3),
        ]),
    );
    let request = extract(&h, &["evil.zip"], Some("out"), ExtractLayout::Contents);
    let plan = h.plan(&request).unwrap();
    let left_out = plan
        .warnings
        .iter()
        .filter(|w| matches!(w, PlanWarning::LeftOut { .. }))
        .count();
    assert_eq!(left_out, 3, "{:?}", plan.warnings);
    ok(&h.run_journalled(request));
    let tree = jwork(&h);
    assert_eq!(tree.get("out/fine.txt"), Some(&file("ok")));
    // Nothing was made anywhere else in the sandbox, whatever it was called.
    let names: Vec<&String> = tree.keys().filter(|k| *k != "evil.zip").collect();
    assert_eq!(names, ["out", "out/fine.txt"], "{tree:?}");
}

#[test]
fn an_archive_that_expands_past_the_limits_asks_first() {
    let (mut h, _g) = archives();
    jbuild(&h, &tree(&[("out/", "")]));
    // Says it holds 3 GiB in a file of a few dozen bytes: a bomb by ratio.
    put(
        &h,
        "bomb.zip",
        &raw_zip(&[("big", b"tiny", 3 * 1024 * 1024 * 1024)]),
    );
    let request = extract(&h, &["bomb.zip"], Some("out"), ExtractLayout::Auto);
    match h.plan(&request) {
        Err(OpsError::ArchiveLimit {
            limit: ArchiveLimit::Ratio { ratio, max },
            ..
        }) => {
            assert!(ratio > max);
        }
        other => panic!("{other:?}"),
    }
    // Too many bytes outright.
    put(&h, "huge.zip", &raw_zip(&[("big", b"x", u32::MAX)]));
    let count = (ARCHIVE_MAX_BYTES / u64::from(u32::MAX)) as usize + 2;
    let entries: Vec<(String, &[u8], u32)> = (0..count)
        .map(|n| (format!("f{n}"), &b"x"[..], u32::MAX))
        .collect();
    let refs: Vec<(&str, &[u8], u32)> = entries
        .iter()
        .map(|(n, d, c)| (n.as_str(), *d, *c))
        .collect();
    put(&h, "heavy.zip", &raw_zip(&refs));
    assert!(matches!(
        h.plan(&extract(
            &h,
            &["heavy.zip"],
            Some("out"),
            ExtractLayout::Auto
        )),
        Err(OpsError::ArchiveLimit {
            limit: ArchiveLimit::Bytes { .. },
            ..
        })
    ));
    // Told to go ahead, the plan is made, and the run still holds an entry to what it declared.
    let mut allowed = extract(&h, &["bomb.zip"], Some("out"), ExtractLayout::Auto);
    allowed.archive = Some(ArchiveSpec::Extract {
        layout: ExtractLayout::Auto,
        allow_large: true,
    });
    assert!(h.plan(&allowed).is_ok());
    let run = h.run_journalled(allowed);
    assert!(
        matches!(run.state, JobState::Failed { .. }),
        "an entry that is short of its size fails"
    );
    let tree = jwork(&h);
    assert!(partials(&tree).is_empty());
    assert!(!tree.keys().any(|k| k.starts_with("out/")), "{tree:?}");
}

#[test]
fn an_entry_that_holds_more_than_it_declared_fails_and_leaves_nothing() {
    let (mut h, _g) = archives();
    jbuild(&h, &tree(&[("out/", "")]));
    // Declares 5 bytes and stores 5 000.
    let data = vec![b'z'; 5_000];
    put(
        &h,
        "liar.zip",
        &raw_zip(&[("liar.txt", &data, 5), ("honest.txt", b"fine", 4)]),
    );
    let run = h.run_journalled(extract(
        &h,
        &["liar.zip"],
        Some("out"),
        ExtractLayout::Contents,
    ));
    assert!(
        matches!(run.state, JobState::Failed { .. }),
        "{:?}",
        run.state
    );
    let tree = jwork(&h);
    assert!(partials(&tree).is_empty(), "{tree:?}");
    assert!(!tree.contains_key("out/liar.txt"));
}

#[test]
fn an_encrypted_archive_asks_for_its_password_and_goes_on_once_given_it() {
    if std::process::Command::new("zip")
        .arg("-v")
        .output()
        .is_err()
    {
        eprintln!("skipped: the `zip` tool is not installed");
        return;
    }
    let (mut h, g, archives) = archives_with();
    jbuild(
        &h,
        &tree(&[("out/", ""), ("src/", ""), ("src/a", "secret text")]),
    );
    let work = g.path().join("work");
    let made = std::process::Command::new("zip")
        .current_dir(&work)
        .args(["-qr", "-P", "pw", "locked.zip", "src"])
        .status()
        .unwrap();
    assert!(made.success());
    let request = extract(&h, &["locked.zip"], Some("out"), ExtractLayout::Contents);
    let asked = h.run_journalled(request.clone());
    match &asked.state {
        JobState::Failed {
            error:
                OpsError::Connection {
                    error: VfsError::AuthRequired { .. },
                },
            ..
        } => {}
        other => panic!("{other:?}"),
    }
    assert!(partials(&jwork(&h)).is_empty());
    assert!(!jwork(&h).keys().any(|k| k.starts_with("out/")));
    archives.unlock(&h.path("locked.zip"), waypoint_vfs::Secret::from("pw"));
    ok(&h.run_journalled(request));
    assert_eq!(jwork(&h).get("out/src/a"), Some(&file("secret text")));
}

#[test]
fn a_damaged_archive_fails_the_plan_without_touching_anything() {
    let (mut h, _g) = archives();
    jbuild(&h, &tree(&[("out/", "")]));
    put(&h, "cut.zip", &raw_zip(&[("a", b"hello", 5)])[..40]);
    let before = jwork(&h);
    let run = h.run_journalled(extract(&h, &["cut.zip"], Some("out"), ExtractLayout::Auto));
    assert!(
        matches!(run.state, JobState::Failed { .. }),
        "{:?}",
        run.state
    );
    assert_eq!(jwork(&h), before);
    // A folder is not an archive.
    let run = h.run_journalled(extract(&h, &["out"], Some("out"), ExtractLayout::Auto));
    assert!(
        matches!(run.state, JobState::Failed { .. }),
        "{:?}",
        run.state
    );
}

#[cfg(unix)]
#[test]
fn modes_times_and_links_survive_a_round_trip_and_unsafe_bits_do_not() {
    use waypoint_vfs::Provider as _;
    let (mut h, _g) = archives();
    jbuild(
        &h,
        &tree(&[
            ("out/", ""),
            ("src/", ""),
            ("src/run.sh", "#!/bin/sh\n"),
            ("src/plain", "p"),
        ]),
    );
    let src = h.path("src/run.sh");
    h.provider
        .set_permissions(
            &src,
            waypoint_vfs::Permissions {
                mode: Some(0o4755),
                readonly: false,
            },
        )
        .ok();
    h.provider
        .set_times(
            &src,
            waypoint_vfs::FileTimes {
                accessed: None,
                modified: Some(
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000),
                ),
            },
        )
        .unwrap();
    h.provider
        .symlink(&h.path("src/link"), std::ffi::OsStr::new("run.sh"))
        .unwrap();
    h.provider.reset();
    ok(&h.run_journalled(compress(&h, &["src"], "", "pack", ArchiveFormat::TarGz)));
    ok(&h.run_journalled(extract(
        &h,
        &["pack.tar.gz"],
        Some("out"),
        ExtractLayout::Contents,
    )));
    let run = h.path("out/src/run.sh");
    let stat = h.provider.stat(&run).unwrap();
    assert_eq!(stat.modified_ms, Some(1_600_000_000_000));
    let mode = h.provider.permissions(&run).unwrap().mode.unwrap();
    assert_eq!(mode & 0o777, 0o755, "the mode is kept");
    assert_eq!(mode & 0o7000, 0, "setuid, setgid and sticky are not");
    let link = h.path("out/src/link");
    assert_eq!(h.provider.read_link(&link).unwrap(), "run.sh");
}
