// The operations engine over the in-process SFTP server and a real OpenSSH server (A84, D151): an
// upload and a download of a tree with its times and modes, a move within one login by rename, the
// undo of an upload, and an upload cut part way. The OpenSSH half of each test skips with a message
// when there is no `sshd` (see `support`).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[macro_use]
mod support;

use std::fs;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use support::{temp_dir, Backend};
use waypoint_ops::testing::journal_harness::JournalHarness;
use waypoint_ops::{JobKind, JobOptions, JobRequest, JobState, OpsError, Sources};
use waypoint_path::{FilePath, VfsPath};
use waypoint_vfs::{LocalProvider, Provider};

struct Engine {
    h: JournalHarness<LocalProvider>,
    /// The local work folder, on this machine.
    work: std::path::PathBuf,
    _dir: tempfile::TempDir,
}

fn engine(server: &dyn Backend) -> Engine {
    let dir = temp_dir();
    let base = VfsPath::File(FilePath::from_path(dir.path()).unwrap());
    let mut h = JournalHarness::new(LocalProvider::new(), base);
    h.harness
        .env
        .providers
        .register(Arc::new(server.provider()));
    let work = dir.path().join("work");
    Engine { h, work, _dir: dir }
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

fn done(state: &JobState) {
    assert_eq!(*state, JobState::Done, "{state:?}");
}

fn mtime(path: &std::path::Path) -> u64 {
    fs::metadata(path)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn a_tree_goes_up_and_comes_back_with_its_bytes_times_and_modes(server: &dyn Backend) {
    let mut e = engine(server);
    let src = e.work.join("tree");
    fs::create_dir_all(src.join("sub")).unwrap();
    let big: Vec<u8> = (0..3_000_000u32).map(|n| (n * 7 % 253) as u8).collect();
    fs::write(src.join("big.bin"), &big).unwrap();
    fs::write(src.join("sub/note.txt"), b"note").unwrap();
    let old = UNIX_EPOCH + Duration::from_secs(1_500_000_000);
    fs::File::options()
        .write(true)
        .open(src.join("sub/note.txt"))
        .unwrap()
        .set_modified(old)
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(src.join("big.bin"), fs::Permissions::from_mode(0o640)).unwrap();
    }

    let up = server.data_location();
    let run =
        e.h.run_journalled(request(JobKind::Copy, &[e.h.path("tree")], &up));
    done(&run.state);
    let plan = run.plan.unwrap();
    assert!(!plan.same_volume);
    assert!(plan.ends.to.unwrap().starts_with("sftp://"));
    assert_eq!(server.get("tree/big.bin"), big);
    assert_eq!(server.mtime("tree/sub/note.txt"), 1_500_000_000);
    // Only a Unix machine gave the source its mode.
    #[cfg(unix)]
    assert_eq!(server.mode("tree/big.bin") & 0o777, 0o640);
    assert_eq!(
        server.names(""),
        ["tree"],
        "no partial is left on the server"
    );

    // And back down, verified.
    fs::create_dir_all(e.work.join("back")).unwrap();
    let mut down = request(
        JobKind::Copy,
        &[up.join("tree").unwrap()],
        &e.h.path("back"),
    );
    down.options.verify = Some(true);
    let run = e.h.run_journalled(down);
    done(&run.state);
    assert_eq!(fs::read(e.work.join("back/tree/big.bin")).unwrap(), big);
    assert_eq!(mtime(&e.work.join("back/tree/sub/note.txt")), 1_500_000_000);
}

fn a_move_within_one_login_renames_on_the_server(server: &dyn Backend) {
    let mut e = engine(server);
    server.mkdir("a");
    server.mkdir("b");
    server.put("a/f.txt", b"moved");
    let identity = server.identity("a/f.txt");
    let root = server.data_location();
    let run = e.h.run_journalled(request(
        JobKind::Move,
        &[root.join("a/f.txt").unwrap()],
        &root.join("b").unwrap(),
    ));
    done(&run.state);
    assert!(run.plan.unwrap().same_volume, "one login is one volume");
    assert!(!server.exists("a/f.txt"));
    assert_eq!(server.get("b/f.txt"), b"moved");
    assert_eq!(server.identity("b/f.txt"), identity, "renamed, not copied");
}

fn undoing_an_upload_removes_it_and_refuses_once_it_changed(server: &dyn Backend) {
    let mut e = engine(server);
    fs::create_dir_all(&e.work).unwrap();
    fs::write(e.work.join("a.txt"), b"alpha").unwrap();
    let up = server.data_location();
    let run =
        e.h.run_journalled(request(JobKind::Copy, &[e.h.path("a.txt")], &up));
    done(&run.state);
    let undo = e.h.undo_last();
    done(&undo.state);
    assert!(!server.exists("a.txt"));

    let redo =
        e.h.run_journalled(request(JobKind::Copy, &[e.h.path("a.txt")], &up));
    done(&redo.state);
    server.put("a.txt", b"changed on the server");
    let refused = e.h.undo_last();
    assert!(
        matches!(
            refused.state,
            JobState::Failed {
                error: OpsError::UndoStale { .. },
                ..
            }
        ),
        "{:?}",
        refused.state
    );
    assert_eq!(server.get("a.txt"), b"changed on the server");
    let _ = e.h.provider.stat(&e.h.path("a.txt")).unwrap();
}

/// Cuts the connection once the copy has sent `after` bytes, tries again at once when it is lost,
/// and notes what the engine kept.
struct Severing<'a> {
    proxy: &'a support::Proxy,
    after: u64,
    severed: bool,
    offline: u32,
    kept: Vec<waypoint_ops::ResumePoint>,
    asked: Vec<OpsError>,
}

