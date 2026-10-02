// Exercises the plugin through Tauri's mock runtime over a fake backend: the list, the free-space timeout, the revisioned change events and the actions
//
// The mock runtime has no capability file, so the IPC layer would refuse plugin commands; these tests call the same `Volumes` methods the commands delegate to, and check the JSON the commands would send. Nothing here touches a real device, the system bus or the mount table.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(target_os = "linux")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{App, Listener};
use tauri_plugin_volumes::{
    Backend, BoxFuture, Flavour, Notify, Options, Passphrase, PluginStatus, Reason, Space, Volume,
    VolumeKind, VolumesChanged, VolumesError, VolumesExt, EVENT_CHANGED, FEATURES, FEATURE_LIST,
};

type Calls = Arc<Mutex<Vec<String>>>;

/// A backend over a list the test edits, that records the calls it gets.
struct Fake {
    volumes: Mutex<Vec<Volume>>,
    notify: Mutex<Option<Notify>>,
    calls: Calls,
    failure: Mutex<Option<VolumesError>>,
    /// Closed until the test has its event listener in place, so the plugin's first list cannot be announced before anyone listens.
    open: AtomicBool,
}

impl Fake {
    fn new(volumes: Vec<Volume>) -> Arc<Self> {
        Arc::new(Fake {
            volumes: Mutex::new(volumes),
            notify: Mutex::new(None),
            calls: Calls::default(),
            failure: Mutex::new(None),
            open: AtomicBool::new(false),
        })
    }

    fn set(&self, volumes: Vec<Volume>) {
        *self.volumes.lock().unwrap() = volumes;
    }

    /// What the system does when something changes: tell the plugin.
    fn announce(&self) {
        let notify = self
            .notify
            .lock()
            .unwrap()
            .clone()
            .expect("the watch started");
        notify();
    }

    fn record(&self, call: impl Into<String>) -> Result<(), VolumesError> {
        self.calls.lock().unwrap().push(call.into());
        match self.failure.lock().unwrap().take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Backend for Fake {
    fn status(&self) -> BoxFuture<'_, PluginStatus> {
        Box::pin(async {
            PluginStatus::build(
                Flavour::Udisks2,
                FEATURES
                    .iter()
                    .map(|name| tauri_plugin_volumes::FeatureStatus::available(name))
                    .collect(),
            )
        })
    }

    fn volumes(&self) -> BoxFuture<'_, Result<Vec<Volume>, VolumesError>> {
        Box::pin(async {
            while !self.open.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            Ok(self.volumes.lock().unwrap().clone())
        })
    }

    fn mount(&self, id: String) -> BoxFuture<'_, Result<String, VolumesError>> {
        Box::pin(async move {
            self.record(format!("mount {id}"))?;
            let mut volumes = self.volumes.lock().unwrap();
            let volume = volumes
                .iter_mut()
                .find(|v| v.id == id)
                .ok_or(VolumesError::NotFound)?;
            let mount = format!("/run/media/test/{}", volume.label);
            volume.mount_point = Some(mount.clone());
            volume.can_mount = false;
            volume.can_unmount = true;
            Ok(mount)
        })
    }

    fn unmount(&self, id: String) -> BoxFuture<'_, Result<(), VolumesError>> {
        Box::pin(async move { self.record(format!("unmount {id}")) })
    }

    fn eject(&self, id: String) -> BoxFuture<'_, Result<(), VolumesError>> {
        Box::pin(async move {
            self.record(format!("eject {id}"))?;
            self.volumes.lock().unwrap().retain(|v| v.id != id);
            Ok(())
        })
    }

    fn unlock(
        &self,
        id: String,
        passphrase: Passphrase,
    ) -> BoxFuture<'_, Result<String, VolumesError>> {
        Box::pin(async move {
            // The passphrase reaches the backend, and its `Debug` shows nothing of it.
            assert!(!format!("{passphrase:?}").contains("hunter2"));
            self.record(format!(
                "unlock {id} with {} characters",
                passphrase.0.len()
            ))?;
            Ok("fake:unlocked".to_string())
        })
    }

    fn watch(&self, notify: Notify) -> BoxFuture<'_, Result<(), VolumesError>> {
        Box::pin(async move {
            *self.notify.lock().unwrap() = Some(notify);
            Ok(())
        })
    }
}

