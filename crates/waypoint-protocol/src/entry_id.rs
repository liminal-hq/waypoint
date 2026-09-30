// The identity of one entry inside one listing.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Names an entry within a single listing handle, stable across re-sorts and live updates.
///
/// Selection and every later operation refer to `(handle, EntryId)`; Rust resolves the real path,
/// so the wire never carries raw file-name bytes. A `u32` (not a `u64`) so it is a JavaScript
/// `number` rather than a `bigint`; no single listing holds four billion entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct EntryId(pub u32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_as_a_bare_number() {
        assert_eq!(serde_json::to_string(&EntryId(7)).unwrap(), "7");
    }
}
