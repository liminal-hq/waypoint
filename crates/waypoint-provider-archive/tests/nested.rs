// Archives inside archives, and archives on a server.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::io::Write;
use std::sync::Arc;

use support::*;
use waypoint_path::{ArchivePath, VfsPath};
use waypoint_protocol::{UnreachableReason, VfsError};
use waypoint_provider_archive::{ArchiveOptions, ArchiveProvider};
use waypoint_vfs::{CancelToken, FakeRemoteProvider, Provider, RemoteFault};

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut out = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    out.write_all(bytes).unwrap();
    out.finish().unwrap()
}

fn tar_of(name: &str, data: &[u8]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, name, data).unwrap();
    builder.into_inner().unwrap()
}

/// The top of the archive that is the file `inner` inside the archive `outer`.
fn nested(outer: &VfsPath, inner: &str) -> VfsPath {
    VfsPath::Archive(ArchivePath::new(at(outer, inner)).unwrap())
}

#[test]
fn archives_nest_through_every_format() {
    let dir = scratch();
    // photos.zip holds bundle.tar.gz, which holds deep.zip, which holds a note.
    let deep = RawZip::new().file("note.txt", b"three levels down").build();
    let bundle = gzip(&tar_of("deep.zip", &deep));
    let outer = RawZip::new()
        .file("bundle.tar.gz", &bundle)
        .file("plain.txt", b"top")
        .write(&dir.path().join("photos.zip"));
    let p = provider();
    let level1 = top(&outer);
    assert_eq!(names(&*p, &level1).len(), 2);
    let level2 = nested(&level1, "bundle.tar.gz");
    assert_eq!(
        names(&*p, &level2),
        ["deep.zip".to_owned()].into_iter().collect()
    );
    let level3 = nested(&level2, "deep.zip");
    assert_eq!(
        names(&*p, &level3),
        ["note.txt".to_owned()].into_iter().collect()
    );
    assert_eq!(read(&*p, &at(&level3, "note.txt")), b"three levels down");
    // The address round-trips and Up from the top of a level goes to the folder holding it.
    let uri = at(&level3, "note.txt").to_uri();
    assert_eq!(VfsPath::from_uri(&uri).unwrap(), at(&level3, "note.txt"));
    assert_eq!(level3.parent().unwrap(), level2);
    assert_eq!(level2.parent().unwrap(), level1);
    assert_eq!(level1.parent().unwrap(), file(dir.path()));
}

#[test]
fn a_nested_archive_too_big_to_copy_is_a_typed_limit() {
    let dir = scratch();
    let inner = RawZip::new().file("a", &vec![7u8; 10_000]).build();
    let outer = RawZip::new()
        .file("inner.zip", &inner)
        .write(&dir.path().join("outer.zip"));
    let p = provider_with(ArchiveOptions {
        max_nested_bytes: 100,
        ..ArchiveOptions::default()
    });
    let error = p
        .list(
            &nested(&top(&outer), "inner.zip"),
            &CancelToken::new(),
            0,
            &mut |_| {},
        )
        .unwrap_err();
    assert!(matches!(error, VfsError::Unsupported { .. }), "{error:?}");
}

#[test]
fn a_nested_archive_uses_the_scratch_folder_and_cleans_up() {
    let dir = scratch();
    let scratch_dir = dir.path().join("scratch");
    std::fs::create_dir(&scratch_dir).unwrap();
    let inner = RawZip::new().file("a", b"x").build();
    let outer = RawZip::new()
        .file("inner.zip", &inner)
        .write(&dir.path().join("outer.zip"));
    let p = provider_with(ArchiveOptions {
        scratch_dir: Some(scratch_dir.clone()),
        cached_archives: 1,
        ..ArchiveOptions::default()
    });
    assert_eq!(names(&*p, &nested(&top(&outer), "inner.zip")).len(), 1);
    assert_eq!(std::fs::read_dir(&scratch_dir).unwrap().count(), 1);
    // Opening another archive pushes the first out of the cache, and its scratch copy goes.
    let other = RawZip::new()
        .file("b", b"y")
        .write(&dir.path().join("other.zip"));
    assert_eq!(names(&*p, &top(&other)).len(), 1);
    assert_eq!(std::fs::read_dir(&scratch_dir).unwrap().count(), 0);
}

fn remote_provider(server: &FakeRemoteProvider) -> Arc<ArchiveProvider> {
    let server = server.clone();
    ArchiveProvider::new(
        Arc::new(
            move |path: &VfsPath| -> Result<Arc<dyn Provider>, VfsError> {
                match path {
                    VfsPath::Remote(_) => Ok(Arc::new(server.clone())),
                    other => Err(VfsError::Unsupported {
                        what: other.scheme().to_owned(),
                    }),
                }
            },
        ),
        ArchiveOptions::default(),
    )
}

#[test]
fn an_archive_on_a_server_browses_and_reads() {
    let server = FakeRemoteProvider::sftp();
    let location = at(&server.root("me@fake.test"), "backups/photos.zip");
    let bytes = RawZip::new()
        .file("2026/a.txt", b"alpha")
        .file("2026/b.txt", b"beta")
        .build();
    server.put_file(&location, &bytes);
    let p = remote_provider(&server);
    let root = VfsPath::Archive(ArchivePath::new(location.clone()).unwrap());
    assert_eq!(names(&*p, &root), ["2026".to_owned()].into_iter().collect());
    assert_eq!(read(&*p, &at(&root, "2026/b.txt")), b"beta");
    // The archive's connection is its container's.
    assert_eq!(p.connection_key(&root), location.connection_key());
    assert!(p.connection_key(&root).is_some());
    // The server going away is its own typed error, not a damaged archive.
    server.set_fault(Some(RemoteFault::Unreachable(UnreachableReason::Offline)));
    p.invalidate(&location);
    let error = p
        .list(&root, &CancelToken::new(), 0, &mut |_| {})
        .unwrap_err();
    assert!(matches!(error, VfsError::Unreachable { .. }), "{error:?}");
    server.set_fault(None);
    assert_eq!(names(&*p, &root).len(), 1);
}

#[test]
fn an_archive_in_a_folder_the_source_cannot_serve_is_unsupported() {
    let dir = scratch();
    let zip = RawZip::new()
        .file("a", b"a")
        .write(&dir.path().join("a.zip"));
    let server = FakeRemoteProvider::sftp();
    // The Local source serves files only; a path on a server has no provider.
    let remote = at(&server.root("me@fake.test"), "a.zip");
    let p = provider();
    let error = p
        .list(
            &VfsPath::Archive(ArchivePath::new(remote).unwrap()),
            &CancelToken::new(),
            0,
            &mut |_| {},
        )
        .unwrap_err();
    assert!(matches!(error, VfsError::Unsupported { .. }), "{error:?}");
    let _ = zip;
}
