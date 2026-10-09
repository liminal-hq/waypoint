// File names that are not Unicode cross the wire without loss.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(unix)]

mod support;

use std::ffi::OsString;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStringExt;
use std::sync::{Arc, Mutex};

use support::*;
use waypoint_elevated::ServeConfig;
use waypoint_vfs::{CancelToken, Change, Provider, WatchEvent, WriteOptions};

fn odd(bytes: &[u8]) -> OsString {
    OsString::from_vec(bytes.to_vec())
}

#[test]
fn a_name_that_is_not_unicode_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::connected(Arc::new(fast_watching()), ServeConfig::default());
    let root = admin(dir.path());
    let name = odd(&[b'n', 0xff, 0xfe, b'.', b't']);
    let path = root.join(&name).unwrap();

    let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::default();
    let sink = {
        let events = events.clone();
        Arc::new(move |event| events.lock().unwrap().push(event))
    };
    let _watch = rig.client.watch(&root, sink).unwrap();

    let mut stream = rig
        .client
        .create_write(&path, WriteOptions::exclusive())
        .unwrap();
    stream.write_all(b"odd").unwrap();
    stream.finish(false).unwrap();
    assert!(dir.path().join(&name).exists());

    let entries = rig
        .client
        .list(&root, &CancelToken::new(), 0, &mut |_| {})
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, name);
    assert_eq!(rig.client.stat(&path).unwrap().name, name);

    let mut back = String::new();
    rig.client
        .open_read(&path)
        .unwrap()
        .read_to_string(&mut back)
        .unwrap();
    assert_eq!(back, "odd");

    wait_until("the new file in the watch", || {
        events.lock().unwrap().iter().any(|event| match event {
            WatchEvent::Changes(changes) => changes
                .iter()
                .any(|c| matches!(c, Change::Upsert(entry) if entry.name == name)),
            _ => false,
        })
    });

    // A rename between two such names, and a link whose target is one.
    let renamed = odd(&[0x80, b'r']);
    rig.client
        .rename(&path, &root.join(&renamed).unwrap(), false)
        .unwrap();
    assert!(dir.path().join(&renamed).exists());
    let link = root.join("link").unwrap();
    rig.client.symlink(&link, &renamed).unwrap();
    assert_eq!(rig.client.read_link(&link).unwrap(), renamed);
    let resolved = rig.client.canonicalize(&link).unwrap();
    assert_eq!(resolved.scheme(), "admin");
    assert_eq!(resolved.file_name(), Some(renamed));
}
