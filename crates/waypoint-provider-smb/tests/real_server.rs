// The SMB provider against a real Samba server run as the current user: the conformance suite, the
// share browser, logins and domains, signing and encryption, the typed errors, ranges, server-side
// copy, resumed writes, and reconnecting. Each test skips with a message when there is no `smbd`
// (see `support`).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(all(feature = "client", not(windows)))]

mod support;

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use support::{Proxy, Smbd, PASSWORD};
use waypoint_path::{ConnectionKey, VfsPath};
use waypoint_protocol::{AuthPrompt, ConnectionState, UnreachableReason, VfsError};
use waypoint_provider_smb::{SmbConfig, SmbOptions, SmbProvider};
use waypoint_vfs::conformance::{self, kind, Subject};
use waypoint_vfs::{
    CancelToken, Credential, CredentialSource, EntryKind, FileTimes, Provider, Secret, WriteOptions,
};

fn names(entries: &[waypoint_vfs::ScannedEntry]) -> BTreeSet<String> {
    entries
        .iter()
        .map(|entry| entry.name.to_string_lossy().into_owned())
        .collect()
}

fn list(
    provider: &SmbProvider,
    path: &VfsPath,
) -> Result<Vec<waypoint_vfs::ScannedEntry>, VfsError> {
    provider.list(path, &CancelToken::new(), 0, &mut |_| {})
}

fn read_all(provider: &SmbProvider, path: &VfsPath, start: u64) -> Vec<u8> {
    let mut out = Vec::new();
    provider
        .open_read_at(path, start)
        .unwrap()
        .read_to_end(&mut out)
        .unwrap();
    out
}

fn write_all(provider: &SmbProvider, path: &VfsPath, bytes: &[u8]) {
    let mut stream = provider
        .create_write(path, WriteOptions::truncate())
        .unwrap();
    stream.write_all(bytes).unwrap();
    stream.finish(true).unwrap();
}

fn run_conformance(server: &Smbd) {
    let provider = server.provider();
    let subject = Subject {
        name: "smb",
        provider: &provider,
        root: server.data_location(),
    };
    conformance::run(&subject);
}

#[test]
fn passes_the_conformance_suite() {
    let Some(server) = Smbd::start() else { return };
    run_conformance(&server);
}

#[test]
fn passes_the_conformance_suite_on_a_server_that_requires_signing() {
    let Some(server) = Smbd::start_with("server signing = mandatory") else {
        return;
    };
    run_conformance(&server);
}

#[test]
fn passes_the_conformance_suite_on_a_server_that_offers_encryption() {
    let Some(server) = Smbd::start_with("server smb encrypt = desired") else {
        return;
    };
    run_conformance(&server);
}

#[test]
fn a_share_that_requires_encryption_is_refused_with_a_typed_error() {
    // The library gets no cipher from Samba 4.24, so the share's encryption cannot start and the
    // tree connect is refused; the page gets an error, not a hang or a broken view.
    let Some(server) = Smbd::start_with_data("", "smb encrypt = required") else {
        return;
    };
    let provider = server.provider();
    let error = list(&provider, &server.data_location()).unwrap_err();
    assert!(
        matches!(
            error,
            VfsError::PermissionDenied { .. } | VfsError::Unsupported { .. }
        ),
        "{error:?}"
    );
}

#[test]
fn a_server_that_requires_encryption_for_every_session_is_a_typed_unsupported_error() {
    // Samba 4.24 selects no cipher for `smb2` 0.27's negotiation, so it refuses the session of a
    // valid account; that refusal is not a bad password, and the page is told so.
    let Some(server) = Smbd::start_with("server smb encrypt = required") else {
        return;
    };
    let provider = server.provider();
    let error = list(&provider, &server.data_location()).unwrap_err();
    let VfsError::Unsupported { what } = &error else {
        panic!("a refused protection is unsupported, not a bad password: {error:?}");
    };
    assert!(what.contains("encryption"), "{what}");
}

