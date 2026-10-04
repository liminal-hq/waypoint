// Browsing zip archives: listing, reading, metadata, odd names and what damage looks like.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::io::{Read, Write};

use support::*;
use waypoint_vfs::{CancelToken, EntryKind, Provider};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// A zip written by the `zip` crate with the given compression.
fn library_zip(path: &std::path::Path, method: CompressionMethod) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(method)
        .unix_permissions(0o640);
    zip.start_file("hello.txt", options).unwrap();
    zip.write_all(b"hello, archive\n").unwrap();
    zip.add_directory("empty dir/", options).unwrap();
    zip.start_file("docs/readme.md", options).unwrap();
    zip.write_all(b"# Readme\n").unwrap();
    zip.start_file("docs/deep/caf\u{e9} \u{65e5}\u{672c}.txt", options)
        .unwrap();
    zip.write_all("unicode name".as_bytes()).unwrap();
    zip.start_file("empty.bin", options).unwrap();
    zip.start_file("docs/big.bin", options).unwrap();
    zip.write_all(&big_bytes()).unwrap();
    zip.add_symlink("link-to-hello", "hello.txt", options)
        .unwrap();
    zip.finish().unwrap();
}

fn check_sample(p: &dyn Provider, root: &waypoint_path::VfsPath) {
    assert_eq!(
        names(p, root),
        [
            "docs",
            "empty dir",
            "empty.bin",
            "hello.txt",
            "link-to-hello"
        ]
        .map(String::from)
        .into_iter()
        .collect()
    );
    assert_eq!(
        names(p, &at(root, "docs")),
        ["big.bin", "deep", "readme.md"]
            .map(String::from)
            .into_iter()
            .collect()
    );
    assert_eq!(
        names(p, &at(root, "docs/deep")),
        ["caf\u{e9} \u{65e5}\u{672c}.txt".to_owned()]
            .into_iter()
            .collect()
    );
    assert!(names(p, &at(root, "empty dir")).is_empty());
    assert_eq!(read(p, &at(root, "hello.txt")), b"hello, archive\n");
    assert_eq!(read(p, &at(root, "empty.bin")), b"");
    assert_eq!(read(p, &at(root, "docs/big.bin")), big_bytes());
    assert_eq!(
        read(p, &at(root, "docs/deep/caf\u{e9} \u{65e5}\u{672c}.txt")),
        b"unicode name"
    );
    // Metadata.
    let hello = p.stat(&at(root, "hello.txt")).unwrap();
    assert_eq!(hello.kind, EntryKind::File);
    assert_eq!(hello.size, Some(15));
    assert!(hello.modified_ms.is_some());
    let docs = p.stat(&at(root, "docs")).unwrap();
    assert_eq!(docs.kind, EntryKind::Directory);
    // The zip library writes Unix modes only when it runs on a Unix host.
    let perms = p.permissions(&at(root, "hello.txt")).unwrap();
    if cfg!(unix) {
        assert_eq!(perms.mode.map(|m| m & 0o777), Some(0o640));
    } else {
        assert_eq!(
            perms.mode,
            Some(0o644),
            "no stored mode reads as the default"
        );
    }
    // A symlink reads as the file it points at, and says what it holds.
    let link = at(root, "link-to-hello");
    let stat = p.stat(&link).unwrap();
    assert_eq!(stat.kind, EntryKind::Symlink);
    assert_eq!(p.read_link(&link).unwrap(), "hello.txt");
    let resolved = p.resolve_link(root, &stat).unwrap();
    assert_eq!(resolved.link_target, Some(EntryKind::File));
    assert!(!resolved.link_pending);
    assert_eq!(read(p, &link), b"hello, archive\n");
    // Errors are typed.
    assert_eq!(kind(&p.stat(&at(root, "nope"))), "notFound");
    assert_eq!(
        kind(&p.open_read(&at(root, "docs")).map(|_| ())),
        "isADirectory"
    );
    assert_eq!(
        kind(&p.list(&at(root, "hello.txt"), &CancelToken::new(), 0, &mut |_| {})),
        "notADirectory"
    );
}

#[test]
fn a_deflated_zip_browses_and_reads() {
    let dir = scratch();
    let zip = dir.path().join("sample.zip");
    library_zip(&zip, CompressionMethod::Deflated);
    let p = provider();
    check_sample(&*p, &top(&zip));
}

#[test]
fn a_stored_zip_browses_and_reads() {
    let dir = scratch();
    let zip = dir.path().join("sample.zip");
    library_zip(&zip, CompressionMethod::Stored);
    let p = provider();
    check_sample(&*p, &top(&zip));
}

#[test]
fn a_bzip2_zip_browses_and_reads() {
    let dir = scratch();
    let zip = dir.path().join("sample.zip");
    library_zip(&zip, CompressionMethod::Bzip2);
    let p = provider();
    check_sample(&*p, &top(&zip));
}

