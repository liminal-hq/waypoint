// The SFTP provider against the in-process server on every platform and against a real OpenSSH
// server run as the current user: the conformance suite, listing, stat, reading, host keys, logins,
// cancelling, timeouts and reconnecting. The OpenSSH half of each test skips with a message when
// there is no `sshd` (see `support`); the in-process half always runs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[macro_use]
mod support;

use std::collections::BTreeSet;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use support::{Backend, FakeSftp, Sshd, PASSPHRASE};
use waypoint_path::VfsPath;
use waypoint_protocol::{AuthPrompt, ConnectionState, UnreachableReason, VfsError};
use waypoint_provider_sftp::{
    AgentSource, KnownHosts, MemoryKnownHosts, ServerKey, SftpConfig, SftpOptions, SftpProvider,
};
use waypoint_vfs::conformance::{self, Subject};
use waypoint_vfs::{
    CancelToken, ConnectAnswer, Credential, CredentialSource, EntryKind, Provider, Secret,
};

fn names(entries: &[waypoint_vfs::ScannedEntry]) -> BTreeSet<String> {
    entries
        .iter()
        .map(|entry| entry.name.to_string_lossy().into_owned())
        .collect()
}

fn read_all(provider: &SftpProvider, path: &VfsPath, start: u64) -> Vec<u8> {
    let mut out = Vec::new();
    provider
        .open_read_at(path, start)
        .unwrap()
        .read_to_end(&mut out)
        .unwrap();
    out
}