#[test]
fn the_server_root_is_a_share_browser() {
    let Some(server) = Smbd::start() else { return };
    let provider = server.provider();
    let root = server.shares_location();
    let shares = list(&provider, &root).unwrap();
    let listed = names(&shares);
    for share in ["data", "readonly", "locked"] {
        assert!(listed.contains(share), "{share} is listed in {listed:?}");
    }
    assert!(
        shares
            .iter()
            .all(|share| share.kind == EntryKind::Directory),
        "every share is a folder"
    );
    assert!(
        listed.iter().all(|name| !name.ends_with('$')),
        "administrative shares stay hidden"
    );
    let entry = provider.stat(&root).unwrap();
    assert_eq!(entry.kind, EntryKind::Directory);
    // A share opens as a folder.
    fs::write(server.data.join("inside.txt"), "x").unwrap();
    let inside = list(&provider, &root.join("data").unwrap()).unwrap();
    assert_eq!(names(&inside), ["inside.txt".to_owned()].into());
    // Up from a share goes back to the browser.
    assert_eq!(root.join("data").unwrap().parent().unwrap(), root);
    // Nothing can be made, renamed or removed at the server's root.
    assert_eq!(
        kind(&provider.create_dir(&root.join("new").unwrap())),
        "unsupported"
    );
    assert_eq!(
        kind(&provider.remove_dir(&root.join("data").unwrap())),
        "unsupported"
    );
    assert_eq!(
        kind(&provider.rename(&root.join("data").unwrap(), &root.join("x").unwrap(), false)),
        "unsupported"
    );
}

