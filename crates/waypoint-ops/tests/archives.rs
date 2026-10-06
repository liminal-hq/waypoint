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
use waypoint_protocol::{Location, VfsError};
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
    let count = (h.archive_limits.max_bytes / u64::from(u32::MAX)) as usize + 2;
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

#[test]
fn the_limits_are_the_ones_in_the_settings() {
    let (mut h, _g) = archives();
    jbuild(&h, &tree(&[("out/", "")]));
    let names: Vec<String> = (0..20).map(|n| format!("f{n}")).collect();
    let entries: Vec<(&str, &[u8], u32)> =
        names.iter().map(|n| (n.as_str(), &b"x"[..], 1)).collect();
    put(&h, "twenty.zip", &raw_zip(&entries));
    let request = extract(&h, &["twenty.zip"], Some("out"), ExtractLayout::Contents);
    assert!(h.plan(&request).is_ok(), "within the defaults");
    // Fewer entries allowed than it holds.
    h.archive_limits.max_entries = 10;
    assert!(matches!(
        h.plan(&request),
        Err(OpsError::ArchiveLimit {
            limit: ArchiveLimit::Entries { found: 20, max: 10 },
            ..
        })
    ));
    h.archive_limits = OpsSettings::default().archive_limits();
    // Fewer bytes allowed than it expands to.
    put(&h, "five.zip", &raw_zip(&[("f", b"x", 5_000)]));
    h.archive_limits.max_bytes = 1_000;
    let five = extract(&h, &["five.zip"], Some("out"), ExtractLayout::Contents);
    assert!(matches!(
        h.plan(&five),
        Err(OpsError::ArchiveLimit {
            limit: ArchiveLimit::Bytes {
                found: 5_000,
                max: 1_000
            },
            ..
        })
    ));
    // A ratio limit that applies once the archive expands past its floor.
    h.archive_limits = OpsSettings::default().archive_limits();
    h.archive_limits.max_ratio = 10;
    h.archive_limits.ratio_floor_bytes = 1_000;
    assert!(matches!(
        h.plan(&five),
        Err(OpsError::ArchiveLimit {
            limit: ArchiveLimit::Ratio { max: 10, .. },
            ..
        })
    ));
    // Below the floor the ratio is not looked at.
    h.archive_limits.ratio_floor_bytes = 10_000;
    assert!(h.plan(&five).is_ok());
    // And allowing large overrides every limit.
    h.archive_limits.max_bytes = 1;
    let mut allowed = five.clone();
    allowed.archive = Some(ArchiveSpec::Extract {
        layout: ExtractLayout::Contents,
        allow_large: true,
    });
    assert!(h.plan(&allowed).is_ok());
    let _ = &mut h;
}

#[test]
fn an_archive_inside_an_archive_can_be_extracted_by_itself() {
    let (mut h, _g) = archives();
    jbuild(&h, &tree(&[("out/", "")]));
    let inner = raw_zip(&[("note.txt", b"nested note", 11)]);
    put(
        &h,
        "outer.zip",
        &raw_zip(&[("inner.zip", &inner, inner.len() as u32)]),
    );
    // The source is the file `inner.zip` as it is seen inside `outer.zip`.
    let outer = waypoint_path::ArchivePath::new(h.path("outer.zip")).unwrap();
    let nested = VfsPath::Archive(outer.join("inner.zip").unwrap()).to_location();
    let mut request = extract(&h, &[], Some("out"), ExtractLayout::Folder);
    request.sources = Sources::Locations {
        locations: vec![nested],
    };
    ok(&h.run_journalled(request));
    let tree = jwork(&h);
    assert_eq!(tree.get("out/inner/note.txt"), Some(&file("nested note")));
}

#[test]
fn extract_all_style_extraction_takes_a_free_folder_name_when_the_name_is_taken() {
    let (mut h, _g) = archives();
    jbuild(&h, &tree(&[("src/", ""), ("src/a", "1"), ("src/b", "2")]));
    ok(&h.run_journalled(compress(&h, &["src"], "", "pack", ArchiveFormat::Zip)));
    // Beside the archive, into a folder named after it, again and again.
    for _ in 0..3 {
        let mut request = extract(&h, &["pack.zip"], None, ExtractLayout::Folder);
        request.options.conflict = Some(ConflictPolicy::KeepBoth);
        ok(&h.run_journalled(request));
    }
    let tree = jwork(&h);
    assert_eq!(tree.get("pack/src/a"), Some(&file("1")));
    assert_eq!(tree.get("pack (2)/src/a"), Some(&file("1")));
    assert_eq!(tree.get("pack (3)/src/b"), Some(&file("2")));
}

