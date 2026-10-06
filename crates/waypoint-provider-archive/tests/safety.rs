// Names that try to escape, archives that lie, and archives that are damaged.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use support::*;
use waypoint_protocol::VfsError;
use waypoint_provider_archive::UnsafeName;
use waypoint_vfs::{CancelToken, EntryKind, Provider};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[test]
fn zip_slip_names_are_shown_safely_and_flagged() {
    let dir = scratch();
    let zip = RawZip::new()
        .file("ok.txt", b"fine")
        .file("../../evil.txt", b"slip")
        .file("/etc/cron.d/job", b"abs")
        .file("a/../../up.txt", b"mid")
        .file("C:\\Windows\\drive.txt", b"drive")
        .file("back\\slash\\name.txt", b"win")
        .unix(b"nul\0byte.txt", b"nul", 0o100_644)
        .write(&dir.path().join("evil.zip"));
    let p = provider();
    let root = top(&zip);
    let all = p
        .entries(&file(&zip), &CancelToken::new(), &mut |_| {})
        .unwrap();
    // Nothing is shown outside the archive's own tree: every component is an ordinary name.
    for entry in &all {
        for part in &entry.components {
            assert_ne!(part.as_slice(), b"..");
            assert_ne!(part.as_slice(), b".");
            assert!(!part.contains(&b'/') && !part.contains(&0u8), "{part:?}");
        }
    }
    let flagged = |components: &[&str]| {
        let wanted: Vec<Vec<u8>> = components.iter().map(|c| c.as_bytes().to_vec()).collect();
        all.iter()
            .find(|entry| entry.components == wanted)
            .unwrap_or_else(|| {
                panic!(
                    "no entry {components:?} in {:?}",
                    all.iter().map(|e| &e.components).collect::<Vec<_>>()
                )
            })
            .clone()
    };
    assert_eq!(flagged(&["ok.txt"]).unsafe_name, None);
    let slip = flagged(&["..\u{2215}..\u{2215}evil.txt"]);
    assert_eq!(slip.unsafe_name, Some(UnsafeName::Traversal));
    assert_eq!(slip.raw_name.as_deref(), Some(&b"../../evil.txt"[..]));
    assert_eq!(
        flagged(&["etc", "cron.d", "job"]).unsafe_name,
        Some(UnsafeName::Absolute)
    );
    assert_eq!(
        flagged(&["a\u{2215}..\u{2215}..\u{2215}up.txt"]).unsafe_name,
        Some(UnsafeName::Traversal)
    );
    assert_eq!(
        flagged(&["Windows", "drive.txt"]).unsafe_name,
        Some(UnsafeName::Absolute)
    );
    assert_eq!(flagged(&["back", "slash", "name.txt"]).unsafe_name, None);
    assert_eq!(
        flagged(&["nul\u{fffd}byte.txt"]).unsafe_name,
        Some(UnsafeName::ControlCharacters)
    );
    assert_eq!(
        p.inspect(&file(&zip), &CancelToken::new())
            .unwrap()
            .unsafe_names,
        5
    );
    // The safe names browse and read like any others.
    assert_eq!(
        read(&*p, &at(&root, "..\u{2215}..\u{2215}evil.txt")),
        b"slip"
    );
    assert_eq!(read(&*p, &at(&root, "etc/cron.d/job")), b"abs");
    // And the top level shows no `..` or root.
    let top_names = names(&*p, &root);
    assert!(
        !top_names.contains("..") && !top_names.contains(""),
        "{top_names:?}"
    );
}

