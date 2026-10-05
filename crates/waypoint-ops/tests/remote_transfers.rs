// Copies and moves between providers (A84, D151): local folders and fake servers, two kinds of
// server, one server under two logins, and servers that copy on their side, write atomically or
// cannot rename. Everything is checked through the providers, and nothing may be left half-written.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;
mod journal_support;
mod xfer;

use std::sync::Arc;

use journal_support::*;
use waypoint_path::RemoteScheme;
use waypoint_protocol::VfsError;
use waypoint_vfs::{FakeRemoteProvider, PermissionModel, RenameSupport};
use xfer::*;

const SERVER: &str = "me@fake.test";

/// A journalled engine over a local (in-memory) work folder, with `servers` registered beside it.
fn engine(servers: &[&FakeRemoteProvider]) -> (JournalHarness<MemoryProvider>, tempfile::TempDir) {
    let (mut h, dir) = memory_jh(CaseRule::Sensitive);
    for server in servers {
        h.harness
            .env
            .providers
            .register(Arc::new((*server).clone()));
    }
    h.chunk_bytes = SMALL_CHUNK;
    (h, dir)
}

fn at(server: &FakeRemoteProvider, authority: &str, relative: &str) -> VfsPath {
    relative
        .split('/')
        .filter(|s| !s.is_empty())
        .fold(server.root(authority), |p, name| p.join(name).unwrap())
}

fn on(server: &FakeRemoteProvider, relative: &str) -> VfsPath {
    at(server, SERVER, relative)
}

fn request(kind: JobKind, sources: &[VfsPath], dest: &VfsPath) -> JobRequest {
    JobRequest {
        kind,
        sources: Sources::Locations {
            locations: sources.iter().map(VfsPath::to_location).collect(),
        },
        destination: Some(dest.to_location()),
        name: None,
        options: JobOptions::default(),
        origin_window: "main-1".to_owned(),
        rename: None,
        archive: None,
    }
}

fn content(provider: &dyn Provider, path: &VfsPath) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut out = Vec::new();
    provider.open_read(path).ok()?.read_to_end(&mut out).ok()?;
    Some(out)
}

fn server_tree(server: &FakeRemoteProvider, relative: &str) -> Tree {
    tree_of(server, &on(server, relative))
}

fn finished(run: &JournalRun) {
    assert_eq!(
        run.state,
        JobState::Done,
        "failure: {:?}",
        run.failure.as_ref().map(|f| (&f.error, &f.item))
    );
}

#[test]
fn an_upload_copies_a_tree_with_its_times_and_modes_and_says_where_it_went() {
    let server = FakeRemoteProvider::sftp();
    server.put_dir(&on(&server, "up"));
    let (mut h, _dir) = engine(&[&server]);
    jbuild(
        &h,
        &tree(&[
            ("src/", ""),
            ("src/a.txt", "alpha"),
            ("src/sub/", ""),
            ("src/sub/b", "beta"),
        ]),
    );
    set_mtime(&h.harness, "src/a.txt", 1_600_000_000_000);
    set_mode(&h.harness, "src/a.txt", 0o640);

    let run = h.run_journalled(request(JobKind::Copy, &[h.path("src")], &on(&server, "up")));
    finished(&run);
    let plan = run.plan.as_ref().unwrap();
    assert!(!plan.same_volume, "a server is another volume");
    assert_eq!(plan.ends.to.as_deref(), Some("sftp://me@fake.test"));
    assert!(plan.ends.from.is_empty());
    let snapshot = h.store.job(run.id).unwrap().clone();
    assert_eq!(
        snapshot.ends.unwrap().to.as_deref(),
        Some("sftp://me@fake.test")
    );

    let up = server_tree(&server, "up");
    assert_eq!(
        up,
        tree(&[
            ("src/", ""),
            ("src/a.txt", "alpha"),
            ("src/sub/", ""),
            ("src/sub/b", "beta")
        ])
    );
    let copied = server.stat(&on(&server, "up/src/a.txt")).unwrap();
    assert_eq!(copied.modified_ms, Some(1_600_000_000_000));
    let mode = server
        .permissions(&on(&server, "up/src/a.txt"))
        .unwrap()
        .mode;
    assert_eq!(mode.map(|m| m & 0o777), Some(0o640));
    assert!(run.report.unwrap().transfer.dropped.is_empty());
}

