// Watching through the helper: events carry `admin:` locations, an overflow is one rescan, a dead
// connection is `Lost`, and a dropped watch is silent.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::ffi::OsString;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use support::*;
use waypoint_elevated::ServeConfig;
use waypoint_path::ConnectionKey;
use waypoint_protocol::{ConnectionState, VfsError};
use waypoint_vfs::{Change, Provider, RescanReason, WatchEvent, WatchSink};

type Events = Arc<Mutex<Vec<WatchEvent>>>;

fn collector() -> (Events, WatchSink) {
    let events: Events = Arc::default();
    let sink: WatchSink = {
        let events = events.clone();
        Arc::new(move |event| events.lock().unwrap().push(event))
    };
    (events, sink)
}

fn changes(events: &Events) -> Vec<Change> {
    events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            WatchEvent::Changes(changes) => Some(changes.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn has_name(change: &Change, wanted: &str) -> bool {
    let wanted = OsString::from(wanted);
    match change {
        Change::Upsert(entry) => entry.name == wanted,
        Change::Remove(name) => *name == wanted,
        Change::Rename { to, .. } => to.name == wanted,
    }
}

#[test]
fn changes_made_through_admin_arrive_with_the_right_names() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::connected(Arc::new(fast_watching()), ServeConfig::default());
    let root = admin(dir.path());
    let (events, sink) = collector();
    let watch = rig.client.watch(&root, sink).unwrap();

    rig.client
        .create_file(&root.join("a.txt").unwrap())
        .unwrap();
    wait_until("the new file", || {
        changes(&events)
            .iter()
            .any(|c| matches!(c, Change::Upsert(e) if e.name == "a.txt"))
    });

    rig.client
        .rename(
            &root.join("a.txt").unwrap(),
            &root.join("b.txt").unwrap(),
            false,
        )
        .unwrap();
    wait_until("the rename", || {
        changes(&events).iter().any(|c| match c {
            Change::Rename { from, to } => *from == "a.txt" && to.name == "b.txt",
            Change::Upsert(e) => e.name == "b.txt",
            _ => false,
        })
    });

    rig.client
        .remove_file(&root.join("b.txt").unwrap())
        .unwrap();
    wait_until("the removal", || {
        changes(&events)
            .iter()
            .any(|c| matches!(c, Change::Remove(name) if *name == "b.txt"))
    });
    for change in changes(&events) {
        assert!(has_name(&change, "a.txt") || has_name(&change, "b.txt"));
    }
    drop(watch);
}

#[test]
fn the_loss_of_a_watched_folder_names_it_as_admin() {
    let dir = tempfile::tempdir().unwrap();
    let watched = dir.path().join("watched");
    std::fs::create_dir(&watched).unwrap();
    let rig = Rig::connected(Arc::new(fast_watching()), ServeConfig::default());
    let (events, sink) = collector();
    let _watch = rig.client.watch(&admin(&watched), sink).unwrap();
    std::fs::remove_dir(&watched).unwrap();
    wait_until("the folder to be lost", || {
        events
            .lock()
            .unwrap()
            .iter()
            .any(|event| matches!(event, WatchEvent::Lost(_)))
    });
    let events = events.lock().unwrap();
    let lost = events
        .iter()
        .find_map(|event| match event {
            WatchEvent::Lost(error) => Some(error.clone()),
            _ => None,
        })
        .unwrap();
    match lost {
        VfsError::NotFound { location } | VfsError::PermissionDenied { location } => {
            assert!(location.uri.starts_with("admin:///"), "{}", location.uri);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_helper_that_cannot_keep_up_sends_one_rescan() {
    let provider = Arc::new(TestProvider::new());
    let gate = Gate::new();
    let config = ServeConfig {
        watch_queue: 4,
        ..ServeConfig::default()
    };
    let launcher = GatedLauncher {
        provider: provider.clone(),
        config,
        gate: gate.clone(),
    };
    let client = waypoint_elevated::ElevatedProvider::new(Box::new(launcher));
    client
        .connect(
            &ConnectionKey::elevated(),
            None,
            &waypoint_vfs::CancelToken::new(),
        )
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (events, sink) = collector();
    let _watch = client.watch(&admin(dir.path()), sink).unwrap();
    let helper_sink = provider.sink.lock().unwrap().clone().unwrap();
    let change = || WatchEvent::Changes(vec![Change::Remove(OsString::from("x"))]);

    // One event is taken by the emitter, which then waits on the shut stream.
    gate.shut();
    helper_sink(change());
    wait_until("the emitter to wait on the stream", || {
        gate.blocked.load(std::sync::atomic::Ordering::SeqCst) == 1
    });
    for _ in 0..200 {
        helper_sink(change());
    }
    gate.open();

    wait_until("the rescan", || {
        events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, WatchEvent::Rescan(RescanReason::Overflow)))
    });
    std::thread::sleep(Duration::from_millis(200));
    let events = events.lock().unwrap();
    let rescans = events
        .iter()
        .filter(|e| matches!(e, WatchEvent::Rescan(_)))
        .count();
    assert_eq!(rescans, 1, "{events:?}");
    assert!(
        events.len() <= 2,
        "the dropped events are not sent: {events:?}"
    );
}

#[test]
fn a_dead_connection_loses_every_watch() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::connected(Arc::new(fast_watching()), ServeConfig::default());
    let (first, first_sink) = collector();
    let (second, second_sink) = collector();
    let _a = rig.client.watch(&admin(dir.path()), first_sink).unwrap();
    let _b = rig.client.watch(&admin(dir.path()), second_sink).unwrap();
    rig.launcher.sever();
    for events in [&first, &second] {
        wait_until("Lost", || {
            events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, WatchEvent::Lost(VfsError::Disconnected { .. })))
        });
    }
    let lost = first
        .lock()
        .unwrap()
        .iter()
        .find_map(|e| match e {
            WatchEvent::Lost(VfsError::Disconnected { location }) => Some(location.clone()),
            _ => None,
        })
        .unwrap();
    assert!(lost.uri.starts_with("admin:///"), "{}", lost.uri);
    wait_until("the state to fail", || {
        matches!(
            rig.client.connection_state(&ConnectionKey::elevated()),
            ConnectionState::Failed {
                error: VfsError::Disconnected { .. }
            }
        )
    });
    // Nothing reconnects by itself.
    assert!(matches!(
        rig.client.stat(&admin(dir.path())),
        Err(VfsError::Disconnected { .. })
    ));
    assert_eq!(rig.launcher.launches(), 1);
}

#[test]
fn a_dropped_watch_stops_the_events() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::connected(Arc::new(fast_watching()), ServeConfig::default());
    let root = admin(dir.path());
    let (events, sink) = collector();
    let watch = rig.client.watch(&root, sink).unwrap();
    rig.client.create_file(&root.join("one").unwrap()).unwrap();
    wait_until("the first event", || !changes(&events).is_empty());
    drop(watch);
    let seen = events.lock().unwrap().len();
    rig.client.create_file(&root.join("two").unwrap()).unwrap();
    // A round trip after the drop: the helper has handled the unwatch by now.
    rig.client.stat(&root).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(events.lock().unwrap().len(), seen);
}
