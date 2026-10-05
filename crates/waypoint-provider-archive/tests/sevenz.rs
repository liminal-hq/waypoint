// Browsing 7z archives, encrypted archives of every format, and damaged archives.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::io::Read;

use support::*;
use waypoint_protocol::{AuthPrompt, VfsError};
use waypoint_vfs::{CancelToken, EntryKind, Provider, Secret};

fn check_sample(p: &dyn Provider, root: &waypoint_path::VfsPath) {
    let top = names(p, root);
    for expected in ["docs", "empty dir", "empty.bin", "hello.txt"] {
        assert!(top.contains(expected), "{expected} in {top:?}");
    }
    assert_eq!(
        names(p, &at(root, "docs")),
        ["big.bin", "deep", "readme.md"]
            .map(String::from)
            .into_iter()
            .collect()
    );
    assert_eq!(read(p, &at(root, "hello.txt")), b"hello, archive\n");
    assert_eq!(read(p, &at(root, "empty.bin")), b"");
    assert_eq!(read(p, &at(root, "docs/big.bin")), big_bytes());
    assert_eq!(
        read(p, &at(root, "docs/deep/caf\u{e9} \u{65e5}\u{672c}.txt")),
        b"unicode name"
    );
    let hello = p.stat(&at(root, "hello.txt")).unwrap();
    assert_eq!((hello.kind, hello.size), (EntryKind::File, Some(15)));
    assert!(hello.modified_ms.is_some());
    assert_eq!(
        p.stat(&at(root, "empty dir")).unwrap().kind,
        EntryKind::Directory
    );
    let mut tail = Vec::new();
    p.open_read_at(&at(root, "docs/big.bin"), 150_000)
        .unwrap()
        .read_to_end(&mut tail)
        .unwrap();
    assert_eq!(tail, big_bytes()[150_000..]);
}

#[test]
fn a_7z_written_by_the_library_browses_and_reads() {
    let dir = scratch();
    let src = dir.path().join("src");
    sample_tree(&src);
    let path = dir.path().join("lib.7z");
    sevenz_rust2::compress_to_path(&src, &path).unwrap();
    let p = provider();
    check_sample(&*p, &top(&path));
}

#[test]
fn a_7z_made_by_the_7z_tool_browses_and_reads() {
    if !have("7z") {
        eprintln!("skipped: the `7z` tool is not installed");
        return;
    }
    let dir = scratch();
    let src = dir.path().join("src");
    sample_tree(&src);
    let p = provider();
    // Solid and not, with LZMA2 and with BZip2.
    for (name, args) in [
        ("solid.7z", vec!["a", "-ms=on"]),
        ("plain.7z", vec!["a", "-ms=off"]),
        ("bzip.7z", vec!["a", "-m0=bzip2"]),
        ("store.7z", vec!["a", "-mx=0"]),
    ] {
        let path = dir.path().join(name);
        let mut full = args.clone();
        full.push(path.to_str().unwrap());
        full.push(".");
        run(&src, "7z", &full);
        check_sample(&*p, &top(&path));
    }
}

fn expect_password_prompt(error: &VfsError) {
    match error {
        VfsError::AuthRequired { prompt, .. } => {
            assert!(
                matches!(**prompt, AuthPrompt::Passphrase { .. }),
                "{prompt:?}"
            )
        }
        other => panic!("expected AuthRequired, got {other:?}"),
    }
}

#[test]
fn an_encrypted_7z_asks_for_its_password_and_checks_it() {
    if !have("7z") {
        eprintln!("skipped: the `7z` tool is not installed");
        return;
    }
    let dir = scratch();
    let src = dir.path().join("src");
    sample_tree(&src);
    let path = dir.path().join("secret.7z");
    run(&src, "7z", &["a", "-psecret", path.to_str().unwrap(), "."]);
    let p = provider();
    let root = top(&path);
    // Names are readable without the password; contents are not.
    assert!(names(&*p, &root).contains("hello.txt"));
    let info = p.inspect(&file(&path), &CancelToken::new()).unwrap();
    assert!(info.encrypted_entries > 0);
    let entry = at(&root, "hello.txt");
    expect_password_prompt(&read_err(&*p, &entry));
    p.unlock(&file(&path), Secret::from("wrong"));
    let wrong = read_err(&*p, &entry);
    assert!(matches!(wrong, VfsError::AuthFailed { .. }), "{wrong:?}");
    p.unlock(&file(&path), Secret::from("secret"));
    assert_eq!(read(&*p, &entry), b"hello, archive\n");
    p.forget_password(&file(&path));
    expect_password_prompt(&read_err(&*p, &entry));
}

