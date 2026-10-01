// The wire types of the session store: tabs, groups, pairs, windows, the snapshots the frontend
// reads and the granular events.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_protocol::Location;

/// Names a tab. Global to the store and never reused while the store lives, so a tab keeps its id
/// when it moves between windows and across a restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct TabId(pub u32);

/// Names a tab group. Global to the store and never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct GroupId(pub u32);

/// Names a pair of tabs shown side by side. Global to the store and never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct PairId(pub u32);

/// A colour label for a tab or a group. The theme decides the actual shade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum TabColour {
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Blue,
    Purple,
    Pink,
    Grey,
}

/// What a tab needs to feel unchanged after a hand-off or a restore: where its list was scrolled
/// and which entry had the focus. Selection does not travel.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct TabHints {
    /// Scroll offset of the list or grid, in pixels.
    pub scroll_top: u32,
    /// The name of the focused entry, which survives a listing being rebuilt.
    pub focused: Option<String>,
}

/// One tab: where it is now, where it can go back and forward to, and how it is marked.
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
    /// Pinned tabs come before every unpinned tab.
    pub pinned: bool,
    pub colour: Option<TabColour>,
    /// The group the tab belongs to; a group's tabs are contiguous.
    pub group: Option<GroupId>,
    pub hints: TabHints,
}

/// The tab type of the store; the same as the wire snapshot of one tab.
pub type Tab = TabSnapshot;

/// A named, coloured run of contiguous tabs. Its members are the tabs whose `group` is its id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    pub colour: Option<TabColour>,
    pub collapsed: bool,
}

/// How a pair's panes sit next to each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum PairLayout {
    SideBySide,
    Stacked,
}

/// How a pair came to be, which decides what toggling the split off does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum PairOrigin {
    /// Toggling the split on made `created` as the second pane; toggling it off closes that tab.
    Toggle { created: TabId },
    /// Existing tabs were joined; toggling the split off only separates them.
    Joined,
}

/// Two or more tabs shown together. The panes are contiguous in the tab order, in pane order, and
/// all belong to the same group (or none) and are all pinned or all unpinned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Pair {
    pub id: PairId,
    pub panes: Vec<TabId>,
    pub layout: PairLayout,
    /// One share of the space per pane, in thousandths; they add up to 1000.
    pub sizes: Vec<u32>,
    pub origin: PairOrigin,
}

/// A window's place on the desktop, in physical pixels. The position is unknown where the
/// compositor does not report one (Wayland).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Geometry {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: u32,
    pub height: u32,
    pub maximised: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ViewMode {
    List,
    Grid,
}

/// The view choices a window restores: list or grid, icon size and hidden files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ViewPrefs {
    pub mode: ViewMode,
    pub show_hidden: bool,
    /// Grid icon size in pixels.
    pub icon_size: u32,
}

impl Default for ViewPrefs {
    fn default() -> Self {
        Self {
            mode: ViewMode::List,
            show_hidden: false,
            icon_size: 64,
        }
    }
}

/// A tab that was closed, with enough to put it back where it was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ClosedTab {
    pub tab: TabSnapshot,
    /// The window it was closed in.
    pub window: String,
    /// Its position in that window.
    pub index: u32,
}

/// Everything one window holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct WindowState {
    /// `main-{n}`; never reused within a process.
    pub label: String,
    /// Tabs in display order.
    pub tabs: Vec<TabSnapshot>,
    /// Exactly one of `tabs` while any exist; `None` only when there are none.
    pub active: Option<TabId>,
    /// Tabs that were explicitly activated, most recent first (see the crate docs).
    pub mru: Vec<TabId>,
    pub groups: Vec<Group>,
    pub pairs: Vec<Pair>,
    pub geometry: Option<Geometry>,
    pub view: ViewPrefs,
}

impl WindowState {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            tabs: Vec::new(),
            active: None,
            mru: Vec::new(),
            groups: Vec::new(),
            pairs: Vec::new(),
            geometry: None,
            view: ViewPrefs::default(),
        }
    }
}

/// One window's session at one global revision. An empty `tabs` with no `active` is a valid state.
/// The first three fields are the milestone 2 snapshot; the rest are additions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SessionSnapshot {
    /// Increases by one for every event in the whole store, so a listener can tell a gap from a
    /// replay. A window sees gaps for events that belong to other windows.
    #[ts(type = "number")]
    pub revision: u64,
    /// Tabs in display order.
    pub tabs: Vec<TabSnapshot>,
    /// Exactly one of `tabs` while any exist; `None` only when there are none.
    pub active: Option<TabId>,
    pub mru: Vec<TabId>,
    pub groups: Vec<Group>,
    pub pairs: Vec<Pair>,
    pub geometry: Option<Geometry>,
    pub view: ViewPrefs,
    /// The store's recently closed tabs, newest first, as of this snapshot. Closing a tab emits no
    /// event for it, so a menu that shows the list reads a fresh snapshot.
    pub closed: Vec<ClosedTab>,
}

