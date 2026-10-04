// The WebDAV provider against real servers run as the current user: Apache `mod_dav` (anonymous,
// Basic and Digest) and `rclone serve webdav`. The shared conformance suite, and what it does not
// reach. Each test skips with a message when its server is not installed (see `support`).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};

use support::{Login, Logins, Server, PASSWORD, USER};
use waypoint_protocol::{AuthPrompt, ConnectionState, UnreachableReason, VfsError};
use waypoint_vfs::conformance::{self, Subject};
use waypoint_vfs::{
    CancelToken, ConnectAnswer, Credential, EntryKind, Provider, RenameSupport, Secret,
    WriteOptions,
};

/// Makes a file an hour old, so its entity tag is a strong one.
fn age(path: &std::path::Path) {
    let file = fs::OpenOptions::new().write(true).open(path).unwrap();
    file.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(3600))
        .unwrap();
}

fn conform(server: &Server) {
    let provider = server.provider();
    let subject = Subject {
        name: server.name,
        provider: &provider,
        root: server.root(),
    };
    conformance::run(&subject);
}

fn names(provider: &dyn Provider, path: &waypoint_path::VfsPath) -> BTreeSet<String> {
    provider
        .list(path, &CancelToken::new(), usize::MAX, &mut |_| {})
        .unwrap()
        .into_iter()
        .map(|entry| entry.name.to_string_lossy().into_owned())
        .collect()
}

fn read_all(provider: &dyn Provider, path: &waypoint_path::VfsPath, start: u64) -> Vec<u8> {
    let mut out = Vec::new();
    provider
        .open_read_at(path, start)
        .unwrap()
        .read_to_end(&mut out)
        .unwrap();
    out
}

#[test]
fn apache_passes_the_conformance_suite() {
    let Some(server) = support::apache(Login::Anonymous) else {
        return;
    };
    conform(&server);
}

#[test]
fn apache_passes_the_conformance_suite_behind_basic_authentication() {
    let Some(server) = support::apache(Login::Basic) else {
        return;
    };
    conform(&server);
}

#[test]
fn apache_passes_the_conformance_suite_behind_digest_authentication() {
    let Some(server) = support::apache(Login::Digest) else {
        return;
    };
    conform(&server);
}

#[test]
fn rclone_passes_the_conformance_suite() {
    let Some(server) = support::rclone(Login::Anonymous) else {
        return;
    };
    conform(&server);
}

#[test]
fn rclone_passes_the_conformance_suite_behind_basic_authentication() {
    let Some(server) = support::rclone(Login::Basic) else {
        return;
    };
    conform(&server);
}

