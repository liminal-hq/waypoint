// Defines serialisable models and event names for native drag and drop IPC
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Emitted to a window when files first move over it, with an `EnterEvent` payload.
pub const ENTER_EVENT: &str = "native-dnd://enter";
/// Emitted to a window as files move over it, with an `OverEvent` payload.
pub const OVER_EVENT: &str = "native-dnd://over";
/// Emitted to a window when files are dropped on it, with a `DropEvent` payload.
pub const DROP_EVENT: &str = "native-dnd://drop";
/// Emitted to a window when files leave it without being dropped, with a `LeaveEvent` payload.
pub const LEAVE_EVENT: &str = "native-dnd://leave";
/// Emitted to the window that started an outbound drag when the drag ends however it ends, with a `DragEnded` payload. The page's own pointer state is stale from the moment the drag starts, so this is its cue to reset it.
pub const DRAG_ENDED_EVENT: &str = "native-dnd://drag-ended";
/// Emitted to every window, with no payload, when the file clipboard may have changed.
pub const CLIPBOARD_CHANGED_EVENT: &str = "native-dnd://clipboard-changed";

/// The features `get_status` reports.
pub const FEATURE_INBOUND: &str = "inbound";
pub const FEATURE_OUTBOUND: &str = "outbound";
pub const FEATURE_POSITIONS: &str = "positions";
pub const FEATURE_MODIFIERS: &str = "modifiers";
pub const FEATURE_CLIPBOARD: &str = "clipboard";
pub const FEATURE_SELF_DROP_FILTER: &str = "self-drop-filter";

/// A point in logical (CSS) pixels from the top-left of the webview.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// The modifier keys held when an event happened.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    /// Alt on Windows and Linux (Option elsewhere).
    pub alt: bool,
}

/// Sent when files first move over a window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct EnterEvent {
    /// The label of the window the files are over.
    pub window: String,
    /// The files as display strings; a name that is not valid Unicode is shown with replacement characters and cannot be reopened from this value, use `uris` for that.
    pub paths: Vec<String>,
    /// The files as `file://` URIs with every byte outside `A-Za-z0-9-._~/` percent-encoded, so a name that is not valid Unicode survives.
    pub uris: Vec<String>,
    pub position: Position,
    pub modifiers: Modifiers,
}

/// Sent as files move over a window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct OverEvent {
    pub window: String,
    pub position: Position,
    pub modifiers: Modifiers,
}

/// Sent when files are dropped on a window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct DropEvent {
    pub window: String,
    pub paths: Vec<String>,
    pub uris: Vec<String>,
    pub position: Position,
    pub modifiers: Modifiers,
    /// True if the drop is the end of an outbound drag this process started with `start_drag`; `uris` are then exactly the ones that were offered.
    pub self_drop: bool,
}

/// Sent when files leave a window without being dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct LeaveEvent {
    pub window: String,
}

/// What an outbound drag may do with the files.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum DragAction {
    Copy,
    Move,
    Link,
}

/// A drag image, as the bytes of a PNG file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct DragIcon {
    pub png: Vec<u8>,
}

/// Arguments of `start_drag`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct StartDragRequest {
    /// The `file://` URIs to offer.
    pub uris: Vec<String>,
    /// The actions the drop target may choose from; at least one.
    pub actions: Vec<DragAction>,
    /// The image dragged with the pointer. Used on Linux; Windows draws its own.
    #[serde(default)]
    pub icon: Option<DragIcon>,
}

/// How an outbound drag ended.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum DragOutcome {
    /// Dropped on a target that copied the files.
    DroppedCopy,
    /// Dropped on a target that moved the files; the plugin never deletes anything, so the caller removes the originals if the target did not.
    DroppedMove,
    DroppedLink,
    /// Released over nothing that took the files, or abandoned with Escape.
    Cancelled,
    /// The drag could not run or broke.
    Failed,
}

/// Sent when an outbound drag ends, and returned by `start_drag` where the drag has already ended by the time the command resolves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct DragEnded {
    /// The `id` `start_drag` returned for the drag.
    pub id: u32,
    pub outcome: DragOutcome,
    /// The URIs that were offered.
    pub uris: Vec<String>,
    /// Why the drag failed, for `failed`.
    pub reason: Option<String>,
}

/// What `start_drag` did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct StartDragReport {
    /// Identifies the drag in `DragEnded`.
    pub id: u32,
    /// Set on Windows, where the command resolves only when the drag has finished; null on Linux, where the drag runs on after the command resolves and `native-dnd://drag-ended` reports it.
    pub ended: Option<DragEnded>,
}

/// The files on the clipboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct ClipboardFiles {
    /// The `file://` URIs.
    pub uris: Vec<String>,
    /// True if the files were cut (to be moved on paste), false if copied.
    pub cut: bool,
}

/// The windowing system the plugin found.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum DisplayServer {
    Wayland,
    X11,
    Windows,
    /// No usable display, or a platform the plugin does not support.
    None,
}

/// Whether one feature works.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct FeatureStatus {
    pub available: bool,
    /// Why the feature does not work, when `available` is false.
    pub reason: Option<String>,
}

/// Every feature the plugin reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct Features {
    /// Normalised `enter`, `over`, `drop` and `leave` events for windows whose drag-drop handler is on.
    pub inbound: FeatureStatus,
    /// `start_drag`.
    pub outbound: FeatureStatus,
    /// Inbound positions in logical pixels.
    pub positions: FeatureStatus,
    /// Modifier keys read at event time.
    pub modifiers: FeatureStatus,
    /// `set_files`, `get_files` and the clipboard event.
    pub clipboard: FeatureStatus,
    /// Telling a drop of this process's own outbound drag from a drop from another application.
    #[serde(rename = "self-drop-filter")]
    #[ts(rename = "self-drop-filter")]
    pub self_drop_filter: FeatureStatus,
}

/// What `get_status` reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PluginStatus {
    /// True if at least one feature works.
    pub available: bool,
    /// Why nothing works, when `available` is false.
    pub reason: Option<String>,
    pub display_server: DisplayServer,
    pub features: Features,
}

/// The kinds of failure a command reports.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum ErrorKind {
    /// Native drag and drop does not work here; `get_status` says why.
    Unsupported,
    /// `start_drag` needs the primary mouse button held; nothing started.
    ButtonNotPressed,
    /// An outbound drag is already running; only one runs at a time.
    AlreadyActive,
    /// The request is malformed.
    Invalid,
    /// The system refused or the call failed.
    Failed,
}

/// The value a failed command rejects with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct NativeDndError {
    pub kind: ErrorKind,
    pub message: String,
}
