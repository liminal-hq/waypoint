// The Rust API of the plugin: the `Volumes` handle behind `app.volumes()`, its revisioned list and its change events
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Runtime};

use crate::backend::{Backend, Notify};
use crate::debounce::Debounce;
use crate::error::{Result, VolumesError};
use crate::models::{
    Passphrase, PluginStatus, Volume, VolumeKind, VolumesChanged, EVENT_CHANGED, FEATURE_WATCH,
};
use crate::space::{system_space, Measurer, SpaceFn};

/// What a host can tune, and what tests inject.
#[derive(Clone)]
pub struct Options {
    /// How long a free-space query may take before the list goes on without it. Default 2 seconds.
    pub space_timeout: Duration,
    /// How long a burst of change notifications must be quiet before the list is read again. Default 100 ms.
    pub debounce: Duration,
    /// The longest a steady trickle of notifications may delay the list. Default 1 second.
    pub max_debounce: Duration,
    /// Measures one mount. Default: `statvfs` on Unix and `GetDiskFreeSpaceExW` on Windows.
    pub space: SpaceFn,
    /// A monotonic clock in milliseconds, for the debounce. Default: the system's.
    pub clock: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl Default for Options {
    fn default() -> Self {
        let start = std::time::Instant::now();
        Options {
            space_timeout: Duration::from_secs(2),
            debounce: Duration::from_millis(100),
            max_debounce: Duration::from_secs(1),
            space: Arc::new(system_space),
            clock: Arc::new(move || start.elapsed().as_millis() as u64),
        }
    }
}

struct State {
    revision: u64,
    volumes: Vec<Volume>,
}

type Emit = Arc<dyn Fn(VolumesChanged) + Send + Sync>;

/// Everything but the Tauri handle, so the logic is the same under a test.
pub(crate) struct Core {
    backend: Arc<dyn Backend>,
    measurer: Measurer,
    options: Options,
    state: Mutex<State>,
    /// Refreshes take turns, so an older reading of the system never overwrites a newer one.
    refreshing: tokio::sync::Mutex<()>,
    emit: Emit,
}

/// Two lists are the same when nothing but free space differs: free space changes with every file written and is no reason to tell the windows.
fn same_ignoring_space(a: &[Volume], b: &[Volume]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            let mut x = x.clone();
            let mut y = y.clone();
            x.free = None;
            y.free = None;
            x.total = None;
            y.total = None;
            x == y
        })
}

impl Core {
    pub(crate) fn new(backend: Arc<dyn Backend>, options: Options, emit: Emit) -> Arc<Self> {
        Arc::new(Core {
            measurer: Measurer::new(Arc::clone(&options.space), options.space_timeout),
            backend,
            options,
            state: Mutex::new(State {
                revision: 0,
                volumes: Vec::new(),
            }),
            refreshing: tokio::sync::Mutex::new(()),
            emit,
        })
    }

    pub(crate) async fn status(&self) -> PluginStatus {
        self.backend.status().await
    }

    /// The last list that was read, and its revision.
    pub(crate) fn current(&self) -> (u64, Vec<Volume>) {
        let state = self.state.lock().expect("state lock");
        (state.revision, state.volumes.clone())
    }

