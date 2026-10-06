// Making archives: every format reads back through the provider and through the real tools.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::io::{self, Read};
use std::path::Path;

use support::*;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{
    ArchiveBuilder, ArchiveKind, ArchiveWriters, EntryAttrs, EntryKind, InjectedError,
    LocalProvider, Provider, WriteOptions,
};

fn begin(kind: ArchiveKind, path: &Path) -> Box<dyn ArchiveBuilder> {
    let out = LocalProvider::new()
        .create_write(&file(path), WriteOptions::exclusive())
        .unwrap();
    provider()
        .begin(kind, out, Location::new("out", "file:///out"))
        .unwrap()
}

const MTIME_MS: i64 = 1_704_110_400_000;

fn attrs(mode: u32) -> EntryAttrs {
    EntryAttrs {
        mode: Some(mode),
        modified_ms: Some(MTIME_MS),
    }
}

/// A small tree through the builder.
fn write_sample(kind: ArchiveKind, path: &Path) {
    let mut builder = begin(kind, path);
    builder.add_dir(b"top", attrs(0o755)).unwrap();
    builder
        .add_file(
            b"top/hello.txt",
            15,
            attrs(0o640),
            &mut &b"hello, archive\n"[..],
        )
        .unwrap();
    builder.add_dir(b"top/empty", attrs(0o755)).unwrap();
    let big = big_bytes();
    builder
        .add_file(
            b"top/big.bin",
            big.len() as u64,
            attrs(0o644),
            &mut &big[..],
        )
        .unwrap();
    builder
        .add_file(
            "top/caf\u{e9} \u{65e5}\u{672c}.txt".as_bytes(),
            4,
            attrs(0o644),
            &mut &b"utf8"[..],
        )
        .unwrap();
    builder
        .add_file(b"top/zero", 0, attrs(0o600), &mut io::empty())
        .unwrap();
    builder
        .add_symlink(b"top/link", b"hello.txt", attrs(0o777))
        .unwrap();
    builder.finish(true).unwrap();
}

fn check_sample(path: &Path) {
    let p = provider();
    let root = top(path);
    let t = at(&root, "top");
    assert_eq!(
        names(&*p, &t),
        [
            "big.bin",
            "caf\u{e9} \u{65e5}\u{672c}.txt",
            "empty",
            "hello.txt",
            "link",
            "zero"
        ]
        .map(String::from)
        .into_iter()
        .collect()
    );
    assert_eq!(read(&*p, &at(&t, "hello.txt")), b"hello, archive\n");
    assert_eq!(read(&*p, &at(&t, "big.bin")), big_bytes());
    assert_eq!(
        read(&*p, &at(&t, "caf\u{e9} \u{65e5}\u{672c}.txt")),
        b"utf8"
    );
    assert_eq!(read(&*p, &at(&t, "zero")), b"");
    assert_eq!(p.stat(&at(&t, "empty")).unwrap().kind, EntryKind::Directory);
    let link = at(&t, "link");
    assert_eq!(p.stat(&link).unwrap().kind, EntryKind::Symlink);
    assert_eq!(p.read_link(&link).unwrap(), "hello.txt");
    assert_eq!(read(&*p, &link), b"hello, archive\n");
    let hello = p.stat(&at(&t, "hello.txt")).unwrap();
    assert_eq!(hello.size, Some(15));
    // Times keep to the second (a zip to two).
    let modified = hello.modified_ms.expect("a time");
    assert!(
        (modified - MTIME_MS).abs() <= 2_000,
        "{modified} vs {MTIME_MS}"
    );
    // The zip library writes Unix modes only when it runs on a Unix host.
    let zip_off_unix = cfg!(not(unix)) && path.extension().is_some_and(|e| e == "zip");
    if !zip_off_unix {
        assert_eq!(
            p.permissions(&at(&t, "hello.txt"))
                .unwrap()
                .mode
                .map(|m| m & 0o777),
            Some(0o640)
        );
    }
}

