// Headless tests of the recursive folder size: totals, skipped links and mounts, cancel latency.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(unix)]

use std::fs;
use std::path::Path;

use waypoint_path::{CaseRule, FilePath, VfsPath};
use waypoint_protocol::VfsError;
use waypoint_vfs::{
    CancelToken, FolderSizeTotals, LocalProvider, MemoryProvider, Provider, VolumeId,
};

fn path(p: &Path) -> VfsPath {
    VfsPath::File(FilePath::from_path(p).unwrap())
}

fn size(provider: &dyn Provider, p: &Path) -> Result<waypoint_vfs::FolderSizeRun, VfsError> {
    provider.folder_size(&path(p), &CancelToken::new(), &mut |_| {})
}

fn write(dir: &Path, name: &str, bytes: usize) {
    fs::write(dir.join(name), vec![b'x'; bytes]).unwrap();
}

#[test]
fn it_adds_up_files_in_nested_folders() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a", 100);
    fs::create_dir_all(dir.path().join("sub/deeper")).unwrap();
    write(&dir.path().join("sub"), "b", 2000);
    write(&dir.path().join("sub/deeper"), "c", 30);
    fs::create_dir(dir.path().join("empty")).unwrap();
    let run = size(&LocalProvider::new(), dir.path()).unwrap();
    assert!(!run.cancelled);
    assert_eq!(run.totals.files, 3);
    assert_eq!(run.totals.folders, 3);
    assert_eq!(run.totals.bytes, 2130);
    assert!(run.totals.allocated_bytes.unwrap() >= 2130);
    assert_eq!(run.totals.unreadable, 0);
}

#[test]
fn symlinks_are_skipped_never_followed() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "big", 5000);
    let tree = dir.path().join("tree");
    fs::create_dir(&tree).unwrap();
    write(&tree, "small", 10);
    std::os::unix::fs::symlink(outside.path(), tree.join("to-folder")).unwrap();
    std::os::unix::fs::symlink(outside.path().join("big"), tree.join("to-file")).unwrap();
    // A loop back to the root must not make the walk run forever.
    std::os::unix::fs::symlink(&tree, tree.join("loop")).unwrap();
    let run = size(&LocalProvider::new(), &tree).unwrap();
    assert_eq!(run.totals.bytes, 10);
    assert_eq!(run.totals.files, 1);
    assert_eq!(run.totals.symlinks_skipped, 3);
}

#[test]
fn a_link_to_a_folder_as_the_root_measures_what_it_shows() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    fs::create_dir(&real).unwrap();
    write(&real, "f", 77);
    std::os::unix::fs::symlink(&real, dir.path().join("link")).unwrap();
    let run = size(&LocalProvider::new(), &dir.path().join("link")).unwrap();
    assert_eq!(run.totals.bytes, 77);
}

#[test]
fn a_hard_linked_file_counts_once() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "one", 400);
    fs::hard_link(dir.path().join("one"), dir.path().join("two")).unwrap();
    let run = size(&LocalProvider::new(), dir.path()).unwrap();
    assert_eq!(run.totals.files, 2);
    assert_eq!(run.totals.bytes, 400);
}

#[test]
fn an_unreadable_subfolder_is_counted_and_the_rest_still_adds_up() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ok", 5);
    let locked = dir.path().join("locked");
    fs::create_dir(&locked).unwrap();
    write(&locked, "hidden", 999);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let readable_anyway = fs::read_dir(&locked).is_ok(); // running as root
    let run = size(&LocalProvider::new(), dir.path()).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
    if !readable_anyway {
        assert_eq!(run.totals.unreadable, 1);
        assert_eq!(run.totals.bytes, 5);
    }
}

#[test]
fn a_missing_or_file_root_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let provider = LocalProvider::new();
    assert!(matches!(
        size(&provider, &dir.path().join("gone")),
        Err(VfsError::NotFound { .. })
    ));
    write(dir.path(), "f", 1);
    assert!(matches!(
        size(&provider, &dir.path().join("f")),
        Err(VfsError::NotADirectory { .. })
    ));
}

#[test]
fn a_mount_point_is_not_crossed() {
    // `/dev` holds mounts of other file systems (`/dev/shm`, `/dev/pts`) on every ordinary Linux
    // system; a machine without one has nothing to check.
    use std::os::unix::fs::MetadataExt;
    let dev = fs::metadata("/dev").map(|m| m.dev()).ok();
    let crosses = ["/dev/shm", "/dev/pts"].iter().any(|p| {
        fs::metadata(p)
            .map(|m| Some(m.dev()) != dev)
            .unwrap_or(false)
    });
    if !crosses {
        return;
    }
    let run = size(&LocalProvider::new(), Path::new("/dev")).unwrap();
    assert!(run.totals.mounts_skipped >= 1, "{:?}", run.totals);
}

#[test]
fn a_walk_cancelled_before_it_starts_returns_nothing_counted() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "f", 10);
    let cancel = CancelToken::new();
    cancel.cancel();
    let run = LocalProvider::new()
        .folder_size(&path(dir.path()), &cancel, &mut |_| {})
        .unwrap();
    assert!(run.cancelled);
    assert_eq!(
        run.totals,
        FolderSizeTotals {
            allocated_bytes: Some(0),
            ..FolderSizeTotals::default()
        }
    );
}

#[test]
fn a_listing_only_provider_is_walked_through_list_and_stops_at_another_volume() {
    let provider = MemoryProvider::new(FilePath::parse("/").unwrap(), CaseRule::NATIVE);
    let p = |s: &str| VfsPath::File(FilePath::parse(s).unwrap());
    provider.put_dir(&p("/root"));
    provider.put_dir(&p("/root/in"));
    provider.put_dir(&p("/root/other"));
    provider.put_file(&p("/root/a"), &[0; 10]);
    provider.put_file(&p("/root/in/b"), &[0; 20]);
    provider.put_file(&p("/root/other/c"), &[0; 1000]);
    provider.set_volume(&p("/root"), VolumeId(1));
    provider.set_volume(&p("/root/in"), VolumeId(1));
    provider.set_volume(&p("/root/other"), VolumeId(2));
    let run = provider
        .folder_size(&p("/root"), &CancelToken::new(), &mut |_| {})
        .unwrap();
    assert_eq!(run.totals.bytes, 30);
    assert_eq!(run.totals.files, 2);
    assert_eq!(run.totals.folders, 1);
    assert_eq!(run.totals.mounts_skipped, 1);
    assert_eq!(run.totals.allocated_bytes, None);
}