    /// Reads the volumes, measures the ones that are cheap to measure, and, if the list changed, bumps the revision and tells the windows.
    pub(crate) async fn refresh(&self, measure_network: bool) -> Result<Vec<Volume>> {
        let _turn = self.refreshing.lock().await;
        let mut volumes = self.backend.volumes().await?;
        let wanted: Vec<String> = volumes
            .iter()
            .filter(|volume| measure_network || volume.kind != VolumeKind::Network)
            .filter_map(|volume| volume.mount_point.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let answers = self.measurer.measure_all(wanted).await;
        for volume in &mut volumes {
            let space = volume
                .mount_point
                .as_ref()
                .and_then(|mount| answers.get(mount));
            if let Some(space) = space {
                volume.total = Some(space.total);
                volume.free = Some(space.free);
            }
        }
        let event = {
            let mut state = self.state.lock().expect("state lock");
            let changed = state.revision == 0 || !same_ignoring_space(&state.volumes, &volumes);
            state.volumes = volumes.clone();
            if changed {
                state.revision += 1;
                Some(VolumesChanged {
                    revision: state.revision,
                    volumes: volumes.clone(),
                })
            } else {
                None
            }
        };
        if let Some(event) = event {
            (self.emit)(event);
        }
        Ok(volumes)
    }

    /// Measures one volume, whatever its kind, and returns it. A stalled query leaves `free` absent.
    pub(crate) async fn refresh_space(&self, id: &str) -> Result<Volume> {
        let known = {
            let state = self.state.lock().expect("state lock");
            state.volumes.iter().find(|volume| volume.id == id).cloned()
        };
        let mut volume = match known {
            Some(volume) => volume,
            None => {
                // The list may not have been read yet.
                self.refresh(false).await?;
                let state = self.state.lock().expect("state lock");
                state
                    .volumes
                    .iter()
                    .find(|volume| volume.id == id)
                    .cloned()
                    .ok_or(VolumesError::NotFound)?
            }
        };
        let Some(mount) = volume.mount_point.clone() else {
            return Ok(volume);
        };
        let answers = self.measurer.measure_all(vec![mount.clone()]).await;
        if let Some(space) = answers.get(&mount) {
            volume.total = Some(space.total);
            volume.free = Some(space.free);
        }
        let mut state = self.state.lock().expect("state lock");
        if let Some(slot) = state.volumes.iter_mut().find(|v| v.id == id) {
            *slot = volume.clone();
        }
        Ok(volume)
    }

    pub(crate) async fn mount(&self, id: &str) -> Result<String> {
        let mount_point = self.backend.mount(id.to_string()).await?;
        let _ = self.refresh(false).await;
        Ok(mount_point)
    }

    pub(crate) async fn unmount(&self, id: &str) -> Result<()> {
        self.backend.unmount(id.to_string()).await?;
        let _ = self.refresh(false).await;
        Ok(())
    }

    pub(crate) async fn eject(&self, id: &str) -> Result<()> {
        self.backend.eject(id.to_string()).await?;
        let _ = self.refresh(false).await;
        Ok(())
    }

    pub(crate) async fn unlock(&self, id: &str, passphrase: Passphrase) -> Result<String> {
        let unlocked = self.backend.unlock(id.to_string(), passphrase).await?;
        let _ = self.refresh(false).await;
        Ok(unlocked)
    }

    /// Reads the first list and then keeps it current: the backend's notifications are debounced and each settled burst reads the list again. Runs until the plugin is dropped.
    pub(crate) async fn run(self: Arc<Self>) {
        if let Err(error) = self.refresh(false).await {
            log::warn!("volumes: the first listing failed: {error}");
        }
        if !self.backend.status().await.has(FEATURE_WATCH) {
            return;
        }
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<()>();
        let notify: Notify = Arc::new(move || {
            let _ = sender.send(());
        });
        if let Err(error) = self.backend.watch(notify).await {
            log::warn!("volumes: could not watch for changes: {error}");
            return;
        }
        let clock = Arc::clone(&self.options.clock);
        let mut debounce = Debounce::new(
            self.options.debounce.as_millis() as u64,
            self.options.max_debounce.as_millis() as u64,
        );
        while receiver.recv().await.is_some() {
            debounce.event(clock());
            while let Some(wait) = debounce.remaining(clock()) {
                if wait == 0 {
                    break;
                }
                tokio::select! {
                    event = receiver.recv() => {
                        if event.is_none() {
                            return;
                        }
                        debounce.event(clock());
                    }
                    _ = tokio::time::sleep(Duration::from_millis(wait)) => {}
                }
            }
            debounce.reset();
            if let Err(error) = self.refresh(false).await {
                log::warn!("volumes: listing after a change failed: {error}");
            }
        }
    }
}

/// The volumes of this system. Every action reports its own outcome: nothing is mounted, unmounted, ejected or unlocked silently.
pub struct Volumes<R: Runtime> {
    _app: AppHandle<R>,
    core: Arc<Core>,
}

impl<R: Runtime> Volumes<R> {
    pub(crate) fn new(app: AppHandle<R>, backend: Arc<dyn Backend>, options: Options) -> Self {
        let handle = app.clone();
        let emit: Emit = Arc::new(move |event| {
            if let Err(error) = handle.emit(EVENT_CHANGED, event) {
                log::warn!("volumes: could not emit {EVENT_CHANGED}: {error}");
            }
        });
        Volumes {
            _app: app,
            core: Core::new(backend, options, emit),
        }
    }

    pub(crate) fn core(&self) -> Arc<Core> {
        Arc::clone(&self.core)
    }

    /// What works on this system, and why anything does not.
    pub async fn get_status(&self) -> PluginStatus {
        self.core.status().await
    }

    /// Every volume now. Local volumes are measured within the space timeout (a stalled one has no `free`); network volumes are measured only when `measure_network` is true, or one at a time through [`Volumes::refresh_space`].
    pub async fn list(&self, measure_network: bool) -> Result<Vec<Volume>> {
        self.core.refresh(measure_network).await
    }

    /// The last list that was read and its revision (0 before the first), without asking the system.
    pub fn current(&self) -> (u64, Vec<Volume>) {
        self.core.current()
    }

    /// Measures one volume, network or not, within the space timeout.
    pub async fn refresh_space(&self, id: &str) -> Result<Volume> {
        self.core.refresh_space(id).await
    }

    /// Mounts a volume and returns its mount point.
    pub async fn mount(&self, id: &str) -> Result<String> {
        self.core.mount(id).await
    }

    pub async fn unmount(&self, id: &str) -> Result<()> {
        self.core.unmount(id).await
    }

    /// Unmounts everything on the volume's drive and ejects it.
    pub async fn eject(&self, id: &str) -> Result<()> {
        self.core.eject(id).await
    }

    /// Unlocks an encrypted volume and returns the id of the volume that appeared. The passphrase is not logged and not kept.
    pub async fn unlock(&self, id: &str, passphrase: Passphrase) -> Result<String> {
        self.core.unlock(id, passphrase).await
    }
}
