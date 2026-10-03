// Reports thumbnails unavailable on systems with neither the freedesktop cache nor the Windows shell
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::{AppHandle, Runtime};

use crate::cache::{file_uri, Store};
use crate::engine::{Limits, Outcome, Processor};
use crate::memcache::MemCache;
use crate::models::{
    Config, FeatureStatus, Flavour, PluginStatus, ReasonKind, SkipWhy, ThumbRequest,
    FEATURE_BUILTIN, FEATURE_CACHE, FEATURE_EXTERNAL, FEATURE_SHELL,
};

const REASON: &str = "this system has no thumbnail support";

/// Nothing to inject: this platform has nothing to configure.
#[derive(Debug, Clone, Default)]
pub struct Env;

impl Env {
    pub fn system<R: Runtime>(_app: &AppHandle<R>) -> Env {
        Env
    }
}

pub struct Platform {
    store: Arc<Store>,
}

impl Platform {
    pub fn new(_env: Env, config: &Config, _mem: Arc<MemCache>, _limits: Arc<Limits>) -> Self {
        // Never written to: no request gets as far as the cache.
        let store = Store::new(
            PathBuf::new(),
            &config.app_name,
            &config.app_version,
            Arc::new(file_uri),
        );
        Platform {
            store: Arc::new(store),
        }
    }

    pub fn store(&self) -> Arc<Store> {
        Arc::clone(&self.store)
    }

    pub fn processor(&self) -> Arc<dyn Processor> {
        Arc::new(Unsupported)
    }

    pub fn status(&self) -> PluginStatus {
        PluginStatus::build(
            Flavour::Unsupported,
            [
                FEATURE_CACHE,
                FEATURE_BUILTIN,
                FEATURE_EXTERNAL,
                FEATURE_SHELL,
            ]
            .into_iter()
            .map(|name| FeatureStatus::unavailable(name, ReasonKind::Unsupported, REASON))
            .collect(),
        )
    }
}

struct Unsupported;

impl Processor for Unsupported {
    fn process(&self, _request: &ThumbRequest, _cancel: &AtomicBool) -> Outcome {
        Outcome::Skipped {
            why: SkipWhy::Unsupported,
        }
    }
}
