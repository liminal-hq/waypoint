// A place a listing, tab or history entry points at, in a form that survives any file name.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A location the frontend can show and hand back, without ever holding a raw path.
///
/// File names are not always valid UTF-8 on Linux, so a plain string cannot name every folder. The
/// frontend displays `display` and passes `uri` back unchanged; Rust parses the `uri` to the real
/// path. History, tabs, favourites and breadcrumbs all carry a `Location`, never a string path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Location {
    /// A human-readable form for the path bar and titles. It may be lossy.
    pub display: String,
    /// The lossless form: a percent-encoded `file://` URI for local paths.
    pub uri: String,
}

impl Location {
    pub fn new(display: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            display: display.into(),
            uri: uri.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_with_stable_field_names() {
        let json = serde_json::to_string(&Location::new("/home/a", "file:///home/a")).unwrap();
        assert_eq!(json, r#"{"display":"/home/a","uri":"file:///home/a"}"#);
    }
}
