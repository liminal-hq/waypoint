// The SFTP provider with OpenSSH's own files against a real server: host keys recorded in a
// `known_hosts` file, host aliases from an SSH config, and jump hosts. Each test skips with a
// message when there is no `sshd` (see `support`).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::fs;
use std::sync::Arc;
use std::time::Duration;

use support::{user, Proxy, Sshd};
use waypoint_path::VfsPath;
use waypoint_protocol::{ConnectionState, VfsError};
use waypoint_provider_sftp::{AgentSource, OpenSshKnownHosts, SftpConfig, SftpProvider, SshConfig};
use waypoint_vfs::{CancelToken, ConnectAnswer, EntryKind, Provider};

fn provider(
    server: &Sshd,
    known_hosts: Arc<dyn waypoint_provider_sftp::KnownHosts>,
) -> SftpProvider {
    SftpProvider::new(
        SftpConfig::new(known_hosts)
            .with_agent(AgentSource::None)
            .with_identity_files(vec![server.client_key.clone()]),
    )
}

#[test]
fn a_trusted_key_is_appended_to_known_hosts_and_used_next_time() {
    let Some(server) = Sshd::start() else { return };
    let file = server.dir.path().join("known_hosts");
    let root = server.data_location();
    let key = root.connection_key().unwrap();
    let first = provider(&server, Arc::new(OpenSshKnownHosts::new(&file)));
    let Err(VfsError::HostKeyUnknown { key: offered, .. }) = first.stat(&root) else {
        panic!("an unknown server asks");
    };
    first
        .connect(
            &key,
            Some(ConnectAnswer::TrustHostKey {
                fingerprint: offered.fingerprint,
                remember: true,
            }),
            &CancelToken::new(),
        )
        .unwrap();
    let text = fs::read_to_string(&file).unwrap();
    assert_eq!(
        text,
        format!(
            "[127.0.0.1]:{} {}\n",
            server.port,
            server.host_key.to_openssh()
        )
    );
    // `ssh` reads the same line; a new provider connects without asking.
    let second = provider(&server, Arc::new(OpenSshKnownHosts::new(&file)));
    assert_eq!(second.stat(&root).unwrap().kind, EntryKind::Directory);
}

#[test]
fn a_changed_key_in_known_hosts_is_replaced_only_by_the_explicit_answer() {
    let Some(server) = Sshd::start() else { return };
    let file = server.dir.path().join("known_hosts");
    let old = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBCb8PDBXvzERLPgkKWxNbNUSsz4pysfnT/WkQUYHuQH";
    fs::write(
        &file,
        format!("other.example {old}\n[127.0.0.1]:{} {old}\n", server.port),
    )
    .unwrap();
    let provider = provider(&server, Arc::new(OpenSshKnownHosts::new(&file)));
    let root = server.data_location();
    let key = root.connection_key().unwrap();
    let Err(VfsError::HostKeyChanged { change, .. }) = provider.stat(&root) else {
        panic!("a changed key never connects silently");
    };
    assert_eq!(change.host, format!("[127.0.0.1]:{}", server.port));
    assert_eq!(change.offered_fingerprint, server.host_key.fingerprint());
    // Nothing is written until the explicit answer.
    assert!(fs::read_to_string(&file).unwrap().contains(old));
    provider
        .connect(
            &key,
            Some(ConnectAnswer::TrustChangedHostKey {
                recorded_fingerprint: change.recorded_fingerprint.clone(),
                offered_fingerprint: change.offered_fingerprint.clone(),
            }),
            &CancelToken::new(),
        )
        .unwrap();
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        format!(
            "other.example {old}\n[127.0.0.1]:{} {}\n",
            server.port,
            server.host_key.to_openssh()
        )
    );
}

#[test]
fn an_alias_from_the_ssh_config_reaches_its_server() {
    let Some(server) = Sshd::start() else { return };
    fs::write(server.data.join("hello.txt"), "hi").unwrap();
    let config = SshConfig::parse(
        &format!(
            "Host box\n  HostName 127.0.0.1\n  Port {}\n  User {}\n  IdentityFile {}\n",
            server.port,
            user(),
            server.client_key.display()
        ),
        server.dir.path(),
    );
    let provider = SftpProvider::new(
        SftpConfig::new(server.known_hosts(server.port))
            .with_agent(AgentSource::None)
            .with_ssh_config(Arc::new(config)),
    );
    let path = VfsPath::from_uri("sftp://box/")
        .unwrap()
        .join(server.data.join("hello.txt").to_str().unwrap())
        .unwrap();
    assert_eq!(provider.stat(&path).unwrap().size, Some(2));
    assert_eq!(
        provider.connection_state(&path.connection_key().unwrap()),
        ConnectionState::Connected
    );
}

#[test]
fn a_jump_host_carries_the_connection() {
    let Some(server) = Sshd::start() else { return };
    // The jump host is reached through a proxy that counts connections; the target is reached
    // from the jump host over a forwarded channel, so the proxy sees one connection only.
    let jump = Proxy::start(server.port, Duration::ZERO);
    let config = SshConfig::parse(
        &format!(
            "Host target\n  HostName 127.0.0.1\n  Port {port}\n  User {user}\n  ProxyJump gateway\n\
             Host gateway\n  HostName 127.0.0.1\n  Port {jump}\n  User {user}\n\
             Host *\n  IdentityFile {key}\n",
            port = server.port,
            jump = jump.port,
            user = user(),
            key = server.client_key.display()
        ),
        server.dir.path(),
    );
    // Only the target is known: the jump host's key is asked about first.
    let known = server.known_hosts(server.port);
    let provider = SftpProvider::new(
        SftpConfig::new(known.clone())
            .with_agent(AgentSource::None)
            .with_ssh_config(Arc::new(config)),
    );
    let root = VfsPath::from_uri("sftp://target/")
        .unwrap()
        .join(server.data.to_str().unwrap())
        .unwrap();
    let key = root.connection_key().unwrap();
    let Err(VfsError::HostKeyUnknown { key: offered, .. }) = provider.stat(&root) else {
        panic!("the jump host's key is checked too");
    };
    assert_eq!(offered.host, format!("[127.0.0.1]:{}", jump.port));
    provider
        .connect(
            &key,
            Some(ConnectAnswer::TrustHostKey {
                fingerprint: offered.fingerprint,
                remember: true,
            }),
            &CancelToken::new(),
        )
        .unwrap();
    let before = jump.connections();
    assert_eq!(provider.stat(&root).unwrap().kind, EntryKind::Directory);
    assert_eq!(jump.connections(), before, "the open session is reused");
    assert_eq!(
        jump.connections(),
        2,
        "the failed attempt and the trusted one"
    );
    // Dropping the jump host's connection drops the target's; the next call goes round again.
    jump.sever();
    assert_eq!(provider.stat(&root).unwrap().kind, EntryKind::Directory);
    assert_eq!(jump.connections(), 3);
}