fn lists_stats_and_reads(server: &dyn Backend) {
    server.put("a.txt", b"hello world");
    server.put(".hidden", b"");
    server.mkdir("sub");
    let big: Vec<u8> = (0..3_000_000u32).map(|n| (n * 7 % 251) as u8).collect();
    server.put("big.bin", &big);
    server.symlink("a.txt", "link");
    server.symlink("sub", "to-folder");
    server.symlink("missing", "dangling");
    server.mkdir("many");
    for n in 0..1_234 {
        server.put(&format!("many/f{n:04}"), b"");
    }

    let provider = server.provider();
    let root = server.data_location();
    let at = |name: &str| root.join(name).unwrap();

    let listed = provider
        .list(&root, &CancelToken::new(), usize::MAX, &mut |_| {})
        .unwrap();
    assert_eq!(
        names(&listed),
        [
            ".hidden",
            "a.txt",
            "big.bin",
            "dangling",
            "link",
            "many",
            "sub",
            "to-folder"
        ]
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
    assert_eq!(by_name("link").kind, EntryKind::Symlink);
    assert_eq!(by_name("link").link_target, Some(EntryKind::File));
    assert_eq!(
        by_name("link").size,
        Some(11),
        "a link shows its target's size"
    );
    assert_eq!(by_name("to-folder").link_target, Some(EntryKind::Directory));
    assert_eq!(by_name("dangling").link_target, None);
    assert!(!by_name("dangling").link_pending);

    // With no budget the links stay pending, and resolve one by one.
    let pending = provider
        .list(&root, &CancelToken::new(), 0, &mut |_| {})
        .unwrap();
    let link = pending.iter().find(|e| e.name == "link").unwrap();
    assert!(link.link_pending);
    let resolved = provider.resolve_link(&root, link).unwrap();
    assert_eq!(resolved.link_target, Some(EntryKind::File));
    assert!(!resolved.link_pending);

    // Many entries arrive in several batches that add up to the listing.
    let many = at("many");
    let mut batches = Vec::new();
    provider
        .list_batches(&many, &CancelToken::new(), 0, &mut |batch| {
            batches.push(batch)
        })
        .unwrap();
    let total: usize = batches.iter().map(Vec::len).sum();
    assert_eq!(total, 1_234);
    let all: BTreeSet<_> = batches.iter().flatten().map(|e| e.name.clone()).collect();
    assert_eq!(all.len(), 1_234);
    let mut counted = 0;
    provider
        .list(&many, &CancelToken::new(), 0, &mut |n| counted = n)
        .unwrap();
    assert_eq!(counted, 1_234);

    // Stat, typed errors.
    assert_eq!(
        provider.stat(&at("sub")).unwrap().kind,
        EntryKind::Directory
    );
    assert_eq!(provider.stat(&at("a.txt")).unwrap().name, "a.txt");
    assert_eq!(
        provider.stat(&at("link")).unwrap().link_target,
        Some(EntryKind::File)
    );
    assert!(matches!(
        provider.stat(&at("nope")),
        Err(VfsError::NotFound { .. })
    ));
    assert!(matches!(
        provider.list(&at("a.txt"), &CancelToken::new(), 0, &mut |_| {}),
        Err(VfsError::NotADirectory { .. })
    ));
    assert!(matches!(
        provider.list(&at("nope"), &CancelToken::new(), 0, &mut |_| {}),
        Err(VfsError::NotFound { .. })
    ));

    // Reading: whole, from an offset, past the end, a large file through the window.
    assert_eq!(read_all(&provider, &at("a.txt"), 0), b"hello world");
    assert_eq!(read_all(&provider, &at("a.txt"), 6), b"world");
    assert!(read_all(&provider, &at("a.txt"), 100).is_empty());
    assert_eq!(read_all(&provider, &at("big.bin"), 0), big);
    assert_eq!(
        read_all(&provider, &at("big.bin"), 1_000_001),
        big[1_000_001..]
    );
    assert!(matches!(
        provider.open_read(&at("sub")).map(|_| ()),
        Err(VfsError::IsADirectory { .. })
    ));
    assert!(matches!(
        provider.open_read(&at("nope")).map(|_| ()),
        Err(VfsError::NotFound { .. })
    ));
    // A stream dropped half way leaves the session usable.
    let mut half = provider.open_read(&at("big.bin")).unwrap();
    let mut buf = vec![0u8; 100_000];
    half.read_exact(&mut buf).unwrap();
    drop(half);
    assert_eq!(read_all(&provider, &at("a.txt"), 0), b"hello world");

    // Details, permissions, link text, canonical form.
    let details = provider.details(&at("a.txt")).unwrap();
    assert_eq!(details.size, Some(11));
    assert!(details.owner.is_some());
    assert!(details.mode.is_some());
    assert_eq!(details.mime_type.as_deref(), Some("text/plain"));
    let link_details = provider.details(&at("link")).unwrap();
    assert_eq!(link_details.symlink_target.as_deref(), Some("a.txt"));
    assert_eq!(link_details.resolves_to, Some(EntryKind::File));
    assert!(provider.permissions(&at("a.txt")).unwrap().mode.is_some());
    assert_eq!(provider.read_link(&at("link")).unwrap(), "a.txt");
    assert_eq!(provider.canonicalize(&at("link")).unwrap(), at("a.txt"));
    assert!(matches!(
        provider.canonicalize(&at("nope")),
        Err(VfsError::NotFound { .. })
    ));
    let key = root.connection_key().unwrap();
    assert_eq!(provider.connection_state(&key), ConnectionState::Connected);
}

fn passes_the_conformance_suite(server: &dyn Backend) {
    server.mkdir("empty");
    let provider = server.provider();
    conformance::run(&Subject {
        name: &format!("sftp/{}", server.name()),
        provider: &provider,
        root: server.location(server.port(), "empty"),
    });
}

fn a_listing_with_one_request_in_flight_gets_the_same_entries(server: &dyn Backend) {
    for n in 0..500 {
        server.put(&format!("n{n}"), b"");
    }
    let provider = server.provider_with(
        server.port(),
        SftpOptions::default().with_listing_requests(1),
    );
    let listed = provider
        .list(&server.data_location(), &CancelToken::new(), 0, &mut |_| {})
        .unwrap();
    assert_eq!(listed.len(), 500);
}

fn a_cancelled_listing_says_cancelled_and_the_session_stays_usable(server: &dyn Backend) {
    for n in 0..5_000 {
        server.put(&format!("n{n}"), b"");
    }
    // Slow enough that the listing is still running when the first batch has arrived.
    let proxy = server.proxy(Duration::from_millis(20));
    let provider =
        server.provider_with(proxy.port, SftpOptions::default().with_listing_requests(1));
    let root = server.location(proxy.port, "");
    let cancel = CancelToken::new();
    let mut seen = 0;
    let result = provider.list_batches(&root, &cancel, 0, &mut |batch| {
        seen += batch.len();
        cancel.cancel();
    });
    assert_eq!(result, Err(VfsError::Cancelled));
    assert!(seen < 5_000);
    assert_eq!(provider.stat(&root).unwrap().kind, EntryKind::Directory);
}

fn an_unknown_host_key_asks_and_is_trusted_by_its_fingerprint(server: &dyn Backend) {
    let known = Arc::new(MemoryKnownHosts::new());
    let provider = SftpProvider::new(
        SftpConfig::new(known.clone())
            .with_agent(AgentSource::None)
            .with_identity_files(vec![server.client_key()]),
    );
    let root = server.data_location();
    let key = root.connection_key().unwrap();
    let Err(VfsError::HostKeyUnknown { key: offered, .. }) = provider.stat(&root) else {
        panic!("an unknown server is not trusted");
    };
    assert_eq!(offered.fingerprint, server.host_key().fingerprint());
    assert_eq!(offered.host, format!("[127.0.0.1]:{}", server.port()));
    assert!(matches!(
        provider.connection_state(&key),
        ConnectionState::Failed {
            error: VfsError::HostKeyUnknown { .. }
        }
    ));
    // A fingerprint the person was not shown answers nothing.
    let wrong = ConnectAnswer::TrustHostKey {
        fingerprint: "SHA256:not-this-one".into(),
        remember: true,
    };
    assert!(matches!(
        provider.connect(&key, Some(wrong), &CancelToken::new()),
        Err(VfsError::HostKeyUnknown { .. })
    ));
    // Trusted for this session only: nothing is recorded, but reconnecting works.
    let once = ConnectAnswer::TrustHostKey {
        fingerprint: offered.fingerprint.clone(),
        remember: false,
    };
    provider
        .connect(&key, Some(once), &CancelToken::new())
        .unwrap();
    assert_eq!(
        known.check("127.0.0.1", server.port(), &server.host_key()),
        waypoint_provider_sftp::HostKeyCheck::Unknown
    );
    provider.disconnect(&key);
    assert_eq!(provider.connection_state(&key), ConnectionState::Idle);
    provider.stat(&root).unwrap();
    // Remembered: recorded in the known hosts.
    let remember = ConnectAnswer::TrustHostKey {
        fingerprint: offered.fingerprint.clone(),
        remember: true,
    };
    provider
        .connect(&key, Some(remember), &CancelToken::new())
        .unwrap();
    assert_eq!(
        known.check("127.0.0.1", server.port(), &server.host_key()),
        waypoint_provider_sftp::HostKeyCheck::Known
    );
}

fn a_changed_host_key_never_connects_without_the_explicit_answer(server: &dyn Backend) {
    let known = Arc::new(MemoryKnownHosts::new());
    let old = ServerKey::from_openssh(
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBCb8PDBXvzERLPgkKWxNbNUSsz4pysfnT/WkQUYHuQH",
    )
    .unwrap();
    known.remember("127.0.0.1", server.port(), &old).unwrap();
    let provider = SftpProvider::new(
        SftpConfig::new(known.clone())
            .with_agent(AgentSource::None)
            .with_identity_files(vec![server.client_key()]),
    );
    let root = server.data_location();
    let key = root.connection_key().unwrap();
    let Err(VfsError::HostKeyChanged { change, .. }) = provider.stat(&root) else {
        panic!("a changed key is refused");
    };
    assert_eq!(change.recorded_fingerprint, old.fingerprint());
    assert_eq!(change.offered_fingerprint, server.host_key().fingerprint());
    let plain = ConnectAnswer::TrustHostKey {
        fingerprint: server.host_key().fingerprint().into(),
        remember: true,
    };
    assert!(matches!(
        provider.connect(&key, Some(plain), &CancelToken::new()),
        Err(VfsError::HostKeyChanged { .. })
    ));
    let explicit = ConnectAnswer::TrustChangedHostKey {
        recorded_fingerprint: old.fingerprint().into(),
        offered_fingerprint: server.host_key().fingerprint().into(),
    };
    provider
        .connect(&key, Some(explicit), &CancelToken::new())
        .unwrap();
    provider.stat(&root).unwrap();
    assert_eq!(
        known.check("127.0.0.1", server.port(), &server.host_key()),
        waypoint_provider_sftp::HostKeyCheck::Known
    );
}

/// A credential source that hands out one passphrase.
struct Remembered(&'static str);

impl CredentialSource for Remembered {
    fn credential(
        &self,
        _: &waypoint_path::ConnectionKey,
        prompt: &AuthPrompt,
    ) -> Option<Credential> {
        matches!(prompt, AuthPrompt::Passphrase { .. })
            .then(|| Credential::Passphrase(Secret::from(self.0)))
    }
}

fn an_encrypted_key_asks_for_its_passphrase(server: &dyn Backend) {
    let config = || {
        SftpConfig::new(server.known_hosts(server.port()))
            .with_agent(AgentSource::None)
            .with_identity_files(vec![server.encrypted_key()])
    };
    let provider = SftpProvider::new(config());
    let root = server.data_location();
    let key = root.connection_key().unwrap();
    let Err(VfsError::AuthRequired { prompt, .. }) = provider.stat(&root) else {
        panic!("an encrypted key needs its passphrase");
    };
    let AuthPrompt::Passphrase { subject } = *prompt else {
        panic!("the prompt asks for a passphrase");
    };
    assert!(subject.ends_with("client_encrypted"));
    let wrong = ConnectAnswer::Credential(Credential::Passphrase(Secret::from("wrong")));
    assert!(matches!(
        provider.connect(&key, Some(wrong), &CancelToken::new()),
        Err(VfsError::AuthFailed { .. })
    ));
    let right = ConnectAnswer::Credential(Credential::Passphrase(Secret::from(PASSPHRASE)));
    provider
        .connect(&key, Some(right), &CancelToken::new())
        .unwrap();
    provider.stat(&root).unwrap();
    // A remembered passphrase logs in without asking.
    let remembered = SftpProvider::new(config().with_credentials(Arc::new(Remembered(PASSPHRASE))));
    remembered.stat(&root).unwrap();
}

fn a_key_the_server_does_not_accept_fails_the_login(server: &dyn Backend) {
    let stranger = server.stranger_key();
    let provider = SftpProvider::new(
        SftpConfig::new(server.known_hosts(server.port()))
            .with_agent(AgentSource::None)
            .with_identity_files(vec![stranger]),
    );
    assert!(matches!(
        provider.stat(&server.data_location()),
        Err(VfsError::AuthFailed { .. })
    ));
}

/// OpenSSH only: the agent needs the real `ssh-agent` and `ssh-add`.
#[test]
fn logs_in_through_the_ssh_agent() {
    let Some(server) = Sshd::start() else { return };
    let socket = server.dir.path().join("agent.sock");
    let Ok(mut agent) = Command::new("ssh-agent")
        .arg("-D")
        .arg("-a")
        .arg(&socket)
        .stdout(Stdio::null())
        .spawn()
    else {
        eprintln!("skipping: no ssh-agent");
        return;
    };
    for _ in 0..200 {
        if socket.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let added = Command::new("ssh-add")
        .arg(&server.client_key)
        .env("SSH_AUTH_SOCK", &socket)
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(added.success());
    let provider = SftpProvider::new(
        SftpConfig::new(server.known_hosts(server.port()))
            .with_agent(AgentSource::Path(socket.clone()))
            .with_identity_files(Vec::new()),
    );
    let result = provider.stat(&server.data_location());
    let _ = agent.kill();
    let _ = agent.wait();
    assert_eq!(result.unwrap().kind, EntryKind::Directory);
}

fn reconnects_after_the_connection_drops(server: &dyn Backend) {
    server.put("a.txt", b"x");
    let proxy = server.proxy(Duration::ZERO);
    let provider = server.provider_with(proxy.port, SftpOptions::default());
    let root = server.location(proxy.port, "");
    let key = root.connection_key().unwrap();
    provider.stat(&root).unwrap();
    proxy.sever();
    // The next call finds the session gone and opens another by itself.
    let listed = provider
        .list(&root, &CancelToken::new(), 0, &mut |_| {})
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(provider.connection_state(&key), ConnectionState::Connected);
}

fn a_server_that_stops_answering_times_out_and_comes_back(server: &dyn Backend) {
    let proxy = server.proxy(Duration::ZERO);
    let provider = server.provider_with(
        proxy.port,
        SftpOptions::default().with_timeout(Duration::from_secs(1)),
    );
    let root = server.location(proxy.port, "");
    let key = root.connection_key().unwrap();
    provider.stat(&root).unwrap();
    proxy.stall();
    assert!(matches!(
        provider.stat(&root),
        Err(VfsError::Timeout { .. })
    ));
    assert!(matches!(
        provider.connection_state(&key),
        ConnectionState::Failed {
            error: VfsError::Timeout { .. }
        }
    ));
    proxy.resume();
    proxy.sever();
    provider.stat(&root).unwrap();
}

#[test]
fn nothing_listening_is_unreachable() {
    // Needs no server: a port nobody listens on.
    let provider = SftpProvider::new(
        SftpConfig::new(Arc::new(MemoryKnownHosts::new())).with_agent(AgentSource::None),
    );
    let port = support::free_port();
    let path = VfsPath::from_uri(&format!("sftp://me@127.0.0.1:{port}/")).unwrap();
    assert_eq!(
        provider.stat(&path).map(|_| ()),
        Err(VfsError::Unreachable {
            location: path.to_location(),
            reason: UnreachableReason::Refused,
        })
    );
    let unknown = VfsPath::from_uri("sftp://me@no-such-host.invalid/").unwrap();
    assert!(matches!(
        provider.stat(&unknown),
        Err(VfsError::Unreachable {
            reason: UnreachableReason::NameNotResolved,
            ..
        })
    ));
}

on_both!(
    lists_stats_and_reads,
    passes_the_conformance_suite,
    a_listing_with_one_request_in_flight_gets_the_same_entries,
    a_cancelled_listing_says_cancelled_and_the_session_stays_usable,
    an_unknown_host_key_asks_and_is_trusted_by_its_fingerprint,
    a_changed_host_key_never_connects_without_the_explicit_answer,
    an_encrypted_key_asks_for_its_passphrase,
    a_key_the_server_does_not_accept_fails_the_login,
    reconnects_after_the_connection_drops,
    a_server_that_stops_answering_times_out_and_comes_back,
);

// What only the in-process server can script.

fn fake_provider(
    server: &FakeSftp,
    credentials: Option<Arc<dyn CredentialSource>>,
) -> SftpProvider {
    let config = SftpConfig::new(server.known_hosts(server.port()))
        .with_agent(AgentSource::None)
        .with_identity_files(Vec::new());
    SftpProvider::new(match credentials {
        Some(credentials) => config.with_credentials(credentials),
        None => config,
    })
}

/// A credential source that hands out one password.
struct Password(&'static str);

impl CredentialSource for Password {
    fn credential(
        &self,
        _: &waypoint_path::ConnectionKey,
        prompt: &AuthPrompt,
    ) -> Option<Credential> {
        matches!(prompt, AuthPrompt::Password { .. }).then(|| Credential::Password {
            user: None,
            password: Secret::from(self.0),
        })
    }
}

#[test]
fn a_password_is_asked_for_and_checked() {
    let server = FakeSftp::start();
    server.allow_passwords(true);
    let provider = fake_provider(&server, None);
    let root = server.data_location();
    let key = root.connection_key().unwrap();
    assert!(matches!(
        provider.stat(&root),
        Err(VfsError::AuthRequired { .. })
    ));
    let wrong = ConnectAnswer::Credential(Credential::Password {
        user: None,
        password: Secret::from("wrong"),
    });
    assert!(matches!(
        provider.connect(&key, Some(wrong), &CancelToken::new()),
        Err(VfsError::AuthFailed { .. })
    ));
    let right = ConnectAnswer::Credential(Credential::Password {
        user: None,
        password: Secret::from(support::fake::PASSWORD),
    });
    provider
        .connect(&key, Some(right), &CancelToken::new())
        .unwrap();
    assert_eq!(provider.stat(&root).unwrap().kind, EntryKind::Directory);
    // A remembered password logs in without asking.
    let remembered = fake_provider(&server, Some(Arc::new(Password(support::fake::PASSWORD))));
    assert_eq!(remembered.stat(&root).unwrap().kind, EntryKind::Directory);
}

#[test]
fn a_refused_login_fails_and_the_next_attempt_after_it_clears_succeeds() {
    let server = FakeSftp::start();
    let provider = server.provider();
    let root = server.data_location();
    server.set_refuse_logins(true);
    assert!(matches!(
        provider.stat(&root),
        Err(VfsError::AuthFailed { .. })
    ));
    server.set_refuse_logins(false);
    assert_eq!(provider.stat(&root).unwrap().kind, EntryKind::Directory);
}

#[test]
fn a_host_key_that_changes_while_connected_is_caught_on_the_next_connection() {
    let server = FakeSftp::start();
    let provider = server.provider();
    let root = server.data_location();
    let key = root.connection_key().unwrap();
    provider.stat(&root).unwrap();
    server.rotate_host_key();
    // The open session is unaffected; a new one is refused.
    provider.stat(&root).unwrap();
    server.drop_connections();
    let Err(VfsError::HostKeyChanged { change, .. }) = provider.stat(&root) else {
        panic!("a new host key is not trusted silently");
    };
    assert_eq!(change.offered_fingerprint, server.host_key().fingerprint());
    let explicit = ConnectAnswer::TrustChangedHostKey {
        recorded_fingerprint: change.recorded_fingerprint.clone(),
        offered_fingerprint: change.offered_fingerprint.clone(),
    };
    provider
        .connect(&key, Some(explicit), &CancelToken::new())
        .unwrap();
    provider.stat(&root).unwrap();
}

#[test]
fn a_connection_dropped_in_the_middle_of_a_listing_is_opened_again() {
    let server = FakeSftp::start();
    for n in 0..1_000 {
        server.put(&format!("n{n}"), b"");
    }
    server.set_readdir_batch(10);
    let provider = server.provider();
    let root = server.data_location();
    provider.stat(&root).unwrap();
    // Cuts the connection a few requests into the listing.
    server.drop_after_requests(5);
    // Either the provider connects again and lists the whole folder, or it reports the cut: never
    // a short listing passed off as the folder.
    if let Ok(first) = provider.list(&root, &CancelToken::new(), 0, &mut |_| {}) {
        assert_eq!(first.len(), 1_000);
    }
    let listed = provider
        .list(&root, &CancelToken::new(), 0, &mut |_| {})
        .unwrap();
    assert_eq!(listed.len(), 1_000);
    assert!(server.connections() > 1, "the session was opened again");
}

#[test]
fn readdir_answers_in_batches_of_the_size_the_server_chooses() {
    let server = FakeSftp::start();
    for n in 0..95 {
        server.put(&format!("n{n}"), b"");
    }
    server.set_readdir_batch(7);
    let provider = server.provider_with(
        server.port(),
        SftpOptions::default().with_listing_requests(1),
    );
    let before = server.requests();
    let listed = provider
        .list(&server.data_location(), &CancelToken::new(), 0, &mut |_| {})
        .unwrap();
    assert_eq!(listed.len(), 95);
    // 95 entries and `.` and `..` at 7 a reply, then the end of the listing.
    assert!(server.requests() - before > 97 / 7);
}

#[test]
fn a_server_without_the_extensions_conforms_too() {
    let server = FakeSftp::start_with(support::fake::Flavour::Plain);
    server.mkdir("empty");
    let provider = server.provider();
    conformance::run(&Subject {
        name: "sftp/in-process-plain",
        provider: &provider,
        root: server.location(server.port(), "empty"),
    });
    // The links it makes hold the text they were given: the paths go in the draft's order.
    server.put("a.txt", b"x");
    provider
        .symlink(&server.location(server.port(), "link"), "a.txt".as_ref())
        .unwrap();
    assert_eq!(server.link_target("link").as_deref(), Some("a.txt"));
    assert!(provider.free_space(&server.data_location()).is_none());
}
