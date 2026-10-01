// The Rust API of the plugin: the `Trash` handle behind `app.trash()`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Runtime};

use crate::error::TrashError;
use crate::models::{EmptyReport, PluginStatus, RestoreTarget, TrashReceipt, TrashedItem};
use crate::platform::Platform;

/// The trash of this system. Every operation reports its own outcome: nothing is trashed, restored or removed silently.
pub struct Trash<R: Runtime> {
    app: AppHandle<R>,
    platform: Arc<Platform>,
}

impl<R: Runtime> Trash<R> {
    pub(crate) fn new(app: AppHandle<R>, platform: Platform) -> Self {
        Trash {
            app,
            platform: Arc::new(platform),
        }
    }

    /// What works on this system, and why anything does not.
    pub fn get_status(&self) -> PluginStatus {
        self.platform.status()
    }

    /// Moves each path to the trash, as one batch. The result has one entry per path, in order: a path that fails does not stop the others, and a failure is never swallowed.
    pub async fn trash(&self, paths: Vec<PathBuf>) -> Vec<Result<TrashReceipt, TrashError>> {
        self.platform.trash(&self.app, paths).await
    }

    /// Everything in the trash, oldest first.
    pub async fn list(&self) -> Result<Vec<TrashedItem>, TrashError> {
        self.platform.list(&self.app).await
    }

    /// Puts an item back where it was (or at a path the caller chooses). Never overwrites.
    pub async fn restore(
        &self,
        receipt: &TrashReceipt,
        target: RestoreTarget,
    ) -> Result<TrashReceipt, TrashError> {
        self.platform.restore(&self.app, receipt, target).await
    }

    /// Removes one item from the trash for good.
    pub async fn delete(&self, receipt: &TrashReceipt) -> Result<(), TrashError> {
        self.platform.delete(&self.app, receipt).await
    }

    /// Empties the trash, or with `older_than_days` only the items trashed that many days ago or earlier.
    pub async fn empty(&self, older_than_days: Option<u32>) -> Result<EmptyReport, TrashError> {
        self.platform.empty(&self.app, older_than_days).await
    }
}
