// Browsing tar archives, plain and compressed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use support::*;
use waypoint_protocol::VfsError;
use waypoint_provider_archive::{ArchiveFormat, ArchiveNotice, ArchiveOptions, TarCompression};
use waypoint_vfs::{CancelToken, EntryKind, Provider};

/// A tar built by the `tar` crate in memory.
fn library_tar() -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    let mut add = |name: &str, data: &[u8], mode: u32| {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(mode);
        header.set_mtime(1_700_000_000);
        header.set_cksum();
        builder.append_data(&mut header, name, data).unwrap();
    };
    add("hello.txt", b"hello, archive\n", 0o640);
    add("docs/readme.md", b"# Readme\n", 0o644);
    add(
        "docs/deep/caf\u{e9} \u{65e5}\u{672c}.txt",
        "unicode name".as_bytes(),
        0o644,
    );
    add("empty.bin", b"", 0o644);
    add("docs/big.bin", &big_bytes(), 0o644);
    let long = format!("{}/long.txt", "d".repeat(150));
    add(&long, b"long name", 0o644);
    let mut dir = tar::Header::new_gnu();
    dir.set_entry_type(tar::EntryType::Directory);
    dir.set_size(0);
    dir.set_mode(0o755);
    dir.set_cksum();
    builder
        .append_data(&mut dir, "empty dir/", std::io::empty())
        .unwrap();
    let mut link = tar::Header::new_gnu();
    link.set_entry_type(tar::EntryType::Symlink);
    link.set_size(0);
    link.set_mode(0o777);
    link.set_cksum();
    builder
        .append_link(&mut link, "link-to-hello", "hello.txt")
        .unwrap();
    let mut hard = tar::Header::new_gnu();
    hard.set_entry_type(tar::EntryType::Link);
    hard.set_size(0);
    hard.set_mode(0o640);
    hard.set_cksum();
    builder
        .append_link(&mut hard, "hard-hello", "hello.txt")
        .unwrap();
    builder.into_inner().unwrap()
}

fn compress(bytes: &[u8], how: TarCompression) -> Vec<u8> {
    match how {
        TarCompression::None => bytes.to_vec(),
        TarCompression::Gzip => {
            let mut out = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
            out.write_all(bytes).unwrap();
            out.finish().unwrap()
        }
        TarCompression::Bzip2 => {
            let mut out = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
            out.write_all(bytes).unwrap();
            out.finish().unwrap()
        }
        TarCompression::Xz => {
            let mut out =
                lzma_rust2::XzWriter::new(Vec::new(), lzma_rust2::XzOptions::with_preset(1))
                    .unwrap();
            out.write_all(bytes).unwrap();
            out.finish().unwrap()
        }
        TarCompression::Zstd => ruzstd::encoding::compress_to_vec(
            std::io::Cursor::new(bytes),
            ruzstd::encoding::CompressionLevel::Fastest,
        ),
    }
}

fn check_sample(p: &dyn Provider, root: &waypoint_path::VfsPath) {
    let top = names(p, root);
    for expected in [
        "docs",
        "empty dir",
        "empty.bin",
        "hello.txt",
        "link-to-hello",
        "hard-hello",
    ] {
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
    assert_eq!(
        read(p, &at(root, &format!("{}/long.txt", "d".repeat(150)))),
        b"long name"
    );
    let hello = p.stat(&at(root, "hello.txt")).unwrap();
    assert_eq!(hello.size, Some(15));
    assert_eq!(hello.modified_ms, Some(1_700_000_000_000));
    assert_eq!(
        p.permissions(&at(root, "hello.txt"))
            .unwrap()
            .mode
            .map(|m| m & 0o777),
        Some(0o640)
    );
    // A symlink says what it holds and what it points at, without reading data.
    let link = p.stat(&at(root, "link-to-hello")).unwrap();
    assert_eq!(link.kind, EntryKind::Symlink);
    assert_eq!(link.link_target, Some(EntryKind::File));
    assert!(!link.link_pending);
    assert_eq!(
        p.read_link(&at(root, "link-to-hello")).unwrap(),
        "hello.txt"
    );
    assert_eq!(read(p, &at(root, "link-to-hello")), b"hello, archive\n");
    // A hard link is a file with its target's data.
    let hard = p.stat(&at(root, "hard-hello")).unwrap();
    assert_eq!((hard.kind, hard.size), (EntryKind::File, Some(15)));
    assert_eq!(read(p, &at(root, "hard-hello")), b"hello, archive\n");
    // A range.
    let mut tail = Vec::new();
    p.open_read_at(&at(root, "docs/big.bin"), 123_456)
        .unwrap()
        .read_to_end(&mut tail)
        .unwrap();
    assert_eq!(tail, big_bytes()[123_456..]);
}

#[test]
fn every_compression_browses_and_reads() {
    let dir = scratch();
    let tar = library_tar();
    let p = provider();
    for (extension, how) in [
        ("tar", TarCompression::None),
        ("tar.gz", TarCompression::Gzip),
        ("tar.bz2", TarCompression::Bzip2),
        ("tar.xz", TarCompression::Xz),
        ("tar.zst", TarCompression::Zstd),
    ] {
        let path = dir.path().join(format!("sample.{extension}"));
        std::fs::write(&path, compress(&tar, how)).unwrap();
        let info = p.inspect(&file(&path), &CancelToken::new()).unwrap();
        assert_eq!(info.format, ArchiveFormat::Tar(how), "{extension}");
        check_sample(&*p, &top(&path));
    }
}

#[test]
fn a_tar_without_a_telling_name_is_found_by_its_content() {
    let dir = scratch();
    let tar = library_tar();
    let p = provider();
    for (name, how) in [
        ("plain.dat", TarCompression::None),
        ("zipped.dat", TarCompression::Gzip),
        ("xzed.dat", TarCompression::Xz),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, compress(&tar, how)).unwrap();
        assert_eq!(
            p.inspect(&file(&path), &CancelToken::new()).unwrap().format,
            ArchiveFormat::Tar(how)
        );
    }
}