// ---- Changing an archive that is open (D170) ----

/// The location of `inner` (`""` for the top) inside the archive file at `archive`.
fn inside(h: &Jh, archive: &str, inner: &str) -> Location {
    let mut path = VfsPath::Archive(waypoint_path::ArchivePath::new(h.path(archive)).unwrap());
    for part in inner.split('/').filter(|p| !p.is_empty()) {
        path = path.join(part).unwrap();
    }
    path.to_location()
}

fn add_request(h: &Jh, sources: &[&str], archive: &str, inner: &str) -> JobRequest {
    let mut request = h.request(JobKind::Copy, sources, None, None);
    request.destination = Some(inside(h, archive, inner));
    request
}

fn edit_request(
    h: &Jh,
    kind: JobKind,
    archive: &str,
    inner: &[&str],
    name: Option<&str>,
) -> JobRequest {
    let mut request = h.request(kind, &[], None, name);
    request.sources = Sources::Locations {
        locations: inner.iter().map(|i| inside(h, archive, i)).collect(),
    };
    request
}

/// What an archive holds, as the files `extract` makes of it.
fn contents(h: &mut Jh, archive: &str) -> Tree {
    static PEEKS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = PEEKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let out = format!("peek-{n}");
    jbuild(h, &tree(&[(&format!("{out}/"), "")]));
    ok(&h.run_journalled(extract(h, &[archive], Some(&out), ExtractLayout::Contents)));
    jwork(h)
        .into_iter()
        .filter_map(|(path, node)| {
            path.strip_prefix(&format!("{out}/"))
                .map(|rest| (rest.to_owned(), node))
        })
        .collect()
}

fn small() -> Tree {
    tree(&[
        ("src/", ""),
        ("src/a.txt", "alpha"),
        ("src/d/", ""),
        ("src/d/b.txt", "beta"),
        ("src/empty/", ""),
        ("extra/", ""),
        ("extra/new.txt", "fresh"),
        ("extra/dir/", ""),
        ("extra/dir/deep.txt", "deeper"),
    ])
}

#[test]
fn files_dropped_into_an_archive_are_added_and_undo_brings_back_the_old_archive() {
    for format in ArchiveFormat::ALL {
        let (mut h, _g) = archives();
        let name = format!("pack{}", format.extension());
        jbuild(&h, &small());
        ok(&h.run_journalled(compress(&h, &["src"], "", "pack", format)));
        let before = jwork(&h);
        let original = contents(&mut h, &name);
        let after_peek = jwork(&h);

        let added = h.run_journalled(add_request(
            &h,
            &["extra/new.txt", "extra/dir"],
            &name,
            "src/d",
        ));
        ok(&added);
        let entry = added.entry.expect("a change to an archive is journalled");
        let tree_now = jwork(&h);
        assert!(
            partials(&tree_now).is_empty(),
            "{format:?}: no partial file is left"
        );
        assert_ne!(
            tree_now.get(&name),
            before.get(&name),
            "{format:?}: the archive changed"
        );
        let now = contents(&mut h, &name);
        // Everything that was there still is, and what was added is below `src/d`.
        for (path, node) in &original {
            assert_eq!(now.get(path), Some(node), "{format:?}: {path} is untouched");
        }
        assert_eq!(now.get("src/d/new.txt"), Some(&file("fresh")), "{format:?}");
        assert_eq!(
            now.get("src/d/dir/deep.txt"),
            Some(&file("deeper")),
            "{format:?}"
        );

        // Undo puts the very same archive file back; redo does it again.
        let _ = after_peek;
        ok(&h.undo(entry));
        let undone = jwork(&h);
        assert_eq!(
            undone.get(&name),
            before.get(&name),
            "{format:?}: the old archive is back"
        );
        ok(&h.redo(entry));
        assert_eq!(
            contents(&mut h, &name).get("src/d/new.txt"),
            Some(&file("fresh")),
            "{format:?}: redo"
        );
    }
}

