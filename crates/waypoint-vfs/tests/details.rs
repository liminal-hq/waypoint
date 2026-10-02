// Headless tests of entry details over temporary directories and the in-memory provider.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(unix)]
use std::fs;
use std::path::Path;

use waypoint_path::{FilePath, VfsPath};
use waypoint_protocol::VfsError;
#[cfg(unix)]
use waypoint_vfs::EntryKind;
use waypoint_vfs::{DetailField, LocalProvider, MemoryProvider, Provider};

fn path(p: &Path) -> VfsPath {
    VfsPath::File(FilePath::from_path(p).unwrap())
}

#[cfg(unix)]
#[test]
fn a_local_file_reports_everything_a_local_provider_can_read() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.md");
    fs::write(&file, "# hello\n").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
    let details = LocalProvider::new().details(&path(&file)).unwrap();
    assert_eq!(details.name, "notes.md");
    assert_eq!(details.kind, EntryKind::File);
    assert_eq!(details.size, Some(8));
    assert!(details.allocated_size.is_some());
    assert!(details.modified_ms.is_some() && details.accessed_ms.is_some());
    assert_eq!(details.mode, Some(0o640));
    assert!(!details.read_only);
    assert!(!details.hidden);
    assert!(details.owner.is_some() && details.group.is_some());
    assert_eq!(details.mime_type.as_deref(), Some("text/markdown"));
    assert!(!details.unavailable.contains(&DetailField::Owner));
}

#[cfg(unix)]
#[test]
fn the_content_sniff_overrides_a_misleading_name_and_finds_text_without_one() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("picture.txt");
    fs::write(&png, b"\x89PNG\r\n\x1a\n0000").unwrap();
    let plain = dir.path().join("LICENCE");
    fs::write(&plain, "Permission is hereby granted\n").unwrap();
    let provider = LocalProvider::new();
    assert_eq!(
        provider.details(&path(&png)).unwrap().mime_type.as_deref(),
        Some("image/png")
    );
    assert_eq!(
        provider
            .details(&path(&plain))
            .unwrap()
            .mime_type
            .as_deref(),
        Some("text/plain")
    );
}

#[cfg(unix)]
#[test]
fn a_symlink_reports_its_text_and_its_targets_size_and_a_broken_one_just_its_text() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("real.txt"), "12345").unwrap();
    std::os::unix::fs::symlink("real.txt", dir.path().join("link")).unwrap();
    std::os::unix::fs::symlink("nowhere", dir.path().join("dangling")).unwrap();
    let provider = LocalProvider::new();
    let link = provider.details(&path(&dir.path().join("link"))).unwrap();
    assert_eq!(link.kind, EntryKind::Symlink);
    assert_eq!(link.resolves_to, Some(EntryKind::File));
    assert_eq!(link.symlink_target.as_deref(), Some("real.txt"));
    assert_eq!(link.size, Some(5));
    let dangling = provider
        .details(&path(&dir.path().join("dangling")))
        .unwrap();
    assert_eq!(dangling.resolves_to, None);
    assert_eq!(dangling.symlink_target.as_deref(), Some("nowhere"));
    assert_eq!(dangling.size, None);
}

#[cfg(unix)]
#[test]
fn a_folder_has_no_size_and_a_hidden_name_is_hidden() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join(".cache")).unwrap();
    let details = LocalProvider::new()
        .details(&path(&dir.path().join(".cache")))
        .unwrap();
    assert_eq!(details.kind, EntryKind::Directory);
    assert_eq!(details.size, None);
    assert!(details.hidden);
    assert_eq!(details.mime_type.as_deref(), Some("inode/directory"));
}

#[cfg(unix)]
#[test]
fn a_pipe_is_described_without_being_opened() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("pipe");
    let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    // SAFETY: `c` is a valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    let details = LocalProvider::new().details(&path(&fifo)).unwrap();
    assert_eq!(details.kind, EntryKind::Other);
    assert_eq!(details.mime_type, None);
}

#[test]
fn a_missing_entry_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let error = LocalProvider::new()
        .details(&path(&dir.path().join("gone")))
        .unwrap_err();
    assert!(matches!(error, VfsError::NotFound { .. }));
}

#[test]
fn a_provider_that_only_lists_marks_the_local_only_fields_unavailable() {
    // The provider never touches the disk; a real directory only gives it a root that is absolute on
    // every platform (`/` is not on Windows).
    let dir = tempfile::tempdir().unwrap();
    let root = FilePath::from_path(dir.path()).unwrap();
    let file = VfsPath::File(FilePath::from_path(&dir.path().join("photo.jpg")).unwrap());
    let provider = MemoryProvider::new(root, waypoint_path::CaseRule::NATIVE);
    provider.put_file(&file, b"abc");
    let details = provider.details(&file).unwrap();
    assert_eq!(details.size, Some(3));
    assert_eq!(details.mime_type.as_deref(), Some("image/jpeg"));
    assert_eq!(details.owner, None);
    for field in [
        DetailField::AllocatedSize,
        DetailField::Created,
        DetailField::Accessed,
        DetailField::Owner,
        DetailField::Group,
        DetailField::Permissions,
    ] {
        assert!(details.unavailable.contains(&field), "{field:?}");
    }
}