#[test]
fn apache_lists_stats_and_reads_what_is_on_disk() {
    let Some(server) = support::apache(Login::Anonymous) else {
        return;
    };
    let data = &server.data;
    fs::write(data.join("a file.txt"), "hello world").unwrap();
    fs::write(data.join(".hidden"), "").unwrap();
    fs::create_dir(data.join("sub")).unwrap();
    // A name that already holds an escape, and every character that reads as syntax.
    fs::write(data.join("caf%C3%A9 100%.dat"), "x").unwrap();
    fs::write(data.join("a#b?c;d,e&f=g+h.txt"), "x").unwrap();
    let big: Vec<u8> = (0..3_000_000u32).map(|n| (n * 7 % 251) as u8).collect();
    fs::write(data.join("big.bin"), &big).unwrap();
    fs::create_dir(data.join("many")).unwrap();
    for n in 0..1_234 {
        fs::write(data.join("many").join(format!("f{n:04}")), "").unwrap();
    }
    let provider = server.provider();
    let root = server.root();
    let at = |name: &str| root.join(name).unwrap();

    let listed = names(&provider, &root);
    assert_eq!(
        listed,
        [
            ".hidden",
            "a file.txt",
            "a#b?c;d,e&f=g+h.txt",
            "big.bin",
            "caf%C3%A9 100%.dat",
            "many",
            "sub"
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    );
    let entries = provider
        .list(&root, &CancelToken::new(), 0, &mut |_| {})
        .unwrap();
    let find = |name: &str| entries.iter().find(|e| e.name == name).unwrap();
    assert_eq!(find("sub").kind, EntryKind::Directory);
    assert_eq!(find("a file.txt").kind, EntryKind::File);
    assert_eq!(find("a file.txt").size, Some(11));
    assert!(find(".hidden").hidden && !find("sub").hidden);
    assert!(find("a file.txt").modified_ms.is_some());

    let stat = provider.stat(&at("a file.txt")).unwrap();
    assert_eq!((stat.size, stat.kind), (Some(11), EntryKind::File));
    assert_eq!(
        provider.stat(&at("sub")).unwrap().kind,
        EntryKind::Directory
    );
    assert_eq!(
        provider.stat(&root).unwrap().kind,
        EntryKind::Directory,
        "the root of a share is a folder"
    );
    assert_eq!(read_all(&provider, &at("a file.txt"), 0), b"hello world");
    assert_eq!(read_all(&provider, &at("a file.txt"), 6), b"world");
    assert_eq!(
        read_all(&provider, &at("big.bin"), 2_999_990),
        &big[2_999_990..]
    );
    assert_eq!(read_all(&provider, &at("big.bin"), 0), big);
    assert!(read_all(&provider, &at("big.bin"), 3_000_000).is_empty());
    assert_eq!(names(&provider, &at("many")).len(), 1_234);
    // Batches add up to the listing, and a small batch size splits it.
    let mut sizes = Vec::new();
    provider
        .list_batches(&at("many"), &CancelToken::new(), 0, &mut |batch| {
            sizes.push(batch.len())
        })
        .unwrap();
    assert_eq!(sizes.iter().sum::<usize>(), 1_234);

    // The typed errors of reading.
    assert!(matches!(
        provider.stat(&at("missing")),
        Err(VfsError::NotFound { .. })
    ));
    assert!(matches!(
        provider.list(&at("a file.txt"), &CancelToken::new(), 0, &mut |_| {}),
        Err(VfsError::NotADirectory { .. })
    ));
    assert!(matches!(
        provider.open_read(&at("sub")),
        Err(VfsError::IsADirectory { .. })
    ));
    assert!(matches!(
        provider.open_read(&at("missing")),
        Err(VfsError::NotFound { .. })
    ));
    assert_eq!(
        provider.connection_state(&root.connection_key().unwrap()),
        ConnectionState::Connected
    );
}

#[test]
fn apache_writes_without_clobbering_and_copies_on_the_server() {
    let Some(server) = support::apache(Login::Anonymous) else {
        return;
    };
    let provider = server.provider();
    let root = server.root();
    let at = |name: &str| root.join(name).unwrap();
    let caps = provider.capabilities();
    assert!(caps.write && caps.server_copy && caps.range_read && caps.remote);
    assert_eq!(caps.rename, RenameSupport::NoReplace);
    assert!(!caps.resume_write && !caps.set_times && !caps.atomic_write);

    let mut stream = provider
        .create_write(&at("one.txt"), WriteOptions::exclusive())
        .unwrap();
    stream.write_all(b"first").unwrap();
    stream.finish(false).unwrap();
    assert_eq!(fs::read(server.data.join("one.txt")).unwrap(), b"first");

    // A file written in the meantime is not replaced by an exclusive create that raced ahead of it.
    let mut stream = provider
        .create_write(&at("raced.txt"), WriteOptions::exclusive())
        .unwrap();
    fs::write(server.data.join("raced.txt"), "theirs").unwrap();
    stream.write_all(b"mine").unwrap();
    assert!(matches!(
        stream.finish(false),
        Err(VfsError::AlreadyExists { .. })
    ));
    assert_eq!(fs::read(server.data.join("raced.txt")).unwrap(), b"theirs");

    // Replacing a file replaces the version that was seen, and no newer one. (Apache's entity tag
    // is weak for a second after a write, and the provider then guards with the date instead.)
    let mut stream = provider
        .create_write(&at("one.txt"), WriteOptions::truncate())
        .unwrap();
    stream.write_all(b"second").unwrap();
    stream.finish(false).unwrap();
    assert_eq!(fs::read(server.data.join("one.txt")).unwrap(), b"second");
    age(&server.data.join("one.txt"));
    let mut stream = provider
        .create_write(&at("one.txt"), WriteOptions::truncate())
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(server.data.join("one.txt"), "changed elsewhere, longer").unwrap();
    stream.write_all(b"stale").unwrap();
    let stale = stream.finish(false);
    assert!(matches!(stale, Err(VfsError::Io { .. })), "{stale:?}");
    assert_eq!(
        fs::read(server.data.join("one.txt")).unwrap(),
        b"changed elsewhere, longer"
    );

    // A file larger than memory waits in a temporary file and still arrives whole.
    let big: Vec<u8> = (0..9_000_000u32).map(|n| (n % 253) as u8).collect();
    let mut stream = provider
        .create_write(&at("big.bin"), WriteOptions::exclusive())
        .unwrap();
    for block in big.chunks(1 << 20) {
        stream.write_all(block).unwrap();
    }
    stream.finish(true).unwrap();
    assert_eq!(fs::read(server.data.join("big.bin")).unwrap(), big);

    // Server-side copy, which refuses a name that is taken.
    let mut copied = 0;
    let done = provider.copy_file_within(
        &at("big.bin"),
        &at("big copy.bin"),
        &mut |n| copied = n,
        &CancelToken::new(),
    );
    assert_eq!(done.unwrap().unwrap(), big.len() as u64);
    assert_eq!(copied, big.len() as u64);
    assert_eq!(fs::read(server.data.join("big copy.bin")).unwrap(), big);
    let again = provider.copy_file_within(
        &at("big.bin"),
        &at("big copy.bin"),
        &mut |_| {},
        &CancelToken::new(),
    );
    assert!(matches!(again, Some(Err(VfsError::AlreadyExists { .. }))));
    let orphan = provider.copy_file_within(
        &at("big.bin"),
        &at("no such folder").join("copy").unwrap(),
        &mut |_| {},
        &CancelToken::new(),
    );
    assert!(
        matches!(orphan, Some(Err(VfsError::NotFound { .. }))),
        "{orphan:?}"
    );
    let missing = provider.copy_file_within(
        &at("nope"),
        &at("nope copy"),
        &mut |_| {},
        &CancelToken::new(),
    );
    assert!(matches!(missing, Some(Err(VfsError::NotFound { .. }))));
    // A folder is not this fast path's to copy: nothing is touched and the caller copies it itself.
    provider.create_dir(&at("folder")).unwrap();
    assert!(provider
        .copy_file_within(
            &at("folder"),
            &at("folder 2"),
            &mut |_| {},
            &CancelToken::new()
        )
        .is_none());
    assert!(!server.data.join("folder 2").exists());

    // Renames: refused over a name that is taken, allowed to replace when asked.
    assert!(matches!(
        provider.rename(&at("one.txt"), &at("big.bin"), false),
        Err(VfsError::AlreadyExists { .. })
    ));
    provider
        .rename(&at("one.txt"), &at("big.bin"), true)
        .unwrap();
    assert_eq!(
        fs::read(server.data.join("big.bin")).unwrap(),
        b"changed elsewhere, longer"
    );
    provider
        .rename(&at("folder"), &at("moved folder"), false)
        .unwrap();
    assert!(server.data.join("moved folder").is_dir());
    assert!(matches!(
        provider.rename(&at("missing"), &at("elsewhere"), false),
        Err(VfsError::NotFound { .. })
    ));
    let orphan = provider.rename(
        &at("big.bin"),
        &at("no such folder").join("x").unwrap(),
        false,
    );
    assert!(
        matches!(orphan, Err(VfsError::NotFound { .. })),
        "{orphan:?}"
    );

    // A removal never reaches into a folder that has something in it.
    provider
        .create_file(&at("moved folder").join("kept").unwrap())
        .unwrap();
    assert!(matches!(
        provider.remove_dir(&at("moved folder")),
        Err(VfsError::NotEmpty { .. })
    ));
    assert!(server.data.join("moved folder/kept").exists());
    assert!(matches!(
        provider.remove_file(&at("moved folder")),
        Err(VfsError::IsADirectory { .. })
    ));
    assert!(matches!(
        provider.remove_dir(&at("big.bin")),
        Err(VfsError::NotADirectory { .. })
    ));
    assert!(matches!(
        provider.create_dir(&at("no such folder").join("x").unwrap()),
        Err(VfsError::NotFound { .. })
    ));
    assert!(matches!(
        provider.create_write(&at("moved folder"), WriteOptions::truncate()),
        Err(VfsError::IsADirectory { .. })
    ));
    assert!(matches!(
        provider.create_file(&at("bad/name")),
        Err(VfsError::NotFound { .. }) | Err(VfsError::InvalidName { .. })
    ));
}

#[test]
fn a_login_is_asked_for_answered_and_refused_with_the_error_that_says_so() {
    for (login, label) in [(Login::Basic, "basic"), (Login::Digest, "digest")] {
        let Some(server) = support::apache(login) else {
            return;
        };
        let root = server.root();
        // Nobody to ask: the call says what is needed, and nothing was sent but the first request.
        let provider = support::plain(None);
        match provider.stat(&root) {
            Err(VfsError::AuthRequired { prompt, .. }) => assert_eq!(
                *prompt,
                AuthPrompt::Password {
                    user: Some(USER.to_owned())
                },
                "{label}"
            ),
            other => panic!("{label}: {other:?}"),
        }
        // The person answers.
        provider
            .connect(
                &root.connection_key().unwrap(),
                Some(ConnectAnswer::Credential(Credential::Password {
                    user: Some(USER.to_owned()),
                    password: Secret::from(PASSWORD),
                })),
                &CancelToken::new(),
            )
            .unwrap_or_else(|e| panic!("{label}: {e:?}"));
        assert_eq!(provider.stat(&root).unwrap().kind, EntryKind::Directory);
        assert_eq!(
            provider.connection_state(&root.connection_key().unwrap()),
            ConnectionState::Connected
        );
        // A password the server refuses is `AuthFailed`, and the source is told.
        let wrong = Logins::with(USER, "not the password");
        let provider = support::plain(Some(wrong.clone()));
        assert!(
            matches!(provider.stat(&root), Err(VfsError::AuthFailed { .. })),
            "{label}"
        );
        assert_eq!(*wrong.rejected.lock().unwrap(), 1, "{label}");
        assert!(matches!(
            provider.connection_state(&root.connection_key().unwrap()),
            ConnectionState::Failed {
                error: VfsError::AuthFailed { .. }
            }
        ));
        // Asking again asks again, instead of repeating the refused login.
        assert!(matches!(
            provider.stat(&root),
            Err(VfsError::AuthFailed { .. })
        ));
        assert_eq!(*wrong.rejected.lock().unwrap(), 2);
    }
}

#[test]
fn a_server_that_is_not_there_is_unreachable() {
    let provider = support::plain(None);
    let path =
        waypoint_path::VfsPath::from_uri(&format!("dav://127.0.0.1:{}/", support::free_port()))
            .unwrap();
    assert!(matches!(
        provider.stat(&path),
        Err(VfsError::Unreachable {
            reason: UnreachableReason::Refused,
            ..
        })
    ));
    assert!(matches!(
        provider.connection_state(&path.connection_key().unwrap()),
        ConnectionState::Failed { .. }
    ));
    let missing = waypoint_path::VfsPath::from_uri("dav://no-such-host.invalid/").unwrap();
    assert!(matches!(
        provider.stat(&missing),
        Err(VfsError::Unreachable {
            reason: UnreachableReason::NameNotResolved,
            ..
        })
    ));
}
