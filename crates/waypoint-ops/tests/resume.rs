// Transfers that continue after a lost connection (D62, D165): Retry and the offline wait carry on
// from what the server holds, a later run continues the partial files a failed one kept, and a
// server that cannot continue a file starts it again and says so.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use std::sync::Arc;

use common::*;
use waypoint_ops::exec::{ExecSink, Executor};
use waypoint_protocol::{Location, UnreachableReason, VfsError};
use waypoint_vfs::{CancelToken, FakeRemoteProvider, MemOp, RemoteFault};

const CHUNK: usize = 1024;
const SERVER: &str = "me@fake.test";

struct Engine {
    h: Harness<MemoryProvider>,
    server: FakeRemoteProvider,
    _dir: tempfile::TempDir,
}

fn engine() -> Engine {
    let dir = tempfile::tempdir().unwrap();
    let root = FilePath::from_path(dir.path()).unwrap();
    let mut h = Harness::new(
        MemoryProvider::new(root.clone(), CaseRule::Sensitive),
        VfsPath::File(root),
    );
    let server = FakeRemoteProvider::sftp();
    server.put_dir(&on(&server, "up"));
    h.env.providers.register(Arc::new(server.clone()));
    Engine {
        h,
        server,
        _dir: dir,
    }
}

fn on(server: &FakeRemoteProvider, relative: &str) -> VfsPath {
    relative
        .split('/')
        .filter(|s| !s.is_empty())
        .fold(server.root(SERVER), |p, name| p.join(name).unwrap())
}

/// Writes a file below the work folder, straight into the memory tree.
fn put(e: &Engine, relative: &str, content: &[u8]) {
    e.h.provider
        .inner()
        .inner()
        .put_file(&e.h.path(relative), content);
}

fn bytes(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 31 % 251) as u8).collect()
}

/// What the sink was asked and told, and how it answers.
#[derive(Default)]
struct Script {
    /// Answers to errors, in turn (the last repeats); empty answers nothing.
    decisions: Vec<Decision>,
    asked: Vec<OpsError>,
    /// How many offline waits to accept, and what to do at each.
    offline_ok: u32,
    offline: Vec<(OpsError, u32)>,
    on_offline: Option<Box<dyn FnMut(u32)>>,
    kept: Vec<ResumePoint>,
    notes: Vec<Option<PartialNote>>,
}

impl ExecSink for Script {
    fn on_error(&mut self, _: &Location, error: &OpsError) -> Option<Decision> {
        self.asked.push(error.clone());
        let at = (self.asked.len() - 1).min(self.decisions.len().saturating_sub(1));
        self.decisions.get(at).copied()
    }

    fn offline(&mut self, _: &Location, error: &OpsError, attempt: u32) -> bool {
        self.offline.push((error.clone(), attempt));
        if let Some(hook) = self.on_offline.as_mut() {
            hook(attempt);
        }
        attempt < self.offline_ok
    }

    fn kept_partial(&mut self, point: &ResumePoint) {
        self.kept.push(point.clone());
    }

    fn partial(&mut self, note: Option<&PartialNote>) {
        self.notes.push(note.cloned());
    }
}

fn run(
    e: &Engine,
    request: &JobRequest,
    sink: &mut Script,
    resume: Vec<ResumePoint>,
    verify: bool,
) -> Result<ExecReport, Box<ExecFailure>> {
    let cancel = CancelToken::new();
    let plan = e.h.plan(request).expect("the request plans");
    let options = RunOptions {
        chunk_bytes: CHUNK,
        resume,
        verify: verify.then_some(VerifyAlgorithm::Blake3),
        ..RunOptions::default()
    };
    Executor::new(e.h.env.clone()).run_with(JobId(9), &plan, &cancel, sink, options)
}

fn upload(e: &Engine, name: &str) -> JobRequest {
    let mut request = e.h.request(JobKind::Copy, &[name], None, None);
    request.destination = Some(on(&e.server, "up").to_location());
    request
}

fn on_server(e: &Engine, relative: &str) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut out = Vec::new();
    e.server
        .open_read(&on(&e.server, relative))
        .ok()?
        .read_to_end(&mut out)
        .ok()?;
    Some(out)
}

fn lose_connection_at_write(e: &Engine, n: usize) {
    e.server.memory().fail_nth(
        MemOp::Write,
        n,
        VfsError::Disconnected {
            location: on(&e.server, "up").to_location(),
        },
    );
}

fn partials(e: &Engine) -> Vec<String> {
    e.server
        .list(&on(&e.server, "up"), &CancelToken::new(), 0, &mut |_| {})
        .unwrap()
        .into_iter()
        .map(|entry| entry.name.to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".waypoint-partial-"))
        .collect()
}

