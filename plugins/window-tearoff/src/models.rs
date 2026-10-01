// Defines serialisable models and event names for window tear-off IPC
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Emitted to the ghost window with the drag's opaque JSON payload, on `begin` and on every `update`.
pub const PAYLOAD_EVENT: &str = "window-tearoff://payload";
/// Emitted to the window that began a drag when the drag ran past its time limit and was ended.
pub const TIMEOUT_EVENT: &str = "window-tearoff://timeout";
/// Emitted to the window that began a drag, with a `bool` payload, when the cursor value stops changing while a button is held (`true`) and when it changes again (`false`).
pub const CURSOR_STALE_EVENT: &str = "window-tearoff://cursor-stale";

/// Emitted to the window that began a toplevel drag, with a `ToplevelDragStarted` payload, once the compositor has taken the drag. The page gets no pointer events from then until the pointer comes back over it, so this is its cue to reset its own drag state.
pub const TOPLEVEL_DRAG_STARTED_EVENT: &str = "window-tearoff://toplevel-drag-started";
/// Emitted to the window that began a toplevel drag and to the window that was dragged, with a `ToplevelDragEnded` payload, when the drag ends however it ends.
pub const TOPLEVEL_DRAG_ENDED_EVENT: &str = "window-tearoff://toplevel-drag-ended";
/// Emitted to the window a toplevel drag was dropped on, with a `PayloadDropped` payload.
pub const TAB_DROPPED_EVENT: &str = "window-tearoff://tab-dropped";

/// The label the plugin gives the ghost window unless `Options` says otherwise.
pub const DEFAULT_GHOST_LABEL: &str = "tear-ghost";

/// The features `get_status` reports.
pub const FEATURE_GHOST: &str = "ghost";
pub const FEATURE_CURSOR_FOLLOW: &str = "cursor_follow";
pub const FEATURE_WINDOW_POSITION: &str = "window_position";
pub const FEATURE_HIT_TEST: &str = "hit_test";
/// Linux, on a Wayland compositor with `xdg_toplevel_drag_manager_v1`: a real window follows the pointer for the whole drag.
pub const FEATURE_TOPLEVEL_DRAG: &str = "toplevel_drag";

/// The MIME type the toplevel drag offers unless `Options` says otherwise.
pub const DEFAULT_TOPLEVEL_DRAG_MIME: &str = "application/x-window-tearoff";

/// A point in physical pixels on the virtual screen, or in logical pixels where a field says so.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// A size in logical pixels.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

/// A rectangle in logical pixels, relative to the top-left of the content area of the window that registered it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct Region {
    /// Returned in `DropReport.hit` so the caller knows which region was hit.
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// How a drag finished.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Outcome {
    /// The user released the drag; the cursor is hit-tested against the registered regions.
    Drop,
    /// The drag was abandoned; nothing is hit-tested.
    Cancel,
}

/// A registered region that the cursor was over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct Hit {
    /// The label of the window that registered the region.
    pub window: String,
    /// The `id` of the region.
    pub region: String,
}

/// What `end` found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct DropReport {
    /// The cursor in physical pixels, or null where the system does not report it.
    pub cursor: Option<Point>,
    /// The scale factor of the window that began the drag, for converting `cursor` to logical pixels.
    pub scale_factor: f64,
    /// True if the cursor value had stopped changing while a button was held, so `cursor` may be out of date.
    pub cursor_stale: bool,
    /// The topmost registered region under the cursor; always null for a cancelled drag and where hit-testing is unavailable.
    pub hit: Option<Hit>,
}

/// What `begin` did.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum BeginState {
    /// The ghost is shown and follows the cursor.
    Following,
    /// No ghost can be shown on this system, so nothing started. Render a preview inside the page; `end` still reports the drop.
    NoGhost,
    /// A drag is already in progress, so nothing changed.
    AlreadyActive,
}

/// The result of `begin`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct BeginReport {
    pub state: BeginState,
}

