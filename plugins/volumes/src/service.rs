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
    FeatureStatus, Passphrase, PluginStatus, Reason, RememberOutcome, Unlocked, Volume, VolumeKind,
    VolumesChanged, EVENT_CHANGED, FEATURE_REMEMBER, FEATURE_UNLOCK, FEATURE_WATCH,
};
use crate::passphrases::SharedStore;
use crate::space::{system_space, Measurer, SpaceFn};

/// How many failed reads of a remembered passphrase are tolerated for one plugged-in volume.
const MAX_RECALL_FAILURES: u32 = 3;

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
    /// Where the passphrases of encrypted volumes are kept, when the host supplies one. Without it nothing can be remembered and the `remember` feature says `not-configured`. Default: none.
    pub passphrases: Option<SharedStore>,
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
            passphrases: None,
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
    /// The containers (by UUID) a remembered passphrase was already tried on since they were plugged in, so a wrong one is not tried again and again.
    attempted: Mutex<HashSet<String>>,
    /// How many times reading a container's remembered passphrase failed (a locked keyring, a dismissed prompt) since it was plugged in. A failed read is retried on the next change, up to `MAX_RECALL_FAILURES`, so a locked keyring is not asked in a loop.
    recall_failures: Mutex<std::collections::HashMap<String, u32>>,
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
            attempted: Mutex::new(HashSet::new()),
            recall_failures: Mutex::new(std::collections::HashMap::new()),
            emit,
        })
    }

    pub(crate) async fn status(&self) -> PluginStatus {
        let mut status = self.backend.status().await;
        let remember = self.remember_status(&status).await;
        // Only the feature: the plugin's own reason stays the backend's, so a host that keeps passphrases off does not make the whole plugin look broken.
        status.features.push(remember);
        status
    }

    async fn remember_status(&self, backend: &PluginStatus) -> FeatureStatus {
        if !backend.has(FEATURE_UNLOCK) {
            return FeatureStatus::unavailable(
                FEATURE_REMEMBER,
                Reason::NotSupported,
                "this system cannot unlock encrypted volumes",
            );
        }
        match &self.options.passphrases {
            None => FeatureStatus::unavailable(
                FEATURE_REMEMBER,
                Reason::NotConfigured,
                "this app does not keep passphrases",
            ),
            Some(store) => match store.status().await {
                None => FeatureStatus::available(FEATURE_REMEMBER),
                Some(why) => FeatureStatus::unavailable(FEATURE_REMEMBER, why.reason, why.message),
            },
        }
    }

    /// Marks the encrypted volumes whose passphrase is kept. Asks only when passphrases can be read at all, and never prompts.
    async fn mark_remembered(&self, volumes: &mut [Volume]) {
        let Some(store) = &self.options.passphrases else {
            return;
        };
        if !volumes.iter().any(|volume| volume.uuid.is_some()) || store.status().await.is_some() {
            return;
        }
        for volume in volumes.iter_mut() {
            if let Some(uuid) = volume.uuid.clone() {
                volume.remembered = store.has(uuid).await.unwrap_or(false);
            }
        }
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
        self.mark_remembered(&mut volumes).await;
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

    /// Reads the list first when it has not been read yet, so a volume the caller names is known.
    async fn ensure_listed(&self) {
        let unread = self.state.lock().expect("state lock").revision == 0;
        if unread {
            let _ = self.refresh(false).await;
        }
    }

    /// The UUID of the container of the volume with this id, from the last list.
    fn uuid_of(&self, id: &str) -> Option<String> {
        let state = self.state.lock().expect("state lock");
        state
            .volumes
            .iter()
            .find(|volume| volume.id == id)
            .and_then(|volume| volume.uuid.clone())
    }

    /// Unlocks, and keeps the passphrase when asked to and when that works. A passphrase that could not be kept does not undo the unlock: the outcome says why.
    pub(crate) async fn unlock(
        &self,
        id: &str,
        passphrase: Passphrase,
        remember: bool,
    ) -> Result<Unlocked> {
        self.ensure_listed().await;
        let uuid = self.uuid_of(id);
        let kept = remember.then(|| passphrase.clone());
        let opened = self.backend.unlock(id.to_string(), passphrase).await?;
        let outcome = match kept {
            None => RememberOutcome::NotAsked,
            Some(passphrase) => self.remember(uuid, passphrase).await,
        };
        if let Some(uuid) = self.uuid_of(id) {
            // The person just unlocked it by hand: nothing for the automatic unlock to do on it.
            self.attempted.lock().expect("attempted lock").insert(uuid);
        }
        let _ = self.refresh(false).await;
        Ok(Unlocked {
            id: opened,
            remember: outcome,
        })
    }

    async fn remember(&self, uuid: Option<String>, passphrase: Passphrase) -> RememberOutcome {
        let failed = |reason, message: &str| RememberOutcome::Failed {
            reason,
            message: message.to_string(),
        };
        let Some(store) = &self.options.passphrases else {
            return failed(Reason::NotConfigured, "this app does not keep passphrases");
        };
        let Some(uuid) = uuid else {
            return failed(
                Reason::NotSupported,
                "this volume has no identifier to remember it by",
            );
        };
        if let Some(why) = store.status().await {
            return RememberOutcome::Failed {
                reason: why.reason,
                message: why.message,
            };
        }
        match store.remember(uuid, passphrase).await {
            Ok(()) => RememberOutcome::Remembered,
            Err(why) => RememberOutcome::Failed {
                reason: why.reason,
                message: why.message,
            },
        }
    }

    /// Forgets the passphrase kept for a volume; true when there was one. It works while remembering is switched off, so what was kept can still be removed.
    pub(crate) async fn forget(&self, id: &str) -> Result<bool> {
        let Some(store) = &self.options.passphrases else {
            return Err(VolumesError::Unsupported);
        };
        self.ensure_listed().await;
        let uuid = {
            let state = self.state.lock().expect("state lock");
            let volume = state
                .volumes
                .iter()
                .find(|volume| volume.id == id)
                .ok_or(VolumesError::NotFound)?;
            volume.uuid.clone()
        };
        let Some(uuid) = uuid else {
            return Ok(false);
        };
        let forgotten = store
            .forget(uuid)
            .await
            .map_err(|why| VolumesError::io(why.message))?;
        let _ = self.refresh(false).await;
        Ok(forgotten)
    }

    /// Unlocks, once, each locked volume that has a remembered passphrase and was not tried since it was plugged in, and mounts it as unlocking by hand does. A wrong passphrase leaves the volume locked and is not tried again until the volume is plugged in anew; a passphrase that cannot be read (a locked keyring) is asked for again on later changes, up to three times.
    pub(crate) async fn auto_unlock(&self) {
        let Some(store) = &self.options.passphrases else {
            return;
        };
        let (_, volumes) = self.current();
        {
            // Volumes that went away may be tried again when they return.
            let present: HashSet<&str> = volumes.iter().filter_map(|v| v.uuid.as_deref()).collect();
            self.attempted
                .lock()
                .expect("attempted lock")
                .retain(|uuid| present.contains(uuid.as_str()));
            self.recall_failures
                .lock()
                .expect("failures lock")
                .retain(|uuid, _| present.contains(uuid.as_str()));
        }
        let wanted: Vec<&Volume> = volumes
            .iter()
            .filter(|volume| volume.locked && volume.uuid.is_some())
            .collect();
        if wanted.is_empty() || store.status().await.is_some() {
            return;
        }
        for volume in wanted {
            let uuid = volume.uuid.clone().expect("filtered on a uuid");
            if self
                .attempted
                .lock()
                .expect("attempted lock")
                .contains(&uuid)
            {
                continue;
            }
            let failures = self
                .recall_failures
                .lock()
                .expect("failures lock")
                .get(&uuid)
                .copied()
                .unwrap_or(0);
            if failures >= MAX_RECALL_FAILURES {
                continue;
            }
            let passphrase = match store.recall(uuid.clone()).await {
                Ok(Some(passphrase)) => passphrase,
                // Nothing kept for this volume: nothing to try, and nothing prompted, so it stays open to a later try.
                Ok(None) => continue,
                Err(why) => {
                    // A locked keyring or a dismissed prompt: try again on a later change, a few times.
                    *self
                        .recall_failures
                        .lock()
                        .expect("failures lock")
                        .entry(uuid)
                        .or_insert(0) += 1;
                    log::debug!(
                        "volumes: could not read the remembered passphrase for {}: {}",
                        volume.label,
                        why.message
                    );
                    continue;
                }
            };
            // Only now is the volume tried: a wrong passphrase is not tried again until it is plugged in anew.
            self.attempted.lock().expect("attempted lock").insert(uuid);
            match self.backend.unlock(volume.id.clone(), passphrase).await {
                Ok(opened) => {
                    log::info!(
                        "volumes: unlocked {} with a remembered passphrase",
                        volume.label
                    );
                    if let Err(error) = self.backend.mount(opened).await {
                        log::warn!(
                            "volumes: could not mount {} after unlocking it: {error}",
                            volume.label
                        );
                    }
                    let _ = self.refresh(false).await;
                }
                Err(error) => {
                    log::warn!(
                        "volumes: the remembered passphrase for {} did not unlock it: {error}",
                        volume.label
                    );
                }
            }
        }
    }

    /// Reads the first list and then keeps it current: the backend's notifications are debounced and each settled burst reads the list again. Runs until the plugin is dropped.
    pub(crate) async fn run(self: Arc<Self>) {
        if let Err(error) = self.refresh(false).await {
            log::warn!("volumes: the first listing failed: {error}");
        }
        self.auto_unlock().await;
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
            self.auto_unlock().await;
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
        Ok(self.core.unlock(id, passphrase, false).await?.id)
    }

    /// Unlocks an encrypted volume and, when `remember` is true, keeps the passphrase through the host's [`crate::PassphraseStore`] so the volume unlocks by itself next time. Failing to keep it does not undo the unlock; the outcome says why.
    pub async fn unlock_and_remember(
        &self,
        id: &str,
        passphrase: Passphrase,
        remember: bool,
    ) -> Result<Unlocked> {
        self.core.unlock(id, passphrase, remember).await
    }

    /// Forgets the passphrase kept for an encrypted volume; true when there was one.
    pub async fn forget(&self, id: &str) -> Result<bool> {
        self.core.forget(id).await
    }
}
