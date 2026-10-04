// The SFTP provider's write primitives against a real OpenSSH server: streams large and resumed,
// the partial-name-and-rename upload the copy engine does, links, times, permissions and the typed
// errors SFTP version 3 only reports as generic failures. Each test skips with a message when
// there is no `sshd` (see `support`).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::fs;
use std::io::{Read, Write};
use std::time::{Duration, UNIX_EPOCH};

use support::Sshd;
use waypoint_protocol::VfsError;
use waypoint_provider_sftp::{AgentSource, SftpConfig, SftpProvider};
use waypoint_vfs::{CancelToken, EntryKind, FileTimes, Permissions, Provider, WriteOptions};

fn read_all(provider: &SftpProvider, path: &waypoint_path::VfsPath) -> Vec<u8> {
    let mut out = Vec::new();
    provider
        .open_read(path)
        .unwrap()
        .read_to_end(&mut out)
        .unwrap();
    out
}

#[test]
fn streams_write_large_files_and_resume_partial_ones() {
    let Some(server) = Sshd::start() else { return };
    let provider = server.provider();
    let root = server.data_location();
    let file = root.join("big.bin").unwrap();
    let bytes: Vec<u8> = (0..5_000_000u32).map(|n| (n * 13 % 251) as u8).collect();
    let mut stream = provider
        .create_write(&file, WriteOptions::exclusive())
        .unwrap();
    // Odd-sized writes cross the chunk boundaries.
    for piece in bytes.chunks(10_007) {
        stream.write_all(piece).unwrap();
    }
    stream.finish(true).unwrap();
    assert_eq!(fs::read(server.data.join("big.bin")).unwrap(), bytes);
    assert_eq!(provider.stat(&file).unwrap().size, Some(5_000_000));

    // Truncating replaces; exclusive refuses.
    let mut again = provider
        .create_write(&file, WriteOptions::truncate())
        .unwrap();
    again.write_all(b"short").unwrap();
    again.finish(false).unwrap();
    assert_eq!(read_all(&provider, &file), b"short");
    assert!(matches!(
        provider
            .create_write(&file, WriteOptions::exclusive())
            .map(|_| ()),
        Err(VfsError::AlreadyExists { .. })
    ));

    // A resumed write drops what is past the offset and goes on from there.
    let mut resumed = provider.resume_write(&file, 3).unwrap();
    resumed.write_all(b"ORE").unwrap();
    resumed.finish(true).unwrap();
    assert_eq!(read_all(&provider, &file), b"shoORE");
    assert!(matches!(
        provider
            .resume_write(&root.join("none").unwrap(), 0)
            .map(|_| ()),
        Err(VfsError::NotFound { .. })
    ));
    // A partial file shorter than the offset is refused, never padded with zeros.
    assert!(matches!(
        provider.resume_write(&file, 10).map(|_| ()),
        Err(VfsError::Io { .. })
    ));
    assert_eq!(read_all(&provider, &file), b"shoORE");

    // A stream dropped without finishing closes the file and leaves the session usable.
    let mut dropped = provider
        .create_write(&root.join("dropped").unwrap(), WriteOptions::exclusive())
        .unwrap();
    dropped.write_all(&[1; 100_000]).unwrap();
    drop(dropped);
    assert_eq!(provider.stat(&root).unwrap().kind, EntryKind::Directory);

    // A mode is given to a new file.
    let private = root.join("private").unwrap();
    let options = WriteOptions {
        exclusive: true,
        mode: Some(0o600),
    };
    provider
        .create_write(&private, options)
        .unwrap()
        .finish(false)
        .unwrap();
    assert_eq!(provider.permissions(&private).unwrap().mode, Some(0o600));
}

#[test]
fn an_upload_lands_through_a_partial_name_and_a_rename() {
    let Some(server) = Sshd::start() else { return };
    let provider = server.provider();
    let root = server.data_location();
    let target = root.join("report.pdf").unwrap();
    fs::write(server.data.join("report.pdf"), "old").unwrap();
    let partial = root.join(".waypoint-partial-1-0-report.pdf").unwrap();
    let mut stream = provider
        .create_write(&partial, WriteOptions::exclusive())
        .unwrap();
    stream.write_all(b"new contents").unwrap();
    stream.finish(true).unwrap();
    // Until the rename the old file is whole.
    assert_eq!(read_all(&provider, &target), b"old");
    assert!(matches!(
        provider.rename(&partial, &target, false),
        Err(VfsError::AlreadyExists { .. })
    ));
    provider.rename(&partial, &target, true).unwrap();
    assert_eq!(read_all(&provider, &target), b"new contents");
    assert!(matches!(
        provider.stat(&partial),
        Err(VfsError::NotFound { .. })
    ));
    let listed = provider
        .list(&root, &CancelToken::new(), 0, &mut |_| {})
        .unwrap();
    assert_eq!(listed.len(), 1);
}

