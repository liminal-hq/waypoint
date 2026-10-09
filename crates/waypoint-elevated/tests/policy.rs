// What the helper refuses, over the real wire: bad paths, bad frames, and tables that are full.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::sync::atomic::Ordering;
use std::sync::Arc;

use support::*;
use waypoint_elevated::wire::{Op, Reply};
use waypoint_elevated::{Fault, ServeConfig, ServeEnd, WireOs, MAX_FRAME};

fn text(path: &str) -> WireOs {
    WireOs::Text(path.to_owned())
}

#[test]
fn a_path_that_is_not_absolute_and_normal_is_refused_and_the_connection_goes_on() {
    let mut raw = Raw::local(ServeConfig::default());
    for (id, bad) in [
        "",
        "relative/path",
        "./here",
        "/a/../etc",
        "/a/./b",
        "/a//b",
        "file:///etc",
        "admin:///etc",
    ]
    .into_iter()
    .enumerate()
    {
        let reply = raw.ask(id as u64 + 1, Op::Stat { path: text(bad) });
        assert_eq!(error_kind(&reply), "invalidLocation", "{bad:?}");
    }
    // Both paths of a rename are checked, and so is every other path.
    let reply = raw.ask(
        50,
        Op::Rename {
            from: text("/tmp"),
            to: text("../up"),
            overwrite: false,
        },
    );
    assert_eq!(error_kind(&reply), "invalidLocation");
    let reply = raw.ask(51, Op::RemoveFile { path: text("x") });
    assert_eq!(error_kind(&reply), "invalidLocation");
    assert!(matches!(raw.ask(52, Op::Hello), Reply::Caps { .. }));
    assert_eq!(raw.finish(), ServeEnd::EndOfInput);
}

#[test]
fn an_entry_to_resolve_must_be_one_name() {
    let dir = tempfile::tempdir().unwrap();
    let mut raw = Raw::local(ServeConfig::default());
    let folder = WireOs::from_os(dir.path().as_os_str());
    let entry = |name: &str| {
        let mut entry = waypoint_elevated::wire::WireEntry::from(&waypoint_vfs::ScannedEntry {
            name: name.into(),
            kind: waypoint_vfs::EntryKind::File,
            link_target: None,
            link_pending: false,
            group: waypoint_vfs::IconGroup::Other,
            special: None,
            size: None,
            modified_ms: None,
            hidden: false,
            trashed: None,
            attributes: None,
        });
        entry.name = text(name);
        entry
    };
    let reply = raw.ask(
        1,
        Op::ResolveLink {
            folder,
            entry: entry("../../etc/passwd"),
        },
    );
    assert_eq!(error_kind(&reply), "invalidName");
}

#[test]
fn an_oversize_frame_closes_the_connection() {
    let mut raw = Raw::local(ServeConfig::default());
    let mut header = ((MAX_FRAME + 1) as u32).to_be_bytes().to_vec();
    header.push(1);
    raw.send_bytes(&header);
    assert_eq!(raw.wait_end(), ServeEnd::ProtocolError(Fault::Oversize));
    // The helper's side is closed, so the client reads the end.
    assert!(raw.frame().is_none());
}

#[test]
fn a_truncated_frame_closes_the_connection() {
    let mut raw = Raw::local(ServeConfig::default());
    raw.send_bytes(&[0, 0, 0, 20, 1, b'{']);
    drop(std::mem::replace(
        &mut raw.writer,
        waypoint_elevated::testing::pipe::pipe().1,
    ));
    assert_eq!(raw.wait_end(), ServeEnd::ProtocolError(Fault::Truncated));
}

