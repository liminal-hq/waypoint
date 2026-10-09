// The messages of the elevated protocol and their conversions to and from the `waypoint-vfs` types.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use waypoint_path::CaseRule;
use waypoint_protocol::VfsError;
use waypoint_vfs::{
    Capabilities, Change, EntryAttributes, EntryDetails, EntryKind, FileTimes, FolderSizeTotals,
    IconGroup, PermissionModel, Permissions, RenameSupport, RescanReason, ScannedEntry,
    SpecialFolder, TrashedMeta, VolumeId, VolumeSpace, WatchEvent, WriteOptions,
};

use crate::frame::{Frame, FrameError};
use crate::os_name::{WireError, WireOs};
use crate::policy;

/// A frame's JSON body. Only the client sends `Request`, only the helper sends the others.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Message {
    Request(Request),
    Response(Response),
    Event(Event),
}

impl Message {
    pub fn to_frame(&self) -> Result<Frame, FrameError> {
        serde_json::to_vec(self)
            .map(Frame::Control)
            .map_err(|_| FrameError::Encode)
    }

    pub fn from_body(body: &[u8]) -> Result<Self, WireError> {
        serde_json::from_slice(body).map_err(|_| WireError("a message that does not parse"))
    }
}

/// Asks for one thing. Every request gets exactly one response with the same `id`, except `Read`,
/// whose answer is a data frame with that id (or a response, if it failed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub op: Op,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    pub reply: Reply,
}

/// Something sent with no request of its own: progress on the request `id`, or a change in the
/// watch `id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: u64,
    pub body: EventBody,
}

/// What can be asked: the allow-list of `docs/architecture/elevated-operations.md`. There is no
/// request that starts a program, changes an owner or reads the environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Op {
    /// The helper's capabilities.
    Hello,
    Stat {
        path: WireOs,
    },
    List {
        path: WireOs,
        inline_link_budget: u32,
    },
    ListBatches {
        path: WireOs,
        inline_link_budget: u32,
    },
    ResolveLink {
        folder: WireOs,
        entry: WireEntry,
    },
    /// Starts watching a folder; the request's id names the watch in its events.
    Watch {
        path: WireOs,
    },
    Unwatch {
        watch: u64,
    },
    /// Trips the cancel token of the request `target`.
    Cancel {
        target: u64,
    },
    CreateDir {
        path: WireOs,
    },
    CreateFile {
        path: WireOs,
    },
    Rename {
        from: WireOs,
        to: WireOs,
        overwrite: bool,
    },
    RemoveFile {
        path: WireOs,
    },
    RemoveDir {
        path: WireOs,
    },
    OpenRead {
        path: WireOs,
    },
    OpenReadAt {
        path: WireOs,
        start: u64,
    },
    /// Reads up to `len` bytes (at most one chunk); an empty chunk is the end of the file.
    Read {
        handle: u64,
        len: u32,
    },
    Details {
        path: WireOs,
    },
    FolderSize {
        path: WireOs,
    },
    CreateWrite {
        path: WireOs,
        exclusive: bool,
        mode: Option<u32>,
    },
    ResumeWrite {
        path: WireOs,
        offset: u64,
    },
    /// Followed at once by a data frame with the same id holding the bytes to write.
    Write {
        handle: u64,
    },
    FinishWrite {
        handle: u64,
        sync: bool,
    },
    CloseHandle {
        handle: u64,
    },
    SetTimes {
        path: WireOs,
        accessed: Option<WireTime>,
        modified: Option<WireTime>,
    },
    Permissions {
        path: WireOs,
    },
    SetPermissions {
        path: WireOs,
        mode: Option<u32>,
        readonly: bool,
    },
    Symlink {
        link: WireOs,
        target: WireOs,
    },
    ReadLink {
        path: WireOs,
    },
    Canonicalize {
        path: WireOs,
    },
    VolumeId {
        path: WireOs,
    },
    FreeSpace {
        path: WireOs,
    },
    CopyFileWithin {
        src: WireOs,
        dst: WireOs,
    },
}