#[test]
fn links_times_permissions_and_space() {
    let Some(server) = Sshd::start() else { return };
    let provider = server.provider();
    let root = server.data_location();
    fs::write(server.data.join("a.txt"), "x").unwrap();
    fs::create_dir(server.data.join("dir")).unwrap();

    // The link holds the text it was given, whichever order the server wants the paths in.
    let link = root.join("link").unwrap();
    provider.symlink(&link, "a.txt".as_ref()).unwrap();
    assert_eq!(
        fs::read_link(server.data.join("link")).unwrap(),
        std::path::PathBuf::from("a.txt")
    );
    assert_eq!(provider.read_link(&link).unwrap(), "a.txt");
    assert_eq!(
        provider.stat(&link).unwrap().link_target,
        Some(EntryKind::File)
    );
    assert!(matches!(
        provider.symlink(&link, "dir".as_ref()),
        Err(VfsError::AlreadyExists { .. })
    ));
    let to_dir = root.join("to-dir").unwrap();
    provider.symlink(&to_dir, "dir".as_ref()).unwrap();
    assert!(matches!(
        provider.remove_dir(&to_dir),
        Err(VfsError::NotADirectory { .. })
    ));
    provider.remove_file(&to_dir).unwrap();
    assert!(
        server.data.join("dir").is_dir(),
        "a link is removed, never its target"
    );

    // Times: the modification time reads back; a link's own times change, not its target's.
    let file = root.join("a.txt").unwrap();
    let when = UNIX_EPOCH + Duration::from_secs(1_500_000_000);
    provider
        .set_times(
            &file,
            FileTimes {
                accessed: None,
                modified: Some(when),
            },
        )
        .unwrap();
    assert_eq!(
        provider.stat(&file).unwrap().modified_ms,
        Some(1_500_000_000_000)
    );
    let later = UNIX_EPOCH + Duration::from_secs(1_600_000_000);
    match provider.set_times(
        &link,
        FileTimes {
            accessed: Some(later),
            modified: Some(later),
        },
    ) {
        Ok(()) => assert_eq!(
            provider.stat(&file).unwrap().modified_ms,
            Some(1_500_000_000_000),
            "the target keeps its time"
        ),
        Err(VfsError::Unsupported { .. }) => {}
        Err(error) => panic!("setting a link's times: {error:?}"),
    }

    // Permissions: mode bits, the read-only state, and none on a link.
    provider
        .set_permissions(
            &file,
            Permissions {
                mode: Some(0o640),
                readonly: false,
            },
        )
        .unwrap();
    assert_eq!(provider.permissions(&file).unwrap().mode, Some(0o640));
    provider
        .set_permissions(
            &file,
            Permissions {
                mode: None,
                readonly: true,
            },
        )
        .unwrap();
    let now = provider.permissions(&file).unwrap();
    assert!(now.readonly);
    assert_eq!(now.mode, Some(0o440));
    assert!(matches!(
        provider.set_permissions(
            &link,
            Permissions {
                mode: Some(0o600),
                readonly: false
            }
        ),
        Err(VfsError::Unsupported { .. })
    ));

    let space = provider.free_space(&root).expect("OpenSSH reports space");
    assert!(space.total_bytes >= space.free_bytes && space.total_bytes > 0);
}