#[test]
fn a_zip_made_by_info_zip_browses_and_reads() {
    if !have("zip") {
        eprintln!("skipped: the `zip` tool is not installed");
        return;
    }
    let dir = scratch();
    let src = dir.path().join("src");
    sample_tree(&src);
    run(&src, "zip", &["-qr", "--symlinks", "../sample.zip", "."]);
    let p = provider();
    check_sample(&*p, &top(&dir.path().join("sample.zip")));
}

#[test]
fn a_range_of_an_entry_reads_from_the_start_given() {
    let dir = scratch();
    let zip = dir.path().join("sample.zip");
    library_zip(&zip, CompressionMethod::Deflated);
    let p = provider();
    let big = at(&top(&zip), "docs/big.bin");
    let mut tail = Vec::new();
    p.open_read_at(&big, 150_000)
        .unwrap()
        .read_to_end(&mut tail)
        .unwrap();
    assert_eq!(tail, big_bytes()[150_000..]);
    let mut none = Vec::new();
    p.open_read_at(&big, 200_000)
        .unwrap()
        .read_to_end(&mut none)
        .unwrap();
    assert!(none.is_empty());
}

#[test]
fn bytes_in_front_of_a_zip_are_skipped() {
    let dir = scratch();
    let zip = RawZip::new()
        .prefix(&vec![b'#'; 1234])
        .file("a.txt", b"alpha")
        .file("dir/b.txt", b"beta")
        .comment("made by hand")
        .write(&dir.path().join("sfx.zip"));
    let p = provider();
    let root = top(&zip);
    assert_eq!(
        names(&*p, &root),
        ["a.txt", "dir"].map(String::from).into_iter().collect()
    );
    assert_eq!(read(&*p, &at(&root, "dir/b.txt")), b"beta");
    let info = p.inspect(&file(&zip), &CancelToken::new()).unwrap();
    assert_eq!(info.entries, 2);
    assert_eq!(info.comment.as_deref(), Some("made by hand"));
}

#[test]
fn names_in_code_page_437_are_decoded() {
    let dir = scratch();
    // "café" in CP437 is 63 61 66 82; it is not UTF-8.
    let zip = RawZip::new()
        .dos(b"caf\x82.txt", b"x")
        .write(&dir.path().join("dos.zip"));
    let p = provider();
    assert_eq!(
        names(&*p, &top(&zip)),
        ["caf\u{e9}.txt".to_owned()].into_iter().collect()
    );
}

#[test]
fn a_zip_name_used_twice_shows_the_later_entry() {
    let dir = scratch();
    let zip = RawZip::new()
        .file("same.txt", b"first")
        .file("other.txt", b"o")
        .file("same.txt", b"second")
        .write(&dir.path().join("dup.zip"));
    let p = provider();
    let root = top(&zip);
    assert_eq!(names(&*p, &root).len(), 2);
    assert_eq!(read(&*p, &at(&root, "same.txt")), b"second");
    assert_eq!(read(&*p, &at(&root, "other.txt")), b"o");
}

#[test]
fn an_empty_zip_lists_nothing() {
    let dir = scratch();
    let zip = RawZip::new().write(&dir.path().join("empty.zip"));
    let p = provider();
    assert!(names(&*p, &top(&zip)).is_empty());
}

#[test]
fn the_top_of_an_archive_is_named_after_the_file() {
    let dir = scratch();
    let zip = dir.path().join("photos.zip");
    library_zip(&zip, CompressionMethod::Deflated);
    let p = provider();
    let root = p.stat(&top(&zip)).unwrap();
    assert_eq!(root.name, "photos.zip");
    assert_eq!(root.kind, EntryKind::Directory);
}

#[test]
fn a_change_to_the_archive_is_seen() {
    let dir = scratch();
    let path = dir.path().join("a.zip");
    RawZip::new().file("one.txt", b"1").write(&path);
    let p = provider();
    assert_eq!(names(&*p, &top(&path)).len(), 1);
    // A different size is a different archive.
    RawZip::new()
        .file("one.txt", b"1")
        .file("two.txt", b"22")
        .write(&path);
    assert_eq!(names(&*p, &top(&path)).len(), 2);
}

#[test]
fn symlink_and_special_modes_are_kept() {
    let dir = scratch();
    let zip = RawZip::new()
        .unix(b"run.sh", b"#!/bin/sh\n", 0o100_755)
        .unix(b"ln", b"run.sh", 0o120_777)
        .write(&dir.path().join("modes.zip"));
    let p = provider();
    let root = top(&zip);
    assert_eq!(
        p.permissions(&at(&root, "run.sh"))
            .unwrap()
            .mode
            .map(|m| m & 0o777),
        Some(0o755)
    );
    assert_eq!(p.stat(&at(&root, "ln")).unwrap().kind, EntryKind::Symlink);
    assert_eq!(read(&*p, &at(&root, "ln")), b"#!/bin/sh\n");
}