#[test]
fn tar_traversal_and_link_attacks_are_flagged() {
    let dir = scratch();
    let mut builder = tar::Builder::new(Vec::new());
    let raw_header = |name: &[u8], kind: tar::EntryType, link: &[u8]| {
        let mut header = tar::Header::new_gnu();
        {
            let bytes = header.as_old_mut();
            bytes.name[..name.len()].copy_from_slice(name);
            bytes.linkname[..link.len()].copy_from_slice(link);
        }
        header.set_entry_type(kind);
        header.set_size(0);
        header.set_mode(0o777);
        header.set_cksum();
        header
    };
    // A link out of the archive, then a file written through it.
    builder
        .append(
            &raw_header(b"escape", tar::EntryType::Symlink, b"/etc"),
            std::io::empty(),
        )
        .unwrap();
    let mut through = raw_header(b"escape/passwd", tar::EntryType::Regular, b"");
    through.set_size(4);
    through.set_cksum();
    builder.append(&through, &b"root"[..]).unwrap();
    let mut slip = raw_header(b"../../outside.txt", tar::EntryType::Regular, b"");
    slip.set_size(1);
    slip.set_cksum();
    builder.append(&slip, &b"x"[..]).unwrap();
    // A link that climbs out, and a hard link to something outside.
    builder
        .append(
            &raw_header(b"up", tar::EntryType::Symlink, b"../../../etc/shadow"),
            std::io::empty(),
        )
        .unwrap();
    builder
        .append(
            &raw_header(b"hard", tar::EntryType::Link, b"/etc/shadow"),
            std::io::empty(),
        )
        .unwrap();
    let path = dir.path().join("evil.tar");
    std::fs::write(&path, builder.into_inner().unwrap()).unwrap();
    let p = provider();
    let all = p
        .entries(&file(&path), &CancelToken::new(), &mut |_| {})
        .unwrap();
    let find = |components: &[&str]| {
        let wanted: Vec<Vec<u8>> = components.iter().map(|c| c.as_bytes().to_vec()).collect();
        all.iter().find(|entry| entry.components == wanted).cloned()
    };
    // The link was replaced by a folder so what is below it can be reached, and flagged.
    assert_eq!(find(&["escape"]).unwrap().kind, EntryKind::Directory);
    assert_eq!(
        find(&["escape", "passwd"]).unwrap().unsafe_name,
        Some(UnsafeName::ThroughLink)
    );
    assert_eq!(
        find(&["..\u{2215}..\u{2215}outside.txt"])
            .unwrap()
            .unsafe_name,
        Some(UnsafeName::Traversal)
    );
    // A link that points outside is shown as broken, never followed.
    let root = top(&path);
    let up = p.stat(&at(&root, "up")).unwrap();
    assert_eq!((up.kind, up.link_target), (EntryKind::Symlink, None));
    assert!(matches!(
        p.open_read(&at(&root, "up")).map(|_| ()).unwrap_err(),
        VfsError::NotFound { .. }
    ));
    // A hard link to something that is not in the archive reads as nothing.
    assert!(p.open_read(&at(&root, "hard")).is_err() || read(&*p, &at(&root, "hard")).is_empty());
}

#[test]
fn a_link_to_a_file_outside_is_not_followed_to_the_disk() {
    // A zip link whose target climbs out of the archive: reading it must not reach the disk.
    let dir = scratch();
    std::fs::write(dir.path().join("secret.txt"), b"top secret").unwrap();
    let zip = RawZip::new()
        .unix(b"sub/leak", b"../../secret.txt", 0o120_777)
        .unix(
            b"abs",
            dir.path().join("secret.txt").to_str().unwrap().as_bytes(),
            0o120_777,
        )
        .write(&dir.path().join("leak.zip"));
    let p = provider();
    let root = top(&zip);
    for name in ["sub/leak", "abs"] {
        let error = read_err(&*p, &at(&root, name));
        assert!(
            matches!(error, VfsError::NotFound { .. }),
            "{name}: {error:?}"
        );
    }
    let stat = p.stat(&at(&root, "abs")).unwrap();
    let resolved = p.resolve_link(&root, &stat).unwrap();
    assert_eq!(resolved.link_target, None);
}

