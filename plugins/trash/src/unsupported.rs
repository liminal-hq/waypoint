// Reports the trash unavailable on systems with no trash support
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use tauri::{AppHandle, Runtime};

use crate::error::TrashError;
use crate::models::{
    EmptyReport, FeatureStatus, Flavour, PluginStatus, RestoreTarget, TrashReceipt, TrashedItem,
    FEATURE_EMPTY, FEATURE_EXPIRY, FEATURE_LIST, FEATURE_PER_VOLUME, FEATURE_RESTORE,
    FEATURE_TRASH,
};

const REASON: &str = "this system has no trash support";

pub struct Platform;

impl Platform {
    pub fn new() -> Self {
        Platform
    }

    pub fn status(&self) -> PluginStatus {
        PluginStatus::build(
            Flavour::Unsupported,
            [
                FEATURE_TRASH,
                FEATURE_LIST,
                FEATURE_RESTORE,
                FEATURE_EMPTY,
                FEATURE_EXPIRY,
                FEATURE_PER_VOLUME,
            ]
            .into_iter()
            .map(|name| FeatureStatus::unavailable(name, REASON))
            .collect(),
        )
    }

    pub async fn trash<R: Runtime>(
        &self,
        _app: &AppHandle<R>,
        paths: Vec<PathBuf>,
    ) -> Vec<Result<TrashReceipt, TrashError>> {
        paths.iter().map(|_| Err(TrashError::Unsupported)).collect()
    }

    pub async fn list<R: Runtime>(
        &self,
        _app: &AppHandle<R>,
    ) -> Result<Vec<TrashedItem>, TrashError> {
        Err(TrashError::Unsupported)
    }

    pub async fn restore<R: Runtime>(
        &self,
        _app: &AppHandle<R>,
        _receipt: &TrashReceipt,
        _target: RestoreTarget,
    ) -> Result<TrashReceipt, TrashError> {
        Err(TrashError::Unsupported)
    }

    pub async fn delete<R: Runtime>(
        &self,
        _app: &AppHandle<R>,
        _receipt: &TrashReceipt,
    ) -> Result<(), TrashError> {
        Err(TrashError::Unsupported)
    }

    pub async fn empty<R: Runtime>(
        &self,
        _app: &AppHandle<R>,
        _older_than_days: Option<u32>,
    ) -> Result<EmptyReport, TrashError> {
        Err(TrashError::Unsupported)
    }
}
