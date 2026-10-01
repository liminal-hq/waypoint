// Defines the serialisable models of the trash plugin: receipts, trashed items, targets and the status
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::TrashError;

/// Serialises a path as text (lossily for bytes that are not UTF-8) and reads one back from text. A JSON string cannot carry other bytes; the lossless handle for an item is its `trash_id`.
pub(crate) mod lossy_path {
    use std::path::{Path, PathBuf};

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(path: &Path, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&path.to_string_lossy())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<PathBuf, D::Error> {
        Ok(PathBuf::from(String::deserialize(deserializer)?))
    }
}

/// Proof that something was trashed, and the handle to restore or delete it later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct TrashReceipt {
    /// A stable id for the item: the trash it is in plus its name there on Linux, the Recycle Bin item's path on Windows. It stays valid across restarts and other apps, until the item leaves the trash.
    pub trash_id: String,
    /// Where the item was before it was trashed.
    #[serde(with = "lossy_path")]
    #[ts(type = "string")]
    pub original_path: PathBuf,
    /// When it was trashed, in seconds since the Unix epoch.
    #[ts(type = "number")]
    pub deleted_at: i64,
}

/// One item in the trash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct TrashedItem {
    pub receipt: TrashReceipt,
    /// The item's name before it was trashed (the trash itself may have renamed it to be unique).
    pub name: String,
    #[serde(with = "lossy_path")]
    #[ts(type = "string")]
    pub original_path: PathBuf,
    /// When it was trashed, in seconds since the Unix epoch.
    #[ts(type = "number")]
    pub deleted_at: i64,
    /// Size in bytes; for a folder, the total of the files in it.
    #[ts(type = "number")]
    pub size: u64,
    /// True for a folder (not for a link to one).
    pub is_dir: bool,
}

/// Where `restore` puts an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum RestoreTarget {
    /// Back where it was trashed from.
    Original,
    /// To this full path (including the item's name), which must not exist and whose folder must. It has to be on the volume the item is trashed on.
    Path {
        #[serde(with = "lossy_path")]
        #[ts(type = "string")]
        path: PathBuf,
    },
}

/// The result of trashing one path of a batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum TrashOutcome {
    Trashed { receipt: TrashReceipt },
    Failed { error: TrashError },
}

impl From<Result<TrashReceipt, TrashError>> for TrashOutcome {
    fn from(result: Result<TrashReceipt, TrashError>) -> Self {
        match result {
            Ok(receipt) => TrashOutcome::Trashed { receipt },
            Err(error) => TrashOutcome::Failed { error },
        }
    }
}

/// One item `empty` could not remove.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct EmptyFailure {
    pub trash_id: String,
    pub error: TrashError,
}

/// What `empty` did. Items that could not be removed are reported and left in the trash; the rest are still removed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct EmptyReport {
    /// How many items were removed.
    pub removed: u32,
    pub failed: Vec<EmptyFailure>,
}

/// Which implementation is behind the plugin on this system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Flavour {
    /// The plugin reads and writes the freedesktop.org Trash layout itself.
    Freedesktop,
    /// The Trash portal of a sandbox, which can only trash.
    Portal,
    /// The Windows Recycle Bin.
    Windows,
    /// No trash on this system.
    Unsupported,
}

pub const FEATURE_TRASH: &str = "trash";
pub const FEATURE_LIST: &str = "list";
pub const FEATURE_RESTORE: &str = "restore";
pub const FEATURE_EMPTY: &str = "empty";
pub const FEATURE_EXPIRY: &str = "expiry";
pub const FEATURE_PER_VOLUME: &str = "per-volume";

/// Whether one feature of the plugin works on this system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct FeatureStatus {
    /// One of `trash`, `list`, `restore`, `empty`, `expiry` or `per-volume`.
    pub name: String,
    pub available: bool,
    /// Why the feature is unavailable or degraded; absent when it works fully.
    pub reason: Option<String>,
}

/// What the plugin can do on the running system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PluginStatus {
    /// True when at least one feature is available.
    pub available: bool,
    /// Why nothing is available; absent otherwise.
    pub reason: Option<String>,
    pub flavour: Flavour,
    pub features: Vec<FeatureStatus>,
}

impl PluginStatus {
    /// Builds a status from the six features' availability; `reasons` gives the reason for each unavailable (or degraded) one.
    pub fn build(flavour: Flavour, features: Vec<FeatureStatus>) -> Self {
        let available = features.iter().any(|feature| feature.available);
        let reason = if available {
            None
        } else {
            features.iter().find_map(|feature| feature.reason.clone())
        };
        PluginStatus {
            available,
            reason,
            flavour,
            features,
        }
    }
}

impl FeatureStatus {
    pub fn available(name: &str) -> Self {
        FeatureStatus {
            name: name.to_string(),
            available: true,
            reason: None,
        }
    }

    pub fn unavailable(name: &str, reason: impl Into<String>) -> Self {
        FeatureStatus {
            name: name.to_string(),
            available: false,
            reason: Some(reason.into()),
        }
    }
}