impl Op {
    /// The operation's name, for a log line: never a path or a name.
    pub fn kind(&self) -> &'static str {
        match self {
            Op::Hello => "hello",
            Op::Stat { .. } => "stat",
            Op::List { .. } => "list",
            Op::ListBatches { .. } => "listBatches",
            Op::ResolveLink { .. } => "resolveLink",
            Op::Watch { .. } => "watch",
            Op::Unwatch { .. } => "unwatch",
            Op::Cancel { .. } => "cancel",
            Op::CreateDir { .. } => "createDir",
            Op::CreateFile { .. } => "createFile",
            Op::Rename { .. } => "rename",
            Op::RemoveFile { .. } => "removeFile",
            Op::RemoveDir { .. } => "removeDir",
            Op::OpenRead { .. } => "openRead",
            Op::OpenReadAt { .. } => "openReadAt",
            Op::Read { .. } => "read",
            Op::Details { .. } => "details",
            Op::FolderSize { .. } => "folderSize",
            Op::CreateWrite { .. } => "createWrite",
            Op::ResumeWrite { .. } => "resumeWrite",
            Op::Write { .. } => "write",
            Op::FinishWrite { .. } => "finishWrite",
            Op::CloseHandle { .. } => "closeHandle",
            Op::SetTimes { .. } => "setTimes",
            Op::Permissions { .. } => "permissions",
            Op::SetPermissions { .. } => "setPermissions",
            Op::Symlink { .. } => "symlink",
            Op::ReadLink { .. } => "readLink",
            Op::Canonicalize { .. } => "canonicalize",
            Op::VolumeId { .. } => "volumeId",
            Op::FreeSpace { .. } => "freeSpace",
            Op::CopyFileWithin { .. } => "copyFileWithin",
        }
    }
}