#[test]
fn every_format_reads_back_through_the_provider() {
    let dir = scratch();
    for kind in ArchiveKind::ALL {
        let path = dir.path().join(format!("made{}", kind.extension()));
        write_sample(kind, &path);
        check_sample(&path);
    }
}

#[test]
fn the_real_tools_read_what_was_written() {
    let dir = scratch();
    let checks: [(ArchiveKind, &str, &[&str]); 6] = [
        (ArchiveKind::Zip, "unzip", &["-tq"]),
        (ArchiveKind::Tar, "tar", &["-tf"]),
        (ArchiveKind::TarGz, "tar", &["-tzf"]),
        (ArchiveKind::TarBz2, "tar", &["-tjf"]),
        (ArchiveKind::TarXz, "tar", &["-tJf"]),
        (ArchiveKind::SevenZ, "7z", &["t"]),
    ];
    for (kind, tool, args) in checks {
        if !have(tool) {
            eprintln!("skipped {kind:?}: the `{tool}` tool is not installed");
            continue;
        }
        let path = dir.path().join(format!("tooled{}", kind.extension()));
        write_sample(kind, &path);
        let mut full: Vec<&str> = args.to_vec();
        full.push(path.to_str().unwrap());
        run(dir.path(), tool, &full);
    }
}

#[test]
fn the_real_tools_extract_the_same_contents() {
    if !(have("unzip") && have("tar") && have("7z")) {
        return;
    }
    let dir = scratch();
    for kind in ArchiveKind::ALL {
        let path = dir.path().join(format!("x{}", kind.extension()));
        write_sample(kind, &path);
        let out = dir.path().join(format!("out-{:?}", kind));
        std::fs::create_dir(&out).unwrap();
        let archive = path.to_str().unwrap();
        match kind {
            ArchiveKind::Zip => run(&out, "unzip", &["-q", archive]),
            ArchiveKind::SevenZ => run(&out, "7z", &["x", "-y", archive]),
            _ => run(&out, "tar", &["-xf", archive]),
        }
        assert_eq!(
            std::fs::read(out.join("top/hello.txt")).unwrap(),
            b"hello, archive\n",
            "{kind:?}"
        );
        assert_eq!(
            std::fs::read(out.join("top/big.bin")).unwrap(),
            big_bytes(),
            "{kind:?}"
        );
        assert!(out.join("top/empty").is_dir(), "{kind:?}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(out.join("top/hello.txt"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777 & !0o022, 0o640 & !0o022, "{kind:?}");
            assert_eq!(
                std::fs::read_link(out.join("top/link"))
                    .unwrap()
                    .to_str()
                    .unwrap(),
                "hello.txt",
                "{kind:?}"
            );
        }
    }
}

#[test]
fn a_source_that_fails_fails_the_call_with_its_own_error() {
    struct Breaks;
    impl Read for Breaks {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(InjectedError(VfsError::Cancelled).into_io())
        }
    }
    let dir = scratch();
    for kind in ArchiveKind::ALL {
        let path = dir.path().join(format!("broken{}", kind.extension()));
        let mut builder = begin(kind, &path);
        let error = builder
            .add_file(b"f", 10, EntryAttrs::default(), &mut Breaks)
            .expect_err("the source failed");
        assert_eq!(error, VfsError::Cancelled, "{kind:?}");
    }
}

#[test]
fn a_source_that_changed_size_is_refused() {
    let dir = scratch();
    for kind in ArchiveKind::ALL {
        let path = dir.path().join(format!("size{}", kind.extension()));
        let mut builder = begin(kind, &path);
        // Shorter than promised, then longer.
        let short = builder.add_file(b"a", 10, EntryAttrs::default(), &mut &b"abc"[..]);
        assert!(
            matches!(short, Err(VfsError::Io { .. })),
            "{kind:?} short: {short:?}"
        );
        let mut builder = begin(kind, &dir.path().join(format!("size2{}", kind.extension())));
        let long = builder.add_file(b"b", 3, EntryAttrs::default(), &mut &b"abcdef"[..]);
        assert!(
            matches!(long, Err(VfsError::Io { .. })),
            "{kind:?} long: {long:?}"
        );
    }
}