/// A feature that does not work on this system, and why.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct UnavailableFeature {
    pub feature: String,
    pub reason: String,
}

/// Whether the plugin can do anything on this system, and which features work.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PluginStatus {
    /// True if at least one feature works.
    pub available: bool,
    /// Why nothing works, when `available` is false.
    pub reason: Option<String>,
    /// The features that work: `ghost`, `cursor_follow`, `window_position`, `hit_test` and `toplevel_drag`.
    pub features: Vec<String>,
    /// The features that do not work, each with its reason.
    pub unavailable: Vec<UnavailableFeature>,
}

/// How the plugin is set up.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// The label of the one shared ghost window; a window-state plugin must be told to ignore it.
    pub ghost_label: String,
    /// The app URL the ghost window loads; it should render the payload it receives and stay transparent until then.
    pub ghost_url: String,
    /// The ghost's initial logical size; `begin` resizes it per drag.
    pub ghost_size: (f64, f64),
    /// The MIME type a toplevel drag carries its payload under. A drop on any window of the app that offers it hands the payload to that window's page.
    pub toplevel_drag_mime: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            ghost_label: DEFAULT_GHOST_LABEL.into(),
            ghost_url: "index.html".into(),
            ghost_size: (240.0, 80.0),
            toplevel_drag_mime: DEFAULT_TOPLEVEL_DRAG_MIME.into(),
        }
    }
}

impl Options {
    /// Labels a `tauri-plugin-window-state` denylist must contain while that plugin is registered: it shows a window it has no saved state for, which defeats the hidden ghost, and saves the ghost's state at exit.
    pub fn window_state_denylist(&self) -> Vec<String> {
        vec![self.ghost_label.clone()]
    }
}

/// What `begin_toplevel_drag` did.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum ToplevelBeginState {
    /// The compositor took the drag: the window follows the pointer.
    Started,
    /// This system cannot drag a real window (see `get_status`); nothing started.
    Unavailable,
    /// A toplevel drag is already running.
    AlreadyActive,
    /// The drag could not start, for example because the button was already released; nothing moved.
    Failed,
}

/// The result of `begin_toplevel_drag`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct ToplevelBeginReport {
    pub state: ToplevelBeginState,
    /// Why nothing started, when `state` is not `started`.
    pub reason: Option<String>,
}

/// How a toplevel drag ended.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum ToplevelOutcome {
    /// Released over another window of the app that took the payload; the dragged window stays where it is.
    DroppedOnWindow,
    /// Released over nothing that took it; the dragged window stays where the compositor left it.
    DroppedElsewhere,
    /// Abandoned (Escape); the dragged window is back where it was.
    Cancelled,
    /// The drag could not run.
    Failed,
}

/// Sent to the window that began a toplevel drag when the compositor has taken it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct ToplevelDragStarted {
    /// The label of the window being dragged.
    pub window: String,
    /// The drag's opaque payload, as `begin_toplevel_drag` was given it.
    #[ts(type = "unknown")]
    pub payload: serde_json::Value,
}

/// Sent when a toplevel drag ends, to the window that began it and to the window that was dragged. A late listener can ask for it with `take_toplevel_drag_result`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct ToplevelDragEnded {
    /// The label of the window that was dragged.
    pub window: String,
    /// The label of the window that began the drag.
    pub source: String,
    pub outcome: ToplevelOutcome,
    /// The window the payload was dropped on, for `dropped-on-window`.
    pub target: Option<String>,
    /// The drag's opaque payload, as `begin_toplevel_drag` was given it.
    #[ts(type = "unknown")]
    pub payload: serde_json::Value,
    /// Why the drag failed, for `failed`.
    pub reason: Option<String>,
}

/// Sent to a window when a toplevel drag's payload is dropped on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PayloadDropped {
    /// The label of the window the payload was dropped on (the receiver).
    pub window: String,
    /// The drag's payload, parsed from what the source offered.
    #[ts(type = "unknown")]
    pub payload: serde_json::Value,
}
