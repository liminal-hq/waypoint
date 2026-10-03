// The plugin's error: a stable kind and a message, plus the range for a refused value
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use thiserror::Error;
use waypoint_settings::{ApplyError, BundleError, FolderViewsError, SettingsError};

#[derive(Debug, Error)]
pub enum Error {
    /// A value was outside its range; nothing changed.
    #[error(transparent)]
    Invalid(#[from] SettingsError),
    /// The settings could not be saved; nothing changed.
    #[error("could not save the settings: {0}")]
    Storage(String),
    /// The file is not one that can be imported, or could not be made; the reason says why.
    #[error(transparent)]
    Transfer(#[from] BundleError),
    /// Applying the import failed part way; what was applied before it was put back.
    #[error(transparent)]
    Apply(#[from] ApplyError),
    /// The file could not be read or written.
    #[error("{0}")]
    Io(String),
    /// A folder's remembered view was refused (a location that cannot be one); nothing changed.
    #[error(transparent)]
    FolderViews(#[from] FolderViewsError),
    /// There is no file dialog to ask with.
    #[error("this system has no file dialog Waypoint can use")]
    NoPicker,
    /// The plan is not the one that was just made (another file was read since, or it was used).
    #[error("that import is no longer the one ready to apply; choose the file again")]
    Stale,
}

impl Error {
    /// The stable name the frontend tells errors apart by.
    fn kind(&self) -> &'static str {
        match self {
            Error::Invalid(_) => "invalid",
            Error::Storage(_) => "storage",
            Error::Transfer(_) => "transfer",
            Error::Apply(_) => "apply",
            Error::FolderViews(_) => "invalid",
            Error::Io(_) => "io",
            Error::NoPicker => "unavailable",
            Error::Stale => "stale",
        }
    }
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        // `{ kind, message }`, plus `field`, `min` and `max` for `invalid`, so the page can put the
        // refusal under the row that caused it.
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("kind", self.kind())?;
        map.serialize_entry("message", &self.to_string())?;
        if let Error::Invalid(SettingsError::OutOfRange { field, min, max }) = self {
            map.serialize_entry("field", field)?;
            map.serialize_entry("min", min)?;
            map.serialize_entry("max", max)?;
        }
        if let Error::Invalid(SettingsError::Invalid { field, .. }) = self {
            map.serialize_entry("field", field)?;
        }
        // For `transfer`, which of the ways a file can be refused, so the page words each one.
        if let Error::Transfer(why) = self {
            map.serialize_entry("reason", why.code())?;
        }
        map.end()
    }
}