#[test]
fn a_download_and_a_copy_between_two_kinds_of_server_stream_through_the_app() {
    let sftp = FakeRemoteProvider::sftp();
    let dav = FakeRemoteProvider::new(RemoteScheme::Davs, CaseRule::Sensitive);
    sftp.put_file(&on(&sftp, "docs/report.pdf"), b"%PDF report");
    dav.put_dir(&on(&dav, "inbox"));
    let (mut h, _dir) = engine(&[&sftp, &dav]);
    jbuild(&h, &tree(&[("dst/", "")]));

    let down = h.run_journalled(request(
        JobKind::Copy,
        &[on(&sftp, "docs/report.pdf")],
        &h.path("dst"),
    ));
    finished(&down);
    assert_eq!(
        content(h.provider.as_ref(), &h.path("dst/report.pdf")).as_deref(),
        Some(&b"%PDF report"[..])
    );
    let ends = &down.plan.as_ref().unwrap().ends;
    assert_eq!(ends.from, ["sftp://me@fake.test"]);
    assert_eq!(ends.to, None);

    let across = h.run_journalled(request(
        JobKind::Copy,
        &[on(&sftp, "docs/report.pdf")],
        &on(&dav, "inbox"),
    ));
    finished(&across);
    assert_eq!(
        content(&dav, &on(&dav, "inbox/report.pdf")).as_deref(),
        Some(&b"%PDF report"[..])
    );
    assert!(leftovers(&server_tree(&dav, "inbox")).is_empty());
}

#[test]
fn a_copy_within_one_login_is_done_on_the_server_and_never_across_two_logins() {
    let server = FakeRemoteProvider::sftp();
    server.memory().enable_fast_copy(true);
    server.set_capabilities(|caps| caps.server_copy = true);
    server.put_file(&on(&server, "a/big.iso"), &[7u8; 5000]);
    server.put_dir(&on(&server, "b"));
    let (mut h, _dir) = engine(&[&server]);

    let within = h.run_journalled(request(
        JobKind::Copy,
        &[on(&server, "a/big.iso")],
        &on(&server, "b"),
    ));
    finished(&within);
    assert_eq!(server.memory().calls(MemOp::CopyFileWithin), 1);
    assert_eq!(
        server.memory().calls(MemOp::Write),
        0,
        "no byte came through the app"
    );

    // The same server under another login shares the files but not a session: the copy streams.
    let other = at(&server, "you@fake.test", "c");
    server.put_dir(&other);
    let across = h.run_journalled(request(JobKind::Copy, &[on(&server, "a/big.iso")], &other));
    finished(&across);
    assert_eq!(
        server.memory().calls(MemOp::CopyFileWithin),
        1,
        "no server copy across logins"
    );
    assert!(server.memory().calls(MemOp::Write) > 0);
    assert_eq!(
        content(&server, &at(&server, "you@fake.test", "c/big.iso"))
            .unwrap()
            .len(),
        5000
    );
}

#[test]
fn a_move_within_one_login_renames_and_a_move_off_the_server_copies_then_deletes() {
    let server = FakeRemoteProvider::sftp();
    server.put_file(&on(&server, "a/notes.txt"), b"notes");
    server.put_file(&on(&server, "a/dir/x"), b"x");
    server.put_dir(&on(&server, "b"));
    let (mut h, _dir) = engine(&[&server]);
    jbuild(&h, &tree(&[("dst/", "")]));

    let renamed = h.run_journalled(request(
        JobKind::Move,
        &[on(&server, "a/notes.txt"), on(&server, "a/dir")],
        &on(&server, "b"),
    ));
    finished(&renamed);
    assert!(
        renamed.plan.as_ref().unwrap().same_volume,
        "one login is one volume"
    );
    assert_eq!(
        server.memory().calls(MemOp::Write),
        0,
        "a rename sends no bytes"
    );
    assert_eq!(
        server_tree(&server, "b"),
        tree(&[("dir/", ""), ("dir/x", "x"), ("notes.txt", "notes")])
    );

    let moved = h.run_journalled(request(
        JobKind::Move,
        &[on(&server, "b/notes.txt")],
        &h.path("dst"),
    ));
    finished(&moved);
    assert!(!moved.plan.as_ref().unwrap().same_volume);
    assert_eq!(
        content(h.provider.as_ref(), &h.path("dst/notes.txt")).as_deref(),
        Some(&b"notes"[..])
    );
    assert!(matches!(
        server.stat(&on(&server, "b/notes.txt")),
        Err(VfsError::NotFound { .. })
    ));
}