#[test]
fn a_compressed_file_that_is_not_a_tar_is_not_an_archive() {
    let dir = scratch();
    let path = dir.path().join("notes.txt.gz");
    std::fs::write(
        &path,
        compress(b"just some text, not a tar archive\n", TarCompression::Gzip),
    )
    .unwrap();
    let p = provider();
    let error = p.stat(&top(&path)).unwrap_err();
    assert!(matches!(error, VfsError::Unsupported { .. }), "{error:?}");
}

#[test]
fn tars_made_by_the_tar_tool_in_every_format_browse() {
    if !have("tar") {
        eprintln!("skipped: the `tar` tool is not installed");
        return;
    }
    let dir = scratch();
    let src = dir.path().join("src");
    sample_tree(&src);
    let p = provider();
    for format in ["gnu", "pax", "ustar", "v7"] {
        let path = dir.path().join(format!("{format}.tar"));
        run(
            &src,
            "tar",
            &["--format", format, "-cf", path.to_str().unwrap(), "."],
        );
        let root = top(&path);
        // The tool prefixes everything with `./`.
        assert_eq!(
            read(&*p, &at(&root, "hello.txt")),
            b"hello, archive\n",
            "{format}"
        );
        assert_eq!(
            read(&*p, &at(&root, "docs/big.bin")),
            big_bytes(),
            "{format}"
        );
        assert_eq!(
            p.stat(&at(&root, "link-to-hello")).unwrap().kind,
            EntryKind::Symlink
        );
    }
    for (flag, extension, tool) in [
        ("-z", "tgz", "gzip"),
        ("-j", "tbz2", "bzip2"),
        ("-J", "txz", "xz"),
        ("--zstd", "tzst", "zstd"),
    ] {
        if !have(tool) {
            continue;
        }
        let path = dir.path().join(format!("tool.{extension}"));
        run(&src, "tar", &[flag, "-cf", path.to_str().unwrap(), "."]);
        let root = top(&path);
        assert_eq!(
            read(&*p, &at(&root, "docs/readme.md")),
            b"# Readme\n",
            "{flag}"
        );
    }
}

#[test]
fn a_long_name_made_by_pax_is_read() {
    if !have("tar") {
        eprintln!("skipped: the `tar` tool is not installed");
        return;
    }
    let dir = scratch();
    let src = dir.path().join("src");
    let long = "n".repeat(200);
    std::fs::create_dir_all(src.join(&long)).unwrap();
    std::fs::write(src.join(&long).join("file.txt"), b"deep").unwrap();
    let path = dir.path().join("pax.tar");
    run(
        &src,
        "tar",
        &["--format=pax", "-cf", path.to_str().unwrap(), "."],
    );
    let p = provider();
    assert_eq!(
        read(&*p, &at(&top(&path), &format!("{long}/file.txt"))),
        b"deep"
    );
}

