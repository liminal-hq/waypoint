// How long a helper lives: it goes idle only with nothing in flight, open or watched.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::io::Read;
use std::sync::Arc;
use std::time::{Duration, Instant};

use support::*;
use waypoint_elevated::{ServeConfig, ServeEnd};
use waypoint_path::ConnectionKey;
use waypoint_protocol::{ConnectionState, VfsError};
use waypoint_vfs::{CancelToken, LocalProvider, Provider, WatchSink};

const IDLE: Duration = Duration::from_millis(300);

fn config() -> ServeConfig {
    ServeConfig {
        idle: IDLE,
        ..ServeConfig::default()
    }
}

fn nothing() -> WatchSink {
    Arc::new(|_| {})
}

#[test]
fn a_helper_with_nothing_open_goes_idle() {
    let started = Instant::now();
    let rig = Rig::connected(Arc::new(LocalProvider::new()), config());
    let ends = rig.launcher.wait_for_ends(1, PATIENCE);
    assert_eq!(ends, vec![ServeEnd::Idle]);
    assert!(started.elapsed() >= IDLE, "it did not wait");
    // The client sees the helper go, and does not start another.
    wait_until("the state to fail", || {
        matches!(
            rig.client.connection_state(&ConnectionKey::elevated()),
            ConnectionState::Failed {
                error: VfsError::Disconnected { .. }
            }
        )
    });
    assert_eq!(rig.launcher.launches(), 1);
}

#[test]
fn a_busy_connection_is_not_idle_but_requests_keep_it_so() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::connected(Arc::new(LocalProvider::new()), config());
    let path = admin(dir.path());
    // Requests spaced inside the idle time keep the helper up for longer than it.
    let started = Instant::now();
    while started.elapsed() < IDLE * 3 {
        rig.client.stat(&path).unwrap();
        std::thread::sleep(Duration::from_millis(60));
    }
    assert!(rig.launcher.ends().is_empty());
}

#[test]
fn an_open_watch_keeps_the_helper_up() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::connected(Arc::new(LocalProvider::new()), config());
    let watch = rig.client.watch(&admin(dir.path()), nothing()).unwrap();
    std::thread::sleep(IDLE * 3);
    assert!(rig.launcher.ends().is_empty());
    drop(watch);
    assert_eq!(
        rig.launcher.wait_for_ends(1, PATIENCE),
        vec![ServeEnd::Idle]
    );
}

#[test]
fn an_open_handle_keeps_the_helper_up() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f"), b"content").unwrap();
    let rig = Rig::connected(Arc::new(LocalProvider::new()), config());
    let mut stream = rig
        .client
        .open_read(&admin(dir.path()).join("f").unwrap())
        .unwrap();
    std::thread::sleep(IDLE * 3);
    assert!(rig.launcher.ends().is_empty());
    let mut content = String::new();
    stream.read_to_string(&mut content).unwrap();
    assert_eq!(content, "content");
    drop(stream);
    assert_eq!(
        rig.launcher.wait_for_ends(1, PATIENCE),
        vec![ServeEnd::Idle]
    );
}

#[test]
fn a_request_in_flight_keeps_the_helper_up() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(TestProvider::new());
    let rig = Rig::connected(provider.clone(), config());
    let path = admin(dir.path());
    let cancel = CancelToken::new();
    let listing = {
        let cancel = cancel.clone();
        let client = &rig.client;
        std::thread::scope(|scope| {
            let call = scope.spawn(|| client.list(&path, &cancel, 0, &mut |_| {}));
            wait_until("the listing to start", || {
                provider.started.load(std::sync::atomic::Ordering::SeqCst) == 1
            });
            std::thread::sleep(IDLE * 3);
            assert!(rig.launcher.ends().is_empty());
            cancel.cancel();
            call.join().unwrap()
        })
    };
    assert!(matches!(listing, Err(VfsError::Cancelled)));
    assert_eq!(
        rig.launcher.wait_for_ends(1, PATIENCE),
        vec![ServeEnd::Idle]
    );
}

#[test]
fn closing_the_stream_ends_the_helper() {
    let rig = Rig::connected(Arc::new(LocalProvider::new()), ServeConfig::default());
    rig.client.disconnect(&ConnectionKey::elevated());
    assert_eq!(
        rig.launcher.wait_for_ends(1, PATIENCE),
        vec![ServeEnd::EndOfInput]
    );
}
