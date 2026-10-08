// Connecting: nothing starts without an explicit `connect`, and a lost connection stays lost.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use support::*;
use waypoint_elevated::testing::pipe::{pipe, PipeWriter};
use waypoint_elevated::{ElevatedProvider, Launcher, ServeConfig, Transport};
use waypoint_path::ConnectionKey;
use waypoint_protocol::{ConnectionState, Location, VfsError};
use waypoint_vfs::{CancelToken, LocalProvider, Provider};

fn state(rig: &Rig) -> ConnectionState {
    rig.client.connection_state(&ConnectionKey::elevated())
}

#[test]
fn calls_made_while_not_connected_launch_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::local();
    let path = admin(dir.path());
    assert_eq!(state(&rig), ConnectionState::Idle);
    let cancel = CancelToken::new();
    let results = [
        kind(&rig.client.stat(&path)),
        kind(&rig.client.list(&path, &cancel, 0, &mut |_| {})),
        kind(&rig.client.create_dir(&path.join("x").unwrap())),
        kind(&rig.client.open_read(&path.join("x").unwrap()).map(|_| ())),
        kind(&rig.client.watch(&path, Arc::new(|_| {})).map(|_| ())),
        kind(&rig.client.canonicalize(&path)),
        kind(&rig.client.permissions(&path)),
    ];
    assert!(
        results.iter().all(|kind| kind == "disconnected"),
        "{results:?}"
    );
    assert!(rig.client.volume_id(&path).is_none());
    assert!(rig.client.free_space(&path).is_none());
    assert_eq!(rig.launcher.launches(), 0);
    assert_eq!(state(&rig), ConnectionState::Idle);
    // The error names the place that was asked about, as `admin:`.
    match rig.client.stat(&path).unwrap_err() {
        VfsError::Disconnected { location } => assert_eq!(location, path.to_location()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn connect_launches_once() {
    let rig = Rig::local();
    let key = ConnectionKey::elevated();
    rig.client.connect(&key, None, &CancelToken::new()).unwrap();
    assert_eq!(state(&rig), ConnectionState::Connected);
    rig.client.connect(&key, None, &CancelToken::new()).unwrap();
    assert_eq!(rig.launcher.launches(), 1);
    // Another login is not this provider's.
    let error = rig
        .client
        .connect(
            &waypoint_path::VfsPath::from_uri("sftp://h/")
                .unwrap()
                .connection_key()
                .unwrap(),
            None,
            &CancelToken::new(),
        )
        .unwrap_err();
    assert!(matches!(error, VfsError::Unsupported { .. }));
}

#[test]
fn a_refused_launch_is_a_failed_state_and_can_be_tried_again() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::local();
    let key = ConnectionKey::elevated();
    let refusal = VfsError::PermissionDenied {
        location: Location::new("admin", "admin:///"),
    };
    rig.launcher.refuse_with(Some(refusal.clone()));
    let error = rig
        .client
        .connect(&key, None, &CancelToken::new())
        .unwrap_err();
    assert_eq!(error, refusal);
    assert_eq!(state(&rig), ConnectionState::Failed { error: refusal });
    assert!(matches!(
        rig.client.stat(&admin(dir.path())),
        Err(VfsError::Disconnected { .. })
    ));
    // Only an explicit connect tries again.
    assert_eq!(rig.launcher.launches(), 1);
    rig.launcher.refuse_with(None);
    rig.client.connect(&key, None, &CancelToken::new()).unwrap();
    assert_eq!(rig.launcher.launches(), 2);
    assert_eq!(state(&rig), ConnectionState::Connected);
    rig.client.stat(&admin(dir.path())).unwrap();
}

#[test]
fn a_connect_cancelled_before_it_starts_launches_nothing() {
    let rig = Rig::local();
    let cancel = CancelToken::new();
    cancel.cancel();
    let result = rig
        .client
        .connect(&ConnectionKey::elevated(), None, &cancel);
    assert!(matches!(result, Err(VfsError::Cancelled)));
    assert_eq!(rig.launcher.launches(), 0);
}

#[test]
fn disconnecting_ends_calls_in_flight_and_returns_to_idle() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(TestProvider::new());
    let rig = Rig::connected(provider.clone(), ServeConfig::default());
    let path = admin(dir.path());
    let result = std::thread::scope(|scope| {
        let call = scope.spawn(|| rig.client.list(&path, &CancelToken::new(), 0, &mut |_| {}));
        wait_until("the listing to start", || {
            provider.started.load(Ordering::SeqCst) == 1
        });
        rig.client.disconnect(&ConnectionKey::elevated());
        call.join().unwrap()
    });
    assert!(matches!(result, Err(VfsError::Disconnected { .. })));
    assert_eq!(state(&rig), ConnectionState::Idle);
    assert!(matches!(
        rig.client.stat(&path),
        Err(VfsError::Disconnected { .. })
    ));
    // The helper ends with the stream, and cancels what it was doing.
    provider.release.store(true, Ordering::SeqCst);
    let ends = rig.launcher.wait_for_ends(1, Duration::from_secs(5));
    assert_eq!(ends.len(), 1);
    // Connecting again starts a second helper.
    rig.connect();
    assert_eq!(rig.launcher.launches(), 2);
}

#[test]
fn a_killed_helper_fails_calls_in_flight_and_stays_down() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(TestProvider::new());
    let rig = Rig::connected(provider.clone(), ServeConfig::default());
    let path = admin(dir.path());
    let result = std::thread::scope(|scope| {
        let call = scope.spawn(|| rig.client.list(&path, &CancelToken::new(), 0, &mut |_| {}));
        wait_until("the listing to start", || {
            provider.started.load(Ordering::SeqCst) == 1
        });
        rig.launcher.sever();
        call.join().unwrap()
    });
    assert!(matches!(result, Err(VfsError::Disconnected { .. })));
    wait_until("the state to fail", || {
        matches!(state(&rig), ConnectionState::Failed { .. })
    });
    assert!(matches!(
        rig.client.stat(&path),
        Err(VfsError::Disconnected { .. })
    ));
    assert_eq!(rig.launcher.launches(), 1);
    provider.release.store(true, Ordering::SeqCst);
}

#[test]
fn the_provider_does_not_outlive_its_helper() {
    let rig = Rig::connected(Arc::new(LocalProvider::new()), ServeConfig::default());
    let launcher = rig.launcher.clone();
    drop(rig);
    assert_eq!(launcher.wait_for_ends(1, PATIENCE).len(), 1);
}

/// A helper that starts and then says nothing: its output never carries a byte.
#[derive(Default)]
struct Silent(Mutex<Vec<PipeWriter>>);

impl Launcher for Silent {
    fn launch(&self) -> Result<Transport, VfsError> {
        let (reader, writer, _closer) = pipe();
        self.0.lock().unwrap().push(writer);
        Ok(Transport {
            reader: Box::new(reader),
            writer: Box::new(std::io::sink()),
        })
    }
}

#[test]
fn a_helper_that_never_says_hello_is_given_up_on() {
    let client = ElevatedProvider::new(Box::new(Silent::default()))
        .with_hello_timeout(Duration::from_millis(200));
    let key = ConnectionKey::elevated();
    let started = Instant::now();
    let error = client.connect(&key, None, &CancelToken::new()).unwrap_err();
    assert!(matches!(error, VfsError::Disconnected { .. }), "{error:?}");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(matches!(
        client.connection_state(&key),
        ConnectionState::Failed { .. }
    ));
}