#[test]
fn a_7z_with_an_encrypted_header_cannot_be_listed_without_the_password() {
    if !have("7z") {
        eprintln!("skipped: the `7z` tool is not installed");
        return;
    }
    let dir = scratch();
    let src = dir.path().join("src");
    sample_tree(&src);
    let path = dir.path().join("hidden.7z");
    run(
        &src,
        "7z",
        &["a", "-psecret", "-mhe=on", path.to_str().unwrap(), "."],
    );
    let p = provider();
    let root = top(&path);
    let error = p
        .list(&root, &CancelToken::new(), 0, &mut |_| {})
        .unwrap_err();
    expect_password_prompt(&error);
    p.unlock(&file(&path), Secret::from("wrong"));
    let error = p
        .list(&root, &CancelToken::new(), 0, &mut |_| {})
        .unwrap_err();
    assert!(
        matches!(
            error,
            VfsError::AuthFailed { .. } | VfsError::Corrupt { .. }
        ),
        "{error:?}"
    );
    p.unlock(&file(&path), Secret::from("secret"));
    assert!(names(&*p, &root).contains("hello.txt"));
    assert_eq!(read(&*p, &at(&root, "hello.txt")), b"hello, archive\n");
}

#[test]
fn an_encrypted_zip_asks_for_its_password_and_checks_it() {
    if !have("zip") {
        eprintln!("skipped: the `zip` tool is not installed");
        return;
    }
    let dir = scratch();
    let src = dir.path().join("src");
    sample_tree(&src);
    let path = dir.path().join("secret.zip");
    // Info-ZIP's own (ZipCrypto) encryption.
    run(
        &src,
        "zip",
        &[
            "-qr",
            "-P",
            "secret",
            path.to_str().unwrap(),
            "hello.txt",
            "docs",
        ],
    );
    let p = provider();
    let root = top(&path);
    assert!(names(&*p, &root).contains("hello.txt"));
    let info = p.entry_info(&at(&root, "hello.txt")).unwrap();
    assert!(info.encrypted);
    let entry = at(&root, "hello.txt");
    expect_password_prompt(&read_err(&*p, &entry));
    p.unlock(&file(&path), Secret::from("wrong"));
    assert!(matches!(read_err(&*p, &entry), VfsError::AuthFailed { .. }));
    p.unlock(&file(&path), Secret::from("secret"));
    assert_eq!(read(&*p, &entry), b"hello, archive\n");
    assert_eq!(read(&*p, &at(&root, "docs/big.bin")), big_bytes());
}

#[test]
fn an_aes_encrypted_zip_asks_for_its_password_and_checks_it() {
    if !have("7z") {
        eprintln!("skipped: the `7z` tool is not installed");
        return;
    }
    let dir = scratch();
    let src = dir.path().join("src");
    sample_tree(&src);
    let path = dir.path().join("aes.zip");
    run(
        &src,
        "7z",
        &[
            "a",
            "-tzip",
            "-mem=AES256",
            "-psecret",
            path.to_str().unwrap(),
            ".",
        ],
    );
    let p = provider();
    let root = top(&path);
    let entry = at(&root, "hello.txt");
    assert!(p.entry_info(&entry).unwrap().encrypted);
    expect_password_prompt(&read_err(&*p, &entry));
    p.unlock(&file(&path), Secret::from("wrong"));
    assert!(matches!(read_err(&*p, &entry), VfsError::AuthFailed { .. }));
    p.unlock(&file(&path), Secret::from("secret"));
    assert_eq!(read(&*p, &entry), b"hello, archive\n");
}

#[test]
fn a_damaged_7z_is_corrupt() {
    let dir = scratch();
    let src = dir.path().join("src");
    sample_tree(&src);
    let path = dir.path().join("good.7z");
    sevenz_rust2::compress_to_path(&src, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let p = provider();
    // Cut short: the header is at the end.
    let cut = dir.path().join("cut.7z");
    std::fs::write(&cut, &bytes[..bytes.len() / 2]).unwrap();
    let error = p
        .list(&top(&cut), &CancelToken::new(), 0, &mut |_| {})
        .unwrap_err();
    assert!(matches!(error, VfsError::Corrupt { .. }), "{error:?}");
    // The header's checksum is wrong.
    let mut flipped = bytes.clone();
    let last = flipped.len() - 5;
    flipped[last] ^= 0xff;
    let bad = dir.path().join("bad.7z");
    std::fs::write(&bad, &flipped).unwrap();
    let error = p
        .list(&top(&bad), &CancelToken::new(), 0, &mut |_| {})
        .unwrap_err();
    assert!(matches!(error, VfsError::Corrupt { .. }), "{error:?}");
    // Only the signature is right.
    let stub = dir.path().join("stub.7z");
    let mut text = b"7z\xbc\xaf\x27\x1c\0\x04".to_vec();
    text.extend_from_slice(&[0u8; 40]);
    std::fs::write(&stub, text).unwrap();
    let error = p
        .list(&top(&stub), &CancelToken::new(), 0, &mut |_| {})
        .unwrap_err();
    assert!(matches!(error, VfsError::Corrupt { .. }), "{error:?}");
}
