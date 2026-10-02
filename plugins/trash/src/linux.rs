// The Linux backend: the freedesktop trash, or the Trash portal inside Flatpak
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Runtime};

use crate::error::TrashError;
use crate::freedesktop::{default_env, Freedesktop, TrashEnv};
use crate::models::{
    EmptyReport, FeatureStatus, Flavour, PluginStatus, RestoreTarget, TrashReceipt, TrashedItem,
    FEATURE_EMPTY, FEATURE_EXPIRY, FEATURE_LIST, FEATURE_PER_VOLUME, FEATURE_RESTORE,
    FEATURE_TRASH,
};

mod portal;

/// Gives the environment for each operation. The real mount table changes as volumes come and go, so the real source reads it afresh; a test's source is one fixed environment.
type EnvSource = Arc<dyn Fn() -> TrashEnv + Send + Sync>;

/// Every command builds its own `Freedesktop`, so nothing else makes two commands take turns. Without this, an `empty` could remove the `.trashinfo` of an item that a `trash` has just reserved and is still moving into `files/`, leaving an orphan. The commands that change a trash hold it; `list` only reads and does not.
static CHANGES: Mutex<()> = Mutex::new(());

enum Mode {
    Native(EnvSource),
    Portal,
}

pub struct Platform {
    mode: Mode,
}

impl Platform {
    /// The real system: the Trash portal inside Flatpak, the freedesktop trash otherwise.
    pub fn new() -> Self {
        let mode = if Path::new("/.flatpak-info").exists() {
            Mode::Portal
        } else {
            Mode::Native(Arc::new(default_env))
        };
        Platform { mode }
    }

    /// The freedesktop trash over a fixed environment.
    pub fn with_env(env: TrashEnv) -> Self {
        Platform {
            mode: Mode::Native(Arc::new(move || env.clone())),
        }
    }

    pub fn flavour(&self) -> Flavour {
        match self.mode {
            Mode::Native(_) => Flavour::Freedesktop,
            Mode::Portal => Flavour::Portal,
        }
    }

    pub fn status(&self) -> PluginStatus {
        let features = match self.mode {
            Mode::Native(_) => [
                FEATURE_TRASH,
                FEATURE_LIST,
                FEATURE_RESTORE,
                FEATURE_EMPTY,
                FEATURE_EXPIRY,
                FEATURE_PER_VOLUME,
            ]
            .into_iter()
            .map(FeatureStatus::available)
            .collect(),
            Mode::Portal => {
                let only_trashes = "the Trash portal can only move files to the trash";
                vec![
                    FeatureStatus::available(FEATURE_TRASH),
                    FeatureStatus::unavailable(FEATURE_LIST, only_trashes),
                    FeatureStatus::unavailable(FEATURE_RESTORE, only_trashes),
                    FeatureStatus::unavailable(FEATURE_EMPTY, only_trashes),
                    FeatureStatus::unavailable(FEATURE_EXPIRY, only_trashes),
                    FeatureStatus::unavailable(
                        FEATURE_PER_VOLUME,
                        "the portal chooses the trash, and the app cannot see which",
                    ),
                ]
            }
        };
        PluginStatus::build(self.flavour(), features)
    }