#[test]
fn a_zip_that_claims_a_huge_entry_lists_at_once_and_reads_lazily() {
    // 96 MiB of zeros deflate to about 100 KiB: listing and a 1 KiB range must not decode it all.
    let dir = scratch();
    let path = dir.path().join("bomb.zip");
    {
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("zeros.bin", options).unwrap();
        let chunk = vec![0u8; 1024 * 1024];
        for _ in 0..96 {
            zip.write_all(&chunk).unwrap();
        }
        zip.finish().unwrap();
    }
    let p = provider();
    let root = top(&path);
    let started = Instant::now();
    let entry = p.entry_info(&at(&root, "zeros.bin")).unwrap();
    assert_eq!(entry.size, Some(96 * 1024 * 1024));
    assert!(entry.compressed_size.unwrap() < 1024 * 1024, "{entry:?}");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "listing took {:?}",
        started.elapsed()
    );
    // Reading the first KiB and dropping the stream does not decode the rest.
    let started = Instant::now();
    let mut stream = p.open_read(&at(&root, "zeros.bin")).unwrap();
    let mut head = [0u8; 1024];
    stream.read_exact(&mut head).unwrap();
    drop(stream);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn damaged_zips_are_corrupt() {
    let dir = scratch();
    let good = RawZip::new()
        .file("a.txt", b"alpha alpha alpha")
        .file("dir/b.txt", b"beta")
        .build();
    let p = provider();
    let corrupt = |name: &str, bytes: Vec<u8>| {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        let error = p
            .list(&top(&path), &CancelToken::new(), 0, &mut |_| {})
            .unwrap_err();
        assert!(
            matches!(error, VfsError::Corrupt { .. }),
            "{name}: {error:?}"
        );
    };
    corrupt("cut.zip", good[..good.len() - 30].to_vec());
    corrupt("head-only.zip", good[..40].to_vec());
    let mut bad_sig = good.clone();
    let central = good
        .windows(4)
        .position(|w| w == [0x50, 0x4b, 0x01, 0x02])
        .unwrap();
    bad_sig[central + 1] = 0;
    corrupt("bad-central.zip", bad_sig);
    corrupt("one-byte.zip", vec![b'P']);
    // A directory that claims to be bigger than the file.
    let mut huge = good.clone();
    let eocd = huge.len() - 22;
    huge[eocd + 12..eocd + 16].copy_from_slice(&0x00ff_ffffu32.to_le_bytes());
    corrupt("huge-dir.zip", huge);
}

#[test]
fn a_damaged_entry_reads_as_an_error_not_a_short_file() {
    let dir = scratch();
    let path = dir.path().join("flip.zip");
    {
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("data.bin", options).unwrap();
        zip.write_all(&big_bytes()).unwrap();
        zip.finish().unwrap();
    }
    let mut bytes = std::fs::read(&path).unwrap();
    // Damage the middle of the compressed data; the central directory is intact.
    let at_byte = 200;
    bytes[at_byte] ^= 0xff;
    bytes[at_byte + 1] ^= 0xff;
    std::fs::write(&path, bytes).unwrap();
    let p = provider();
    let listing = p
        .list(&top(&path), &CancelToken::new(), 0, &mut |_| {})
        .unwrap();
    assert_eq!(listing.len(), 1);
    let error = read_err(&*p, &at(&top(&path), "data.bin"));
    assert!(matches!(error, VfsError::Corrupt { .. }), "{error:?}");
}

