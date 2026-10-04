// The shared conformance suite against the local provider, the in-memory one and the fake server,
// and the remote contract's behaviour that only the fake server can show: polling, batches and
// connections that come and go.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use waypoint_path::{CaseRule, FilePath, RemoteScheme, VfsPath};
use waypoint_protocol::{ConnectionState, UnreachableReason, VfsError};
use waypoint_vfs::conformance::{self, Subject};
use waypoint_vfs::{
    CancelToken, Change, FakeRemoteProvider, LocalProvider, MemoryProvider, Provider,
    ProviderRegistry, RemoteFault, WatchEvent,
};

#[test]
fn the_local_provider_conforms() {
    let dir = tempfile::tempdir().unwrap();
    let provider = LocalProvider::new();
    conformance::run(&Subject {
        name: "local",
        provider: &provider,
        root: VfsPath::File(FilePath::from_path(dir.path()).unwrap()),
    });
}

#[test]
fn the_memory_provider_conforms_under_both_case_rules() {
    for (name, rule) in [
        ("memory/sensitive", CaseRule::Sensitive),
        ("memory/insensitive", CaseRule::Insensitive),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let root = FilePath::from_path(dir.path()).unwrap();
        let provider = MemoryProvider::new(root.clone(), rule);
        conformance::run(&Subject {
            name,
            provider: &provider,
            root: VfsPath::File(root),
        });
    }
}

#[test]
fn the_fake_server_conforms_for_each_scheme() {
    for (scheme, rule) in [
        (RemoteScheme::Sftp, CaseRule::Sensitive),
        (RemoteScheme::Smb, CaseRule::Insensitive),
        (RemoteScheme::Davs, CaseRule::Sensitive),
        (RemoteScheme::S3, CaseRule::Sensitive),
    ] {
        let fake = FakeRemoteProvider::new(scheme, rule);
        fake.set_latency(Duration::from_micros(200));
        let root = match scheme {
            RemoteScheme::S3 => fake.root("bucket"),
            _ => fake.root("me@fake.test").join("share").unwrap(),
        };
        fake.put_dir(&root);
        conformance::run(&Subject {
            name: scheme.as_str(),
            provider: &fake,
            root,
        });
    }
}

#[test]
fn a_fake_server_that_polls_conforms_and_reports_changes() {
    let fake = FakeRemoteProvider::sftp();
    fake.set_poll(Some(Duration::from_millis(20)));
    let root = fake.root("h").join("watched").unwrap();
    fake.put_dir(&root);
    conformance::reading(&Subject {
        name: "sftp/polling",
        provider: &fake,
        root: root.clone(),
    });
    let probe = fake.root("h").join("probe").unwrap();
    fake.put_dir(&probe);
    conformance::capabilities(&Subject {
        name: "sftp/polling",
        provider: &fake,
        root: probe,
    });
    let (send, receive) = mpsc::channel();
    let sink = Arc::new(move |event: WatchEvent| {
        let _ = send.send(event);
    });
    let watch = fake.watch(&root, sink).unwrap();
    fake.put_file(&root.join("new.txt").unwrap(), b"x");
    let event = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    let WatchEvent::Changes(changes) = event else {
        panic!("expected changes, got {event:?}");
    };
    assert!(matches!(&changes[..], [Change::Upsert(entry)] if entry.name == "new.txt"));
    // Going offline is a loss the listing shows as the server's state.
    fake.set_fault(Some(RemoteFault::Unreachable(UnreachableReason::Offline)));
    let event = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(
        event,
        WatchEvent::Lost(VfsError::Unreachable { .. })
    ));
    drop(watch);
}

#[test]
fn a_cancelled_connect_says_cancelled_and_leaves_no_session() {
    let fake = FakeRemoteProvider::sftp();
    let key = fake.root("h").connection_key().unwrap();
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(fake.connect(&key, None, &cancel), Err(VfsError::Cancelled));
    assert_eq!(fake.connection_state(&key), ConnectionState::Idle);
    fake.connect(&key, None, &CancelToken::new()).unwrap();
    assert_eq!(fake.connection_state(&key), ConnectionState::Connected);
}

#[test]
fn the_registry_serves_each_scheme_from_its_provider() {
    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(LocalProvider::new()));
    let fake = FakeRemoteProvider::sftp();
    registry.register(Arc::new(fake.clone()));
    let remote = fake.root("h");
    fake.put_dir(&remote.join("a").unwrap());
    let provider = registry.for_path(&remote).unwrap();
    assert_eq!(provider.scheme(), "sftp");
    assert!(provider.capabilities().remote);
    assert!(
        !registry
            .for_path(
                &VfsPath::from_uri(if cfg!(windows) {
                    "file:///C:/"
                } else {
                    "file:///"
                })
                .unwrap()
            )
            .unwrap()
            .capabilities()
            .remote
    );
    assert_eq!(registry.schemes().collect::<Vec<_>>(), ["file", "sftp"]);
}