#[test]
fn typed_errors_for_what_sftp_reports_as_a_generic_failure() {
    let Some(server) = Sshd::start() else { return };
    let provider = server.provider();
    let root = server.data_location();
    let folder = root.join("folder").unwrap();
    provider.create_dir(&folder).unwrap();
    provider
        .create_file(&folder.join("inside").unwrap())
        .unwrap();
    assert!(matches!(
        provider.create_dir(&folder),
        Err(VfsError::AlreadyExists { .. })
    ));
    assert!(matches!(
        provider.create_dir(&root.join("no/such").unwrap()),
        Err(VfsError::NotFound { .. })
    ));
    assert!(matches!(
        provider.remove_dir(&folder),
        Err(VfsError::NotEmpty { .. })
    ));
    assert!(matches!(
        provider.remove_file(&folder),
        Err(VfsError::IsADirectory { .. })
    ));
    assert!(matches!(
        provider
            .create_write(&folder, WriteOptions::truncate())
            .map(|_| ()),
        Err(VfsError::IsADirectory { .. })
    ));
    assert!(matches!(
        provider.rename(
            &root.join("ghost").unwrap(),
            &root.join("x").unwrap(),
            false
        ),
        Err(VfsError::NotFound { .. })
    ));
    // A name that cannot exist is refused before anything is sent.
    assert!(matches!(
        provider.create_file(&root.join("n".repeat(256)).unwrap()),
        Err(VfsError::InvalidName { .. })
    ));
    // Another server is another volume.
    let elsewhere = waypoint_path::VfsPath::from_uri("sftp://other.example/x").unwrap();
    assert!(matches!(
        provider.rename(&folder, &elsewhere, false),
        Err(VfsError::CrossesDevices { .. })
    ));
    // A folder the user may not write to.
    let locked = server.data.join("locked");
    fs::create_dir(&locked).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o500)).unwrap();
        let denied = provider.create_file(&root.join("locked").unwrap().join("x").unwrap());
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(matches!(denied, Err(VfsError::PermissionDenied { .. })));
    }
}

/// A provider that uses none of OpenSSH's extensions, as against a server with version 3 only.
fn plain_provider(server: &Sshd) -> SftpProvider {
    SftpProvider::new(
        SftpConfig::new(server.known_hosts(server.port))
            .with_agent(AgentSource::None)
            .with_identity_files(vec![server.client_key.clone()])
            .with_plain_protocol(),
    )
}

#[test]
fn a_replacing_rename_without_the_extension_never_loses_the_target() {
    let Some(server) = Sshd::start() else { return };
    let root = server.data_location();
    for provider in [server.provider(), plain_provider(&server)] {
        fs::write(server.data.join("keep.txt"), "only copy").unwrap();
        let keep = root.join("keep.txt").unwrap();
        // Onto itself: a no-op, with or without `overwrite`.
        provider.rename(&keep, &keep, true).unwrap();
        provider.rename(&keep, &keep, false).unwrap();
        assert_eq!(
            fs::read(server.data.join("keep.txt")).unwrap(),
            b"only copy"
        );

        // A missing source leaves the target alone.
        assert!(matches!(
            provider.rename(&root.join("ghost").unwrap(), &keep, true),
            Err(VfsError::NotFound { .. })
        ));
        assert_eq!(
            fs::read(server.data.join("keep.txt")).unwrap(),
            b"only copy"
        );

        // A replacing rename replaces and leaves nothing beside.
        fs::write(server.data.join("new.txt"), "new").unwrap();
        provider
            .rename(&root.join("new.txt").unwrap(), &keep, true)
            .unwrap();
        assert_eq!(fs::read(server.data.join("keep.txt")).unwrap(), b"new");
        let left: Vec<_> = fs::read_dir(&server.data)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(left, ["keep.txt"], "nothing left beside");
        fs::remove_file(server.data.join("keep.txt")).unwrap();
    }

    // A rename that fails after the target was moved aside puts it back: the source sits in a
    // folder the user may not change, so it cannot be moved out of it.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let provider = plain_provider(&server);
        fs::write(server.data.join("target.txt"), "old").unwrap();
        let locked = server.data.join("locked");
        fs::create_dir(&locked).unwrap();
        fs::write(locked.join("source.txt"), "new").unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o500)).unwrap();
        let result = provider.rename(
            &root.join("locked/source.txt").unwrap(),
            &root.join("target.txt").unwrap(),
            true,
        );
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(matches!(result, Err(VfsError::PermissionDenied { .. })));
        assert_eq!(fs::read(server.data.join("target.txt")).unwrap(), b"old");
        let left: Vec<_> = fs::read_dir(&server.data)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".waypoint"))
            .collect();
        assert!(left.is_empty(), "nothing left beside: {left:?}");
    }
}

#[test]
fn a_time_sftp_cannot_carry_is_refused_not_written_as_1970() {
    let Some(server) = Sshd::start() else { return };
    let provider = server.provider();
    fs::write(server.data.join("a.txt"), "x").unwrap();
    let file = server.data_location().join("a.txt").unwrap();
    let before = provider.stat(&file).unwrap().modified_ms;
    let old = UNIX_EPOCH - Duration::from_secs(86_400);
    assert!(matches!(
        provider.set_times(
            &file,
            FileTimes {
                accessed: None,
                modified: Some(old),
            },
        ),
        Err(VfsError::Unsupported { .. })
    ));
    assert_eq!(provider.stat(&file).unwrap().modified_ms, before);
}