#[test]
fn garbage_closes_the_connection() {
    for bytes in [
        // Not JSON.
        framed(1, b"{not json"),
        // JSON that is not a message.
        framed(1, br#"{"hello":"there"}"#),
        // An operation that does not exist.
        framed(
            1,
            br#"{"request":{"id":1,"op":{"op":"launch","program":"sh"}}}"#,
        ),
        // A response where a request belongs.
        framed(1, br#"{"response":{"id":1,"reply":{"kind":"unit"}}}"#),
    ] {
        let mut raw = Raw::local(ServeConfig::default());
        raw.send_bytes(&bytes);
        let end = raw.wait_end();
        assert!(
            matches!(
                end,
                ServeEnd::ProtocolError(Fault::Malformed | Fault::Unexpected)
            ),
            "{end:?}"
        );
    }
    let mut raw = Raw::local(ServeConfig::default());
    raw.send_bytes(&[0, 0, 0, 2, 9, 0]);
    assert_eq!(raw.wait_end(), ServeEnd::ProtocolError(Fault::BadFrame));
    let mut raw = Raw::local(ServeConfig::default());
    raw.send_bytes(&[0, 0, 0, 0]);
    assert_eq!(raw.wait_end(), ServeEnd::ProtocolError(Fault::BadFrame));
}

fn framed(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut out = ((body.len() + 1) as u32).to_be_bytes().to_vec();
    out.push(kind);
    out.extend_from_slice(body);
    out
}

#[test]
fn data_nobody_asked_for_closes_the_connection() {
    let mut raw = Raw::local(ServeConfig::default());
    let mut body = 5u64.to_be_bytes().to_vec();
    body.extend_from_slice(b"bytes");
    raw.send_bytes(&framed(2, &body));
    assert_eq!(raw.wait_end(), ServeEnd::ProtocolError(Fault::Unexpected));
}

#[test]
fn a_write_request_must_be_followed_by_its_data() {
    let mut raw = Raw::local(ServeConfig::default());
    raw.send(1, Op::Write { handle: 1 });
    raw.send(2, Op::Hello);
    assert_eq!(raw.wait_end(), ServeEnd::ProtocolError(Fault::Unexpected));
}

#[test]
fn a_handle_that_is_not_open_is_stale() {
    let mut raw = Raw::local(ServeConfig::default());
    for (id, op) in [
        Op::Read {
            handle: 77,
            len: 10,
        },
        Op::FinishWrite {
            handle: 77,
            sync: false,
        },
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(error_kind(&raw.ask(id as u64 + 1, op)), "staleHandle");
    }
}

#[test]
fn the_handle_table_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f");
    std::fs::write(&path, b"x").unwrap();
    let config = ServeConfig {
        max_handles: 2,
        ..ServeConfig::default()
    };
    let mut raw = Raw::local(config);
    let open = |raw: &mut Raw, id| {
        raw.ask(
            id,
            Op::OpenRead {
                path: WireOs::from_os(path.as_os_str()),
            },
        )
    };
    let Reply::Handle { handle: first } = open(&mut raw, 1) else {
        panic!("a handle")
    };
    assert!(matches!(open(&mut raw, 2), Reply::Handle { .. }));
    assert_eq!(error_kind(&open(&mut raw, 3)), "io");
    // Closing one makes room, and the connection was never closed.
    assert!(matches!(
        raw.ask(4, Op::CloseHandle { handle: first }),
        Reply::Unit
    ));
    assert!(matches!(open(&mut raw, 5), Reply::Handle { .. }));
}

#[test]
fn the_watch_table_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let config = ServeConfig {
        max_watches: 1,
        ..ServeConfig::default()
    };
    let mut raw = Raw::local(config);
    let watch = |raw: &mut Raw, id| {
        raw.ask(
            id,
            Op::Watch {
                path: WireOs::from_os(dir.path().as_os_str()),
            },
        )
    };
    assert!(matches!(watch(&mut raw, 1), Reply::Unit));
    assert_eq!(error_kind(&watch(&mut raw, 2)), "io");
    assert!(matches!(raw.ask(3, Op::Unwatch { watch: 1 }), Reply::Unit));
    assert!(matches!(watch(&mut raw, 4), Reply::Unit));
}

#[test]
fn requests_in_flight_are_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(TestProvider::new());
    let config = ServeConfig {
        max_in_flight: 1,
        ..ServeConfig::default()
    };
    let mut raw = Raw::new(provider.clone(), config);
    raw.send(
        1,
        Op::List {
            path: WireOs::from_os(dir.path().as_os_str()),
            inline_link_budget: 0,
        },
    );
    wait_until("the listing to start", || {
        provider.started.load(Ordering::SeqCst) == 1
    });
    assert_eq!(error_kind(&raw.ask(2, Op::Hello)), "io");
    provider.release.store(true, Ordering::SeqCst);
    let (id, reply) = raw.reply().unwrap();
    assert_eq!((id, matches!(reply, Reply::Unit)), (1, true));
    assert!(matches!(raw.ask(3, Op::Hello), Reply::Caps { .. }));
}

#[test]
fn a_request_id_in_use_closes_the_connection() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(TestProvider::new());
    let mut raw = Raw::new(provider.clone(), ServeConfig::default());
    let list = || Op::List {
        path: WireOs::from_os(dir.path().as_os_str()),
        inline_link_budget: 0,
    };
    raw.send(1, list());
    wait_until("the listing to start", || {
        provider.started.load(Ordering::SeqCst) == 1
    });
    raw.send(1, list());
    assert_eq!(raw.wait_end(), ServeEnd::ProtocolError(Fault::DuplicateId));
}

#[test]
fn the_helper_leaves_other_people_s_paths_to_the_operating_system() {
    // A valid path the person may not read is the system's to refuse, as a typed error.
    let mut raw = Raw::local(ServeConfig::default());
    let reply = raw.ask(
        1,
        Op::Stat {
            path: text("/no/such/place"),
        },
    );
    assert_eq!(error_kind(&reply), "notFound");
    match reply {
        Reply::Error {
            error: waypoint_protocol::VfsError::NotFound { location },
        } => assert_eq!(location.uri, "file:///no/such/place"),
        other => panic!("{other:?}"),
    }
}