#[test]
fn a_server_that_writes_atomically_gets_no_partial_names_and_a_failed_write_shows_nothing() {
    let server = FakeRemoteProvider::new(RemoteScheme::Dav, CaseRule::Sensitive);
    server.set_capabilities(|caps| {
        caps.atomic_write = true;
        caps.rename = RenameSupport::None;
        caps.set_times = false;
        caps.permissions = PermissionModel::None;
        caps.symlinks = false;
        caps.resume_write = false;
    });
    server.put_dir(&on(&server, "bucket"));
    server.put_file(&on(&server, "bucket/old.txt"), b"old");
    let (mut h, _dir) = engine(&[&server]);
    jbuild(
        &h,
        &tree(&[
            ("src/", ""),
            ("src/new.txt", "fresh"),
            ("src/old.txt", "newer"),
            ("src/d/", ""),
            ("src/d/e", "e"),
        ]),
    );

    // The third write of the copy fails: the file never appears and nothing is left behind.
    jbuild(&h, &tree(&[("big", &"z".repeat(4 * SMALL_CHUNK))]));
    server.memory().fail_nth(
        MemOp::CreateWrite,
        1,
        VfsError::Disconnected {
            location: on(&server, "bucket").to_location(),
        },
    );
    let failed = h.run_journalled(request(
        JobKind::Copy,
        &[h.path("big")],
        &on(&server, "bucket"),
    ));
    assert!(
        matches!(failed_with(&failed), OpsError::Connection { .. }),
        "{:?}",
        failed.state
    );
    server.memory().clear_failures();
    assert_eq!(server_tree(&server, "bucket"), tree(&[("old.txt", "old")]));

    let mut req = request(
        JobKind::Copy,
        &[
            h.path("src/new.txt"),
            h.path("src/old.txt"),
            h.path("src/d"),
        ],
        &on(&server, "bucket"),
    );
    req.options.conflict = Some(ConflictPolicy::Replace);
    let run = h.run_journalled(req);
    finished(&run);
    assert_eq!(
        server_tree(&server, "bucket"),
        tree(&[
            ("d/", ""),
            ("d/e", "e"),
            ("new.txt", "fresh"),
            ("old.txt", "newer")
        ])
    );
    // What the destination cannot hold is said, once each.
    let snapshot = h.store.job(run.id).unwrap().clone();
    let report = run.report.unwrap();
    assert_eq!(
        report.transfer.dropped,
        [DroppedDetail::ModifiedTimes, DroppedDetail::Permissions]
    );
    assert_eq!(
        snapshot.ends.unwrap().to.as_deref(),
        Some("dav://me@fake.test")
    );
}

#[test]
fn a_server_that_cannot_rename_moves_by_copy_and_delete_and_never_replaces_a_folder() {
    let server = FakeRemoteProvider::new(RemoteScheme::Dav, CaseRule::Sensitive);
    server.memory().enable_fast_copy(true);
    server.set_capabilities(|caps| {
        caps.atomic_write = true;
        caps.rename = RenameSupport::None;
        caps.server_copy = true;
    });
    server.put_file(&on(&server, "a/f.bin"), &[1u8; 3000]);
    server.put_file(&on(&server, "a/d/g"), b"g");
    server.put_file(&on(&server, "b/d/h"), b"h");
    let (mut h, _dir) = engine(&[&server]);

    let moved = h.run_journalled(request(
        JobKind::Move,
        &[on(&server, "a/f.bin")],
        &on(&server, "b"),
    ));
    finished(&moved);
    assert!(
        !moved.plan.as_ref().unwrap().same_volume,
        "no rename, so no move by rename"
    );
    assert_eq!(
        server.memory().calls(MemOp::CopyFileWithin),
        1,
        "the server copies, then the source goes"
    );
    assert_eq!(server.memory().calls(MemOp::Rename), 0);
    assert!(matches!(
        server.stat(&on(&server, "a/f.bin")),
        Err(VfsError::NotFound { .. })
    ));

    // Replacing a folder would need a rename to be safe: it is refused, and skipping leaves both.
    // (A Replace for all would merge folders; only an answer for this folder replaces it.)
    let req = request(JobKind::Copy, &[on(&server, "a/d")], &on(&server, "b"));
    let mut answers = Answers {
        conflicts: Box::new(|conflicts| {
            conflicts
                .iter()
                .map(|c| Resolution {
                    source: Some(c.source.clone()),
                    policy: ConflictPolicy::Replace,
                })
                .collect()
        }),
        errors: Box::new(|_, error| {
            assert!(matches!(error, OpsError::Unsupported { .. }), "{error:?}");
            Some(Decision::Skip)
        }),
    };
    let result = run(&mut h.harness, req, &mut answers);
    assert_eq!(result.state, JobState::Done);
    assert_eq!(server_tree(&server, "b/d"), tree(&[("h", "h")]));
}

