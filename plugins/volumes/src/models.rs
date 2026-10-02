// Defines the serialisable models of the volumes plugin: volumes, the change event and the status
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The event the plugin emits to every window when the list of volumes changes: `volumes://changed`, carrying a [`VolumesChanged`].
pub const EVENT_CHANGED: &str = "volumes://changed";

/// What sort of volume this is, which decides its icon and its place in a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum VolumeKind {
    /// A fixed disk or partition.
    Internal,
    /// A USB stick, an SD card or another drive that can be taken away.
    Removable,
    /// A CD, DVD or Blu-ray drive.
    Optical,
    /// A network share or a FUSE mount (NFS, SMB, `sshfs`, a mapped drive, …).
    Network,
    /// A disk image attached as a loop device.
    Loop,
    /// An encrypted volume, locked or unlocked.
    Encrypted,
}

/// One volume: a disk, partition, drive letter, mount or network share, and what can be done to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct Volume {
    /// A stable handle for the volume, valid for the commands `mount`, `unmount`, `eject`, `unlock` and `refreshSpace` while the volume exists. Its form is opaque to callers.
    pub id: String,
    /// The name to show: the file system label, or a description when it has none.
    pub label: String,
    pub kind: VolumeKind,
    /// The file system type as the system names it (`ext4`, `ntfs`, `crypto_LUKS`, `nfs4`), when known.
    pub file_system: Option<String>,
    /// Where it is mounted: a folder on Linux, a drive root (`E:\`) on Windows. Absent when it is not mounted.
    pub mount_point: Option<String>,
    /// A URL for a network volume (`smb://server/share`), when one can be made.
    pub uri: Option<String>,
    /// Size in bytes. For a volume that is not mounted, the size of the device when known.
    #[ts(type = "number | null")]
    pub total: Option<u64>,
    /// Free bytes available to this user. Absent when the volume is not mounted, when measuring it timed out, or when it was not measured (network volumes are measured only when asked).
    #[ts(type = "number | null")]
    pub free: Option<u64>,
    pub can_mount: bool,
    pub can_unmount: bool,
    pub can_eject: bool,
    /// The drive can be powered off after it is ejected (`eject` does that as well).
    pub can_power_off: bool,
    /// An encrypted volume that must be unlocked before it can be mounted.
    pub locked: bool,
    /// A fixed, internal device (the system's own disks), as opposed to something plugged in or remote.
    pub is_system: bool,
    /// The device node (`/dev/sdb1`) or, for a mapped drive, the remote name (`\\server\share`).
    pub device: Option<String>,
}

/// The payload of `volumes://changed`: the whole list after the change, with a revision that only ever grows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct VolumesChanged {
    /// Starts at 1 with the first list and grows by one with each change. An event older than the revision already seen is stale and can be ignored.
    #[ts(type = "number")]
    pub revision: u64,
    pub volumes: Vec<Volume>,
}

/// A passphrase. It is never logged: its `Debug` is redacted, and the plugin keeps no copy.
#[derive(Clone, Deserialize)]
#[serde(transparent)]
pub struct Passphrase(pub String);

impl std::fmt::Debug for Passphrase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Passphrase(…)")
    }
}

/// Which implementation is behind the plugin on this system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Flavour {
    /// UDisks2 over the system bus, plus `/proc/self/mountinfo` for the mounts it does not manage.
    Udisks2,
    /// `/proc/self/mountinfo` alone: listing works, actions do not.
    Mountinfo,
    /// The Windows drive and network APIs.
    Windows,
    /// No volume support on this system.
    Unsupported,
}

/// Why a feature is unavailable. A code the front end can branch on; `message` beside it is a sentence for people.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Reason {
    /// There is no system D-Bus to talk to.
    NoSystemBus,
    /// The bus is there but UDisks2 is not installed or cannot be started.
    Udisks2Missing,
    /// A Flatpak sandbox without access to the system bus.
    FlatpakSandbox,
    /// UDisks2 answered but refused to describe its objects.
    Udisks2Failed,
    /// This operating system has no volume support in the plugin.
    UnsupportedPlatform,
    /// The system has no such operation (Windows mounts drives by itself and has no unlock of its own).
    NotSupported,
}

pub const FEATURE_LIST: &str = "list";
pub const FEATURE_MOUNT: &str = "mount";
pub const FEATURE_UNMOUNT: &str = "unmount";
pub const FEATURE_EJECT: &str = "eject";
pub const FEATURE_UNLOCK: &str = "unlock";
pub const FEATURE_WATCH: &str = "watch";

/// Every feature, in the order `get_status` lists them.
pub const FEATURES: [&str; 6] = [
    FEATURE_LIST,
    FEATURE_MOUNT,
    FEATURE_UNMOUNT,
    FEATURE_EJECT,
    FEATURE_UNLOCK,
    FEATURE_WATCH,
];

/// Whether one feature of the plugin works on this system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct FeatureStatus {
    /// One of `list`, `mount`, `unmount`, `eject`, `unlock` or `watch`.
    pub name: String,
    pub available: bool,
    /// Why the feature is unavailable; absent when it works.
    pub reason: Option<Reason>,
    /// A sentence that explains the reason; absent when the feature works.
    pub message: Option<String>,
}

impl FeatureStatus {
    pub fn available(name: &str) -> Self {
        FeatureStatus {
            name: name.to_string(),
            available: true,
            reason: None,
            message: None,
        }
    }

    pub fn unavailable(name: &str, reason: Reason, message: impl Into<String>) -> Self {
        FeatureStatus {
            name: name.to_string(),
            available: false,
            reason: Some(reason),
            message: Some(message.into()),
        }
    }
}

/// What the plugin can do on the running system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PluginStatus {
    /// True when at least one feature is available.
    pub available: bool,
    /// Why nothing is available, or why the actions are; absent when everything works.
    pub reason: Option<Reason>,
    pub message: Option<String>,
    pub flavour: Flavour,
    pub features: Vec<FeatureStatus>,
}

impl PluginStatus {
    /// Builds a status from the features. The top-level reason is the first unavailable feature's.
    pub fn build(flavour: Flavour, features: Vec<FeatureStatus>) -> Self {
        let available = features.iter().any(|feature| feature.available);
        let first = features.iter().find(|feature| !feature.available);
        PluginStatus {
            available,
            reason: first.and_then(|feature| feature.reason),
            message: first.and_then(|feature| feature.message.clone()),
            flavour,
            features,
        }
    }

    /// A status in which every feature is unavailable for the same reason.
    pub fn all_unavailable(flavour: Flavour, reason: Reason, message: &str) -> Self {
        PluginStatus::build(
            flavour,
            FEATURES
                .iter()
                .map(|name| FeatureStatus::unavailable(name, reason, message))
                .collect(),
        )
    }

    /// Whether the named feature is available.
    pub fn has(&self, name: &str) -> bool {
        self.features
            .iter()
            .any(|feature| feature.name == name && feature.available)
    }
}