#[test]
fn a_name_that_is_not_unicode_is_refused_where_the_format_needs_it() {
    let dir = scratch();
    for kind in [ArchiveKind::Zip, ArchiveKind::SevenZ] {
        let path = dir.path().join(format!("bad-name{}", kind.extension()));
        let mut builder = begin(kind, &path);
        let error = builder
            .add_file(b"bad\xffname", 0, EntryAttrs::default(), &mut io::empty())
            .unwrap_err();
        assert!(
            matches!(error, VfsError::InvalidName { .. }),
            "{kind:?}: {error:?}"
        );
    }
    // A tar keeps the bytes it was given.
    #[cfg(unix)]
    {
        let path = dir.path().join("bytes.tar");
        let mut builder = begin(ArchiveKind::Tar, &path);
        builder
            .add_file(b"bad\xffname", 1, EntryAttrs::default(), &mut &b"x"[..])
            .unwrap();
        builder.finish(false).unwrap();
        let entries = provider()
            .entries(&file(&path), &waypoint_vfs::CancelToken::new(), &mut |_| {})
            .unwrap();
        assert_eq!(entries[0].components[0], b"bad\xffname");
    }
}

#[test]
fn a_very_long_tar_name_is_written() {
    let dir = scratch();
    let path = dir.path().join("long.tar.gz");
    let long = format!("{}/{}", "d".repeat(120), "f".repeat(120));
    let mut builder = begin(ArchiveKind::TarGz, &path);
    builder
        .add_file(long.as_bytes(), 2, EntryAttrs::default(), &mut &b"hi"[..])
        .unwrap();
    builder.finish(false).unwrap();
    assert_eq!(read(&*provider(), &at(&top(&path), &long)), b"hi");
}

#[test]
fn an_archive_says_whether_it_can_be_rewritten_and_in_what_format() {
    use waypoint_vfs::{ArchiveCatalog, ArchiveRefusal, CancelToken, Writability};
    let dir = scratch();
    let p = provider();
    for kind in ArchiveKind::ALL {
        let path = dir.path().join(format!("w{}", kind.extension()));
        write_sample(kind, &path);
        let found = p.writability(&top(&path), &CancelToken::new()).unwrap();
        assert_eq!(found, Writability::Writable(kind), "{kind:?}");
        assert!(p.rewritable(&top(&path)), "{kind:?}");
    }
    // A name that had to be made safe is a reason: a rewrite would store the safe name.
    let evil = RawZip::new()
        .file("../escape.txt", b"x")
        .write(&dir.path().join("evil.zip"));
    assert_eq!(
        p.writability(&top(&evil), &CancelToken::new()).unwrap(),
        Writability::Refused(ArchiveRefusal::UnsafeNames)
    );
    assert!(!p.rewritable(&top(&evil)));
}

#[test]
fn a_view_of_an_archive_hears_that_the_file_was_rewritten() {
    use std::sync::mpsc;
    use waypoint_vfs::WatchEvent;
    let dir = scratch();
    let path = dir.path().join("watched.zip");
    RawZip::new().file("a.txt", b"one").write(&path);
    let p = provider();
    let (sender, receiver) = mpsc::channel();
    let sender = std::sync::Mutex::new(sender);
    let _watch = p
        .watch(
            &top(&path),
            std::sync::Arc::new(move |event| {
                let _ = sender.lock().unwrap().send(event);
            }),
        )
        .unwrap();
    RawZip::new()
        .file("a.txt", b"one")
        .file("b.txt", b"two, and longer")
        .write(&path);
    let event = receiver
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("a rewrite is noticed");
    assert!(matches!(event, WatchEvent::Rescan(_)), "{event:?}");
}
