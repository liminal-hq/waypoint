// The S3 provider against a small in-memory S3 service (`support::fake_s3`): the shared conformance
// suite with real folder markers, multipart uploads and their aborts, conditional writes, storage
// classes, region redirects, paging and injected failures. It needs no server program, so it runs
// everywhere.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::sync::Arc;

use support::fake_s3::{FakeS3, Injected};
use support::{FixedKey, KEY_ID, SECRET};
use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;
use waypoint_provider_s3::{S3Config, S3Options, S3Provider, StorageClass};
use waypoint_vfs::conformance::{self, Subject};
use waypoint_vfs::{CancelToken, Provider, RenameSupport, ScannedEntry, WriteOptions};

struct Env {
    fake: FakeS3,
    provider: S3Provider,
}

impl Env {
    fn new() -> Self {
        Self::with(S3Options::default())
    }

    fn with(options: S3Options) -> Self {
        Self {
            fake: FakeS3::start(),
            provider: S3Provider::new(
                S3Config::new(Arc::new(FixedKey {
                    key_id: KEY_ID.to_owned(),
                    secret: SECRET.to_owned(),
                }))
                .with_options(options),
            ),
        }
    }

    fn at(&self, bucket: &str, key: &str) -> VfsPath {
        let key: String = key
            .bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' => {
                    (b as char).to_string()
                }
                other => format!("%{other:02X}"),
            })
            .collect();
        VfsPath::from_uri(&format!(
            "s3://{bucket}/{key}?endpoint=http%3A%2F%2F127.0.0.1%3A{}",
            self.fake.port
        ))
        .unwrap()
    }

    fn bucket(&self, bucket: &str) -> VfsPath {
        self.fake.bucket(bucket);
        VfsPath::from_uri(&format!(
            "s3://{bucket}?endpoint=http%3A%2F%2F127.0.0.1%3A{}",
            self.fake.port
        ))
        .unwrap()
    }
}

fn list(provider: &S3Provider, path: &VfsPath) -> Result<Vec<ScannedEntry>, VfsError> {
    provider.list(path, &CancelToken::new(), 0, &mut |_| {})
}

