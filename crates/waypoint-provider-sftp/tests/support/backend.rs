// What the SFTP provider's tests need from a server, so one test body runs against the in-process
// server on every platform and against OpenSSH where `sshd` is installed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use waypoint_path::VfsPath;
use waypoint_provider_sftp::{
    AgentSource, KnownHosts, MemoryKnownHosts, ServerKey, SftpConfig, SftpOptions, SftpProvider,
};

use super::fake::{Content, FakeSftp, DATA, USER};
use super::{Proxy, Sshd};

/// A server with a folder the test fills and checks. Paths are relative to that folder, with `/`
/// between names; `""` is the folder itself.
pub trait Backend {
    /// Named in a failure.
    fn name(&self) -> &'static str;
    fn port(&self) -> u16;
    /// A folder on this machine for the test's own files (keys, known hosts).
    fn dir(&self) -> &Path;
    fn host_key(&self) -> ServerKey;
    fn client_key(&self) -> PathBuf;
    fn encrypted_key(&self) -> PathBuf;
    /// A private key the server does not accept.
    fn stranger_key(&self) -> PathBuf;
    fn user(&self) -> String;
    /// The folder's absolute path on the server.
    fn data_path(&self) -> String;
    /// A delaying, stallable, severable proxy in front of the server.
    fn proxy(&self, round_trip: Duration) -> Proxy;

    fn put(&self, rel: &str, bytes: &[u8]);
    fn mkdir(&self, rel: &str);
    fn symlink(&self, target: &str, rel: &str);
    fn get(&self, rel: &str) -> Vec<u8>;
    /// The names in a folder, sorted.
    fn names(&self, rel: &str) -> Vec<String>;
    fn exists(&self, rel: &str) -> bool;
    fn is_dir(&self, rel: &str) -> bool;
    /// What a link holds, when `rel` is one.
    fn link_target(&self, rel: &str) -> Option<String>;
    fn remove(&self, rel: &str);
    /// Sets permission bits (Unix servers only: a no-op where the file system has none).
    fn chmod(&self, rel: &str, mode: u32);
    /// The permission bits of a file.
    fn mode(&self, rel: &str) -> u32;
    /// The modification time in seconds.
    fn mtime(&self, rel: &str) -> u64;
    /// What stays the same when an entry is renamed (an inode), where the server has one.
    fn identity(&self, rel: &str) -> Option<u64>;

    /// Known hosts that trust this server at `port` (its own, or a proxy's).
    fn known_hosts(&self, port: u16) -> Arc<MemoryKnownHosts> {
        let known = Arc::new(MemoryKnownHosts::new());
        known.remember("127.0.0.1", port, &self.host_key()).unwrap();
        known
    }

    /// A provider that trusts this server and logs in with the plain client key.
    fn provider(&self) -> SftpProvider {
        self.provider_with(self.port(), SftpOptions::default())
    }

    fn provider_with(&self, port: u16, options: SftpOptions) -> SftpProvider {
        SftpProvider::new(
            SftpConfig::new(self.known_hosts(port))
                .with_agent(AgentSource::None)
                .with_identity_files(vec![self.client_key()])
                .with_options(options),
        )
    }

    /// A provider that uses none of OpenSSH's extensions, as against a server with version 3 only.
    fn plain_provider(&self) -> SftpProvider {
        SftpProvider::new(
            SftpConfig::new(self.known_hosts(self.port()))
                .with_agent(AgentSource::None)
                .with_identity_files(vec![self.client_key()])
                .with_plain_protocol(),
        )
    }

    /// The location of `rel` through `port`.
    fn location(&self, port: u16, rel: &str) -> VfsPath {
        let root = VfsPath::from_uri(&format!("sftp://{}@127.0.0.1:{port}/", self.user())).unwrap();
        let path = format!("{}/{rel}", self.data_path());
        root.join(path.trim_end_matches('/')).unwrap()
    }

    /// The folder's location on this server.
    fn data_location(&self) -> VfsPath {
        self.location(self.port(), "")
    }
}

impl Backend for Sshd {
    fn name(&self) -> &'static str {
        "openssh"
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn dir(&self) -> &Path {
        self.dir.path()
    }

    fn host_key(&self) -> ServerKey {
        self.host_key.clone()
    }

    fn client_key(&self) -> PathBuf {
        self.client_key.clone()
    }

    fn encrypted_key(&self) -> PathBuf {
        self.encrypted_key.clone()
    }

    fn stranger_key(&self) -> PathBuf {
        let path = self.dir.path().join("stranger");
        let status = std::process::Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        path
    }

    fn user(&self) -> String {
        super::user()
    }

    fn data_path(&self) -> String {
        self.data.to_str().unwrap().to_owned()
    }

    fn proxy(&self, round_trip: Duration) -> Proxy {
        Proxy::start(self.port, round_trip)
    }

    fn put(&self, rel: &str, bytes: &[u8]) {
        fs::write(self.data.join(rel), bytes).unwrap();
    }