#[test]
fn an_entry_is_renamed_in_an_archive_with_everything_below_it() {
    for format in ArchiveFormat::ALL {
        let (mut h, _g) = archives();
        let name = format!("pack{}", format.extension());
        jbuild(&h, &small());
        ok(&h.run_journalled(compress(&h, &["src"], "", "pack", format)));
        let before = jwork(&h);
        let renamed = h.run_journalled(edit_request(
            &h,
            JobKind::Rename,
            &name,
            &["src/d"],
            Some("deep"),
        ));
        ok(&renamed);
        let now = contents(&mut h, &name);
        assert_eq!(now.get("src/deep/b.txt"), Some(&file("beta")), "{format:?}");
        assert!(!now.contains_key("src/d/b.txt"), "{format:?}");
        assert_eq!(now.get("src/a.txt"), Some(&file("alpha")), "{format:?}");
        ok(&h.undo(renamed.entry.expect("journalled")));
        assert_eq!(jwork(&h).get(&name), before.get(&name), "{format:?}");
    }
}

#[test]
fn a_name_taken_in_the_archive_cannot_be_renamed_to() {
    let (mut h, _g) = archives();
    jbuild(&h, &small());
    ok(&h.run_journalled(compress(&h, &["src"], "", "pack", ArchiveFormat::Zip)));
    let before = jwork(&h);
    let run = h.run_journalled(edit_request(
        &h,
        JobKind::Rename,
        "pack.zip",
        &["src/a.txt"],
        Some("d"),
    ));
    assert!(
        matches!(failed_with(&run), OpsError::NameInUse { .. }),
        "{:?}",
        run.state
    );
    assert_eq!(jwork(&h), before);
}

#[test]
fn entries_are_deleted_from_an_archive_and_undo_restores_it() {
    for kind in [JobKind::Delete, JobKind::Trash] {
        for format in ArchiveFormat::ALL {
            let (mut h, _g) = archives();
            let name = format!("pack{}", format.extension());
            jbuild(&h, &small());
            ok(&h.run_journalled(compress(&h, &["src"], "", "pack", format)));
            let before = jwork(&h);
            let run =
                h.run_journalled(edit_request(&h, kind, &name, &["src/d", "src/a.txt"], None));
            ok(&run);
            let now = contents(&mut h, &name);
            assert!(
                !now.keys()
                    .any(|k| k.starts_with("src/d") || k == "src/a.txt"),
                "{format:?}: {now:?}"
            );
            assert!(
                now.contains_key("src/empty"),
                "{format:?}: what was not deleted stays"
            );
            ok(&h.undo(run.entry.expect("journalled")));
            assert_eq!(jwork(&h).get(&name), before.get(&name), "{format:?}");
        }
    }
}

#[test]
fn a_name_taken_inside_the_archive_is_a_conflict_answered_like_a_copy() {
    let (mut h, _g) = archives();
    jbuild(&h, &small());
    ok(&h.run_journalled(compress(&h, &["src"], "", "pack", ArchiveFormat::Zip)));
    jbuild(&h, &tree(&[("more/", ""), ("more/a.txt", "newer")]));
    let before = jwork(&h);
    let request = add_request(&h, &["more/a.txt"], "pack.zip", "src");
    let asked = h.plan(&request).unwrap();
    assert_eq!(asked.conflicts.len(), 1);
    // Nobody said what to do: nothing is overwritten.
    let refused = h.run_journalled(request.clone());
    assert!(
        matches!(refused.state, JobState::Failed { .. }),
        "{:?}",
        refused.state
    );
    assert_eq!(jwork(&h), before);

    let mut keep = request.clone();
    keep.options.conflict = Some(ConflictPolicy::KeepBoth);
    ok(&h.run_journalled(keep));
    let now = contents(&mut h, "pack.zip");
    assert_eq!(now.get("src/a.txt"), Some(&file("alpha")));
    assert_eq!(now.get("src/a (2).txt"), Some(&file("newer")));

    let mut replace = request.clone();
    replace.options.conflict = Some(ConflictPolicy::Replace);
    ok(&h.run_journalled(replace));
    assert_eq!(
        contents(&mut h, "pack.zip").get("src/a.txt"),
        Some(&file("newer"))
    );

    let mut skip = request;
    skip.options.conflict = Some(ConflictPolicy::Skip);
    let skipped = h.run_journalled(skip);
    ok(&skipped);
    assert_eq!(skipped.entry, None, "nothing was done");
}

