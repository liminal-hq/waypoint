// A checksum of a file on disk: the real local provider hashes a file, refuses a folder and a link to nowhere, and reports a missing file
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_ops::{run_checksum, ChecksumEvent, VerifyAlgorithm};
use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;
use waypoint_vfs::{CancelToken, LocalProvider};

fn file_with(contents: &[u8]) -> (tempfile::TempDir, VfsPath) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("data.bin");
    std::fs::write(&file, contents).unwrap();
    let path = VfsPath::from(waypoint_path::FilePath::from_path(file).unwrap());
    (dir, path)
}

#[test]
fn a_file_on_disk_is_hashed_and_ends_in_done() {
    let (_dir, path) = file_with(b"abc");
    let mut events = Vec::new();
    run_checksum(
        &LocalProvider::new(),
        &path,
        VerifyAlgorithm::Sha256,
        &CancelToken::new(),
        &mut |event| events.push(event),
    );
    assert_eq!(
        events.last(),
        Some(&ChecksumEvent::Done {
            algorithm: VerifyAlgorithm::Sha256,
            digest: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
            bytes: 3,
        })
    );
}

#[test]
fn a_folder_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = VfsPath::from(waypoint_path::FilePath::from_path(dir.path()).unwrap());
    let mut events = Vec::new();
    run_checksum(
        &LocalProvider::new(),
        &path,
        VerifyAlgorithm::Sha256,
        &CancelToken::new(),
        &mut |event| events.push(event),
    );
    assert!(matches!(
        events.as_slice(),
        [ChecksumEvent::Failed {
            error: VfsError::IsADirectory { .. }
        }]
    ));
}

#[cfg(unix)]
#[test]
fn a_link_to_a_file_is_hashed_and_one_to_nowhere_is_refused() {
    let (dir, _) = file_with(b"abc");
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(dir.path().join("data.bin"), &link).unwrap();
    let broken = dir.path().join("broken");
    std::os::unix::fs::symlink(dir.path().join("nowhere"), &broken).unwrap();
    let run = |path: std::path::PathBuf| {
        let mut events = Vec::new();
        run_checksum(
            &LocalProvider::new(),
            &VfsPath::from(waypoint_path::FilePath::from_path(path).unwrap()),
            VerifyAlgorithm::Sha256,
            &CancelToken::new(),
            &mut |event| events.push(event),
        );
        events
    };
    assert!(matches!(
        run(link).last(),
        Some(ChecksumEvent::Done { bytes: 3, .. })
    ));
    assert!(matches!(
        run(broken).last(),
        Some(ChecksumEvent::Failed { .. })
    ));
}

#[test]
fn a_missing_file_fails() {
    let dir = tempfile::tempdir().unwrap();
    let path = VfsPath::from(waypoint_path::FilePath::from_path(dir.path().join("gone")).unwrap());
    let mut events = Vec::new();
    run_checksum(
        &LocalProvider::new(),
        &path,
        VerifyAlgorithm::Blake3,
        &CancelToken::new(),
        &mut |event| events.push(event),
    );
    assert!(matches!(
        events.as_slice(),
        [ChecksumEvent::Failed {
            error: VfsError::NotFound { .. }
        }]
    ));
}