#[test]
fn retry_after_a_dropped_connection_continues_without_sending_the_finished_bytes_again() {
    let e = engine();
    let content = bytes(8 * CHUNK);
    put(&e, "big.bin", &content);
    lose_connection_at_write(&e, 3);
    let mut sink = Script {
        decisions: vec![Decision::Retry],
        ..Script::default()
    };
    let report = run(&e, &upload(&e, "big.bin"), &mut sink, Vec::new(), true).unwrap();
    // Two chunks went, the third failed, and the retry sent the remaining six: none twice. (The
    // fake server rewrites what it kept in one write of its own when a file is continued.)
    assert_eq!(e.server.memory().calls(MemOp::Write), 2 + 1 + 1 + 6);
    assert_eq!(on_server(&e, "up/big.bin"), Some(content));
    assert!(partials(&e).is_empty());
    assert_eq!(
        sink.kept.len(),
        1,
        "the partial was recorded before the question"
    );
    assert_eq!(sink.kept[0].offset, 2 * CHUNK as u64);
    assert_eq!(
        sink.notes,
        [
            Some(PartialNote {
                item: e.h.loc("big.bin"),
                resumes: true,
                kept: Some(2 * CHUNK as u64),
            }),
            None
        ]
    );
    // The resumed copy was verified whole.
    assert_eq!(report.transfer.verified.map(|v| v.files), Some(1));
    assert!(report.transfer.kept.is_empty());
}

#[test]
fn the_job_waits_offline_and_carries_on_by_itself_when_the_server_is_back() {
    let e = engine();
    let content = bytes(6 * CHUNK);
    put(&e, "big.bin", &content);
    lose_connection_at_write(&e, 4);
    let server = e.server.clone();
    let mut sink = Script {
        offline_ok: 5,
        on_offline: Some(Box::new(move |attempt| {
            // The network is gone for the first wait and back for the second.
            server.set_fault(
                (attempt == 0).then_some(RemoteFault::Unreachable(UnreachableReason::Offline)),
            );
        })),
        ..Script::default()
    };
    run(&e, &upload(&e, "big.bin"), &mut sink, Vec::new(), false).unwrap();
    assert!(sink.asked.is_empty(), "nobody was asked: {:?}", sink.asked);
    let attempts: Vec<_> = sink.offline.iter().map(|(_, n)| *n).collect();
    assert_eq!(attempts, [0, 1]);
    assert!(matches!(
        sink.offline[1].0,
        OpsError::Connection {
            error: VfsError::Unreachable { .. }
        }
    ));
    assert_eq!(on_server(&e, "up/big.bin"), Some(content));
    assert_eq!(e.server.memory().calls(MemOp::Write), 3 + 1 + 1 + 3);
}

#[test]
fn after_the_offline_tries_the_person_is_asked() {
    let e = engine();
    put(&e, "a.bin", &bytes(3 * CHUNK));
    lose_connection_at_write(&e, 2);
    let server = e.server.clone();
    let mut sink = Script {
        offline_ok: 2,
        // Away for both waits, and back by the time the person is asked.
        on_offline: Some(Box::new(move |attempt| {
            server.set_fault(
                (attempt < 2).then_some(RemoteFault::Unreachable(UnreachableReason::Offline)),
            )
        })),
        decisions: vec![Decision::Cancel],
        ..Script::default()
    };
    let failure = run(&e, &upload(&e, "a.bin"), &mut sink, Vec::new(), false).unwrap_err();
    e.server.set_fault(None);
    assert_eq!(failure.error, OpsError::Cancelled);
    assert_eq!(
        sink.offline.len(),
        3,
        "two waits, then the third is refused"
    );
    assert_eq!(sink.asked.len(), 1);
    // Cancel removes what was written, the kept partial too.
    assert!(partials(&e).is_empty());
    assert!(failure.report.transfer.kept.is_empty());
}

#[test]
fn a_failed_job_hands_its_partial_files_to_the_next_run_which_continues_them() {
    let e = engine();
    let content = bytes(5 * CHUNK);
    put(&e, "big.bin", &content);
    put(&e, "small.txt", b"small");
    lose_connection_at_write(&e, 3);
    // Nobody answers: the job fails at the item, keeping the partial.
    let mut first = Script::default();
    let failure = run(
        &e,
        &e.h.request(JobKind::Copy, &["big.bin", "small.txt"], None, None)
            .tap(|r| r.destination = Some(on(&e.server, "up").to_location())),
        &mut first,
        Vec::new(),
        false,
    )
    .unwrap_err();
    let kept = failure.report.transfer.kept.clone();
    assert_eq!(kept.len(), 1);
    assert_eq!(partials(&e).len(), 1);

    let writes = e.server.memory().calls(MemOp::Write);
    let mut second = Script::default();
    let request =
        e.h.request(JobKind::Copy, &["big.bin", "small.txt"], None, None)
            .tap(|r| r.destination = Some(on(&e.server, "up").to_location()));
    run(&e, &request, &mut second, kept, false).unwrap();
    assert_eq!(on_server(&e, "up/big.bin"), Some(content));
    assert_eq!(
        on_server(&e, "up/small.txt").as_deref(),
        Some(&b"small"[..])
    );
    // Three chunks of the big file (and the fake's own rewrite of what it kept) and the small one:
    // the two kept chunks were not sent again.
    assert_eq!(e.server.memory().calls(MemOp::Write) - writes, 1 + 3 + 1);
    assert!(partials(&e).is_empty());
}