#[test]
fn a_cancel_or_a_dropped_connection_mid_upload_leaves_nothing_on_the_server_and_retry_finishes() {
    let server = FakeRemoteProvider::sftp();
    server.put_dir(&on(&server, "up"));
    let (mut h, _dir) = engine(&[&server]);
    jbuild(&h, &tree(&[("big", &"q".repeat(8 * SMALL_CHUNK))]));

    // The connection drops at the third chunk; the job asks, and Retry after it is back finishes.
    server.memory().fail_nth(
        MemOp::Write,
        3,
        VfsError::Disconnected {
            location: on(&server, "up").to_location(),
        },
    );
    let mut asked = Vec::new();
    let mut answers = Answers {
        conflicts: Box::new(|_| Vec::new()),
        errors: Box::new(move |_, error| {
            asked.push(error.clone());
            assert!(
                matches!(
                    error,
                    OpsError::Connection {
                        error: VfsError::Disconnected { .. }
                    }
                ),
                "{error:?}"
            );
            Some(Decision::Retry)
        }),
    };
    let big = request(JobKind::Copy, &[h.path("big")], &on(&server, "up"));
    let result = run(&mut h.harness, big.clone(), &mut answers);
    assert_eq!(result.state, JobState::Done);
    assert_eq!(
        content(&server, &on(&server, "up/big")).unwrap().len(),
        8 * SMALL_CHUNK
    );
    assert!(leftovers(&server_tree(&server, "up")).is_empty());

    // Cancel, answered to the same failure part way, removes the partial file.
    server.memory().fail_nth(
        MemOp::Write,
        3,
        VfsError::Disconnected {
            location: on(&server, "up").to_location(),
        },
    );
    let mut cancel = Answers::always(Some(ConflictPolicy::KeepBoth), Some(Decision::Cancel));
    let cancelled = run(&mut h.harness, big, &mut cancel);
    assert_eq!(cancelled.state, JobState::Cancelled);
    assert_eq!(
        server_tree(&server, "up"),
        tree(&[("big", &"q".repeat(8 * SMALL_CHUNK))])
    );
}

#[test]
fn undoing_an_upload_removes_the_copy_and_refuses_once_it_changed_on_the_server() {
    let server = FakeRemoteProvider::sftp();
    server.put_dir(&on(&server, "up"));
    let (mut h, _dir) = engine(&[&server]);
    jbuild(&h, &tree(&[("a.txt", "alpha"), ("b.txt", "beta")]));

    let upload = h.run_journalled(request(
        JobKind::Copy,
        &[h.path("a.txt")],
        &on(&server, "up"),
    ));
    finished(&upload);
    let undo = h.undo_last();
    finished(&undo);
    assert!(matches!(
        server.stat(&on(&server, "up/a.txt")),
        Err(VfsError::NotFound { .. })
    ));

    let second = h.run_journalled(request(
        JobKind::Copy,
        &[h.path("b.txt")],
        &on(&server, "up"),
    ));
    finished(&second);
    // Someone changes the copy on the server: the undo leaves it.
    server.put_file(&on(&server, "up/b.txt"), b"edited on the server");
    let refused = h.undo_last();
    assert!(
        matches!(failed_with(&refused), OpsError::UndoStale { .. }),
        "{:?}",
        refused.state
    );
    assert_eq!(
        content(&server, &on(&server, "up/b.txt")).as_deref(),
        Some(&b"edited on the server"[..])
    );
}

#[test]
fn a_verified_upload_reads_the_copy_back_through_the_server() {
    let server = FakeRemoteProvider::sftp();
    server.put_dir(&on(&server, "up"));
    let (mut h, _dir) = engine(&[&server]);
    jbuild(&h, &tree(&[("a.txt", "alpha")]));
    let mut req = request(JobKind::Copy, &[h.path("a.txt")], &on(&server, "up"));
    req.options.verify = Some(true);
    let reads_before = server.memory().calls(MemOp::OpenRead);
    let run = h.run_journalled(req);
    finished(&run);
    assert_eq!(
        server.memory().calls(MemOp::OpenRead),
        reads_before + 1,
        "one read-back"
    );
    assert_eq!(
        run.report.unwrap().transfer.verified.map(|v| v.files),
        Some(1)
    );
}