#[test]
fn lists_stats_and_reads_a_real_server() {
    let Some(server) = Smbd::start() else { return };
    let data = &server.data;
    fs::write(data.join("a.txt"), "hello world").unwrap();
    fs::write(data.join(".hidden"), "").unwrap();
    fs::create_dir(data.join("sub")).unwrap();
    let big: Vec<u8> = (0..5_000_000u32).map(|n| (n * 7 % 251) as u8).collect();
    fs::write(data.join("big.bin"), &big).unwrap();
    fs::create_dir(data.join("many")).unwrap();
    for n in 0..1_234 {
        fs::write(data.join("many").join(format!("f{n:04}")), "").unwrap();
    }
    let provider = server.provider();
    let root = server.data_location();
    let at = |name: &str| root.join(name).unwrap();

    let listed = list(&provider, &root).unwrap();
    assert_eq!(
        names(&listed),
        [".hidden", "a.txt", "big.bin", "many", "sub"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    let by_name = |name: &str| listed.iter().find(|e| e.name == name).unwrap();
    assert_eq!(by_name("a.txt").kind, EntryKind::File);
    assert_eq!(by_name("a.txt").size, Some(11));
    assert!(by_name("a.txt").modified_ms.is_some());
    assert!(by_name(".hidden").hidden);
    assert_eq!(by_name("sub").kind, EntryKind::Directory);
    assert_eq!(by_name("sub").size, None);

    let stat = provider.stat(&at("a.txt")).unwrap();
    assert_eq!((stat.kind, stat.size), (EntryKind::File, Some(11)));
    assert_eq!(provider.stat(&root).unwrap().kind, EntryKind::Directory);
    assert_eq!(
        provider.stat(&at("sub")).unwrap().kind,
        EntryKind::Directory
    );

    // Many entries arrive in several batches that add up to the listing.
    let options = SmbOptions::default().with_listing_batch(500);
    let batched = server.provider_with(options);
    let mut sizes = Vec::new();
    batched
        .list_batches(&at("many"), &CancelToken::new(), 0, &mut |batch| {
            sizes.push(batch.len())
        })
        .unwrap();
    assert_eq!(sizes, vec![500, 500, 234]);

    // A big file reads back whole, from an offset, and from the middle of a request.
    assert_eq!(read_all(&provider, &at("big.bin"), 0), big);
    assert_eq!(
        read_all(&provider, &at("big.bin"), 1_234_567),
        big[1_234_567..]
    );
    assert_eq!(
        read_all(&provider, &at("big.bin"), 4_999_999),
        big[4_999_999..]
    );
    assert!(read_all(&provider, &at("big.bin"), 9_000_000).is_empty());
    assert_eq!(read_all(&provider, &at("a.txt"), 6), b"world");

    // The file is not left open on the server: it can be removed at once.
    drop(provider.open_read(&at("big.bin")).unwrap());
    std::thread::sleep(Duration::from_millis(200));
    provider.remove_file(&at("big.bin")).unwrap();

    // A cancelled listing says so.
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(
        kind(&provider.list(&at("many"), &cancel, 0, &mut |_| {})),
        "cancelled"
    );
}

#[test]
fn names_are_matched_without_regard_to_case_and_stay_as_written() {
    let Some(server) = Smbd::start() else { return };
    let provider = server.provider();
    let root = server.data_location();
    write_all(&provider, &root.join("Report.TXT").unwrap(), b"x");
    assert_eq!(
        provider
            .stat(&root.join("report.txt").unwrap())
            .unwrap()
            .size,
        Some(1)
    );
    assert_eq!(
        names(&list(&provider, &root).unwrap()),
        ["Report.TXT".to_owned()].into()
    );
    assert_eq!(
        kind(&provider.create_file(&root.join("REPORT.txt").unwrap())),
        "alreadyExists"
    );
    // A name the Windows rules forbid is refused before anything is sent.
    assert_eq!(
        kind(&provider.create_file(&root.join("what?").unwrap())),
        "invalidName"
    );
}

#[test]
fn an_upload_through_a_temporary_name_ends_as_one_file() {
    let Some(server) = Smbd::start() else { return };
    let provider = server.provider();
    let root = server.data_location();
    let partial = root.join(".waypoint-partial-1-0-final.bin").unwrap();
    let target = root.join("final.bin").unwrap();
    let bytes: Vec<u8> = (0..3_000_000u32).map(|n| (n % 253) as u8).collect();
    let mut stream = provider
        .create_write(&partial, WriteOptions::exclusive())
        .unwrap();
    for chunk in bytes.chunks(65_536) {
        stream.write_all(chunk).unwrap();
    }
    stream.finish(true).unwrap();
    // The name that must not clobber refuses; the replacing one swaps the file in.
    write_all(&provider, &target, b"old");
    assert_eq!(
        kind(&provider.rename(&partial, &target, false)),
        "alreadyExists"
    );
    provider.rename(&partial, &target, true).unwrap();
    assert_eq!(read_all(&provider, &target, 0), bytes);
    assert_eq!(kind(&provider.stat(&partial)), "notFound");
    assert_eq!(
        names(&list(&provider, &root).unwrap()),
        ["final.bin".to_owned()].into()
    );
    // A folder moves with what is in it.
    let folder = root.join("folder").unwrap();
    provider.create_dir(&folder).unwrap();
    write_all(&provider, &folder.join("inner.txt").unwrap(), b"inner");
    let moved = root.join("moved").unwrap();
    provider.rename(&folder, &moved, false).unwrap();
    assert_eq!(
        read_all(&provider, &moved.join("inner.txt").unwrap(), 0),
        b"inner"
    );
}

#[test]
fn times_are_set_and_read_back_for_files_and_folders() {
    let Some(server) = Smbd::start() else { return };
    let provider = server.provider();
    let root = server.data_location();
    let file = root.join("t.txt").unwrap();
    let folder = root.join("dir").unwrap();
    write_all(&provider, &file, b"x");
    provider.create_dir(&folder).unwrap();
    let when = UNIX_EPOCH + Duration::from_secs(1_500_000_000);
    for path in [&file, &folder] {
        provider
            .set_times(
                path,
                FileTimes {
                    accessed: None,
                    modified: Some(when),
                },
            )
            .unwrap();
        assert_eq!(
            provider.stat(path).unwrap().modified_ms,
            Some(1_500_000_000_000)
        );
    }
    assert_eq!(
        kind(&provider.set_times(&root.join("missing").unwrap(), FileTimes::default())),
        "notFound"
    );
}

#[test]
fn a_copy_within_a_share_is_made_on_the_server() {
    let Some(server) = Smbd::start() else { return };
    let provider = server.provider();
    let root = server.data_location();
    let bytes: Vec<u8> = (0..2_500_000u32).map(|n| (n % 241) as u8).collect();
    let from = root.join("from.bin").unwrap();
    let to = root.join("to.bin").unwrap();
    write_all(&provider, &from, &bytes);
    let mut seen = 0;
    let copied = provider
        .copy_file_within(&from, &to, &mut |n| seen = n, &CancelToken::new())
        .expect("Samba copies on the server")
        .unwrap();
    assert_eq!(copied, bytes.len() as u64);
    assert_eq!(seen, copied);
    assert_eq!(read_all(&provider, &to, 0), bytes);
    // An existing destination is never overwritten.
    let again = provider
        .copy_file_within(&from, &to, &mut |_| {}, &CancelToken::new())
        .unwrap();
    assert_eq!(kind(&again), "alreadyExists");
    assert_eq!(read_all(&provider, &to, 0), bytes);
    // A copy across shares is the engine's to stream.
    let other = server.location("readonly/x");
    assert!(provider
        .copy_file_within(&from, &other, &mut |_| {}, &CancelToken::new())
        .is_none());
}

#[test]
fn a_resumed_write_continues_the_partial_file_and_drops_what_followed() {
    let Some(server) = Smbd::start() else { return };
    let provider = server.provider();
    let root = server.data_location();
    let file = root.join("partial.bin").unwrap();
    write_all(&provider, &file, b"0123456789-and-more-than-needed");
    let mut stream = provider.resume_write(&file, 10).unwrap();
    stream.write_all(b"ABC").unwrap();
    stream.finish(true).unwrap();
    assert_eq!(read_all(&provider, &file, 0), b"0123456789ABC");
    assert_eq!(kind(&provider.resume_write(&file, 50).map(|_| ())), "io");
}

#[test]
fn removing_distinguishes_files_folders_and_full_folders() {
    let Some(server) = Smbd::start() else { return };
    let provider = server.provider();
    let root = server.data_location();
    let folder = root.join("f").unwrap();
    provider.create_dir(&folder).unwrap();
    let file = folder.join("x").unwrap();
    provider.create_file(&file).unwrap();
    assert_eq!(kind(&provider.remove_dir(&folder)), "notEmpty");
    assert_eq!(kind(&provider.remove_file(&folder)), "isADirectory");
    assert_eq!(kind(&provider.remove_dir(&file)), "notADirectory");
    assert_eq!(
        kind(&provider.open_read(&folder).map(|_| ())),
        "isADirectory"
    );
    provider.remove_file(&file).unwrap();
    provider.remove_dir(&folder).unwrap();
}

#[test]
fn a_domain_names_the_account_and_a_wrong_password_is_refused() {
    let Some(server) = Smbd::start() else { return };
    let provider = server.provider();
    let with_domain = VfsPath::from_uri(&format!(
        "smb://WORKGROUP;{}@127.0.0.1:{}/data",
        server.user, server.port
    ))
    .unwrap();
    fs::write(server.data.join("a"), "x").unwrap();
    assert_eq!(
        names(&list(&provider, &with_domain).unwrap()),
        ["a".to_owned()].into()
    );

    // Asked for a password and given a wrong one.
    let asking = server.provider_asking();
    let root = server.data_location();
    let key = asking.connection_key(&root).unwrap();
    let needed = list(&asking, &root).unwrap_err();
    let VfsError::AuthRequired { prompt, .. } = &needed else {
        panic!("a login with no password asks for one: {needed:?}");
    };
    assert_eq!(
        **prompt,
        AuthPrompt::Password {
            user: Some(server.user.clone())
        }
    );
    let wrong = asking
        .connect(
            &key,
            Some(server.answer("not the password")),
            &CancelToken::new(),
        )
        .unwrap_err();
    assert!(matches!(wrong, VfsError::AuthFailed { .. }), "{wrong:?}");
    assert!(matches!(
        asking.connection_state(&key),
        ConnectionState::Failed { .. }
    ));
    // The right one connects, and the listing then works without asking again.
    asking
        .connect(&key, Some(server.answer(PASSWORD)), &CancelToken::new())
        .unwrap();
    assert_eq!(asking.connection_state(&key), ConnectionState::Connected);
    assert_eq!(
        names(&list(&asking, &root).unwrap()),
        ["a".to_owned()].into()
    );
    // Disconnecting closes the session, and the next call logs in again from the source.
    asking.disconnect(&key);
    assert_eq!(asking.connection_state(&key), ConnectionState::Idle);
    assert!(list(&asking, &root).is_err());
}

#[test]
fn a_location_without_a_user_asks_for_one() {
    let Some(server) = Smbd::start() else { return };
    let provider = server.provider_asking();
    let bare = VfsPath::from_uri(&format!("smb://127.0.0.1:{}/data", server.port)).unwrap();
    let error = list(&provider, &bare).unwrap_err();
    let VfsError::AuthRequired { prompt, .. } = &error else {
        panic!("a guest login the server refuses asks for a login: {error:?}");
    };
    assert_eq!(**prompt, AuthPrompt::Password { user: None });
}

/// Counts what the provider tells its credential source.
struct Recorder {
    asked: Mutex<usize>,
    rejected: Mutex<usize>,
}

impl CredentialSource for Recorder {
    fn credential(&self, _: &ConnectionKey, _: &AuthPrompt) -> Option<Credential> {
        *self.asked.lock().unwrap() += 1;
        Some(Credential::Password {
            user: None,
            password: Secret::from("a stale password"),
        })
    }

    fn rejected(&self, _: &ConnectionKey, _: &AuthPrompt) {
        *self.rejected.lock().unwrap() += 1;
    }
}

#[test]
fn a_remembered_password_the_server_refuses_is_reported_as_rejected() {
    let Some(server) = Smbd::start() else { return };
    let recorder = Arc::new(Recorder {
        asked: Mutex::new(0),
        rejected: Mutex::new(0),
    });
    let provider = SmbProvider::new(SmbConfig::new().with_credentials(recorder.clone()));
    let error = list(&provider, &server.data_location()).unwrap_err();
    assert!(matches!(error, VfsError::AuthFailed { .. }), "{error:?}");
    assert_eq!(*recorder.asked.lock().unwrap(), 1);
    assert_eq!(*recorder.rejected.lock().unwrap(), 1);
}

#[test]
fn access_denied_and_a_missing_share_are_told_apart() {
    let Some(server) = Smbd::start() else { return };
    let provider = server.provider();
    assert_eq!(
        kind(&list(&provider, &server.location("locked")).map(|_| ())),
        "permissionDenied"
    );
    assert_eq!(
        kind(&list(&provider, &server.location("nosuchshare")).map(|_| ())),
        "notFound"
    );
    assert_eq!(
        kind(&provider.stat(&server.location("nosuchshare/x"))),
        "notFound"
    );
    // A share that only reads refuses changes with a permission error, not a broken connection.
    fs::write(server.readonly.join("r.txt"), "r").unwrap();
    let readonly = server.location("readonly");
    assert_eq!(
        read_all(&provider, &readonly.join("r.txt").unwrap(), 0),
        b"r"
    );
    assert_eq!(
        kind(&provider.create_file(&readonly.join("new").unwrap())),
        "permissionDenied"
    );
    assert_eq!(
        kind(&provider.remove_file(&readonly.join("r.txt").unwrap())),
        "permissionDenied"
    );
    assert_eq!(
        names(&list(&provider, &readonly).unwrap()),
        ["r.txt".to_owned()].into()
    );
}

#[test]
fn a_server_that_is_not_there_is_unreachable_and_says_why() {
    let provider = SmbProvider::new(
        SmbConfig::new().with_credentials(Arc::new(support::Password("any".to_owned()))),
    );
    let refused = format!("smb://me@127.0.0.1:{}/share", support::free_port());
    let error = list(&provider, &VfsPath::from_uri(&refused).unwrap()).unwrap_err();
    assert!(
        matches!(
            error,
            VfsError::Unreachable {
                reason: UnreachableReason::Refused,
                ..
            }
        ),
        "{error:?}"
    );
    let unresolved = VfsPath::from_uri("smb://me@no-such-host.invalid/share").unwrap();
    let error = list(&provider, &unresolved).unwrap_err();
    assert!(
        matches!(
            error,
            VfsError::Unreachable {
                reason: UnreachableReason::NameNotResolved,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn a_server_that_speaks_only_smb1_is_an_unsupported_dialect_not_a_hang() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut request = [0u8; 512];
            let _ = stream.read(&mut request);
            // An SMB 1 negotiate response: `\xFFSMB`, command 0x72.
            let mut reply = vec![0, 0, 0, 36, 0xFF, b'S', b'M', b'B', 0x72];
            reply.resize(40, 0);
            let _ = stream.write_all(&reply);
        }
    });
    let provider = SmbProvider::new(SmbConfig::new());
    let location = VfsPath::from_uri(&format!("smb://127.0.0.1:{port}/share")).unwrap();
    let error = list(&provider, &location).unwrap_err();
    let VfsError::Unsupported { what } = &error else {
        panic!("a dialect nobody shares is unsupported: {error:?}");
    };
    assert!(what.contains("SMB 2"), "{what}");
}

#[test]
fn reconnects_after_the_connection_drops() {
    let Some(server) = Smbd::start() else { return };
    let proxy = Proxy::start(server.port, Duration::ZERO);
    let provider = server.provider();
    let root = server.location_at(proxy.port, "data");
    fs::write(server.data.join("a"), "x").unwrap();
    assert_eq!(
        names(&list(&provider, &root).unwrap()),
        ["a".to_owned()].into()
    );
    let connections = proxy.connections();
    proxy.sever();
    std::thread::sleep(Duration::from_millis(200));
    // The next call finds the session gone and logs in again by itself.
    assert_eq!(
        names(&list(&provider, &root).unwrap()),
        ["a".to_owned()].into()
    );
    assert!(
        proxy.connections() > connections,
        "a new connection was made"
    );
}

#[test]
fn a_server_that_stops_answering_times_out_instead_of_hanging() {
    let Some(server) = Smbd::start() else { return };
    let proxy = Proxy::start(server.port, Duration::ZERO);
    let options = SmbOptions::default().with_timeout(Duration::from_millis(1500));
    let provider = server.provider_with(options);
    let root = server.location_at(proxy.port, "data");
    list(&provider, &root).unwrap();
    proxy.stall();
    let error = list(&provider, &root).unwrap_err();
    assert!(
        matches!(
            error,
            VfsError::Timeout { .. } | VfsError::Disconnected { .. }
        ),
        "{error:?}"
    );
    proxy.resume();
    proxy.sever();
    std::thread::sleep(Duration::from_millis(200));
    assert!(list(&provider, &root).is_ok(), "the next call reconnects");
}

#[test]
fn a_file_changed_after_the_listing_keeps_its_modification_time_in_the_past() {
    // The server's clock is this machine's: a file written now is listed as modified now.
    let Some(server) = Smbd::start() else { return };
    let provider = server.provider();
    let root = server.data_location();
    let before = SystemTime::now();
    write_all(&provider, &root.join("now.txt").unwrap(), b"x");
    let modified = provider
        .stat(&root.join("now.txt").unwrap())
        .unwrap()
        .modified_ms
        .unwrap();
    let before_ms = before.duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
    assert!(
        (modified - before_ms).abs() < 5_000,
        "{modified} vs {before_ms}"
    );
}