fn volume(id: &str, kind: VolumeKind, mount: Option<&str>) -> Volume {
    Volume {
        id: id.to_string(),
        label: id.to_string(),
        kind,
        file_system: Some("ext4".into()),
        mount_point: mount.map(str::to_string),
        uri: None,
        total: None,
        free: None,
        can_mount: mount.is_none(),
        can_unmount: mount.is_some(),
        can_eject: false,
        can_power_off: false,
        locked: false,
        is_system: false,
        device: None,
    }
}

struct Fixture {
    app: App<MockRuntime>,
    fake: Arc<Fake>,
    events: mpsc::Receiver<VolumesChanged>,
    measured: Arc<Mutex<Vec<String>>>,
}

fn options(
    space: impl Fn(&str) -> Option<Space> + Send + Sync + 'static,
    timeout_ms: u64,
) -> Options {
    Options {
        space_timeout: Duration::from_millis(timeout_ms),
        debounce: Duration::from_millis(100),
        space: Arc::new(space),
        ..Options::default()
    }
}

fn fixture_with(volumes: Vec<Volume>, options: Options) -> Fixture {
    let fake = Fake::new(volumes);
    let measured = Arc::new(Mutex::new(Vec::new()));
    let app = mock_builder()
        .plugin(tauri_plugin_volumes::init_with(fake.clone(), options))
        .build(mock_context(noop_assets()))
        .expect("the plugin should initialise");
    let (sender, events) = mpsc::channel();
    app.listen(EVENT_CHANGED, move |event| {
        let _ = sender.send(serde_json::from_str::<VolumesChanged>(event.payload()).unwrap());
    });
    fake.open.store(true, Ordering::SeqCst);
    Fixture {
        app,
        fake,
        events,
        measured,
    }
}

fn fixture(volumes: Vec<Volume>) -> Fixture {
    let measured = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&measured);
    let mut fx = fixture_with(
        volumes,
        options(
            move |mount| {
                log.lock().unwrap().push(mount.to_string());
                Some(Space {
                    total: 1000,
                    free: 400,
                })
            },
            2000,
        ),
    );
    fx.measured = measured;
    fx
}

fn next_event(fx: &Fixture) -> VolumesChanged {
    fx.events
        .recv_timeout(Duration::from_secs(3))
        .expect("an event arrives")
}