impl SessionSnapshot {
    /// A window with no tabs at revision zero.
    pub fn empty() -> Self {
        Self {
            revision: 0,
            tabs: Vec::new(),
            active: None,
            mru: Vec::new(),
            groups: Vec::new(),
            pairs: Vec::new(),
            geometry: None,
            view: ViewPrefs::default(),
            closed: Vec::new(),
        }
    }
}

/// Something that happened to a window's session. Applying the events in order to a snapshot at
/// the revision before the first reproduces the session; each carries the revision it produced.
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
    /// A tab was removed (closed, or moved to another window). When it was the active one, a
    /// `TabActivated` for its successor follows.
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
    /// A window came into being. Carries its own label because no window exists to receive it.
    WindowOpened {
        window: String,
        #[ts(type = "number")]
        revision: u64,
    },
    /// A window went away: its last tab closed or moved, or it was closed.
    WindowClosed {
        window: String,
        #[ts(type = "number")]
        revision: u64,
    },
    /// A tab's pinned state, colour, group or hints changed; the tab is whole.
    TabChanged {
        tab: TabSnapshot,
        #[ts(type = "number")]
        revision: u64,
    },
    /// A closed tab came back at `index`. A `TabActivated` follows, as it does for an open.
    TabReopened {
        tab: TabSnapshot,
        index: u32,
        #[ts(type = "number")]
        revision: u64,
    },
    GroupCreated {
        group: Group,
        #[ts(type = "number")]
        revision: u64,
    },
    GroupChanged {
        group: Group,
        #[ts(type = "number")]
        revision: u64,
    },
    GroupRemoved {
        group: GroupId,
        #[ts(type = "number")]
        revision: u64,
    },
    PairCreated {
        pair: Pair,
        #[ts(type = "number")]
        revision: u64,
    },
    PairChanged {
        pair: Pair,
        #[ts(type = "number")]
        revision: u64,
    },
    PairRemoved {
        pair: PairId,
        #[ts(type = "number")]
        revision: u64,
    },
    /// The most-recently-used list changed; it is whole.
    MruChanged {
        mru: Vec<TabId>,
        #[ts(type = "number")]
        revision: u64,
    },
    ViewChanged {
        view: ViewPrefs,
        #[ts(type = "number")]
        revision: u64,
    },
    GeometryChanged {
        geometry: Geometry,
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
            | Self::TabNavigated { revision, .. }
            | Self::WindowOpened { revision, .. }
            | Self::WindowClosed { revision, .. }
            | Self::TabChanged { revision, .. }
            | Self::TabReopened { revision, .. }
            | Self::GroupCreated { revision, .. }
            | Self::GroupChanged { revision, .. }
            | Self::GroupRemoved { revision, .. }
            | Self::PairCreated { revision, .. }
            | Self::PairChanged { revision, .. }
            | Self::PairRemoved { revision, .. }
            | Self::MruChanged { revision, .. }
            | Self::ViewChanged { revision, .. }
            | Self::GeometryChanged { revision, .. } => *revision,
        }
    }

    pub(crate) fn set_revision(&mut self, value: u64) {
        match self {
            Self::TabOpened { revision, .. }
            | Self::TabClosed { revision, .. }
            | Self::TabActivated { revision, .. }
            | Self::TabMoved { revision, .. }
            | Self::TabNavigated { revision, .. }
            | Self::WindowOpened { revision, .. }
            | Self::WindowClosed { revision, .. }
            | Self::TabChanged { revision, .. }
            | Self::TabReopened { revision, .. }
            | Self::GroupCreated { revision, .. }
            | Self::GroupChanged { revision, .. }
            | Self::GroupRemoved { revision, .. }
            | Self::PairCreated { revision, .. }
            | Self::PairChanged { revision, .. }
            | Self::PairRemoved { revision, .. }
            | Self::MruChanged { revision, .. }
            | Self::ViewChanged { revision, .. }
            | Self::GeometryChanged { revision, .. } => *revision = value,
        }
    }
}

/// An event and the window it belongs to. The composition root emits `event` to `window` only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct WindowEvent {
    pub window: String,
    pub event: SessionEvent,
}

/// The whole store as plain data: what a document persists and a store restores from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct StoreSnapshot {
    #[ts(type = "number")]
    pub revision: u64,
    pub windows: Vec<WindowState>,
    /// Recently closed tabs, newest first, at most ten.
    pub closed: Vec<ClosedTab>,
    pub next_tab: u32,
    pub next_group: u32,
    pub next_pair: u32,
    pub next_window: u32,
}
