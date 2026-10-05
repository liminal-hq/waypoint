// The S3 provider against real S3 servers run as the current user (`rclone serve s3`, and MinIO when
// `WAYPOINT_MINIO` names its binary): listing, paging, stat, reads, ranges, plain writes, deletes,
// server-side copies, the typed errors, and (on MinIO, which is complete enough) the shared
// conformance suite and multipart uploads. Each test skips with a message when there is no server
// (see `support`).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::sync::Arc;

use support::{FixedKey, Kind, Server, KEY_ID, SECRET};
use waypoint_path::VfsPath;
use waypoint_protocol::{AuthPrompt, ConnectionState, UnreachableReason, VfsError};
use waypoint_provider_s3::{S3Config, S3Options, S3Provider, StorageClass};
use waypoint_vfs::conformance::{self, Subject};
use waypoint_vfs::{
    CancelToken, ConnectAnswer, Credential, EntryKind, NoCredentials, Provider, ScannedEntry,
    Secret, WriteOptions,
};

fn names(entries: &[ScannedEntry]) -> BTreeSet<String> {
    entries
        .iter()
        .map(|entry| entry.name.to_string_lossy().into_owned())
        .collect()
}

fn list(provider: &S3Provider, path: &VfsPath) -> Result<Vec<ScannedEntry>, VfsError> {
    provider.list(path, &CancelToken::new(), 0, &mut |_| {})
}

fn read_all(provider: &S3Provider, path: &VfsPath, start: u64) -> Vec<u8> {
    let mut out = Vec::new();
    provider
        .open_read_at(path, start)
        .unwrap()
        .read_to_end(&mut out)
        .unwrap();
    out
}

fn put(provider: &S3Provider, path: &VfsPath, bytes: &[u8]) {
    let mut stream = provider
        .create_write(path, WriteOptions::exclusive())
        .unwrap();
    stream.write_all(bytes).unwrap();
    stream.finish(false).unwrap();
}