fn no_event(fx: &Fixture, wait_ms: u64) {
    assert!(
        fx.events
            .recv_timeout(Duration::from_millis(wait_ms))
            .is_err(),
        "no event was expected"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_status_lists_every_feature_and_the_flavour() {
    let fx = fixture(vec![]);
    let status = fx.app.volumes().get_status().await;
    assert!(status.available);
    assert_eq!(status.flavour, Flavour::Udisks2);
    let names: Vec<&str> = status.features.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        ["list", "mount", "unmount", "eject", "unlock", "watch"]
    );
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(json["flavour"], "udisks2");
    assert_eq!(
        json["features"][0],
        json!({ "name": "list", "available": true, "reason": null, "message": null })
    );
    assert!(status.has(FEATURE_LIST));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unavailable_feature_carries_a_typed_reason() {
    let status = PluginStatus::all_unavailable(Flavour::Mountinfo, Reason::NoSystemBus, "no bus");
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(json["reason"], "no-system-bus");
    assert_eq!(json["features"][1]["reason"], "no-system-bus");
    assert_eq!(json["features"][1]["message"], "no bus");
    assert_eq!(
        serde_json::to_value(Reason::Udisks2Missing).unwrap(),
        "udisks2-missing"
    );
    assert_eq!(
        serde_json::to_value(Reason::FlatpakSandbox).unwrap(),
        "flatpak-sandbox"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_measures_local_mounts_but_not_network_ones() {
    let fx = fixture(vec![
        volume("local", VolumeKind::Internal, Some("/data")),
        volume("nas", VolumeKind::Network, Some("/mnt/nas")),
        volume("unmounted", VolumeKind::Removable, None),
    ]);
    // The plugin's own first list has been read; start counting from here.
    assert_eq!(next_event(&fx).revision, 1);
    fx.measured.lock().unwrap().clear();
    let volumes = fx.app.volumes().list(false).await.unwrap();
    assert_eq!(volumes[0].free, Some(400));
    assert_eq!(volumes[0].total, Some(1000));
    assert_eq!(
        volumes[1].free, None,
        "a network mount is not measured unless asked"
    );
    assert_eq!(volumes[2].free, None);
    assert_eq!(*fx.measured.lock().unwrap(), ["/data"]);

    let all = fx.app.volumes().list(true).await.unwrap();
    assert_eq!(all[1].free, Some(400));
}

#[tokio::test(flavor = "multi_thread")]
async fn refresh_space_measures_one_volume_of_any_kind() {
    let fx = fixture(vec![volume("nas", VolumeKind::Network, Some("/mnt/nas"))]);
    let measured = fx.app.volumes().refresh_space("nas").await.unwrap();
    assert_eq!(measured.free, Some(400));
    assert_eq!(*fx.measured.lock().unwrap(), ["/mnt/nas"]);
    assert_eq!(
        fx.app.volumes().refresh_space("gone").await,
        Err(VolumesError::NotFound)
    );
    // Not mounted: nothing to measure, and the volume comes back as it is.
    fx.fake
        .set(vec![volume("usb", VolumeKind::Removable, None)]);
    let usb = fx.app.volumes().refresh_space("usb").await.unwrap();
    assert_eq!(usb.free, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stalled_measurement_cannot_hold_the_list_back() {
    let fx = fixture_with(
        vec![
            volume("good", VolumeKind::Internal, Some("/good")),
            volume("hung", VolumeKind::Removable, Some("/hung")),
        ],
        options(
            |mount| {
                if mount == "/hung" {
                    std::thread::sleep(Duration::from_secs(5));
                }
                Some(Space { total: 10, free: 5 })
            },
            200,
        ),
    );
    let started = Instant::now();
    let volumes = fx.app.volumes().list(false).await.unwrap();
    let took = started.elapsed();
    assert!(took >= Duration::from_millis(190), "{took:?}");
    assert!(took < Duration::from_millis(1500), "{took:?}");
    assert_eq!(volumes[0].free, Some(5));
    assert_eq!(volumes[1].free, None);
    assert_eq!(
        volumes[1].mount_point.as_deref(),
        Some("/hung"),
        "the rest of the volume is still listed"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_first_list_is_announced_with_revision_one() {
    let fx = fixture(vec![volume("a", VolumeKind::Internal, None)]);
    let first = next_event(&fx);
    assert_eq!(first.revision, 1);
    assert_eq!(first.volumes.len(), 1);
    assert_eq!(fx.app.volumes().current().0, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_burst_of_notifications_makes_one_event_after_the_debounce() {
    let fx = fixture(vec![volume("a", VolumeKind::Internal, None)]);
    assert_eq!(next_event(&fx).revision, 1);
    // Wait for the watch to be installed.
    tokio::time::sleep(Duration::from_millis(100)).await;

    fx.fake.set(vec![
        volume("a", VolumeKind::Internal, None),
        volume("b", VolumeKind::Removable, None),
    ]);
    let started = Instant::now();
    for _ in 0..6 {
        fx.fake.announce();
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let event = next_event(&fx);
    assert_eq!(event.revision, 2);
    assert_eq!(event.volumes.len(), 2);
    // The burst lasted about 120 ms and the event came 100 ms after its last notification.
    assert!(
        started.elapsed() >= Duration::from_millis(200),
        "{:?}",
        started.elapsed()
    );
    no_event(&fx, 400);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_notification_that_changes_nothing_or_only_free_space_sends_no_event() {
    let fx = fixture(vec![volume("a", VolumeKind::Internal, Some("/data"))]);
    assert_eq!(next_event(&fx).revision, 1);
    tokio::time::sleep(Duration::from_millis(100)).await;
    fx.fake.announce();
    no_event(&fx, 500);
    // The list is read again, and its numbers are kept current, with the revision unchanged.
    assert_eq!(fx.app.volumes().current().0, 1);
    fx.app.volumes().list(false).await.unwrap();
    assert_eq!(fx.app.volumes().current().0, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn free_space_that_changes_with_every_read_does_not_make_events() {
    let reads = Arc::new(Mutex::new(0u64));
    let counter = Arc::clone(&reads);
    let fx = fixture_with(
        vec![volume("a", VolumeKind::Internal, Some("/data"))],
        options(
            move |_| {
                let mut n = counter.lock().unwrap();
                *n += 1;
                Some(Space {
                    total: 1000,
                    free: 1000 - *n,
                })
            },
            2000,
        ),
    );
    assert_eq!(next_event(&fx).revision, 1);
    let first = fx.app.volumes().list(false).await.unwrap()[0].free;
    let second = fx.app.volumes().list(false).await.unwrap()[0].free;
    assert_ne!(first, second, "the free space did change");
    no_event(&fx, 300);
    assert_eq!(fx.app.volumes().current().0, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn revisions_only_grow() {
    let fx = fixture(vec![]);
    assert_eq!(next_event(&fx).revision, 1);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut last = 1;
    for n in 0..3 {
        let volumes = (0..=n)
            .map(|i| volume(&format!("v{i}"), VolumeKind::Removable, None))
            .collect();
        fx.fake.set(volumes);
        fx.app.volumes().list(false).await.unwrap();
        let event = next_event(&fx);
        assert!(event.revision > last);
        last = event.revision;
    }
    assert_eq!(last, 4);
}

#[tokio::test(flavor = "multi_thread")]
async fn mount_runs_the_backend_and_the_new_list_is_announced() {
    let fx = fixture(vec![volume("usb", VolumeKind::Removable, None)]);
    assert_eq!(next_event(&fx).revision, 1);
    let mount = fx.app.volumes().mount("usb").await.unwrap();
    assert_eq!(mount, "/run/media/test/usb");
    let event = next_event(&fx);
    assert_eq!(event.revision, 2);
    assert_eq!(
        event.volumes[0].mount_point.as_deref(),
        Some("/run/media/test/usb")
    );
    assert_eq!(*fx.fake.calls.lock().unwrap(), ["mount usb"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn eject_removes_the_volume_from_the_list() {
    let fx = fixture(vec![volume("usb", VolumeKind::Removable, Some("/m"))]);
    assert_eq!(next_event(&fx).revision, 1);
    fx.app.volumes().eject("usb").await.unwrap();
    assert!(next_event(&fx).volumes.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_action_reports_its_typed_error_and_changes_nothing() {
    let fx = fixture(vec![volume("usb", VolumeKind::Removable, Some("/m"))]);
    assert_eq!(next_event(&fx).revision, 1);
    *fx.fake.failure.lock().unwrap() = Some(VolumesError::Busy {
        by: Some("vim".into()),
    });
    let error = fx.app.volumes().unmount("usb").await.unwrap_err();
    assert_eq!(
        error,
        VolumesError::Busy {
            by: Some("vim".into())
        }
    );
    assert_eq!(
        serde_json::to_value(&error).unwrap(),
        json!({ "kind": "busy", "by": "vim" })
    );
    no_event(&fx, 300);
    *fx.fake.failure.lock().unwrap() = Some(VolumesError::NotAuthorised);
    let error = fx.app.volumes().unmount("usb").await.unwrap_err();
    assert_eq!(
        serde_json::to_value(&error).unwrap(),
        json!({ "kind": "notAuthorised" })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn unlock_passes_the_passphrase_on_and_returns_the_new_volume() {
    let fx = fixture(vec![]);
    let id = fx
        .app
        .volumes()
        .unlock("luks", Passphrase("hunter2".into()))
        .await
        .unwrap();
    assert_eq!(id, "fake:unlocked");
    // The recorded call has a length, never the text.
    let calls = fx.fake.calls.lock().unwrap().clone();
    assert_eq!(calls, ["unlock luks with 7 characters"]);
    assert!(!format!("{:?}", Passphrase("hunter2".into())).contains("hunter2"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_volume_serialises_in_camel_case() {
    let json = serde_json::to_value(volume("v", VolumeKind::Optical, Some("/m"))).unwrap();
    assert_eq!(json["kind"], "optical");
    assert_eq!(json["fileSystem"], "ext4");
    assert_eq!(json["mountPoint"], "/m");
    assert_eq!(json["canMount"], false);
    assert_eq!(json["canPowerOff"], false);
    assert_eq!(json["isSystem"], false);
}
