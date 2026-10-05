// Defines the serialisable models of the thumbnails plugin: requests, events, sizes and the status
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The four sizes of the freedesktop.org Thumbnail Managing Standard. A thumbnail fits within a square of `pixels()` on each side and is never scaled up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum ThumbSize {
    #[serde(rename = "normal")]
    #[ts(rename = "normal")]
    Normal,
    #[serde(rename = "large")]
    #[ts(rename = "large")]
    Large,
    #[serde(rename = "x-large")]
    #[ts(rename = "x-large")]
    XLarge,
    #[serde(rename = "xx-large")]
    #[ts(rename = "xx-large")]
    XXLarge,
}

impl ThumbSize {
    pub const ALL: [ThumbSize; 4] = [
        ThumbSize::Normal,
        ThumbSize::Large,
        ThumbSize::XLarge,
        ThumbSize::XXLarge,
    ];

    /// The longest side of a thumbnail of this size, in pixels.
    pub fn pixels(self) -> u32 {
        match self {
            ThumbSize::Normal => 128,
            ThumbSize::Large => 256,
            ThumbSize::XLarge => 512,
            ThumbSize::XXLarge => 1024,
        }
    }

    /// The name of the cache folder for this size.
    pub fn dir_name(self) -> &'static str {
        match self {
            ThumbSize::Normal => "normal",
            ThumbSize::Large => "large",
            ThumbSize::XLarge => "x-large",
            ThumbSize::XXLarge => "xx-large",
        }
    }

    pub fn from_dir_name(name: &str) -> Option<ThumbSize> {
        ThumbSize::ALL
            .into_iter()
            .find(|size| size.dir_name() == name)
    }
}

/// One thumbnail the caller wants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct ThumbRequest {
    /// The caller's own handle for the item, returned in every event for it. Requests with the same key are one job.
    pub key: String,
    /// The absolute path of a local file.
    pub path: String,
    pub size: ThumbSize,
    /// The file's modified time in milliseconds since the Unix epoch; a cached thumbnail made from another time is stale.
    #[ts(type = "number")]
    pub mtime_ms: i64,
}

/// Why a request was skipped rather than failed: nothing is wrong, the plugin chose not to (or cannot) make this thumbnail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum SkipWhy {
    /// The path is not a local file (a remote location).
    Remote,
    /// The file is larger than the limit for decoding it.
    TooLarge,
    /// The file is a cloud placeholder with no thumbnail already cached; making one would download it.
    Cloud,
    /// No generator on this system handles this kind of file.
    NoGenerator,
    /// Thumbnails are not supported on this system.
    Unsupported,
}

/// What happened to one requested thumbnail, sent through the request's channel. A cancelled request gets nothing more.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum ThumbEvent {
    /// The thumbnail is ready: load `url` (a `thumb://` address that names a cache entry, never a path).
    Ready { key: String, url: String },
    /// Making the thumbnail failed; the failure is remembered, so it is not retried until the file changes.
    Failed { key: String, reason: String },
    /// No thumbnail was made, and that is not an error.
    Skipped { key: String, why: SkipWhy },
}

impl ThumbEvent {
    pub fn key(&self) -> &str {
        match self {
            ThumbEvent::Ready { key, .. }
            | ThumbEvent::Failed { key, .. }
            | ThumbEvent::Skipped { key, .. } => key,
        }
    }
}

/// The handle of one `request`, to cancel or reprioritise it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ticket(pub u64);

/// Which implementation is behind the plugin on this system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Flavour {
    /// The freedesktop.org thumbnail cache, with built-in and external generators.
    Freedesktop,
    /// The Windows shell's thumbnail cache.
    Windows,
    /// No thumbnails on this system.
    Unsupported,
}

pub const FEATURE_CACHE: &str = "cache";
pub const FEATURE_BUILTIN: &str = "builtin";
pub const FEATURE_EXTERNAL: &str = "external";
pub const FEATURE_SHELL: &str = "shell";

/// What kind of thing stops a feature from working.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum ReasonKind {
    /// This operating system has no implementation.
    Unsupported,
    /// The feature belongs to another operating system.
    OtherPlatform,
    /// The cache folder cannot be found or created.
    NoCacheDirectory,
    /// No `*.thumbnailer` file is installed.
    NoThumbnailers,
}

/// Why a feature does not work: a kind to branch on and a sentence to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct Reason {
    pub kind: ReasonKind,
    pub message: String,
}

/// Whether one feature of the plugin works on this system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct FeatureStatus {
    /// One of `cache`, `builtin`, `external` or `shell`.
    pub name: String,
    pub available: bool,
    pub reason: Option<Reason>,
    /// How many of something the feature found: the thumbnailers for `external`.
    pub count: Option<u32>,
}

impl FeatureStatus {
    pub fn available(name: &str) -> Self {
        FeatureStatus {
            name: name.to_string(),
            available: true,
            reason: None,
            count: None,
        }
    }

    pub fn available_with_count(name: &str, count: u32) -> Self {
        FeatureStatus {
            count: Some(count),
            ..FeatureStatus::available(name)
        }
    }

    pub fn unavailable(name: &str, kind: ReasonKind, message: impl Into<String>) -> Self {
        FeatureStatus {
            name: name.to_string(),
            available: false,
            reason: Some(Reason {
                kind,
                message: message.into(),
            }),
            count: None,
        }
    }
}

/// What the plugin can do on the running system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PluginStatus {
    /// True when at least one feature can make or serve thumbnails.
    pub available: bool,
    /// Why nothing is available; absent otherwise.
    pub reason: Option<Reason>,
    pub flavour: Flavour,
    pub features: Vec<FeatureStatus>,
}

impl PluginStatus {
    /// Builds a status from the features; the plugin is available when any generator (`builtin`, `external` or `shell`) works and the `cache` does.
    pub fn build(flavour: Flavour, features: Vec<FeatureStatus>) -> Self {
        let works = |name: &str| features.iter().any(|f| f.name == name && f.available);
        let available = works(FEATURE_CACHE)
            && (works(FEATURE_BUILTIN) || works(FEATURE_EXTERNAL) || works(FEATURE_SHELL));
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

/// How the plugin runs. Every field has a default; the app can change the limits later through [`crate::Thumbnails`].
#[derive(Debug, Clone)]
pub struct Config {
    /// Worker threads; `None` is `min(4, cores / 2)`, and at least one.
    pub workers: Option<usize>,
    /// The size of the in-memory cache of encoded thumbnails, in bytes (64 MB).
    pub memory_cache_bytes: u64,
    /// The size of the cache of thumbnails made from bytes the caller read itself
    /// ([`crate::Thumbnails::from_bytes`]), in bytes (32 MB). They are kept in memory only, apart from the shared cache folder.
    pub bytes_cache_bytes: u64,
    /// A file larger than this is never decoded by the built-in generator (50 MiB).
    pub max_file_bytes: u64,
    /// How long an external thumbnailer may run (10 seconds).
    pub external_timeout: Duration,
    /// The name recorded in the failure cache folder, so a failure of one application's version does not hide a file from another's.
    pub app_name: String,
    pub app_version: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            workers: None,
            memory_cache_bytes: 64 * 1_000_000,
            bytes_cache_bytes: 32 * 1_000_000,
            max_file_bytes: 50 * 1024 * 1024,
            external_timeout: Duration::from_secs(10),
            app_name: "tauri-plugin-thumbnails".to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl Config {
    /// The number of workers this configuration runs.
    pub fn worker_count(&self) -> usize {
        self.workers.unwrap_or_else(|| {
            let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
            (cores / 2).clamp(1, 4)
        })
    }
}