fn names(entries: &[ScannedEntry]) -> BTreeSet<String> {
    entries
        .iter()
        .map(|e| e.name.to_string_lossy().into_owned())
        .collect()
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

fn pattern(len: usize) -> Vec<u8> {
    (0..len as u32)
        .map(|n| (n.wrapping_mul(7) % 251) as u8)
        .collect()
}

fn small_parts() -> S3Options {
    S3Options {
        part_size: Some(5 * 1024 * 1024),
        ..S3Options::default()
    }
}

#[test]
fn passes_the_conformance_suite() {
    let env = Env::new();
    conformance::run(&Subject {
        name: "s3/fake",
        provider: &env.provider,
        root: env.bucket("conformance"),
    });
}

#[test]
fn folders_are_markers_and_prefixes() {
    let env = Env::new();
    let root = env.bucket("b");
    env.fake.put("b", "docs/readme.txt", b"hi", "STANDARD");
    env.fake.put("b", "docs/deep/er/file", b"x", "STANDARD");
    env.fake.put("b", "empty/", b"", "STANDARD");
    env.fake.put("b", "top.txt", b"top", "STANDARD");
    env.fake.put("b", "odd//name", b"x", "STANDARD");
    let p = &env.provider;

    // A prefix is a folder with no object of its own; a marker is one that may be empty.
    assert_eq!(
        names(&list(p, &root).unwrap()),
        ["docs", "empty", "odd", "top.txt"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert_eq!(
        names(&list(p, &env.at("b", "docs")).unwrap()),
        ["deep", "readme.txt"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert!(list(p, &env.at("b", "empty")).unwrap().is_empty());
    assert_eq!(
        p.stat(&env.at("b", "empty")).unwrap().kind,
        waypoint_vfs::EntryKind::Directory
    );
    assert_eq!(
        p.stat(&env.at("b", "docs/deep")).unwrap().kind,
        waypoint_vfs::EntryKind::Directory
    );
    // The key no path can address is left out of its folder.
    assert!(list(p, &env.at("b", "odd")).unwrap().is_empty());

    // Creating a folder writes its marker, removing an empty one deletes it.
    p.create_dir(&env.at("b", "new")).unwrap();
    assert!(env.fake.keys("b").contains(&"new/".to_owned()));
    assert!(matches!(
        p.create_dir(&env.at("b", "docs")),
        Err(VfsError::AlreadyExists { .. })
    ));
    assert!(matches!(
        p.remove_dir(&env.at("b", "docs")),
        Err(VfsError::NotEmpty { .. })
    ));
    p.remove_dir(&env.at("b", "new")).unwrap();
    assert!(!env.fake.keys("b").contains(&"new/".to_owned()));
    assert!(matches!(
        p.remove_dir(&env.at("b", "top.txt")),
        Err(VfsError::NotADirectory { .. })
    ));
    assert!(matches!(
        p.create_dir(&root),
        Err(VfsError::Unsupported { .. })
    ));
}

#[test]
fn a_listing_pages_by_continuation_token_and_keeps_going_across_prefixes() {
    let env = Env::new();
    let root = env.bucket("many");
    for n in 0..2_345 {
        env.fake
            .put("many", &format!("files/f{n:05}"), b"", "STANDARD");
    }
    for n in 0..30 {
        env.fake
            .put("many", &format!("dirs/d{n:02}/x"), b"", "STANDARD");
    }
    let mut sizes = Vec::new();
    let path = env.at("many", "files");
    env.provider
        .list_batches(&path, &CancelToken::new(), 0, &mut |b| sizes.push(b.len()))
        .unwrap();
    assert_eq!(sizes.iter().sum::<usize>(), 2_345);
    assert_eq!(sizes[0], 1_000, "the first page is handed over at once");
    let log = env.fake.log();
    let pages = log.iter().filter(|l| l.contains("list-type=2")).count();
    assert_eq!(pages, 3, "1,000 keys per page: {log:?}");
    assert!(log.iter().any(|l| l.contains("continuation-token")));
    assert!(log.iter().all(|l| !l.contains("list-type=2")
        || l.contains("delimiter=%2F")
        || l.contains("delimiter=/")));
    // A folder of folders lists each once.
    assert_eq!(
        list(&env.provider, &env.at("many", "dirs")).unwrap().len(),
        30
    );
    assert_eq!(list(&env.provider, &root).unwrap().len(), 2);
}

#[test]
fn storage_classes_are_listed_and_an_archived_object_needs_a_restore() {
    let env = Env::new();
    let root = env.bucket("cold");
    env.fake.put("cold", "std.txt", b"s", "STANDARD");
    env.fake.put("cold", "ia.txt", b"i", "STANDARD_IA");
    env.fake.put("cold", "glacier.txt", b"g", "GLACIER");
    env.fake.put("cold", "deep.txt", b"d", "DEEP_ARCHIVE");
    env.fake.put("cold", "ir.txt", b"r", "GLACIER_IR");
    let mut listed = Vec::new();
    env.provider
        .list_batches_with_attributes(&root, &CancelToken::new(), &mut |b| listed.extend(b))
        .unwrap();
    let class = |name: &str| {
        listed
            .iter()
            .find(|l| l.entry.name == name)
            .unwrap()
            .attributes
            .clone()
            .unwrap()
    };
    assert_eq!(class("std.txt").storage_class, StorageClass::Standard);
    assert_eq!(class("ia.txt").storage_class, StorageClass::StandardIa);
    assert_eq!(class("glacier.txt").storage_class, StorageClass::Glacier);
    assert!(class("glacier.txt").archived() && class("deep.txt").archived());
    // The same facts ride on the entry itself, for the column.
    let glacier = listed
        .iter()
        .find(|l| l.entry.name == "glacier.txt")
        .unwrap();
    assert_eq!(
        glacier
            .entry
            .attributes
            .as_ref()
            .unwrap()
            .get("s3.storageClass"),
        Some("GLACIER")
    );
    let (entry, _) = env
        .provider
        .stat_with_attributes(&env.at("cold", "ia.txt"))
        .unwrap();
    assert_eq!(
        entry.attributes.as_ref().unwrap().get("s3.storageClass"),
        Some("STANDARD_IA")
    );
    assert!(!class("ir.txt").archived());
    // HEAD says it too.
    let (_, head) = env
        .provider
        .stat_with_attributes(&env.at("cold", "deep.txt"))
        .unwrap();
    assert_eq!(head.unwrap().storage_class, StorageClass::DeepArchive);

    // Archived objects are listed and can be seen, but reading them is a typed error, and nothing
    // asks the service to restore anything.
    for name in ["glacier.txt", "deep.txt"] {
        assert!(
            matches!(
                env.provider.open_read(&env.at("cold", name)),
                Err(VfsError::Archived { .. })
            ),
            "reading {name} should say a restore is needed"
        );
    }
    assert_eq!(read_all(&env.provider, &env.at("cold", "ir.txt"), 0), b"r");
    assert!(env.fake.log().iter().all(|l| !l.contains("restore")));
    // Copying one on the server fails the same way, and leaves no destination.
    let copied = env.provider.copy_file_within(
        &env.at("cold", "glacier.txt"),
        &env.at("cold", "copy.txt"),
        &mut |_| {},
        &CancelToken::new(),
    );
    assert!(matches!(copied, Some(Err(VfsError::Archived { .. }))));
    assert!(!env.fake.keys("cold").contains(&"copy.txt".to_owned()));
}

#[test]
fn a_large_write_is_a_multipart_upload_and_failures_abort_it() {
    let env = Env::with(small_parts());
    let root = env.bucket("big");
    let bytes = pattern(23 * 1024 * 1024 + 321);
    let path = env.at("big", "dir/large.bin");
    let mut stream = env
        .provider
        .create_write(&path, WriteOptions::exclusive())
        .unwrap();
    for chunk in bytes.chunks(900_001) {
        stream.write_all(chunk).unwrap();
    }
    assert!(
        env.fake.get("big", "dir/large.bin").is_none(),
        "nothing is visible before finish"
    );
    stream.finish(true).unwrap();
    assert_eq!(env.fake.get("big", "dir/large.bin").unwrap(), bytes);
    let log = env.fake.log();
    assert!(log
        .iter()
        .any(|l| l.contains("POST /big/dir/large.bin?uploads")));
    let parts = log.iter().filter(|l| l.contains("partNumber=")).count();
    assert_eq!(parts, 5, "four full 5 MiB parts and the remainder: {log:?}");
    assert_eq!(env.fake.open_uploads(), 0);

    // Dropped after parts went up: the upload is aborted and the object never appears.
    {
        let mut stream = env
            .provider
            .create_write(&env.at("big", "dropped.bin"), WriteOptions::exclusive())
            .unwrap();
        stream.write_all(&bytes[..12 * 1024 * 1024]).unwrap();
    }
    for _ in 0..50 {
        if env.fake.open_uploads() == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(env.fake.open_uploads(), 0, "the dropped upload was aborted");
    assert!(env
        .fake
        .state
        .lock()
        .unwrap()
        .aborted
        .contains(&"big/dropped.bin".to_owned()));
    assert!(env.fake.get("big", "dropped.bin").is_none());

    // A part the service refuses fails the write, aborts the upload and leaves no object.
    env.fake.inject(Injected {
        status: 403,
        code: "AccessDenied",
        retry_after: None,
        when: "partNumber=2",
        times: 10,
    });
    let mut stream = env
        .provider
        .create_write(&env.at("big", "refused.bin"), WriteOptions::exclusive())
        .unwrap();
    let mut failed = None;
    for chunk in bytes.chunks(1_000_000) {
        if let Err(error) = stream.write_all(chunk) {
            failed = Some(error);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let outcome = match failed {
        Some(error) => Err(waypoint_vfs::from_io(&error, &root.to_location())),
        None => stream.finish(true),
    };
    assert!(
        matches!(outcome, Err(VfsError::PermissionDenied { .. })),
        "{outcome:?}"
    );
    for _ in 0..50 {
        if env.fake.open_uploads() == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(env.fake.open_uploads(), 0);
    assert!(env.fake.get("big", "refused.bin").is_none());
}

#[test]
fn an_exclusive_write_is_conditional_where_the_service_honours_it() {
    let env = Env::with(S3Options {
        conditional_writes: Some(true),
        ..small_parts()
    });
    env.bucket("c");
    let path = env.at("c", "once.txt");
    // The name appears between the check and the write: the service refuses to replace it.
    let mut stream = env
        .provider
        .create_write(&path, WriteOptions::exclusive())
        .unwrap();
    env.fake
        .put("c", "once.txt", b"somebody else's", "STANDARD");
    stream.write_all(b"mine").unwrap();
    assert!(matches!(
        stream.finish(false),
        Err(VfsError::AlreadyExists { .. })
    ));
    assert_eq!(env.fake.get("c", "once.txt").unwrap(), b"somebody else's");
    // The same through a multipart completion.
    let big = pattern(6 * 1024 * 1024);
    let path = env.at("c", "twice.bin");
    let mut stream = env
        .provider
        .create_write(&path, WriteOptions::exclusive())
        .unwrap();
    env.fake.put("c", "twice.bin", b"first", "STANDARD");
    stream.write_all(&big).unwrap();
    assert!(matches!(
        stream.finish(false),
        Err(VfsError::AlreadyExists { .. })
    ));
    assert_eq!(env.fake.get("c", "twice.bin").unwrap(), b"first");
    // Without it (a service of unknown make) the check alone guards the name, and an overwrite
    // that is allowed replaces.
    let plain = Env::new();
    plain.bucket("c");
    plain.fake.put("c", "x", b"old", "STANDARD");
    let mut stream = plain
        .provider
        .create_write(&plain.at("c", "x"), WriteOptions::truncate())
        .unwrap();
    stream.write_all(b"new").unwrap();
    stream.finish(false).unwrap();
    assert_eq!(plain.fake.get("c", "x").unwrap(), b"new");
}

#[test]
fn a_write_resumes_from_an_offset() {
    let env = Env::with(small_parts());
    env.bucket("r");
    let head = pattern(7 * 1024 * 1024);
    env.fake.put("r", "partial.bin", &head, "STANDARD");
    let path = env.at("r", "partial.bin");
    let offset = 6 * 1024 * 1024;
    let mut stream = env.provider.resume_write(&path, offset as u64).unwrap();
    stream.write_all(b"the tail").unwrap();
    assert_eq!(
        env.fake.get("r", "partial.bin").unwrap(),
        head,
        "the old object stands until finish"
    );
    stream.finish(true).unwrap();
    let mut wanted = head[..offset].to_vec();
    wanted.extend_from_slice(b"the tail");
    assert_eq!(env.fake.get("r", "partial.bin").unwrap(), wanted);
    assert!(matches!(
        env.provider
            .resume_write(&env.at("r", "none"), 0)
            .map(|_| ()),
        Err(VfsError::NotFound { .. })
    ));
    assert!(env.provider.resume_write(&path, 1 << 40).is_err());
}

#[test]
fn copies_happen_on_the_server_and_big_ones_in_parts() {
    let env = Env::with(S3Options {
        copy_part_threshold: Some(1024),
        ..S3Options::default()
    });
    env.bucket("a");
    env.bucket("b");
    let bytes = pattern(600 * 1024);
    env.fake.put("a", "src/é #1.bin", &bytes, "STANDARD");
    let mut progress = 0;
    let size = env
        .provider
        .copy_file_within(
            &env.at("a", "src/é #1.bin"),
            &env.at("b", "dst/copy.bin"),
            &mut |n| progress = n,
            &CancelToken::new(),
        )
        .unwrap()
        .unwrap();
    assert_eq!((size, progress), (bytes.len() as u64, bytes.len() as u64));
    assert_eq!(env.fake.get("b", "dst/copy.bin").unwrap(), bytes);
    let log = env.fake.log();
    assert!(
        log.iter()
            .any(|l| l.contains("POST /b/dst/copy.bin?uploads")),
        "{log:?}"
    );
    assert!(log.iter().any(|l| l.contains("partNumber=1")));
    // A small one is a single CopyObject.
    env.fake.put("a", "small", b"tiny", "STANDARD");
    env.provider
        .copy_file_within(
            &env.at("a", "small"),
            &env.at("a", "small2"),
            &mut |_| {},
            &CancelToken::new(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(env.fake.get("a", "small2").unwrap(), b"tiny");
    // Nothing is replaced.
    assert!(matches!(
        env.provider.copy_file_within(
            &env.at("a", "small"),
            &env.at("a", "small2"),
            &mut |_| {},
            &CancelToken::new()
        ),
        Some(Err(VfsError::AlreadyExists { .. }))
    ));
}

#[test]
fn a_bucket_in_another_region_is_followed_once_and_remembered() {
    let env = Env::with(S3Options {
        follow_region_redirects: Some(true),
        ..S3Options::default()
    });
    let root = env.bucket("far");
    env.fake
        .state
        .lock()
        .unwrap()
        .bucket_regions
        .insert("far".to_owned(), "eu-west-3".to_owned());
    env.fake.put("far", "a.txt", b"a", "STANDARD");
    assert_eq!(
        names(&list(&env.provider, &root).unwrap()),
        ["a.txt".to_owned()].into_iter().collect()
    );
    assert_eq!(read_all(&env.provider, &env.at("far", "a.txt"), 0), b"a");
    let regions = env.fake.state.lock().unwrap().regions.clone();
    assert_eq!(regions.first().map(String::as_str), Some("us-east-1"));
    assert!(
        regions.iter().skip(1).all(|r| r == "eu-west-3"),
        "{regions:?}"
    );
    let redirects = regions.iter().filter(|r| *r == "us-east-1").count();
    assert_eq!(
        redirects, 1,
        "only the first request went to the wrong region"
    );
}

#[test]
fn a_service_that_asks_to_slow_down_is_a_rate_limit_with_its_retry_after() {
    let env = Env::with(S3Options {
        max_attempts: Some(2),
        ..S3Options::default()
    });
    let root = env.bucket("slow");
    env.fake.inject(Injected {
        status: 503,
        code: "SlowDown",
        retry_after: Some(3),
        when: "list-type",
        times: 10,
    });
    match list(&env.provider, &root) {
        Err(VfsError::RateLimited { retry_after_ms, .. }) => assert_eq!(retry_after_ms, Some(3000)),
        other => panic!("expected a rate limit, got {other:?}"),
    }
}

#[test]
fn a_skewed_clock_and_a_refused_signature_are_told_apart() {
    let env = Env::with(S3Options {
        max_attempts: Some(1),
        ..S3Options::default()
    });
    let root = env.bucket("auth");
    env.fake.inject(Injected {
        status: 403,
        code: "RequestTimeTooSkewed",
        retry_after: None,
        when: "list-type",
        times: 1,
    });
    match list(&env.provider, &root) {
        Err(VfsError::ClockSkew { skew_ms: None, .. }) => {}
        other => panic!("expected a clock error, got {other:?}"),
    }
    env.fake.inject(Injected {
        status: 403,
        code: "SignatureDoesNotMatch",
        retry_after: None,
        when: "list-type",
        times: 1,
    });
    assert!(matches!(
        list(&env.provider, &root),
        Err(VfsError::AuthFailed { .. })
    ));
}

#[test]
fn the_buckets_of_a_service_are_listed_and_one_can_be_made() {
    let env = Env::new();
    let seed = env.bucket("alpha");
    env.bucket("beta");
    let buckets = env.provider.list_buckets(&seed).unwrap();
    assert_eq!(
        buckets.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
        ["alpha", "beta"]
    );
    assert!(buckets.iter().all(|b| b.created_ms.is_some()));
    let gamma = VfsPath::from_uri(&format!(
        "s3://gamma?endpoint=http%3A%2F%2F127.0.0.1%3A{}",
        env.fake.port
    ))
    .unwrap();
    env.provider.create_bucket(&gamma).unwrap();
    assert!(env.fake.keys("gamma").is_empty());
}

#[test]
fn capabilities_say_what_s3_is() {
    let caps = S3Provider::new(S3Config::new(Arc::new(waypoint_vfs::NoCredentials))).capabilities();
    assert_eq!(
        caps.rename,
        RenameSupport::None,
        "a move is a copy and a delete"
    );
    assert!(caps.remote && caps.write && caps.server_copy);
    assert!(caps.atomic_write && caps.resume_write && caps.range_read);
    assert!(!caps.watch && !caps.symlinks && !caps.set_times);
    assert_eq!(caps.permissions, waypoint_vfs::PermissionModel::None);
}

/// A credential source that records the question it was asked, and answers it with a fixed secret.
struct Recording {
    asked: std::sync::Mutex<Vec<Option<String>>>,
}

impl waypoint_vfs::CredentialSource for Recording {
    fn credential(
        &self,
        _: &waypoint_path::ConnectionKey,
        prompt: &waypoint_protocol::AuthPrompt,
    ) -> Option<waypoint_vfs::Credential> {
        let waypoint_protocol::AuthPrompt::AccessKey { key_id } = prompt else {
            return None;
        };
        self.asked.lock().unwrap().push(key_id.clone());
        Some(waypoint_vfs::Credential::AccessKey {
            key_id: key_id.clone()?,
            secret: waypoint_vfs::Secret::from("secret"),
            session_token: None,
        })
    }
}

#[test]
fn a_saved_connections_key_id_is_part_of_the_question_the_credential_source_is_asked() {
    let fake = FakeS3::start();
    fake.bucket("keyed");
    let source = Arc::new(Recording {
        asked: std::sync::Mutex::new(Vec::new()),
    });
    let provider = S3Provider::new(S3Config::new(source.clone()));
    let path = VfsPath::from_uri(&format!(
        "s3://keyed?endpoint=http%3A%2F%2F127.0.0.1%3A{}",
        fake.port
    ))
    .unwrap();
    // With no key id known, the source is asked without one, and (as the app's source does) has
    // nothing to offer: the login asks for an access key.
    let Err(VfsError::AuthRequired { prompt, .. }) = provider.stat(&path) else {
        panic!("a source that cannot pair a secret with an unnamed key asks the person");
    };
    assert_eq!(
        *prompt,
        waypoint_protocol::AuthPrompt::AccessKey { key_id: None }
    );
    assert_eq!(*source.asked.lock().unwrap(), [None]);

    // With the id from the saved connection, the same source answers and the login works.
    provider.set_options(
        &path.connection_key().unwrap(),
        S3Options {
            access_key_id: Some("AKIA".to_owned()),
            ..S3Options::default()
        },
    );
    assert!(provider.stat(&path).is_ok());
    assert_eq!(
        *source.asked.lock().unwrap().last().unwrap(),
        Some("AKIA".to_owned())
    );
    // And a prompt for a person to answer names the id, so the dialog starts with it filled in.
    let nobody = S3Provider::new(S3Config::new(Arc::new(waypoint_vfs::NoCredentials)));
    nobody.set_options(
        &path.connection_key().unwrap(),
        S3Options {
            access_key_id: Some("AKIA".to_owned()),
            ..S3Options::default()
        },
    );
    let Err(VfsError::AuthRequired { prompt, .. }) = nobody.stat(&path) else {
        panic!("no credential asks");
    };
    assert_eq!(
        *prompt,
        waypoint_protocol::AuthPrompt::AccessKey {
            key_id: Some("AKIA".to_owned())
        }
    );
}
