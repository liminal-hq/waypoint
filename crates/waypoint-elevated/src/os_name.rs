// A file name or path that crosses the wire without losing a byte.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::{OsStr, OsString};
use std::fmt;

use serde::{Deserialize, Serialize};

/// Something in the wire form that the other side cannot use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireError(pub &'static str);

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for WireError {}

/// An `OsString`, as text when it is valid Unicode (nearly every name, and a short form) and
/// otherwise in the platform's own units: bytes on Unix and 16-bit units on Windows. A name that is
/// not Unicode on one platform has no meaning on the other, so the other side refuses it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireOs {
    #[serde(rename = "s")]
    Text(String),
    #[serde(rename = "b")]
    Bytes(Vec<u8>),
    #[serde(rename = "w")]
    Wide(Vec<u16>),
}

impl WireOs {
    pub fn from_os(name: &OsStr) -> Self {
        match name.to_str() {
            Some(text) => WireOs::Text(text.to_owned()),
            None => platform::raw(name),
        }
    }

    pub fn to_os_string(&self) -> Result<OsString, WireError> {
        match self {
            WireOs::Text(text) => Ok(OsString::from(text)),
            other => platform::from_raw(other),
        }
    }
}

impl From<&OsStr> for WireOs {
    fn from(name: &OsStr) -> Self {
        WireOs::from_os(name)
    }
}

impl TryFrom<WireOs> for OsString {
    type Error = WireError;

    fn try_from(wire: WireOs) -> Result<Self, WireError> {
        wire.to_os_string()
    }
}

#[cfg(unix)]
mod platform {
    use std::ffi::{OsStr, OsString};
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    use super::{WireError, WireOs};

    pub(super) fn raw(name: &OsStr) -> WireOs {
        WireOs::Bytes(name.as_bytes().to_vec())
    }

    pub(super) fn from_raw(wire: &WireOs) -> Result<OsString, WireError> {
        match wire {
            WireOs::Bytes(bytes) => Ok(OsString::from_vec(bytes.clone())),
            _ => Err(WireError("a name in another platform's form")),
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::{OsStr, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    use super::{WireError, WireOs};

    pub(super) fn raw(name: &OsStr) -> WireOs {
        WireOs::Wide(name.encode_wide().collect())
    }

    pub(super) fn from_raw(wire: &WireOs) -> Result<OsString, WireError> {
        match wire {
            WireOs::Wide(units) => Ok(OsString::from_wide(units)),
            _ => Err(WireError("a name in another platform's form")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_is_text() {
        let wire = WireOs::from_os(OsStr::new("café 日本語"));
        assert_eq!(wire, WireOs::Text("café 日本語".to_owned()));
        assert_eq!(wire.to_os_string().unwrap(), OsString::from("café 日本語"));
        assert_eq!(
            serde_json::to_string(&wire).unwrap(),
            r#"{"s":"café 日本語"}"#
        );
    }

    #[cfg(unix)]
    #[test]
    fn bytes_that_are_not_unicode_survive() {
        use std::os::unix::ffi::OsStringExt;
        let name = OsString::from_vec(vec![b'a', 0xff, 0xfe, b'b']);
        let wire = WireOs::from_os(&name);
        assert_eq!(wire, WireOs::Bytes(vec![b'a', 0xff, 0xfe, b'b']));
        let json = serde_json::to_string(&wire).unwrap();
        let back: WireOs = serde_json::from_str(&json).unwrap();
        assert_eq!(back.to_os_string().unwrap(), name);
        assert!(WireOs::Wide(vec![1]).to_os_string().is_err());
    }

    #[cfg(windows)]
    #[test]
    fn units_that_are_not_unicode_survive() {
        use std::os::windows::ffi::OsStringExt;
        let name = OsString::from_wide(&[0x61, 0xd800, 0x62]);
        let wire = WireOs::from_os(&name);
        assert_eq!(wire, WireOs::Wide(vec![0x61, 0xd800, 0x62]));
        assert_eq!(wire.to_os_string().unwrap(), name);
        assert!(WireOs::Bytes(vec![1]).to_os_string().is_err());
    }
}