#[test]
fn a_folder_dropped_on_a_folder_of_the_archive_merges() {
    let (mut h, _g) = archives();
    jbuild(&h, &small());
    ok(&h.run_journalled(compress(&h, &["src"], "", "pack", ArchiveFormat::Zip)));
    jbuild(&h, &tree(&[("d/", ""), ("d/c.txt", "gamma")]));
    let mut request = add_request(&h, &["d"], "pack.zip", "src");
    request.options.conflict = Some(ConflictPolicy::MergeFolders);
    ok(&h.run_journalled(request));
    let now = contents(&mut h, "pack.zip");
    assert_eq!(now.get("src/d/b.txt"), Some(&file("beta")));
    assert_eq!(now.get("src/d/c.txt"), Some(&file("gamma")));
}

#[test]
fn what_cannot_be_rewritten_is_refused_with_its_reason() {
    let (mut h, _g) = archives();
    jbuild(&h, &small());
    // An entry name that was changed to be safe would be stored under the changed name.
    put(&h, "evil.zip", &raw_zip(&[("../escape.txt", b"x", 1)]));
    let run = h.run_journalled(add_request(&h, &["extra/new.txt"], "evil.zip", ""));
    assert!(
        matches!(
            failed_with(&run),
            OpsError::ArchiveNotWritable {
                reason: ArchiveWriteRefusal::UnsafeNames,
                ..
            }
        ),
        "{:?}",
        run.state
    );
    // An archive inside an archive is read only.
    let inner = raw_zip(&[("note.txt", b"nested", 6)]);
    put(
        &h,
        "outer.zip",
        &raw_zip(&[("inner.zip", &inner, inner.len() as u32)]),
    );
    let outer = waypoint_path::ArchivePath::new(h.path("outer.zip")).unwrap();
    let file_in = VfsPath::Archive(outer.join("inner.zip").unwrap());
    let nested = VfsPath::Archive(waypoint_path::ArchivePath::new(file_in).unwrap()).to_location();
    let mut request = h.request(JobKind::Copy, &["extra/new.txt"], None, None);
    request.destination = Some(nested);
    let run = h.run_journalled(request);
    assert!(
        matches!(
            failed_with(&run),
            OpsError::ArchiveNotWritable {
                reason: ArchiveWriteRefusal::Nested,
                ..
            }
        ),
        "{:?}",
        run.state
    );
    // Moving into an archive is not offered: it is a copy.
    ok(&h.run_journalled(compress(&h, &["src"], "", "pack", ArchiveFormat::Zip)));
    let mut request = add_request(&h, &["extra/new.txt"], "pack.zip", "");
    request.kind = JobKind::Move;
    let run = h.run_journalled(request);
    assert!(
        matches!(failed_with(&run), OpsError::Unsupported { .. }),
        "{:?}",
        run.state
    );
}

#[test]
fn a_cancel_at_any_step_leaves_the_original_archive_alone() {
    for format in [
        ArchiveFormat::Zip,
        ArchiveFormat::TarGz,
        ArchiveFormat::SevenZ,
    ] {
        let name = format!("pack{}", format.extension());
        let steps = {
            let (mut h, _g) = archives();
            jbuild(&h, &small());
            ok(&h.run_journalled(compress(&h, &["src"], "", "pack", format)));
            h.provider.reset();
            ok(&h.run_journalled(add_request(&h, &["extra/new.txt"], &name, "")));
            h.provider.calls()
        };
        for step in 1..=steps {
            let (mut h, _g) = archives();
            jbuild(&h, &small());
            ok(&h.run_journalled(compress(&h, &["src"], "", "pack", format)));
            let before = jwork(&h);
            h.provider.reset();
            let run = h.run_journalled_hooked(
                add_request(&h, &["extra/new.txt"], &name, ""),
                &mut |h, token| h.provider.cancel_at(step, token),
            );
            let tree = jwork(&h);
            assert!(
                partials(&tree).is_empty(),
                "{format:?} step {step}: {:?}",
                partials(&tree)
            );
            match run.state {
                JobState::Done => assert_ne!(tree.get(&name), before.get(&name)),
                _ => assert_eq!(
                    tree.get(&name),
                    before.get(&name),
                    "{format:?} step {step}: the original is intact"
                ),
            }
        }
    }
}

