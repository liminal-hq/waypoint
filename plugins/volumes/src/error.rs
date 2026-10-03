// Defines the typed errors the volumes plugin reports for one action on one volume
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Why one action on a volume failed. Serialised as `{ "kind": "busy", "by": "bash" }`, so the front end can branch on `kind`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum VolumesError {
    /// Something holds the volume open. `by` names the program when it could be found.
    #[error("busy{}", by.as_ref().map(|name| format!(" ({name})")).unwrap_or_default())]
    Busy { by: Option<String> },
    /// The system (polkit) refused the action, or the person dismissed its prompt.
    #[error("not authorised")]
    NotAuthorised,
    /// The passphrase did not unlock the volume.
    #[error("wrong passphrase")]
    WrongPassphrase,
    /// This system, or this kind of volume, cannot do that.
    #[error("not supported")]
    Unsupported,
    /// The volume is gone (unplugged, or already removed).
    #[error("not found")]
    NotFound,
    /// Anything else the operating system reported.
    #[error("{message}")]
    Io { message: String },
}

impl VolumesError {
    /// An `Io` error with a message of the plugin's own.
    pub fn io(message: impl Into<String>) -> Self {
        VolumesError::Io {
            message: message.into(),
        }
    }

    /// Maps a UDisks2 (or D-Bus) error name and message to the closest typed error. `holder` finds the program that holds a busy volume; it is asked only for `DeviceBusy`.
    pub fn from_dbus(name: &str, message: &str, holder: impl FnOnce() -> Option<String>) -> Self {
        const UDISKS: &str = "org.freedesktop.UDisks2.Error.";
        match name {
            "org.freedesktop.UDisks2.Error.DeviceBusy" => VolumesError::Busy { by: holder() },
            "org.freedesktop.UDisks2.Error.NotSupported"
            | "org.freedesktop.DBus.Error.ServiceUnknown"
            | "org.freedesktop.DBus.Error.UnknownMethod" => VolumesError::Unsupported,
            "org.freedesktop.DBus.Error.UnknownObject" => VolumesError::NotFound,
            "org.freedesktop.DBus.Error.AccessDenied" => VolumesError::NotAuthorised,
            _ if name.starts_with("org.freedesktop.UDisks2.Error.NotAuthorized") => {
                VolumesError::NotAuthorised
            }
            _ if name.starts_with(UDISKS) && is_wrong_passphrase(message) => {
                VolumesError::WrongPassphrase
            }
            _ => VolumesError::Io {
                message: if message.is_empty() {
                    name.to_string()
                } else {
                    message.to_string()
                },
            },
        }
    }
}

/// UDisks2 reports a wrong passphrase as a plain `Failed` whose message carries the cryptsetup wording.
fn is_wrong_passphrase(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("operation not permitted")
        || message.contains("no key available")
        || message.contains("incorrect passphrase")
        || message.contains("wrong passphrase")
}

pub type Result<T> = std::result::Result<T, VolumesError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_busy_device_names_its_holder() {
        let error = VolumesError::from_dbus(
            "org.freedesktop.UDisks2.Error.DeviceBusy",
            "target is busy",
            || Some("bash".into()),
        );
        assert_eq!(
            error,
            VolumesError::Busy {
                by: Some("bash".into())
            }
        );
        assert_eq!(error.to_string(), "busy (bash)");
    }

    #[test]
    fn a_busy_device_without_a_holder_says_so_with_none() {
        let error =
            VolumesError::from_dbus("org.freedesktop.UDisks2.Error.DeviceBusy", "", || None);
        assert_eq!(error, VolumesError::Busy { by: None });
    }

    #[test]
    fn the_holder_is_looked_up_only_for_a_busy_device() {
        let error = VolumesError::from_dbus("org.freedesktop.UDisks2.Error.Failed", "x", || {
            panic!("not asked")
        });
        assert!(matches!(error, VolumesError::Io { .. }));
    }

    #[test]
    fn every_polkit_refusal_is_not_authorised() {
        for name in [
            "org.freedesktop.UDisks2.Error.NotAuthorized",
            "org.freedesktop.UDisks2.Error.NotAuthorizedCanObtain",
            "org.freedesktop.UDisks2.Error.NotAuthorizedDismissed",
            "org.freedesktop.DBus.Error.AccessDenied",
        ] {
            assert_eq!(
                VolumesError::from_dbus(name, "", || None),
                VolumesError::NotAuthorised,
                "{name}"
            );
        }
    }

    #[test]
    fn a_missing_object_or_service_maps_to_not_found_or_unsupported() {
        assert_eq!(
            VolumesError::from_dbus("org.freedesktop.DBus.Error.UnknownObject", "", || None),
            VolumesError::NotFound
        );
        assert_eq!(
            VolumesError::from_dbus("org.freedesktop.DBus.Error.ServiceUnknown", "", || None),
            VolumesError::Unsupported
        );
        assert_eq!(
            VolumesError::from_dbus("org.freedesktop.UDisks2.Error.NotSupported", "", || None),
            VolumesError::Unsupported
        );
    }

    #[test]
    fn the_cryptsetup_wording_means_a_wrong_passphrase() {
        let error = VolumesError::from_dbus(
            "org.freedesktop.UDisks2.Error.Failed",
            "Error unlocking /dev/sdb1: Failed to activate device: Operation not permitted",
            || None,
        );
        assert_eq!(error, VolumesError::WrongPassphrase);
    }

    #[test]
    fn anything_else_keeps_the_system_message() {
        let error = VolumesError::from_dbus(
            "org.freedesktop.UDisks2.Error.Failed",
            "something broke",
            || None,
        );
        assert_eq!(error, VolumesError::io("something broke"));
    }

    #[test]
    fn errors_serialise_with_a_kind() {
        let json = serde_json::to_value(VolumesError::Busy { by: None }).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "busy", "by": null }));
        let json = serde_json::to_value(VolumesError::NotAuthorised).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "notAuthorised" }));
    }
}