impl waypoint_ops::ExecSink for Severing<'_> {
    fn progress(&mut self, progress: &waypoint_ops::Progress, _: &waypoint_ops::Counts) {
        if !self.severed && progress.bytes_done >= self.after {
            self.severed = true;
            self.proxy.sever();
        }
    }

    fn offline(&mut self, _: &waypoint_protocol::Location, _: &OpsError, attempt: u32) -> bool {
        self.offline += 1;
        attempt < 3
    }

    fn kept_partial(&mut self, point: &waypoint_ops::ResumePoint) {
        self.kept.push(point.clone());
    }

    fn on_error(
        &mut self,
        _: &waypoint_protocol::Location,
        error: &OpsError,
    ) -> Option<waypoint_ops::Decision> {
        self.asked.push(error.clone());
        None
    }
}

fn an_upload_cut_part_way_continues_from_what_the_server_holds(server: &dyn Backend) {
    let proxy = server.proxy(Duration::from_millis(4));
    let dir = temp_dir();
    let base = VfsPath::File(FilePath::from_path(dir.path()).unwrap());
    let mut h = waypoint_ops::testing::harness::Harness::new(LocalProvider::new(), base);
    h.env.providers.register(Arc::new(
        server.provider_with(proxy.port, Default::default()),
    ));
    let work = dir.path().join("work");
    let content: Vec<u8> = (0..12_000_000u32).map(|n| (n * 17 % 241) as u8).collect();
    fs::write(work.join("big.bin"), &content).unwrap();
    let up = server.location(proxy.port, "");
    let request = request(JobKind::Copy, &[h.path("big.bin")], &up);
    let plan = h.plan(&request).unwrap();
    let mut sink = Severing {
        proxy: &proxy,
        after: 4 * 1024 * 1024,
        severed: false,
        offline: 0,
        kept: Vec::new(),
        asked: Vec::new(),
    };
    let options = waypoint_ops::RunOptions {
        chunk_bytes: 1024 * 1024,
        verify: Some(waypoint_ops::VerifyAlgorithm::Blake3),
        ..Default::default()
    };
    let result = waypoint_ops::Executor::new(h.env.clone()).run_with(
        waypoint_ops::JobId(1),
        &plan,
        &waypoint_vfs::CancelToken::new(),
        &mut sink,
        options,
    );
    assert!(
        result.is_ok(),
        "{:?}; asked {:?}",
        result.err().map(|f| f.error),
        sink.asked
    );
    assert!(sink.severed);
    assert!(sink.offline >= 1, "the cut was waited out");
    assert_eq!(sink.kept.len(), 1, "the partial file was kept");
    assert_eq!(server.get("big.bin"), content);
    assert_eq!(server.names(""), ["big.bin"]);
    assert!(proxy.connections() >= 2, "it connected again");
}

on_both!(
    a_tree_goes_up_and_comes_back_with_its_bytes_times_and_modes,
    a_move_within_one_login_renames_on_the_server,
    undoing_an_upload_removes_it_and_refuses_once_it_changed,
    an_upload_cut_part_way_continues_from_what_the_server_holds,
);
