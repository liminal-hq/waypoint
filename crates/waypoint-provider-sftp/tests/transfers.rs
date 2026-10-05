// The operations engine over a real OpenSSH server (A84, D151): an upload and a download of a tree
// with its times and modes, a move within one login by rename, and the undo of an upload. Each
// test skips with a message when there is no `sshd` (see `support`).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::fs;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use support::{temp_dir, Sshd};
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

fn engine(server: &Sshd) -> Engine {
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

#[test]
fn a_tree_goes_up_and_comes_back_with_its_bytes_times_and_modes() {
    let Some(server) = Sshd::start() else { return };
    let mut e = engine(&server);
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
    assert_eq!(fs::read(server.data.join("tree/big.bin")).unwrap(), big);
    assert_eq!(mtime(&server.data.join("tree/sub/note.txt")), 1_500_000_000);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(server.data.join("tree/big.bin"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o640);
    }
    let names: Vec<_> = fs::read_dir(&server.data)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["tree"], "no partial is left on the server");

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

#[test]
fn a_move_within_one_login_renames_on_the_server() {
    let Some(server) = Sshd::start() else { return };
    let mut e = engine(&server);
    fs::create_dir_all(server.data.join("a")).unwrap();
    fs::create_dir_all(server.data.join("b")).unwrap();
    fs::write(server.data.join("a/f.txt"), b"moved").unwrap();
    let inode = {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(server.data.join("a/f.txt")).unwrap().ino()
        }
        #[cfg(not(unix))]
        0
    };
    let root = server.data_location();
    let run = e.h.run_journalled(request(
        JobKind::Move,
        &[root.join("a/f.txt").unwrap()],
        &root.join("b").unwrap(),
    ));
    done(&run.state);
    assert!(run.plan.unwrap().same_volume, "one login is one volume");
    assert!(!server.data.join("a/f.txt").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let moved = fs::metadata(server.data.join("b/f.txt")).unwrap().ino();
        assert_eq!(moved, inode, "renamed, not copied");
    }
    let _ = inode;
}

#[test]
fn undoing_an_upload_removes_it_and_refuses_once_it_changed() {
    let Some(server) = Sshd::start() else { return };
    let mut e = engine(&server);
    fs::create_dir_all(&e.work).unwrap();
    fs::write(e.work.join("a.txt"), b"alpha").unwrap();
    let up = server.data_location();
    let run =
        e.h.run_journalled(request(JobKind::Copy, &[e.h.path("a.txt")], &up));
    done(&run.state);
    let undo = e.h.undo_last();
    done(&undo.state);
    assert!(!server.data.join("a.txt").exists());

    let redo =
        e.h.run_journalled(request(JobKind::Copy, &[e.h.path("a.txt")], &up));
    done(&redo.state);
    fs::write(server.data.join("a.txt"), b"changed on the server").unwrap();
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
    assert_eq!(
        fs::read(server.data.join("a.txt")).unwrap(),
        b"changed on the server"
    );
    let _ = e.h.provider.stat(&e.h.path("a.txt")).unwrap();
}
