// The plugin's error: a stable kind and a message, plus the range for a refused value
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use thiserror::Error;
use waypoint_settings::SettingsError;

#[derive(Debug, Error)]
pub enum Error {
    /// A value was outside its range; nothing changed.
    #[error(transparent)]
    Invalid(#[from] SettingsError),
    /// The settings could not be saved; nothing changed.
    #[error("could not save the settings: {0}")]
    Storage(String),
}

impl Error {
    /// The stable name the frontend tells errors apart by.
    fn kind(&self) -> &'static str {
        match self {
            Error::Invalid(_) => "invalid",
            Error::Storage(_) => "storage",
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
        map.end()
    }
}