/// The one answer to a request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Reply {
    Unit,
    Entry {
        entry: WireEntry,
    },
    Caps {
        caps: WireCaps,
    },
    Handle {
        handle: u64,
    },
    Details {
        details: EntryDetails,
    },
    FolderSize {
        totals: FolderSizeTotals,
        cancelled: bool,
    },
    Path {
        path: WireOs,
    },
    Name {
        name: WireOs,
    },
    Volume {
        id: Option<VolumeId>,
    },
    Space {
        space: Option<VolumeSpace>,
    },
    /// `bytes` is `None` when the fast path did not handle the copy and nothing was touched.
    Copied {
        bytes: Option<u64>,
    },
    Permissions {
        mode: Option<u32>,
        readonly: bool,
    },
    Error {
        error: VfsError,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EventBody {
    ListProgress {
        count: u32,
    },
    /// A batch of a listing; the listing is the batches in order.
    Entries {
        entries: Vec<WireEntry>,
    },
    FolderSize {
        totals: FolderSizeTotals,
    },
    CopyProgress {
        bytes: u64,
    },
    Watch {
        event: WireWatchEvent,
    },
}

/// A `ScannedEntry`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireEntry {
    pub name: WireOs,
    pub kind: EntryKind,
    pub link_target: Option<EntryKind>,
    pub link_pending: bool,
    pub group: IconGroup,
    pub special: Option<SpecialFolder>,
    pub size: Option<u64>,
    pub modified_ms: Option<i64>,
    pub hidden: bool,
    pub trashed: Option<WireTrashed>,
    pub attributes: Option<Vec<(String, String)>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireTrashed {
    pub display_name: String,
    pub original_path: String,
    pub deleted_ms: i64,
}

impl From<&ScannedEntry> for WireEntry {
    fn from(entry: &ScannedEntry) -> Self {
        WireEntry {
            name: WireOs::from_os(&entry.name),
            kind: entry.kind,
            link_target: entry.link_target,
            link_pending: entry.link_pending,
            group: entry.group,
            special: entry.special,
            size: entry.size,
            modified_ms: entry.modified_ms,
            hidden: entry.hidden,
            trashed: entry.trashed.as_deref().map(|meta| WireTrashed {
                display_name: meta.display_name.clone(),
                original_path: meta.original_path.clone(),
                deleted_ms: meta.deleted_ms,
            }),
            attributes: entry.attributes.as_deref().map(|attributes| {
                attributes
                    .iter()
                    .map(|(key, value)| (key.to_owned(), value.to_owned()))
                    .collect()
            }),
        }
    }
}

/// An entry's name must be one name: the listing joins it onto the folder it came from.
impl TryFrom<WireEntry> for ScannedEntry {
    type Error = WireError;

    fn try_from(wire: WireEntry) -> Result<Self, WireError> {
        let name = wire.name.to_os_string()?;
        if !policy::name_ok(&name) {
            return Err(WireError("an entry whose name is not one name"));
        }
        Ok(ScannedEntry {
            name,
            kind: wire.kind,
            link_target: wire.link_target,
            link_pending: wire.link_pending,
            group: wire.group,
            special: wire.special,
            size: wire.size,
            modified_ms: wire.modified_ms,
            hidden: wire.hidden,
            trashed: wire.trashed.map(|meta| {
                Box::new(TrashedMeta {
                    display_name: meta.display_name,
                    original_path: meta.original_path,
                    deleted_ms: meta.deleted_ms,
                })
            }),
            attributes: wire.attributes.map(|pairs| {
                Box::new(
                    pairs
                        .iter()
                        .fold(EntryAttributes::new(), |all, (k, v)| all.with(k, v)),
                )
            }),
        })
    }
}

pub fn entries_from_wire(entries: Vec<WireEntry>) -> Result<Vec<ScannedEntry>, WireError> {
    entries.into_iter().map(ScannedEntry::try_from).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WireCase {
    Sensitive,
    Insensitive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WireRename {
    None,
    Replacing,
    NoReplace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WirePermissionModel {
    None,
    ReadOnlyFlag,
    Unix,
}

/// A `Capabilities`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireCaps {
    pub watch: bool,
    pub case_rule: WireCase,
    pub remote: bool,
    pub write: bool,
    pub rename: WireRename,
    pub server_copy: bool,
    pub atomic_write: bool,
    pub resume_write: bool,
    pub range_read: bool,
    pub permissions: WirePermissionModel,
    pub symlinks: bool,
    pub set_times: bool,
    pub max_name_len: Option<u32>,
    pub time_resolution_ms: u32,
}

impl From<Capabilities> for WireCaps {
    fn from(caps: Capabilities) -> Self {
        WireCaps {
            watch: caps.watch,
            case_rule: match caps.case_rule {
                CaseRule::Sensitive => WireCase::Sensitive,
                CaseRule::Insensitive => WireCase::Insensitive,
            },
            remote: caps.remote,
            write: caps.write,
            rename: match caps.rename {
                RenameSupport::None => WireRename::None,
                RenameSupport::Replacing => WireRename::Replacing,
                RenameSupport::NoReplace => WireRename::NoReplace,
            },
            server_copy: caps.server_copy,
            atomic_write: caps.atomic_write,
            resume_write: caps.resume_write,
            range_read: caps.range_read,
            permissions: match caps.permissions {
                PermissionModel::None => WirePermissionModel::None,
                PermissionModel::ReadOnlyFlag => WirePermissionModel::ReadOnlyFlag,
                PermissionModel::Unix => WirePermissionModel::Unix,
            },
            symlinks: caps.symlinks,
            set_times: caps.set_times,
            max_name_len: caps.max_name_len,
            time_resolution_ms: caps.time_resolution_ms,
        }
    }
}

impl From<WireCaps> for Capabilities {
    fn from(wire: WireCaps) -> Self {
        let mut caps = Capabilities::new(match wire.case_rule {
            WireCase::Sensitive => CaseRule::Sensitive,
            WireCase::Insensitive => CaseRule::Insensitive,
        });
        caps.watch = wire.watch;
        caps.remote = wire.remote;
        caps.write = wire.write;
        caps.rename = match wire.rename {
            WireRename::None => RenameSupport::None,
            WireRename::Replacing => RenameSupport::Replacing,
            WireRename::NoReplace => RenameSupport::NoReplace,
        };
        caps.server_copy = wire.server_copy;
        caps.atomic_write = wire.atomic_write;
        caps.resume_write = wire.resume_write;
        caps.range_read = wire.range_read;
        caps.permissions = match wire.permissions {
            WirePermissionModel::None => PermissionModel::None,
            WirePermissionModel::ReadOnlyFlag => PermissionModel::ReadOnlyFlag,
            WirePermissionModel::Unix => PermissionModel::Unix,
        };
        caps.symlinks = wire.symlinks;
        caps.set_times = wire.set_times;
        caps.max_name_len = wire.max_name_len;
        caps.time_resolution_ms = wire.time_resolution_ms;
        caps
    }
}

/// A point in time as seconds and nanoseconds from the Unix epoch; the seconds are negative
/// before it and the nanoseconds always count forward from them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireTime {
    pub secs: i64,
    pub nanos: u32,
}

impl From<SystemTime> for WireTime {
    fn from(time: SystemTime) -> Self {
        match time.duration_since(UNIX_EPOCH) {
            Ok(after) => WireTime {
                secs: after.as_secs() as i64,
                nanos: after.subsec_nanos(),
            },
            Err(before) => {
                let before = before.duration();
                if before.subsec_nanos() == 0 {
                    WireTime {
                        secs: -(before.as_secs() as i64),
                        nanos: 0,
                    }
                } else {
                    WireTime {
                        secs: -(before.as_secs() as i64) - 1,
                        nanos: 1_000_000_000 - before.subsec_nanos(),
                    }
                }
            }
        }
    }
}

impl TryFrom<WireTime> for SystemTime {
    type Error = WireError;

    fn try_from(wire: WireTime) -> Result<Self, WireError> {
        const BAD: WireError = WireError("a time out of range");
        if wire.nanos >= 1_000_000_000 {
            return Err(BAD);
        }
        let forward = Duration::from_nanos(u64::from(wire.nanos));
        if wire.secs >= 0 {
            UNIX_EPOCH
                .checked_add(Duration::from_secs(wire.secs as u64))
                .and_then(|time| time.checked_add(forward))
                .ok_or(BAD)
        } else {
            UNIX_EPOCH
                .checked_sub(Duration::from_secs(wire.secs.unsigned_abs()))
                .and_then(|time| time.checked_add(forward))
                .ok_or(BAD)
        }
    }
}

pub fn times_to_wire(times: FileTimes) -> (Option<WireTime>, Option<WireTime>) {
    (
        times.accessed.map(WireTime::from),
        times.modified.map(WireTime::from),
    )
}

pub fn times_from_wire(
    accessed: Option<WireTime>,
    modified: Option<WireTime>,
) -> Result<FileTimes, WireError> {
    Ok(FileTimes {
        accessed: accessed.map(SystemTime::try_from).transpose()?,
        modified: modified.map(SystemTime::try_from).transpose()?,
    })
}

pub fn permissions_reply(permissions: Permissions) -> Reply {
    Reply::Permissions {
        mode: permissions.mode,
        readonly: permissions.readonly,
    }
}

pub fn write_options(exclusive: bool, mode: Option<u32>) -> WriteOptions {
    WriteOptions { exclusive, mode }
}

/// A `Change`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WireChange {
    Upsert { entry: WireEntry },
    Remove { name: WireOs },
    Rename { from: WireOs, to: WireEntry },
}

impl From<&Change> for WireChange {
    fn from(change: &Change) -> Self {
        match change {
            Change::Upsert(entry) => WireChange::Upsert {
                entry: entry.into(),
            },
            Change::Remove(name) => WireChange::Remove {
                name: WireOs::from_os(name),
            },
            Change::Rename { from, to } => WireChange::Rename {
                from: WireOs::from_os(from),
                to: to.into(),
            },
        }
    }
}

impl TryFrom<WireChange> for Change {
    type Error = WireError;

    fn try_from(wire: WireChange) -> Result<Self, WireError> {
        let one_name = |name: WireOs| {
            let name = name.to_os_string()?;
            if policy::name_ok(&name) {
                Ok(name)
            } else {
                Err(WireError("a change whose name is not one name"))
            }
        };
        Ok(match wire {
            WireChange::Upsert { entry } => Change::Upsert(entry.try_into()?),
            WireChange::Remove { name } => Change::Remove(one_name(name)?),
            WireChange::Rename { from, to } => Change::Rename {
                from: one_name(from)?,
                to: to.try_into()?,
            },
        })
    }
}

/// A `WatchEvent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WireWatchEvent {
    Changes { changes: Vec<WireChange> },
    RescanOverflow,
    RescanUnknown { reason: String },
    Degraded { reason: String },
    Lost { error: VfsError },
}

