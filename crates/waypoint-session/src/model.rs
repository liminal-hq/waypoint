// The wire types of a session: tabs, the snapshot the frontend reads and the granular events.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_protocol::Location;

/// Names a tab within one window's session. Never reused while the session lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct TabId(pub u32);

/// One tab: where it is now and where it can go back and forward to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct TabSnapshot {
    pub id: TabId,
    pub location: Location,
    /// Earlier locations, oldest first; going back pops the last one.
    pub back: Vec<Location>,
    /// Locations to return to after going back, nearest last; navigating clears it.
    pub forward: Vec<Location>,
}

/// The whole session at one revision. An empty `tabs` with no `active` is a valid state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SessionSnapshot {
    /// Increases by one for every event, so a listener can tell a gap from a replay.
    #[ts(type = "number")]
    pub revision: u64,
    /// Tabs in display order.
    pub tabs: Vec<TabSnapshot>,
    /// Exactly one of `tabs` while any exist; `None` only when there are none.
    pub active: Option<TabId>,
}

/// Something that happened to the session. Applying the events in order to a snapshot at the
/// revision before the first reproduces the session; each carries the revision it produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum SessionEvent {
    /// A tab was inserted at `index`. It does not itself change the active tab; an activation
    /// follows as `TabActivated` when the new tab is active.
    TabOpened {
        tab: TabSnapshot,
        index: u32,
        #[ts(type = "number")]
        revision: u64,
    },
    /// A tab was removed. When it was the active one, a `TabActivated` for its neighbour follows.
    TabClosed {
        tab: TabId,
        #[ts(type = "number")]
        revision: u64,
    },
    TabActivated {
        tab: TabId,
        #[ts(type = "number")]
        revision: u64,
    },
    /// A tab moved to `index` in the display order.
    TabMoved {
        tab: TabId,
        index: u32,
        #[ts(type = "number")]
        revision: u64,
    },
    /// A tab's location or history changed (navigate, back or forward); the tab is whole.
    TabNavigated {
        tab: TabSnapshot,
        #[ts(type = "number")]
        revision: u64,
    },
}

impl SessionEvent {
    pub fn revision(&self) -> u64 {
        match self {
            Self::TabOpened { revision, .. }
            | Self::TabClosed { revision, .. }
            | Self::TabActivated { revision, .. }
            | Self::TabMoved { revision, .. }
            | Self::TabNavigated { revision, .. } => *revision,
        }
    }
}