#[test]
fn a_change_to_an_archive_needs_room_for_the_new_one() {
    let (mut h, _g) = archives();
    jbuild(&h, &small());
    ok(&h.run_journalled(compress(&h, &["src"], "", "pack", ArchiveFormat::Zip)));
    // Added to itself.
    let run = h.run_journalled(add_request(&h, &["pack.zip"], "pack.zip", ""));
    assert!(matches!(run.state, JobState::Failed { .. }));
}

/// Whether the program is installed; a test that needs it says so and passes without it.
fn have(program: &str) -> bool {
    let found = std::process::Command::new(program)
        .arg(if program == "7z" { "i" } else { "--help" })
        .output()
        .is_ok();
    if !found {
        eprintln!("skipping: {program} is not installed");
    }
    found
}

fn os_path(h: &Jh, relative: &str) -> String {
    h.path(relative).display()
}

fn run_tool(program: &str, args: &[&str], dir: &str) -> String {
    let out = std::process::Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("{program}: {e}"));
    assert!(
        out.status.success(),
        "{program} {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn archives_made_by_the_real_tools_are_changed_and_still_read_by_them() {
    // zip, tar and 7z each make an archive; Waypoint adds a file, renames one and deletes one; the
    // same tool lists the result and tests it.
    let (mut h, _g) = archives();
    jbuild(&h, &small());
    let dir = os_path(&h, "");
    let dir = dir.trim_end_matches(['/', '\\']).to_owned();
    let mut ran = 0;
    if have("zip") && have("unzip") {
        ran += 1;
        run_tool("zip", &["-qr", "tool.zip", "src"], &dir);
        h.provider.reset();
        ok(&h.run_journalled(add_request(&h, &["extra/new.txt"], "tool.zip", "src")));
        ok(&h.run_journalled(edit_request(
            &h,
            JobKind::Rename,
            "tool.zip",
            &["src/a.txt"],
            Some("first.txt"),
        )));
        ok(&h.run_journalled(edit_request(
            &h,
            JobKind::Delete,
            "tool.zip",
            &["src/d"],
            None,
        )));
        run_tool("unzip", &["-tq", "tool.zip"], &dir);
        let listing = run_tool("unzip", &["-Z1", "tool.zip"], &dir);
        assert!(
            listing.contains("src/new.txt") && listing.contains("src/first.txt"),
            "{listing}"
        );
        assert!(
            !listing.contains("src/d/b.txt") && !listing.contains("src/a.txt"),
            "{listing}"
        );
        let text = run_tool("unzip", &["-p", "tool.zip", "src/new.txt"], &dir);
        assert_eq!(text, "fresh");
    }
    if have("tar") {
        ran += 1;
        run_tool("tar", &["czf", "tool.tar.gz", "src"], &dir);
        h.provider.reset();
        ok(&h.run_journalled(add_request(&h, &["extra/new.txt"], "tool.tar.gz", "src")));
        ok(&h.run_journalled(edit_request(
            &h,
            JobKind::Rename,
            "tool.tar.gz",
            &["src/a.txt"],
            Some("first.txt"),
        )));
        ok(&h.run_journalled(edit_request(
            &h,
            JobKind::Delete,
            "tool.tar.gz",
            &["src/d"],
            None,
        )));
        let listing = run_tool("tar", &["tzf", "tool.tar.gz"], &dir);
        assert!(
            listing.contains("src/new.txt") && listing.contains("src/first.txt"),
            "{listing}"
        );
        assert!(
            !listing.contains("src/d/b.txt") && !listing.contains("src/a.txt"),
            "{listing}"
        );
        let text = run_tool("tar", &["xzOf", "tool.tar.gz", "src/new.txt"], &dir);
        assert_eq!(text, "fresh");
    }
    if have("7z") {
        ran += 1;
        run_tool("7z", &["a", "-bd", "tool.7z", "src"], &dir);
        h.provider.reset();
        ok(&h.run_journalled(add_request(&h, &["extra/new.txt"], "tool.7z", "src")));
        ok(&h.run_journalled(edit_request(
            &h,
            JobKind::Rename,
            "tool.7z",
            &["src/a.txt"],
            Some("first.txt"),
        )));
        ok(&h.run_journalled(edit_request(
            &h,
            JobKind::Delete,
            "tool.7z",
            &["src/d"],
            None,
        )));
        run_tool("7z", &["t", "-bd", "tool.7z"], &dir);
        // 7-Zip lists with the separator of the system it runs on.
        let listing = run_tool("7z", &["l", "-ba", "tool.7z"], &dir).replace('\\', "/");
        assert!(
            listing.contains("src/new.txt") && listing.contains("src/first.txt"),
            "{listing}"
        );
        assert!(
            !listing.contains("src/d/b.txt") && !listing.contains("src/a.txt"),
            "{listing}"
        );
    }
    eprintln!("{ran} of 3 real tools were available");
}