#[test]
fn a_drag_out_downloads_small_files_and_refuses_folders_and_large_ones() {
    let server = FakeRemoteProvider::sftp();
    server.put_file(&on(&server, "a/x.txt"), b"one");
    server.put_file(&on(&server, "b/x.txt"), b"two");
    server.put_file(&on(&server, "big.bin"), &[0u8; 4096]);
    server.put_dir(&on(&server, "dir"));
    let (h, _dir) = engine(&[&server]);
    jbuild(&h, &tree(&[("stage/", ""), ("local.txt", "here")]));
    let providers = &h.harness.env.providers;
    let cancel = CancelToken::new();
    let stage = h.path("stage");

    let staged = stage_files(
        providers,
        &[
            on(&server, "a/x.txt").to_location(),
            on(&server, "b/x.txt").to_location(),
            h.loc("local.txt"),
        ],
        &stage,
        STAGE_LIMIT_BYTES,
        &cancel,
    )
    .unwrap();
    // Two files of one name keep both; a local file is handed back as it is.
    assert_eq!(staged[2], h.loc("local.txt"));
    assert_eq!(
        tree_of(h.provider.as_ref(), &stage),
        tree(&[("x.txt", "one"), ("x (2).txt", "two")])
    );

    let folder = stage_files(
        providers,
        &[on(&server, "dir").to_location()],
        &stage,
        STAGE_LIMIT_BYTES,
        &cancel,
    );
    assert!(matches!(folder, Err(OpsError::Unsupported { .. })));
    let large = stage_files(
        providers,
        &[on(&server, "big.bin").to_location()],
        &stage,
        1024,
        &cancel,
    );
    assert!(matches!(large, Err(OpsError::Unsupported { .. })));

    // A failure part way removes what was written.
    jbuild(&h, &tree(&[("stage2/", "")]));
    server.memory().fail_nth(
        MemOp::OpenRead,
        2,
        VfsError::Disconnected {
            location: on(&server, "b").to_location(),
        },
    );
    let failed = stage_files(
        providers,
        &[
            on(&server, "a/x.txt").to_location(),
            on(&server, "b/x.txt").to_location(),
        ],
        &h.path("stage2"),
        STAGE_LIMIT_BYTES,
        &cancel,
    );
    assert!(
        matches!(failed, Err(OpsError::Connection { .. })),
        "{failed:?}"
    );
    assert!(tree_of(h.provider.as_ref(), &h.path("stage2")).is_empty());
}

#[test]
fn a_protocol_turned_off_refuses_a_transfer_with_its_typed_reason_and_writes_nothing() {
    let server = FakeRemoteProvider::sftp();
    server.put_dir(&on(&server, "up"));
    let (mut h, _dir) = memory_jh(CaseRule::Sensitive);
    let registry = waypoint_vfs::ProviderRegistry::new();
    registry.register(Arc::new(server.clone()));
    h.harness.env.providers = h.harness.env.providers.clone().with_live(registry.clone());
    jbuild(&h, &tree(&[("a.txt", "alpha")]));
    let upload = request(JobKind::Copy, &[h.path("a.txt")], &on(&server, "up"));

    registry.turn_off("sftp").expect("the protocol was on");
    let refused = h.run_journalled(upload.clone());
    assert_eq!(
        *failed_with(&refused),
        OpsError::ProtocolOff {
            scheme: "sftp".to_owned()
        }
    );
    assert_eq!(server.connects(), 0, "nothing reached the server");
    // A download from it, and a drag out, are refused the same way.
    let down = request(JobKind::Copy, &[on(&server, "up/x")], &h.path(""));
    assert!(matches!(
        failed_with(&h.run_journalled(down)),
        OpsError::ProtocolOff { .. }
    ));
    let staged = stage_files(
        &h.harness.env.providers,
        &[on(&server, "up/x").to_location()],
        &h.path(""),
        STAGE_LIMIT_BYTES,
        &CancelToken::new(),
    );
    assert!(matches!(staged, Err(OpsError::ProtocolOff { .. })));

    // Turned on again: the same request goes through.
    registry.register(Arc::new(server.clone()));
    finished(&h.run_journalled(upload));
    assert_eq!(
        content(&server, &on(&server, "up/a.txt")).as_deref(),
        Some(&b"alpha"[..])
    );
}