fn pattern(len: usize) -> Vec<u8> {
    (0..len as u32)
        .map(|n| (n.wrapping_mul(7) % 251) as u8)
        .collect()
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn passes_the_conformance_suite() {
    for server in Server::start_all() {
        let provider = server.provider();
        let subject = Subject {
            name: &format!("s3/{}", server.name()),
            provider: &provider,
            root: server.bucket("conformance"),
        };
        match server.kind {
            Kind::Minio => conformance::run(&subject),
            // rclone cannot make a folder (`dir/` becomes a file), which the rest of the suite
            // does first; the in-memory server runs all of it.
            Kind::Rclone => conformance::reading(&subject),
        }
    }
}

#[test]
fn lists_stats_and_reads_buckets_prefixes_and_keys() {
    for server in Server::start_all() {
        let bucket = server.bucket("photos");
        let provider = server.provider();
        let at = |rel: &str| bucket.join(rel).unwrap();
        put(&provider, &at("a.txt"), b"hello world");
        put(&provider, &at(".hidden"), b"");
        put(&provider, &at("2026/trip/one.jpg"), &pattern(300_000));
        put(&provider, &at("2026/two.jpg"), b"two");

        let root = list(&provider, &bucket).unwrap();
        assert_eq!(names(&root), set(&[".hidden", "2026", "a.txt"]));
        let by = |entries: &[ScannedEntry], name: &str| {
            entries.iter().find(|e| e.name == name).cloned().unwrap()
        };
        assert_eq!(by(&root, "2026").kind, EntryKind::Directory);
        assert_eq!(by(&root, "2026").size, None);
        assert_eq!(by(&root, "a.txt").kind, EntryKind::File);
        assert_eq!(by(&root, "a.txt").size, Some(11));
        assert!(by(&root, "a.txt").modified_ms.is_some());
        assert!(by(&root, ".hidden").hidden);

        let year = list(&provider, &at("2026")).unwrap();
        assert_eq!(names(&year), set(&["trip", "two.jpg"]));
        assert_eq!(
            provider.stat(&at("2026/trip")).unwrap().kind,
            EntryKind::Directory
        );
        assert_eq!(provider.stat(&at("2026/two.jpg")).unwrap().size, Some(3));
        assert_eq!(provider.stat(&bucket).unwrap().name, "photos");
        assert!(matches!(
            provider.stat(&at("nothing")),
            Err(VfsError::NotFound { .. })
        ));
        assert!(matches!(
            list(&provider, &at("a.txt")),
            Err(VfsError::NotADirectory { .. })
        ));
        assert!(matches!(
            provider.open_read(&at("2026")),
            Err(VfsError::IsADirectory { .. })
        ));

        // The storage class and ETag ride along with each object.
        let mut classes = Vec::new();
        provider
            .list_batches_with_attributes(&at("2026"), &CancelToken::new(), &mut |b| {
                classes.extend(b)
            })
            .unwrap();
        let two = classes.iter().find(|l| l.entry.name == "two.jpg").unwrap();
        assert_eq!(
            two.attributes.as_ref().unwrap().storage_class,
            StorageClass::Standard
        );
        assert!(classes
            .iter()
            .find(|l| l.entry.name == "trip")
            .unwrap()
            .attributes
            .is_none());

        // Whole, ranged and past-the-end reads, and a delete.
        let big = pattern(300_000);
        assert_eq!(
            read_all(&provider, &at("2026/trip/one.jpg"), 0),
            big,
            "{}",
            server.name()
        );
        assert_eq!(
            read_all(&provider, &at("2026/trip/one.jpg"), 299_990),
            big[299_990..]
        );
        assert!(read_all(&provider, &at("a.txt"), 100).is_empty());
        assert_eq!(read_all(&provider, &at("a.txt"), 6), b"world");
        provider.remove_file(&at("2026/two.jpg")).unwrap();
        assert!(matches!(
            provider.stat(&at("2026/two.jpg")),
            Err(VfsError::NotFound { .. })
        ));
        assert!(matches!(
            provider.remove_file(&at("2026/two.jpg")),
            Err(VfsError::NotFound { .. })
        ));
        assert!(matches!(
            provider.remove_file(&at("2026")),
            Err(VfsError::IsADirectory { .. })
        ));
    }
}

#[test]
fn a_big_listing_arrives_in_pages_that_add_up() {
    for server in Server::start_all() {
        let bucket = server.bucket("many");
        let provider = server.provider();
        let path = bucket.join("files").unwrap();
        for n in 0..2_500 {
            put(&provider, &path.join(format!("f{n:04}")).unwrap(), b"");
        }
        let mut batches = Vec::new();
        provider
            .list_batches(&path, &CancelToken::new(), 0, &mut |batch| {
                batches.push(batch.len())
            })
            .unwrap();
        assert_eq!(batches.iter().sum::<usize>(), 2_500, "{}", server.name());
        assert!(
            batches[0] <= 1_000,
            "the first page is handed over at once: {batches:?}"
        );
        assert!(batches.len() >= 2);
        let mut seen = BTreeSet::new();
        let mut progress = 0;
        let all = provider
            .list(&path, &CancelToken::new(), 0, &mut |n| progress = n)
            .unwrap();
        for entry in &all {
            assert!(seen.insert(entry.name.clone()), "no entry twice");
        }
        assert_eq!((all.len(), progress), (2_500, 2_500));
    }
}

#[test]
fn a_large_write_is_a_multipart_upload_and_a_dropped_one_leaves_nothing() {
    for server in Server::start_all() {
        if server.kind == Kind::Rclone {
            eprintln!(
                "skipping on rclone, whose CreateMultipartUpload answer has the wrong XML root"
            );
            continue;
        }
        let bucket = server.bucket("bigwrite");
        let provider = server.provider_with(S3Options {
            part_size: Some(5 * 1024 * 1024),
            ..S3Options::default()
        });
        let bytes = pattern(23 * 1024 * 1024 + 123);
        let path = bucket.join("large.bin").unwrap();
        let mut stream = provider
            .create_write(&path, WriteOptions::exclusive())
            .unwrap();
        for chunk in bytes.chunks(777_777) {
            stream.write_all(chunk).unwrap();
        }
        assert!(matches!(
            provider.stat(&path),
            Err(VfsError::NotFound { .. })
        ));
        stream.finish(true).unwrap();
        assert_eq!(provider.stat(&path).unwrap().size, Some(bytes.len() as u64));
        assert_eq!(read_all(&provider, &path, 0), bytes);
        assert_eq!(read_all(&provider, &path, 10_000_000), bytes[10_000_000..]);

        let abandoned = bucket.join("abandoned.bin").unwrap();
        {
            let mut stream = provider
                .create_write(&abandoned, WriteOptions::exclusive())
                .unwrap();
            stream.write_all(&bytes[..12 * 1024 * 1024]).unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert!(matches!(
            provider.stat(&abandoned),
            Err(VfsError::NotFound { .. })
        ));
    }
}

#[test]
fn objects_are_copied_on_the_server_within_and_across_buckets() {
    for server in Server::start_all() {
        let one = server.bucket("copyone");
        let two = server.bucket("copytwo");
        let provider = server.provider();
        let src = one.join("src.bin").unwrap();
        let bytes = pattern(1_234_567);
        put(&provider, &src, &bytes);
        let mut progress = 0;
        let copied = provider
            .copy_file_within(
                &src,
                &one.join("dst.bin").unwrap(),
                &mut |n| progress = n,
                &CancelToken::new(),
            )
            .expect("the copy is handled")
            .unwrap();
        assert_eq!((copied, progress), (bytes.len() as u64, bytes.len() as u64));
        assert_eq!(read_all(&provider, &one.join("dst.bin").unwrap(), 0), bytes);
        let across = two.join("a b/é.bin").unwrap();
        provider
            .copy_file_within(&src, &across, &mut |_| {}, &CancelToken::new())
            .unwrap()
            .unwrap();
        assert_eq!(read_all(&provider, &across, 0), bytes, "{}", server.name());
        // It never replaces, and a missing source is the source's.
        assert!(matches!(
            provider.copy_file_within(&src, &across, &mut |_| {}, &CancelToken::new()),
            Some(Err(VfsError::AlreadyExists { .. }))
        ));
        let missing = one.join("missing").unwrap();
        assert!(matches!(
            provider.copy_file_within(
                &missing,
                &one.join("x").unwrap(),
                &mut |_| {},
                &CancelToken::new()
            ),
            Some(Err(VfsError::NotFound { .. }))
        ));
        let local = VfsPath::from_uri("file:///tmp/x").unwrap();
        assert!(provider
            .copy_file_within(&src, &local, &mut |_| {}, &CancelToken::new())
            .is_none());
    }
}

#[test]
fn the_errors_are_typed() {
    for server in Server::start_all() {
        let bucket = server.bucket("errors");
        let provider = server.provider();
        put(&provider, &bucket.join("here").unwrap(), b"x");

        assert!(matches!(
            provider.stat(&server.location("nosuchbucket")),
            Err(VfsError::NotFound { .. })
        ));
        assert!(matches!(
            list(&provider, &server.location("nosuchbucket")),
            Err(VfsError::NotFound { .. })
        ));

        // No credential: asked for.
        let nobody = S3Provider::new(S3Config::new(Arc::new(NoCredentials)));
        let Err(VfsError::AuthRequired { prompt, .. }) = nobody.stat(&bucket) else {
            panic!("no credential asks for one");
        };
        assert_eq!(*prompt, AuthPrompt::AccessKey { key_id: None });

        // A wrong secret is refused, and a right answer connects.
        let wrong = S3Provider::new(S3Config::new(Arc::new(FixedKey {
            key_id: KEY_ID.to_owned(),
            secret: "wrong-secret-value".to_owned(),
        })));
        assert!(
            matches!(wrong.stat(&bucket), Err(VfsError::AuthFailed { .. })),
            "{}",
            server.name()
        );
        let key = bucket.connection_key().unwrap();
        assert!(matches!(
            wrong.connection_state(&key),
            ConnectionState::Failed { .. }
        ));
        wrong
            .connect(
                &key,
                Some(ConnectAnswer::Credential(Credential::AccessKey {
                    key_id: KEY_ID.to_owned(),
                    secret: Secret::from(SECRET),
                })),
                &CancelToken::new(),
            )
            .unwrap();
        assert_eq!(wrong.connection_state(&key), ConnectionState::Connected);
        assert_eq!(
            wrong.stat(&bucket.join("here").unwrap()).unwrap().size,
            Some(1)
        );
        wrong.disconnect(&key);
        assert_eq!(wrong.connection_state(&key), ConnectionState::Idle);

        // Nothing listens at the endpoint.
        let dead = VfsPath::from_uri(&format!(
            "s3://errors?endpoint=http%3A%2F%2F127.0.0.1%3A{}",
            support::free_port()
        ))
        .unwrap();
        assert!(matches!(
            provider.stat(&dead),
            Err(VfsError::Unreachable {
                reason: UnreachableReason::Refused,
                ..
            })
        ));

        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(matches!(
            provider.list(&bucket, &cancel, 0, &mut |_| {}),
            Err(VfsError::Cancelled)
        ));
    }
}

#[test]
fn rename_is_not_offered_because_a_move_is_a_copy_and_a_delete() {
    for server in Server::start_all() {
        let bucket = server.bucket("moves");
        let provider = server.provider();
        let caps = provider.capabilities();
        assert_eq!(caps.rename, waypoint_vfs::RenameSupport::None);
        put(&provider, &bucket.join("a").unwrap(), b"x");
        assert!(matches!(
            provider.rename(
                &bucket.join("a").unwrap(),
                &bucket.join("b").unwrap(),
                false
            ),
            Err(VfsError::Unsupported { .. })
        ));
    }
}

#[test]
fn the_buckets_of_a_service_are_listed() {
    for server in Server::start_all() {
        let seed = server.bucket("alpha");
        server.bucket("beta");
        let provider = server.provider();
        let found: BTreeSet<String> = provider
            .list_buckets(&seed)
            .unwrap()
            .into_iter()
            .map(|b| b.name)
            .collect();
        assert!(
            found.contains("alpha") && found.contains("beta"),
            "{found:?}"
        );
    }
}