#[test]
fn a_new_folder_and_a_new_file_are_made_inside_an_archive() {
    for format in ArchiveFormat::ALL {
        let (mut h, _g) = archives();
        let name = format!("pack{}", format.extension());
        jbuild(&h, &small());
        ok(&h.run_journalled(compress(&h, &["src"], "", "pack", format)));
        let before = jwork(&h);
        let mut folder = h.request(JobKind::CreateFolder, &[], None, Some("fresh"));
        folder.destination = Some(inside(&h, &name, "src/d"));
        let made = h.run_journalled(folder);
        ok(&made);
        let mut file_request = h.request(JobKind::CreateFile, &[], None, Some("empty.txt"));
        file_request.destination = Some(inside(&h, &name, "src/d/fresh"));
        ok(&h.run_journalled(file_request));
        let now = contents(&mut h, &name);
        assert_eq!(now.get("src/d/fresh"), Some(&Node::Dir), "{format:?}");
        assert_eq!(
            now.get("src/d/fresh/empty.txt"),
            Some(&file("")),
            "{format:?}"
        );
        assert_eq!(now.get("src/d/b.txt"), Some(&file("beta")), "{format:?}");
        // With no name, or a name that is taken and Keep both, the next free one is used.
        let mut again = h.request(JobKind::CreateFolder, &[], None, Some("fresh"));
        again.destination = Some(inside(&h, &name, "src/d"));
        again.options.conflict = Some(ConflictPolicy::KeepBoth);
        let last = h.run_journalled(again);
        ok(&last);
        assert_eq!(
            contents(&mut h, &name).get("src/d/fresh (2)"),
            Some(&Node::Dir)
        );
        // Undo brings back the archive as it was before the last change.
        let kept = jwork(&h);
        ok(&h.undo(last.entry.expect("journalled")));
        assert_ne!(jwork(&h).get(&name), kept.get(&name), "{format:?}");
        assert!(!contents(&mut h, &name).contains_key("src/d/fresh (2)"));
        let _ = (before, made);
    }
}

#[test]
fn deleting_everything_leaves_an_empty_archive_that_still_opens() {
    for format in ArchiveFormat::ALL {
        let (mut h, _g) = archives();
        let name = format!("pack{}", format.extension());
        jbuild(&h, &small());
        ok(&h.run_journalled(compress(&h, &["src"], "", "pack", format)));
        let run = h.run_journalled(edit_request(&h, JobKind::Delete, &name, &["src"], None));
        ok(&run);
        assert!(contents(&mut h, &name).is_empty(), "{format:?}");
    }
}

/// A Trash that says it works but refuses every file, as the real one does on a file system with
/// no Trash folder the person may make.
struct RefusesFiles(Arc<waypoint_ops::testing::trash::FakeTrash>);

