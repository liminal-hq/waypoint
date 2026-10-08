// The shared provider conformance suite, run against the elevated provider over a loopback stream.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::io::{Read, Write};

use support::*;
use waypoint_path::ConnectionKey;
use waypoint_protocol::ConnectionState;
use waypoint_vfs::conformance::{self, Subject};
use waypoint_vfs::{Provider, WriteOptions};

#[test]
fn the_elevated_provider_passes_the_conformance_suite() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::local();
    rig.connect();
    let subject = Subject {
        name: "elevated",
        provider: &rig.client,
        root: admin(dir.path()),
    };
    conformance::run(&subject);
    assert_eq!(rig.launcher.launches(), 1);
}

#[test]
fn the_capabilities_are_the_helpers_and_never_remote() {
    let rig = Rig::local();
    let before = rig.client.capabilities();
    assert!(before.watch && !before.remote && before.server_copy && !before.atomic_write);
    rig.connect();
    let caps = rig.client.capabilities();
    assert!(caps.watch && !caps.remote && caps.server_copy && !caps.atomic_write);
    assert!(caps.write && caps.symlinks && caps.range_read);
    assert_eq!(
        caps.permissions,
        waypoint_vfs::LocalProvider::new()
            .capabilities()
            .permissions
    );
    assert!(!rig.client.read_only());
    assert_eq!(rig.client.scheme(), "admin");
}

#[test]
fn a_multi_chunk_file_streams_both_ways() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::local();
    rig.connect();
    let path = admin(dir.path()).join("big.bin").unwrap();
    let content: Vec<u8> = (0..700_000u32).map(|n| (n % 251) as u8).collect();
    let mut stream = rig
        .client
        .create_write(&path, WriteOptions::exclusive())
        .unwrap();
    stream.write_all(&content[..1000]).unwrap();
    stream.write_all(&content[1000..]).unwrap();
    stream.finish(true).unwrap();
    assert_eq!(std::fs::read(dir.path().join("big.bin")).unwrap(), content);

    let mut back = Vec::new();
    rig.client
        .open_read(&path)
        .unwrap()
        .read_to_end(&mut back)
        .unwrap();
    assert_eq!(back, content);

    let mut tail = Vec::new();
    rig.client
        .open_read_at(&path, 650_000)
        .unwrap()
        .read_to_end(&mut tail)
        .unwrap();
    assert_eq!(tail, &content[650_000..]);

    let entry = rig.client.stat(&path).unwrap();
    assert_eq!(entry.size, Some(700_000));
}

#[test]
fn the_conformance_root_is_seen_as_an_admin_location() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::local();
    rig.connect();
    let root = admin(dir.path());
    assert_eq!(
        rig.client.connection_key(&root),
        Some(ConnectionKey::elevated())
    );
    assert_eq!(rig.client.connection_key(&file(dir.path())), None);
    // A path that is not `admin:` is not this provider's.
    let error = rig.client.stat(&file(dir.path())).unwrap_err();
    assert!(matches!(
        error,
        waypoint_protocol::VfsError::InvalidLocation { .. }
    ));
    // An error from the helper names the place as `admin:`.
    let error = rig.client.stat(&root.join("missing").unwrap()).unwrap_err();
    match error {
        waypoint_protocol::VfsError::NotFound { location } => {
            assert!(location.uri.starts_with("admin:///"), "{}", location.uri);
        }
        other => panic!("{other:?}"),
    }
    let resolved = rig.client.canonicalize(&root).unwrap();
    assert_eq!(resolved.scheme(), "admin");
    assert_eq!(
        rig.client.connection_state(&ConnectionKey::elevated()),
        ConnectionState::Connected
    );
}