    fn mkdir(&self, rel: &str) {
        fs::create_dir_all(self.data.join(rel)).unwrap();
    }

    fn symlink(&self, target: &str, rel: &str) {
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, self.data.join(rel)).unwrap();
        #[cfg(not(unix))]
        unreachable!("{target} {rel}: the real-server tests run on Linux only");
    }

    fn get(&self, rel: &str) -> Vec<u8> {
        fs::read(self.data.join(rel)).unwrap()
    }

    fn names(&self, rel: &str) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.data.join(rel))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn exists(&self, rel: &str) -> bool {
        fs::symlink_metadata(self.data.join(rel)).is_ok()
    }

    fn is_dir(&self, rel: &str) -> bool {
        self.data.join(rel).is_dir()
    }

    fn link_target(&self, rel: &str) -> Option<String> {
        fs::read_link(self.data.join(rel))
            .ok()
            .map(|target| target.to_string_lossy().into_owned())
    }

    fn remove(&self, rel: &str) {
        let path = self.data.join(rel);
        if path.is_dir() {
            fs::remove_dir_all(path).unwrap();
        } else {
            fs::remove_file(path).unwrap();
        }
    }

    fn chmod(&self, rel: &str, mode: u32) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(self.data.join(rel), fs::Permissions::from_mode(mode)).unwrap();
        }
        #[cfg(not(unix))]
        let _ = (rel, mode);
    }

    fn mode(&self, rel: &str) -> u32 {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::metadata(self.data.join(rel))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777
        }
        #[cfg(not(unix))]
        {
            let _ = rel;
            0
        }
    }

    fn identity(&self, rel: &str) -> Option<u64> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(self.data.join(rel))
                .ok()
                .map(|meta| meta.ino())
        }
        #[cfg(not(unix))]
        {
            let _ = rel;
            None
        }
    }

    fn mtime(&self, rel: &str) -> u64 {
        fs::metadata(self.data.join(rel))
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

fn absolute(rel: &str) -> String {
    if rel.is_empty() {
        DATA.to_owned()
    } else {
        format!("{DATA}/{rel}")
    }
}

impl Backend for FakeSftp {
    fn name(&self) -> &'static str {
        "in-process"
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn dir(&self) -> &Path {
        self.dir.path()
    }

    fn host_key(&self) -> ServerKey {
        FakeSftp::host_key(self)
    }

    fn client_key(&self) -> PathBuf {
        self.client_key.clone()
    }

    fn encrypted_key(&self) -> PathBuf {
        self.encrypted_key.clone()
    }

    fn stranger_key(&self) -> PathBuf {
        FakeSftp::stranger_key(self)
    }

    fn user(&self) -> String {
        USER.to_owned()
    }

    fn data_path(&self) -> String {
        DATA.to_owned()
    }

    fn proxy(&self, round_trip: Duration) -> Proxy {
        FakeSftp::proxy(self, round_trip)
    }

    fn put(&self, rel: &str, bytes: &[u8]) {
        self.tree().put(&absolute(rel), bytes);
    }

    fn mkdir(&self, rel: &str) {
        self.tree().mkdirs(&absolute(rel));
    }

    fn symlink(&self, target: &str, rel: &str) {
        self.tree().link(&absolute(rel), target);
    }

    fn get(&self, rel: &str) -> Vec<u8> {
        match self
            .tree()
            .get(&absolute(rel))
            .map(|node| node.content.clone())
        {
            Some(Content::File(bytes)) => bytes,
            _ => panic!("{rel} is not a file on the server"),
        }
    }

    fn names(&self, rel: &str) -> Vec<String> {
        let mut names = self.tree().children(&absolute(rel));
        names.sort();
        names
    }

    fn exists(&self, rel: &str) -> bool {
        self.tree().get(&absolute(rel)).is_some()
    }

    fn is_dir(&self, rel: &str) -> bool {
        matches!(
            self.tree().get(&absolute(rel)).map(|node| &node.content),
            Some(Content::Dir)
        )
    }

    fn link_target(&self, rel: &str) -> Option<String> {
        match self.tree().get(&absolute(rel)).map(|node| &node.content) {
            Some(Content::Link(target)) => Some(target.clone()),
            _ => None,
        }
    }

    fn remove(&self, rel: &str) {
        self.tree().remove_tree(&absolute(rel));
    }

    fn chmod(&self, rel: &str, mode: u32) {
        self.tree().set_mode(&absolute(rel), mode);
    }

    fn mode(&self, rel: &str) -> u32 {
        self.tree()
            .get(&absolute(rel))
            .expect("the entry exists")
            .mode
    }

    fn identity(&self, rel: &str) -> Option<u64> {
        let _ = rel;
        None
    }

    fn mtime(&self, rel: &str) -> u64 {
        u64::from(
            self.tree()
                .get(&absolute(rel))
                .expect("the entry exists")
                .mtime,
        )
    }
}