impl Trash for RefusesFiles {
    fn available(&self) -> Result<(), String> {
        Ok(())
    }
    fn trash(&self, items: &[Location]) -> Vec<Result<TrashReceipt, OpsError>> {
        items
            .iter()
            .map(|_| {
                Err(OpsError::TrashUnavailable {
                    reason: "cannot create /home/.Trash-1000: Permission denied".to_owned(),
                })
            })
            .collect()
    }
    fn restore(&self, receipt: &TrashReceipt) -> Result<Location, OpsError> {
        self.0.restore(receipt)
    }
    fn delete(&self, receipt: &TrashReceipt) -> Result<(), OpsError> {
        self.0.delete(receipt)
    }
    fn empty(&self, older_than_days: Option<u32>) -> Result<u64, OpsError> {
        self.0.empty(older_than_days)
    }
    fn receipt_for(&self, trashed: &Location) -> Result<TrashReceipt, OpsError> {
        self.0.receipt_for(trashed)
    }
}

#[test]
fn a_trash_that_refuses_the_archive_stops_the_change_instead_of_replacing_it_for_good() {
    let (mut h, _g) = archives();
    jbuild(&h, &small());
    ok(&h.run_journalled(compress(&h, &["src"], "", "pack", ArchiveFormat::Zip)));
    let before = jwork(&h);
    let original = contents(&mut h, "pack.zip");
    h.harness.env.trash = Arc::new(RefusesFiles(h.harness.trash.clone()));

    for request in [
        add_request(&h, &["extra/new.txt"], "pack.zip", ""),
        edit_request(&h, JobKind::Delete, "pack.zip", &["src/a.txt"], None),
        edit_request(
            &h,
            JobKind::Rename,
            "pack.zip",
            &["src/a.txt"],
            Some("z.txt"),
        ),
    ] {
        let run = h.run_journalled(request);
        assert!(
            matches!(failed_with(&run), OpsError::TrashUnavailable { .. }),
            "{:?}",
            run.state
        );
        let now = jwork(&h);
        assert_eq!(
            now.get("pack.zip"),
            before.get("pack.zip"),
            "the archive was not replaced"
        );
        assert!(partials(&now).is_empty(), "no partial file is left");
    }
    h.harness.env.trash = h.harness.trash.clone();
    assert_eq!(contents(&mut h, "pack.zip"), original);
}

#[test]
fn a_plan_says_whether_undo_can_bring_the_old_archive_back() {
    let (mut h, _g) = archives();
    jbuild(&h, &small());
    ok(&h.run_journalled(compress(&h, &["src"], "", "pack", ArchiveFormat::Zip)));
    let request = add_request(&h, &["extra/new.txt"], "pack.zip", "");
    let undo = |h: &Jh| h.plan(&request).unwrap().archive_edit.unwrap().undo;
    assert_eq!(undo(&h), ArchiveUndo::Trash);
    h.trash.set_available(Err("no Trash here".to_owned()));
    assert_eq!(
        undo(&h),
        ArchiveUndo::Unavailable("no Trash here".to_owned())
    );
    // With no Trash the plan said so up front, and the change goes ahead as that plan told it.
    ok(&h.run_journalled(request));
}

#[test]
fn what_an_extraction_leaves_out_is_listed_with_names_written_for_reading() {
    let (h, _g) = archives();
    jbuild(&h, &tree(&[("out/", "")]));
    put(
        &h,
        "evil.zip",
        &raw_zip(&[
            ("ok.txt", b"ok", 2),
            ("../evil.txt", b"slip", 4),
            ("/abs.txt", b"abs", 3),
            ("a/../deep.txt", b"deep", 4),
            ("a:b.txt", b"drive", 5),
        ]),
    );
    let request = extract(&h, &["evil.zip"], Some("out"), ExtractLayout::Contents);
    let note = h.plan(&request).unwrap().left_out_note().unwrap();
    assert_eq!(note.count, 4);
    let shown: Vec<(&str, LeftOutWhy)> = note
        .shown
        .iter()
        .map(|left| (left.name.as_str(), left.why))
        .collect();
    assert_eq!(
        shown,
        [
            ("../evil.txt", LeftOutWhy::Traversal),
            ("abs.txt", LeftOutWhy::Absolute),
            ("a/../deep.txt", LeftOutWhy::Traversal),
            ("a:b.txt", LeftOutWhy::Absolute),
        ]
    );
    // A plan that leaves nothing out has no note.
    put(&h, "fine.zip", &raw_zip(&[("ok.txt", b"ok", 2)]));
    let fine = extract(&h, &["fine.zip"], Some("out"), ExtractLayout::Contents);
    assert_eq!(h.plan(&fine).unwrap().left_out_note(), None);
}