impl From<&WatchEvent> for WireWatchEvent {
    fn from(event: &WatchEvent) -> Self {
        match event {
            WatchEvent::Changes(changes) => WireWatchEvent::Changes {
                changes: changes.iter().map(WireChange::from).collect(),
            },
            WatchEvent::Rescan(RescanReason::Overflow) => WireWatchEvent::RescanOverflow,
            WatchEvent::Rescan(RescanReason::Unknown(reason)) => WireWatchEvent::RescanUnknown {
                reason: reason.clone(),
            },
            WatchEvent::Degraded { reason } => WireWatchEvent::Degraded {
                reason: reason.clone(),
            },
            WatchEvent::Lost(error) => WireWatchEvent::Lost {
                error: error.clone(),
            },
        }
    }
}

impl TryFrom<WireWatchEvent> for WatchEvent {
    type Error = WireError;

    fn try_from(wire: WireWatchEvent) -> Result<Self, WireError> {
        Ok(match wire {
            WireWatchEvent::Changes { changes } => WatchEvent::Changes(
                changes
                    .into_iter()
                    .map(Change::try_from)
                    .collect::<Result<_, _>>()?,
            ),
            WireWatchEvent::RescanOverflow => WatchEvent::Rescan(RescanReason::Overflow),
            WireWatchEvent::RescanUnknown { reason } => {
                WatchEvent::Rescan(RescanReason::Unknown(reason))
            }
            WireWatchEvent::Degraded { reason } => WatchEvent::Degraded { reason },
            WireWatchEvent::Lost { error } => WatchEvent::Lost(error),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::{OsStr, OsString};

    use waypoint_protocol::Location;

    use super::*;

    fn entry(name: &str) -> ScannedEntry {
        ScannedEntry {
            name: OsString::from(name),
            kind: EntryKind::File,
            link_target: Some(EntryKind::Directory),
            link_pending: true,
            group: IconGroup::Pdf,
            special: Some(SpecialFolder::Documents),
            size: Some(u64::MAX),
            modified_ms: Some(-5),
            hidden: true,
            trashed: Some(Box::new(TrashedMeta {
                display_name: "a".to_owned(),
                original_path: "/x".to_owned(),
                deleted_ms: 9,
            })),
            attributes: Some(Box::new(EntryAttributes::new().with("k", "v"))),
        }
    }

    fn json_round_trip<T: Serialize + for<'a> Deserialize<'a>>(value: &T) -> T {
        serde_json::from_str(&serde_json::to_string(value).unwrap()).unwrap()
    }

    #[test]
    fn an_entry_round_trips_every_field() {
        let original = entry("a b");
        let back = ScannedEntry::try_from(json_round_trip(&WireEntry::from(&original))).unwrap();
        assert_eq!(back, original);
        let plain = ScannedEntry {
            trashed: None,
            attributes: None,
            link_target: None,
            ..original
        };
        let back = ScannedEntry::try_from(json_round_trip(&WireEntry::from(&plain))).unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn an_entry_whose_name_is_a_path_is_refused() {
        for bad in ["a/b", "..", "", "."] {
            let mut wire = WireEntry::from(&entry("x"));
            wire.name = WireOs::Text(bad.to_owned());
            assert!(ScannedEntry::try_from(wire).is_err(), "{bad:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_name_that_is_not_unicode_round_trips() {
        use std::os::unix::ffi::OsStringExt;
        let mut original = entry("x");
        original.name = OsString::from_vec(vec![0x66, 0xff, 0x80]);
        let back = ScannedEntry::try_from(json_round_trip(&WireEntry::from(&original))).unwrap();
        assert_eq!(back.name, original.name);
    }

    #[test]
    fn capabilities_round_trip() {
        for caps in [
            Capabilities::local(),
            Capabilities::new(CaseRule::Insensitive),
        ] {
            assert_eq!(
                Capabilities::from(json_round_trip(&WireCaps::from(caps))),
                caps
            );
        }
        let mut odd = Capabilities::local();
        odd.rename = RenameSupport::Replacing;
        odd.permissions = PermissionModel::ReadOnlyFlag;
        odd.max_name_len = None;
        odd.time_resolution_ms = 1000;
        assert_eq!(Capabilities::from(WireCaps::from(odd)), odd);
    }

    #[test]
    fn times_round_trip_either_side_of_the_epoch() {
        for time in [
            UNIX_EPOCH,
            UNIX_EPOCH + Duration::new(1_700_000_000, 123_456_789),
            UNIX_EPOCH - Duration::new(10, 0),
            UNIX_EPOCH - Duration::new(10, 250_000_000),
        ] {
            let wire = WireTime::from(time);
            assert!(wire.nanos < 1_000_000_000);
            assert_eq!(SystemTime::try_from(json_round_trip(&wire)).unwrap(), time);
        }
        assert!(SystemTime::try_from(WireTime {
            secs: 0,
            nanos: 2_000_000_000
        })
        .is_err());
    }

    #[test]
    fn watch_events_round_trip() {
        let events = vec![
            WatchEvent::Changes(vec![
                Change::Upsert(entry("a")),
                Change::Remove(OsString::from("b")),
                Change::Rename {
                    from: OsString::from("c"),
                    to: entry("d"),
                },
            ]),
            WatchEvent::Rescan(RescanReason::Overflow),
            WatchEvent::Rescan(RescanReason::Unknown("why".to_owned())),
            WatchEvent::Degraded {
                reason: "polling".to_owned(),
            },
            WatchEvent::Lost(VfsError::NotFound {
                location: Location::new("/x", "file:///x"),
            }),
        ];
        for event in events {
            let back =
                WatchEvent::try_from(json_round_trip(&WireWatchEvent::from(&event))).unwrap();
            assert_eq!(back, event);
        }
    }

    #[test]
    fn a_change_with_a_name_that_is_a_path_is_refused() {
        let wire = WireChange::Remove {
            name: WireOs::Text("../x".to_owned()),
        };
        assert!(Change::try_from(wire).is_err());
    }

    #[test]
    fn messages_round_trip_through_a_frame() {
        let message = Message::Request(Request {
            id: u64::MAX,
            op: Op::Rename {
                from: WireOs::from_os(OsStr::new("/a")),
                to: WireOs::from_os(OsStr::new("/b")),
                overwrite: true,
            },
        });
        let Frame::Control(body) = message.to_frame().unwrap() else {
            panic!("a message is a control frame");
        };
        let Message::Request(request) = Message::from_body(&body).unwrap() else {
            panic!("a request comes back as one");
        };
        assert_eq!(request.id, u64::MAX);
        assert_eq!(request.op.kind(), "rename");
        assert!(Message::from_body(b"{not json").is_err());
        assert!(Message::from_body(br#"{"request":{"id":1,"op":{"op":"launch"}}}"#).is_err());
    }

    #[test]
    fn an_error_reply_keeps_its_type() {
        let reply = Reply::Error {
            error: VfsError::CrossesDevices {
                from: Location::new("/a", "file:///a"),
                to: Location::new("/b", "file:///b"),
            },
        };
        match json_round_trip(&reply) {
            Reply::Error {
                error: VfsError::CrossesDevices { to, .. },
            } => assert_eq!(to.uri, "file:///b"),
            other => panic!("{other:?}"),
        }
    }
}