#[test]
fn things_that_are_not_archives_say_so() {
    let dir = scratch();
    let p = provider();
    let text = dir.path().join("notes.txt");
    std::fs::write(&text, b"just text").unwrap();
    let error = p
        .list(&top(&text), &CancelToken::new(), 0, &mut |_| {})
        .unwrap_err();
    assert!(matches!(error, VfsError::Unsupported { .. }), "{error:?}");
    // Right name, nothing in it.
    let empty = dir.path().join("empty.zip");
    std::fs::write(&empty, b"").unwrap();
    let error = p
        .list(&top(&empty), &CancelToken::new(), 0, &mut |_| {})
        .unwrap_err();
    assert!(matches!(error, VfsError::Corrupt { .. }), "{error:?}");
    // Right name, text inside.
    let fake = dir.path().join("fake.zip");
    std::fs::write(
        &fake,
        b"this is not a zip file at all, only pretending to be one",
    )
    .unwrap();
    let error = p
        .list(&top(&fake), &CancelToken::new(), 0, &mut |_| {})
        .unwrap_err();
    assert!(matches!(error, VfsError::Corrupt { .. }), "{error:?}");
    // Missing archive and a folder in its place.
    let missing = dir.path().join("missing.zip");
    assert_eq!(
        kind(&p.list(&top(&missing), &CancelToken::new(), 0, &mut |_| {})),
        "notFound"
    );
    let folder = dir.path().join("folder.zip");
    std::fs::create_dir(&folder).unwrap();
    assert_eq!(
        kind(&p.list(&top(&folder), &CancelToken::new(), 0, &mut |_| {})),
        "isADirectory"
    );
}

#[test]
fn the_provider_is_read_only_and_says_so() {
    let dir = scratch();
    let zip = RawZip::new()
        .file("a.txt", b"a")
        .write(&dir.path().join("ro.zip"));
    let p = provider();
    assert!(p.read_only());
    assert!(!p.capabilities().write);
    let root = top(&zip);
    assert_eq!(kind(&p.create_dir(&at(&root, "new"))), "unsupported");
    assert_eq!(kind(&p.remove_file(&at(&root, "a.txt"))), "unsupported");
    assert_eq!(
        kind(&p.rename(&at(&root, "a.txt"), &at(&root, "b.txt"), false)),
        "unsupported"
    );
    assert_eq!(
        kind(
            &p.create_write(&at(&root, "w"), Default::default())
                .map(|_| ())
        ),
        "unsupported"
    );
}

/// Names that Windows would read as something else (a drive, a parent) are listed as stored, as
/// one name each, marked as left out of an extraction, and never as a folder that opens.
#[test]
fn unsafe_names_are_listed_as_stored_and_marked() {
    let dir = scratch();
    let zip = RawZip::new()
        .file("../evil.txt", b"slip")
        .file("ok.txt", b"fine")
        .file("CON.txt", b"con")
        .file("a:b.txt", b"drive")
        .unix(b"../", b"", 0o040_755)
        .unix(b"..", b"", 0o040_755)
        .write(&dir.path().join("windows.zip"));
    let p = provider();
    let root = top(&zip);
    let listed = p
        .list(&root, &CancelToken::new(), usize::MAX, &mut |_| {})
        .unwrap();
    let find = |name: &str| {
        listed
            .iter()
            .find(|entry| entry.name == std::ffi::OsStr::new(name))
            .unwrap_or_else(|| panic!("no {name:?} in {:?}", names(&*p, &root)))
    };
    let marked = |name: &str| {
        find(name)
            .attributes
            .as_ref()
            .and_then(|a| a.get("archive.unsafe").map(str::to_owned))
    };
    // `a:b.txt` is not read as drive `a:` and `b.txt`.
    assert_eq!(marked("a:b.txt").as_deref(), Some("absolute"));
    assert_eq!(read(&*p, &at(&root, "a:b.txt")), b"drive");
    assert_eq!(marked("..\u{2215}evil.txt").as_deref(), Some("traversal"));
    // `..` is one name that does not open as a folder.
    assert_eq!(marked("%2E%2E").as_deref(), Some("traversal"));
    assert_ne!(find("%2E%2E").kind, EntryKind::Directory);
    assert_ne!(find("..\u{2215}evil.txt").kind, EntryKind::Directory);
    assert!(p
        .list(
            &at(&root, "%2E%2E"),
            &CancelToken::new(),
            usize::MAX,
            &mut |_| {}
        )
        .is_err());
    // An ordinary name carries nothing.
    assert_eq!(marked("ok.txt"), None);
    assert_eq!(marked("CON.txt"), None);
}