#[test]
fn a_source_that_changed_or_a_server_that_cannot_continue_starts_the_file_again() {
    let e = engine();
    put(&e, "a.bin", &bytes(4 * CHUNK));
    lose_connection_at_write(&e, 2);
    let failure = run(
        &e,
        &upload(&e, "a.bin"),
        &mut Script::default(),
        Vec::new(),
        false,
    )
    .unwrap_err();
    let kept = failure.report.transfer.kept.clone();
    assert_eq!(kept.len(), 1);
    // The source changes: the kept partial is not continued but replaced.
    let changed = bytes(4 * CHUNK + 7);
    put(&e, "a.bin", &changed);
    run(
        &e,
        &upload(&e, "a.bin"),
        &mut Script::default(),
        kept,
        false,
    )
    .unwrap();
    assert_eq!(on_server(&e, "up/a.bin"), Some(changed));
    assert!(partials(&e).is_empty());

    // A server that cannot continue a file: Retry starts it again and the note says so.
    e.server.set_capabilities(|caps| caps.resume_write = false);
    put(&e, "b.bin", &bytes(4 * CHUNK));
    lose_connection_at_write(&e, 2);
    let mut sink = Script {
        decisions: vec![Decision::Retry],
        ..Script::default()
    };
    let writes = e.server.memory().calls(MemOp::Write);
    run(&e, &upload(&e, "b.bin"), &mut sink, Vec::new(), false).unwrap();
    assert_eq!(e.server.memory().calls(MemOp::Write) - writes, 2 + 4);
    assert!(sink.kept.is_empty());
    assert_eq!(sink.notes[0].as_ref().map(|n| n.resumes), Some(false));
    assert!(partials(&e).is_empty());
}

trait Tap: Sized {
    fn tap(self, f: impl FnOnce(&mut Self)) -> Self;
}

impl<T> Tap for T {
    fn tap(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}

#[test]
fn a_kept_partial_survives_a_restart_and_is_offered_never_resumed() {
    use waypoint_ops::testing::journal_harness::JournalHarness;
    let dir = tempfile::tempdir().unwrap();
    let root = FilePath::from_path(dir.path()).unwrap();
    let mut h = JournalHarness::new(
        MemoryProvider::new(root.clone(), CaseRule::Sensitive),
        VfsPath::File(root),
    );
    let server = FakeRemoteProvider::sftp();
    server.put_dir(&on(&server, "up"));
    h.harness.env.providers.register(Arc::new(server.clone()));
    let memory = h.harness.provider.inner().inner().clone();
    memory.put_file(&h.path("src.bin"), &bytes(10));
    // A job wrote two partial files here; one of them is kept for resuming.
    memory.put_file(&h.path(".waypoint-partial-7-1-kept.bin"), b"kept");
    memory.put_file(&h.path(".waypoint-partial-7-2-other.bin"), b"junk");
    let request = h
        .harness
        .request(JobKind::Copy, &["src.bin"], Some(""), None);
    h.journal
        .begin(PendingRecord {
            job: JobId(7),
            at_ms: 1,
            kind: JobKind::Copy,
            label: "Copy src.bin".to_owned(),
            items: vec![h.loc("src.bin")],
            folders: vec![h.loc(""), on(&server, "up").to_location()],
            renames: Vec::new(),
        })
        .unwrap();
    let point = ResumePoint {
        source: h.loc("src.bin"),
        target: h.loc("kept.bin"),
        partial: h.loc(".waypoint-partial-7-1-kept.bin"),
        source_size: Some(10),
        source_modified_ms: None,
        offset: 4,
    };
    h.journal
        .keep_partial(JobId(7), "Copy src.bin", 1, &request, point.clone())
        .unwrap();

    let report = h.restart();
    let job = &report.interrupted[0];
    assert_eq!(job.removed, [h.loc(".waypoint-partial-7-2-other.bin")]);
    // A server's folder is not looked at during start-up: it is reported, not touched.
    assert_eq!(job.left, [on(&server, "up").to_location()]);
    assert_eq!(server.connects(), 0, "start-up never connects to a server");
    assert!(memory
        .stat(&h.path(".waypoint-partial-7-1-kept.bin"))
        .is_ok());
    assert!(report.needs_notice());
    assert_eq!(report.resumable.len(), 1);
    assert_eq!(report.resumable[0].points, [point]);
    assert_eq!(report.resumable[0].request, request);
    // Taking it hands it over once, and it is gone after the next restart.
    assert!(h.journal.take_resumable(JobId(7)).is_some());
    assert!(h.journal.resumable().is_empty());
}