    /// Runs `job` over the freedesktop trash on a blocking thread.
    async fn native<T: Send + 'static>(
        source: &EnvSource,
        job: impl FnOnce(Freedesktop) -> T + Send + 'static,
    ) -> Result<T, TrashError> {
        let source = Arc::clone(source);
        tauri::async_runtime::spawn_blocking(move || job(Freedesktop::new(source())))
            .await
            .map_err(|error| TrashError::io(format!("the trash task failed: {error}")))
    }

    /// Like `native`, for a job that changes a trash: it waits its turn behind any other such job (see `CHANGES`).
    async fn native_change<T: Send + 'static>(
        source: &EnvSource,
        job: impl FnOnce(Freedesktop) -> T + Send + 'static,
    ) -> Result<T, TrashError> {
        Self::native(source, move |trash| {
            let _turn = CHANGES
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            job(trash)
        })
        .await
    }

    pub async fn trash<R: Runtime>(
        &self,
        _app: &AppHandle<R>,
        paths: Vec<PathBuf>,
    ) -> Vec<Result<TrashReceipt, TrashError>> {
        match &self.mode {
            Mode::Native(source) => {
                let count = paths.len();
                let batch = paths.clone();
                match Self::native_change(source, move |trash| {
                    batch
                        .iter()
                        .map(|path| trash.trash(path))
                        .collect::<Vec<_>>()
                })
                .await
                {
                    Ok(results) => results,
                    Err(error) => (0..count).map(|_| Err(error.clone())).collect(),
                }
            }
            Mode::Portal => {
                let mut results = Vec::with_capacity(paths.len());
                for path in &paths {
                    results.push(portal::trash(path).await);
                }
                results
            }
        }
    }

    pub async fn list<R: Runtime>(
        &self,
        _app: &AppHandle<R>,
    ) -> Result<Vec<TrashedItem>, TrashError> {
        match &self.mode {
            Mode::Native(source) => Self::native(source, |trash| trash.list()).await,
            Mode::Portal => Err(TrashError::Unsupported),
        }
    }

    pub async fn restore<R: Runtime>(
        &self,
        _app: &AppHandle<R>,
        receipt: &TrashReceipt,
        target: RestoreTarget,
    ) -> Result<TrashReceipt, TrashError> {
        match &self.mode {
            Mode::Native(source) => {
                let id = receipt.trash_id.clone();
                Self::native_change(source, move |trash| trash.restore(&id, &target)).await?
            }
            Mode::Portal => Err(TrashError::Unsupported),
        }
    }

    pub async fn delete<R: Runtime>(
        &self,
        _app: &AppHandle<R>,
        receipt: &TrashReceipt,
    ) -> Result<(), TrashError> {
        match &self.mode {
            Mode::Native(source) => {
                let id = receipt.trash_id.clone();
                Self::native_change(source, move |trash| trash.delete(&id)).await?
            }
            Mode::Portal => Err(TrashError::Unsupported),
        }
    }

    pub async fn empty<R: Runtime>(
        &self,
        _app: &AppHandle<R>,
        older_than_days: Option<u32>,
    ) -> Result<EmptyReport, TrashError> {
        match &self.mode {
            Mode::Native(source) => {
                Self::native_change(source, move |trash| trash.empty(older_than_days)).await
            }
            Mode::Portal => Err(TrashError::Unsupported),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_flatpak_only_trashing_is_available_and_the_rest_says_why() {
        let status = Platform { mode: Mode::Portal }.status();
        assert!(status.available);
        assert_eq!(status.flavour, Flavour::Portal);
        let available: Vec<&str> = status
            .features
            .iter()
            .filter(|feature| feature.available)
            .map(|feature| feature.name.as_str())
            .collect();
        assert_eq!(available, [FEATURE_TRASH]);
        for feature in status.features.iter().filter(|feature| !feature.available) {
            assert!(
                feature
                    .reason
                    .as_deref()
                    .is_some_and(|reason| !reason.is_empty()),
                "{feature:?}"
            );
        }
    }

    #[test]
    fn commands_that_change_a_trash_take_turns() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::Duration;

        let platform = Arc::new(Platform::with_env(TrashEnv {
            data_home: PathBuf::from("/nonexistent/share"),
            home_dir: PathBuf::from("/nonexistent"),
            uid: 1000,
            mounts: Vec::new(),
            now: crate::freedesktop::system_clock(),
        }));
        let done = Arc::new(AtomicBool::new(false));
        let turn = CHANGES.lock().unwrap();
        let worker = {
            let (platform, done) = (Arc::clone(&platform), Arc::clone(&done));
            std::thread::spawn(move || {
                let app = tauri::test::mock_app();
                let handle = app.handle().clone();
                tauri::async_runtime::block_on(platform.empty(&handle, None)).unwrap();
                done.store(true, Ordering::SeqCst);
            })
        };
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            !done.load(Ordering::SeqCst),
            "`empty` ran while another change held the turn"
        );
        drop(turn);
        worker.join().unwrap();
        assert!(done.load(Ordering::SeqCst));
    }

    #[test]
    fn the_portal_flavour_has_no_list_restore_or_empty() {
        let platform = Platform { mode: Mode::Portal };
        let app = tauri::test::mock_app();
        let handle = app.handle().clone();
        tauri::async_runtime::block_on(async {
            assert_eq!(platform.list(&handle).await, Err(TrashError::Unsupported));
            let receipt = TrashReceipt {
                trash_id: "x".into(),
                original_path: PathBuf::from("/x"),
                deleted_at: 0,
            };
            assert_eq!(
                platform
                    .restore(&handle, &receipt, RestoreTarget::Original)
                    .await,
                Err(TrashError::Unsupported)
            );
            assert_eq!(
                platform.delete(&handle, &receipt).await,
                Err(TrashError::Unsupported)
            );
            assert_eq!(
                platform.empty(&handle, None).await,
                Err(TrashError::Unsupported)
            );
        });
    }
}
