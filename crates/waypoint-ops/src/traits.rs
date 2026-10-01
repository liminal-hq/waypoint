// The seams the engine reaches the world through: the Trash, selections, settings, providers, the
// clock and the id source. The app implements them; tests substitute fakes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_path::{CaseRule, VfsPath};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{ListingHandle, Provider, SelectionSpec};

use crate::model::{OpsError, OpsSettings};

/// What the Trash hands back for an item it took, which is everything needed to put it back or
/// delete it for good.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct TrashReceipt {
    /// Names the item in the Trash; meaningful to the implementation only.
    pub id: String,
    /// Where the item was before it was trashed.
    pub original: Location,
    /// When it was trashed, in milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub deleted_at: i64,
}

/// The Trash (A53). The app implements it over the `trash` plugin; `FakeTrash` implements it over
/// a provider for tests.
pub trait Trash: Send + Sync {
    /// Whether trashing works here, and if not why (shown in the Services panel and in the error).
    fn available(&self) -> Result<(), String>;

    /// Moves each location to the Trash, one result per location in order. One item failing does
    /// not stop the others.
    fn trash(&self, items: &[Location]) -> Vec<Result<TrashReceipt, OpsError>>;

    /// Puts an item back where it was and returns that place. A name already taken there is
    /// `NameInUse`, and nothing is replaced.
    fn restore(&self, receipt: &TrashReceipt) -> Result<Location, OpsError>;

    /// Deletes an item in the Trash for good.
    fn delete(&self, receipt: &TrashReceipt) -> Result<(), OpsError>;

    /// Deletes everything in the Trash, or only what has been there more than `older_than_days`
    /// days, and returns how many items went.
    fn empty(&self, older_than_days: Option<u32>) -> Result<u64, OpsError>;

    /// The receipt for an item the Trash view shows at `trashed`.
    fn receipt_for(&self, trashed: &Location) -> Result<TrashReceipt, OpsError>;
}

/// Turns what a window has selected into the locations it covers (A47). `src-tauri` implements it
/// over the vfs plugin's listings.
pub trait SelectionResolver: Send + Sync {
    fn resolve(
        &self,
        handle: ListingHandle,
        spec: &SelectionSpec,
        window: &str,
    ) -> Result<Vec<Location>, OpsError>;
}

/// Where the engine reads its settings. It reads on every use, so a change in the Settings window
/// reaches the next job.
pub trait SettingsReader: Send + Sync {
    fn ops_settings(&self) -> OpsSettings;
}

/// Settings that never change.
#[derive(Debug, Clone, Copy, Default)]
pub struct StaticSettings(pub OpsSettings);

impl SettingsReader for StaticSettings {
    fn ops_settings(&self) -> OpsSettings {
        self.0
    }
}

/// The time, in milliseconds since the Unix epoch.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> i64;
}

/// The system clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(after) => after.as_millis() as i64,
            Err(before) => -(before.duration().as_millis() as i64),
        }
    }
}

/// A source of numbers that are not repeated, for the names of partial files and Trash items.
pub trait IdSource: Send + Sync {
    fn next(&self) -> u64;
}

/// Counts up from 1, so names are the same on every run.
#[derive(Debug, Default)]
pub struct CounterIds(AtomicU64);

impl IdSource for CounterIds {
    fn next(&self) -> u64 {
        self.0.fetch_add(1, Ordering::Relaxed) + 1
    }
}

/// The providers the engine can serve, by scheme. A path goes to the provider that owns its
/// scheme, so one engine serves local folders and, later, remote ones.
#[derive(Clone, Default)]
pub struct Providers {
    by_scheme: HashMap<&'static str, Arc<dyn Provider>>,
}

impl Providers {
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry holding one provider.
    pub fn single(provider: Arc<dyn Provider>) -> Self {
        let mut providers = Self::new();
        providers.register(provider);
        providers
    }

    /// Adds a provider under its scheme, replacing any earlier one for that scheme.
    pub fn register(&mut self, provider: Arc<dyn Provider>) {
        self.by_scheme.insert(provider.scheme(), provider);
    }

    pub fn for_path(&self, path: &VfsPath) -> Result<Arc<dyn Provider>, OpsError> {
        self.by_scheme
            .get(path.scheme())
            .cloned()
            .ok_or_else(|| OpsError::Unsupported {
                what: format!("the {} scheme", path.scheme()),
            })
    }

    /// Reads a location's URI to a path and finds its provider.
    pub fn for_location(
        &self,
        location: &Location,
    ) -> Result<(VfsPath, Arc<dyn Provider>), OpsError> {
        let path = VfsPath::from_location(location).map_err(|_| {
            OpsError::from(VfsError::InvalidLocation {
                input: location.uri.clone(),
            })
        })?;
        let provider = self.for_path(&path)?;
        Ok((path, provider))
    }

    /// How the provider of `path` compares names.
    pub fn case_rule(&self, path: &VfsPath) -> Result<CaseRule, OpsError> {
        Ok(self.for_path(path)?.capabilities().case_rule)
    }
}

/// Paths an operation that removes or moves things never touches: filesystem roots, the home
/// folder, mount points. The app fills it in; the engine reads no environment itself.
#[derive(Debug, Clone, Default)]
pub struct Protected {
    paths: Vec<VfsPath>,
}

impl Protected {
    pub fn new(paths: Vec<VfsPath>) -> Self {
        Self { paths }
    }

    /// Whether `path` is a root or one of the listed paths (compared under `rule`).
    pub fn contains(&self, path: &VfsPath, rule: CaseRule) -> bool {
        path.parent().is_none()
            || self
                .paths
                .iter()
                .any(|p| crate::names::same_path(p, path, rule))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_ids_count_up_from_one() {
        let ids = CounterIds::default();
        assert_eq!((ids.next(), ids.next(), ids.next()), (1, 2, 3));
    }

    #[test]
    fn the_system_clock_is_after_2020() {
        assert!(SystemClock.now_ms() > 1_577_836_800_000);
    }

    #[test]
    fn an_unknown_scheme_is_unsupported() {
        let providers = Providers::new();
        let path = VfsPath::parse_input(if cfg!(windows) { r"C:\a" } else { "/a" }).unwrap();
        assert!(matches!(
            providers.for_path(&path),
            Err(OpsError::Unsupported { .. })
        ));
    }
}