#[cfg(unix)]
#[test]
fn a_sparse_file_lists_but_will_not_read() {
    if !have("tar") {
        eprintln!("skipped: the `tar` tool is not installed");
        return;
    }
    let dir = scratch();
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    {
        let file = std::fs::File::create(src.join("holey")).unwrap();
        file.set_len(4 * 1024 * 1024).unwrap();
        use std::os::unix::fs::FileExt;
        file.write_all_at(b"data", 3 * 1024 * 1024).unwrap();
    }
    let path = dir.path().join("sparse.tar");
    run(
        &src,
        "tar",
        &[
            "--sparse",
            "--format=gnu",
            "-cf",
            path.to_str().unwrap(),
            "holey",
        ],
    );
    let p = provider();
    let entry = at(&top(&path), "holey");
    assert_eq!(p.stat(&entry).unwrap().kind, EntryKind::File);
    assert!(matches!(
        p.open_read(&entry).map(|_| ()).unwrap_err(),
        VfsError::Unsupported { .. }
    ));
}

#[test]
fn a_cut_short_tar_is_corrupt() {
    let dir = scratch();
    let tar = library_tar();
    let p = provider();
    for how in [
        TarCompression::None,
        TarCompression::Gzip,
        TarCompression::Xz,
    ] {
        let bytes = compress(&tar, how);
        let path = dir.path().join(format!("cut-{how:?}.tar"));
        // Cut inside the data of the big entry (the middle of the file, for every compression).
        let keep = bytes.len() / 2;
        std::fs::write(&path, &bytes[..keep]).unwrap();
        let error = p
            .list(&top(&path), &CancelToken::new(), 0, &mut |_| {})
            .expect_err("a cut-short archive must not list as if whole");
        assert!(
            matches!(error, VfsError::Corrupt { .. }),
            "{how:?}: {error:?}"
        );
    }
}

#[test]
fn a_compressed_tar_raises_the_slow_notice_before_scanning() {
    let dir = scratch();
    let path = dir.path().join("big.tar.gz");
    std::fs::write(&path, compress(&library_tar(), TarCompression::Gzip)).unwrap();
    let p = provider_with(ArchiveOptions {
        slow_scan_bytes: 0,
        ..ArchiveOptions::default()
    });
    let seen: Arc<Mutex<Vec<ArchiveNotice>>> = Arc::default();
    let sink = seen.clone();
    p.set_notice_sink(Some(Arc::new(move |notice| {
        sink.lock().unwrap().push(notice)
    })));
    let mut progress = Vec::new();
    p.list(&top(&path), &CancelToken::new(), 0, &mut |n| {
        progress.push(n)
    })
    .unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert!(matches!(
        &seen[0],
        ArchiveNotice::SlowListing {
            format: ArchiveFormat::Tar(TarCompression::Gzip),
            ..
        }
    ));
    assert!(!progress.is_empty());
    assert!(
        p.inspect(&file(&path), &CancelToken::new())
            .unwrap()
            .slow_listing
    );
    // A second listing uses the kept index: no second notice.
    p.list(&top(&path), &CancelToken::new(), 0, &mut |_| {})
        .unwrap();
    assert_eq!(seen.len(), 1);
}

#[test]
fn a_scan_can_be_cancelled() {
    let dir = scratch();
    let path = dir.path().join("a.tar.gz");
    std::fs::write(&path, compress(&library_tar(), TarCompression::Gzip)).unwrap();
    let p = provider();
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(
        p.list(&top(&path), &cancel, 0, &mut |_| {}).unwrap_err(),
        VfsError::Cancelled
    );
    // The cancelled scan left nothing behind: a normal one works.
    assert!(!names(&*p, &top(&path)).is_empty());
}

#[test]
fn too_many_entries_is_a_typed_limit_not_a_partial_listing() {
    let dir = scratch();
    let path = dir.path().join("many.tar");
    let mut builder = tar::Builder::new(Vec::new());
    for n in 0..50 {
        let mut header = tar::Header::new_gnu();
        header.set_size(0);
        header.set_cksum();
        builder
            .append_data(&mut header, format!("f{n}"), std::io::empty())
            .unwrap();
    }
    std::fs::write(&path, builder.into_inner().unwrap()).unwrap();
    let p = provider_with(ArchiveOptions {
        max_entries: 10,
        ..ArchiveOptions::default()
    });
    let error = p
        .list(&top(&path), &CancelToken::new(), 0, &mut |_| {})
        .unwrap_err();
    assert!(matches!(error, VfsError::Unsupported { .. }), "{error:?}");
    assert!(Path::new(&path).exists());
}
