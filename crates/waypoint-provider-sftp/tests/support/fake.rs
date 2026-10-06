// An in-process SFTP server over an in-memory tree, built on `russh`'s server and `russh-sftp`'s
// server trait, so the provider's tests run on every platform and without `sshd`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `FakeSftp::start` listens on a free loopback port and serves an in-memory tree that the test
//! fills through `FakeSftp::tree`. It is a test double (nothing of it is in the shipped crate):
//! `tests/support` is compiled into the test binaries only.
//!
//! What it does like OpenSSH's `sftp-server`: SFTP version 3; the `posix-rename@openssh.com`,
//! `statvfs@openssh.com`, `fsync@openssh.com` and `lsetstat@openssh.com` extensions, announced
//! when `Flavour::OpenSsh` (the default) and left out for `Flavour::Plain`; `symlink` taking the
//! target first when the extensions are announced; a `rename` that fails when the target exists;
//! `readdir` answering in batches (`set_readdir_batch`, 100 by default, with `.` and `..` first);
//! generic `Failure` for what `errno` values SFTP version 3 has no code for (an entry that exists,
//! a directory that is not empty, a file where a folder should be); and `PermissionDenied` for an
//! entry created, removed or renamed in a folder without its owner-write bit.
//!
//! What a test can script: users and passwords (`USER`, `PASSWORD`) and public keys (the plain
//! and the encrypted client key written to `dir`), `set_refuse_logins`, `rotate_host_key`,
//! `drop_connections`, `drop_after_requests` and a count of connections. Latency is added by
//! putting a `Proxy` in front (`FakeSftp::proxy`), which delays the wire the way a network does,
//! so pipelined requests overlap as they would against a real server; a delay inside the server
//! would serialise them, because `russh-sftp` answers one request at a time.
//!
//! What it does not do: serve more than one user, forward ports other than as a jump host (`direct-tcpip`), apply real
//! owners or a `umask` other than `022`, or enforce read permissions.

#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::net::{Shutdown, TcpStream as StdStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use russh::keys::ssh_key::{Algorithm, LineEnding};
use russh::keys::{PrivateKey, PublicKey};
use russh::server::{Auth, Config, Handler, Msg, Session};
use russh::{Channel, ChannelId};
use russh::{MethodKind, MethodSet};
use russh_sftp::extensions::Statvfs;
use russh_sftp::protocol::{
    Attrs, Data, ExtendedReply, File, FileAttributes, Handle, Name, OpenFlags, Packet, Status,
    StatusCode, Version,
};
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use waypoint_provider_sftp::ServerKey;

use super::{temp_dir, Proxy, PASSPHRASE};

/// The one account the server knows.
pub const USER: &str = "tester";
/// Its password.
pub const PASSWORD: &str = "open sesame";
/// The folder the tests fill.
pub const DATA: &str = "/data";

const DIR: u32 = 0o040000;
const REG: u32 = 0o100000;
const LNK: u32 = 0o120000;

/// Which SFTP server it imitates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavour {
    /// OpenSSH's: the extensions the provider uses are announced.
    OpenSsh,
    /// Version 3 and nothing else.
    Plain,
}

#[derive(Clone)]
pub enum Content {
    File(Vec<u8>),
    Dir,
    Link(String),
}

#[derive(Clone)]
pub struct Node {
    pub content: Content,
    /// The permission bits (`0o644`), without the type.
    pub mode: u32,
    pub atime: u32,
    pub mtime: u32,
}

fn now() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as u32)
}

impl Node {
    fn new(content: Content, mode: u32) -> Node {
        Node {
            content,
            mode,
            atime: now(),
            mtime: now(),
        }
    }

    fn is_dir(&self) -> bool {
        matches!(self.content, Content::Dir)
    }

    fn attrs(&self) -> FileAttributes {
        let (kind, size) = match &self.content {
            Content::File(bytes) => (REG, bytes.len() as u64),
            Content::Dir => (DIR, 4096),
            Content::Link(target) => (LNK, target.len() as u64),
        };
        FileAttributes {
            size: Some(size),
            uid: Some(1000),
            user: Some(USER.to_owned()),
            gid: Some(1000),
            group: Some(USER.to_owned()),
            permissions: Some(kind | self.mode),
            atime: Some(self.atime),
            mtime: Some(self.mtime),
        }
    }
}

/// The in-memory file system, keyed by canonical absolute path.
pub struct Tree {
    nodes: BTreeMap<String, Node>,
}

type Reply<T> = Result<T, StatusCode>;

fn parent_of(path: &str) -> &str {
    match path.rfind('/') {
        Some(0) | None => "/",
        Some(at) => &path[..at],
    }
}

fn name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn join(folder: &str, name: &str) -> String {
    if folder == "/" {
        format!("/{name}")
    } else {
        format!("{folder}/{name}")
    }
}

impl Tree {
    fn new() -> Tree {
        let mut nodes = BTreeMap::new();
        nodes.insert("/".to_owned(), Node::new(Content::Dir, 0o755));
        let mut tree = Tree { nodes };
        tree.mkdirs(DATA);
        tree
    }

    pub fn get(&self, path: &str) -> Option<&Node> {
        self.nodes.get(path)
    }

    /// Creates `path` and the folders above it.
    pub fn mkdirs(&mut self, path: &str) {
        let mut current = String::new();
        for part in path.split('/').filter(|part| !part.is_empty()) {
            current = join(if current.is_empty() { "/" } else { &current }, part);
            self.nodes
                .entry(current.clone())
                .or_insert_with(|| Node::new(Content::Dir, 0o755));
        }
    }

    /// Writes a file, and the folders above it.
    pub fn put(&mut self, path: &str, bytes: &[u8]) {
        self.mkdirs(parent_of(path));
        self.nodes.insert(
            path.to_owned(),
            Node::new(Content::File(bytes.to_vec()), 0o644),
        );
    }

    pub fn link(&mut self, path: &str, target: &str) {
        self.mkdirs(parent_of(path));
        self.nodes.insert(
            path.to_owned(),
            Node::new(Content::Link(target.to_owned()), 0o777),
        );
    }

    /// Removes an entry and everything inside it.
    pub fn remove_tree(&mut self, path: &str) {
        let prefix = format!("{path}/");
        self.nodes
            .retain(|key, _| key != path && !key.starts_with(&prefix));
    }

    pub fn set_mode(&mut self, path: &str, mode: u32) {
        self.nodes.get_mut(path).expect("the entry exists").mode = mode;
    }

    /// The names inside a folder.
    pub fn children(&self, folder: &str) -> Vec<String> {
        let prefix = if folder == "/" {
            "/".to_owned()
        } else {
            format!("{folder}/")
        };
        self.nodes
            .range(prefix.clone()..)
            .take_while(|(path, _)| path.starts_with(&prefix))
            .filter(|(path, _)| !path[prefix.len()..].contains('/') && path.len() > prefix.len())
            .map(|(path, _)| path[prefix.len()..].to_owned())
            .collect()
    }

    /// The canonical path of `path`: `.`, `..` and links resolved (the last one only when
    /// `follow_last`). A missing last part is returned as it is; a missing folder is an error.
    fn resolve(&self, path: &str, follow_last: bool) -> Reply<String> {
        let mut pending: std::collections::VecDeque<String> =
            path.split('/').map(str::to_owned).collect();
        let mut current = "/".to_owned();
        let mut links = 0;
        while let Some(part) = pending.pop_front() {
            match part.as_str() {
                "" | "." => continue,
                ".." => {
                    current = parent_of(&current).to_owned();
                    continue;
                }
                _ => {}
            }
            let next = join(&current, &part);
            match self.nodes.get(&next) {
                None if pending.iter().all(|rest| rest.is_empty()) => return Ok(next),
                None => return Err(StatusCode::NoSuchFile),
                Some(node) => match &node.content {
                    Content::Link(target) if follow_last || !pending.is_empty() => {
                        links += 1;
                        if links > 40 {
                            return Err(StatusCode::Failure);
                        }
                        if target.starts_with('/') {
                            current = "/".to_owned();
                        }
                        for part in target.split('/').rev() {
                            pending.push_front(part.to_owned());
                        }
                    }
                    Content::File(_) if !pending.is_empty() => return Err(StatusCode::Failure),
                    _ => current = next,
                },
            }
        }
        Ok(current)
    }

    fn existing(&self, path: &str, follow_last: bool) -> Reply<(String, &Node)> {
        let real = self.resolve(path, follow_last)?;
        match self.nodes.get(&real) {
            Some(node) => Ok((real, node)),
            None => Err(StatusCode::NoSuchFile),
        }
    }

    /// The canonical path of an entry that is to be created, whose folder must be writable.
    fn creatable(&self, path: &str) -> Reply<String> {
        let real = self.resolve(path, false)?;
        match self.nodes.get(parent_of(&real)) {
            None => Err(StatusCode::NoSuchFile),
            Some(folder) if !folder.is_dir() => Err(StatusCode::Failure),
            Some(folder) if folder.mode & 0o200 == 0 => Err(StatusCode::PermissionDenied),
            Some(_) => Ok(real),
        }
    }

    fn check_writable_folder(&self, path: &str) -> Reply<()> {
        match self.nodes.get(parent_of(path)) {
            Some(folder) if folder.mode & 0o200 == 0 => Err(StatusCode::PermissionDenied),
            _ => Ok(()),
        }
    }

    fn rename(&mut self, from: &str, to: &str, replace: bool) -> Reply<()> {
        let from = self.resolve(from, false)?;
        if !self.nodes.contains_key(&from) {
            return Err(StatusCode::NoSuchFile);
        }
        let to = self.resolve(to, false)?;
        if from == to {
            return Ok(());
        }
        self.check_writable_folder(&from)?;
        let target_folder = self
            .nodes
            .get(parent_of(&to))
            .ok_or(StatusCode::NoSuchFile)?;
        if target_folder.mode & 0o200 == 0 {
            return Err(StatusCode::PermissionDenied);
        }
        if let Some(existing) = self.nodes.get(&to) {
            let moving_dir = self.nodes[&from].is_dir();
            if !replace
                || existing.is_dir() != moving_dir
                || (moving_dir && !self.children(&to).is_empty())
            {
                return Err(StatusCode::Failure);
            }
        }
        let prefix = format!("{from}/");
        let moved: Vec<String> = self
            .nodes
            .range(from.clone()..)
            .take_while(|(path, _)| **path == from || path.starts_with(&prefix))
            .map(|(path, _)| path.clone())
            .collect();
        for path in moved {
            let node = self.nodes.remove(&path).expect("collected above");
            self.nodes
                .insert(format!("{to}{}", &path[from.len()..]), node);
        }
        Ok(())
    }
}

/// What a test can change while the server runs.
struct Settings {
    flavour: Flavour,
    batch: usize,
    host_key: Arc<PrivateKey>,
    authorized: Vec<PublicKey>,
    passwords: bool,
}

struct Shared {
    tree: Mutex<Tree>,
    settings: Mutex<Settings>,
    refuse_logins: AtomicBool,
    connections: AtomicUsize,
    requests: AtomicUsize,
    /// Requests left before the connection that carries the next one is cut.
    drop_after: Mutex<Option<usize>>,
    sockets: Mutex<Vec<StdStream>>,
}

/// The server. It stops when it is dropped.
pub struct FakeSftp {
    pub port: u16,
    /// The client keys.
    pub dir: tempfile::TempDir,
    pub client_key: PathBuf,
    pub encrypted_key: PathBuf,
    shared: Arc<Shared>,
    runtime: Option<Runtime>,
}

fn new_key() -> PrivateKey {
    PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).expect("a key is generated")
}

fn write_key(key: &PrivateKey, path: &std::path::Path) {
    let text = key.to_openssh(LineEnding::LF).expect("the key encodes");
    std::fs::write(path, text.as_bytes()).unwrap();
}

impl FakeSftp {
    pub fn start() -> FakeSftp {
        Self::start_with(Flavour::OpenSsh)
    }

    pub fn start_with(flavour: Flavour) -> FakeSftp {
        let dir = temp_dir();
        let client = new_key();
        let encrypted = new_key();
        let client_key = dir.path().join("client_ed25519");
        let encrypted_key = dir.path().join("client_encrypted");
        write_key(&client, &client_key);
        write_key(
            &encrypted
                .encrypt(&mut rand::rng(), PASSPHRASE)
                .expect("the key is encrypted"),
            &encrypted_key,
        );
        let shared = Arc::new(Shared {
            tree: Mutex::new(Tree::new()),
            settings: Mutex::new(Settings {
                flavour,
                batch: 100,
                host_key: Arc::new(new_key()),
                authorized: vec![client.public_key().clone(), encrypted.public_key().clone()],
                passwords: false,
            }),
            refuse_logins: AtomicBool::new(false),
            connections: AtomicUsize::new(0),
            requests: AtomicUsize::new(0),
            drop_after: Mutex::new(None),
            sockets: Mutex::new(Vec::new()),
        });
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let listener = runtime
            .block_on(TcpListener::bind("127.0.0.1:0"))
            .expect("a loopback port");
        let port = listener.local_addr().unwrap().port();
        runtime.spawn(accept(listener, shared.clone()));
        FakeSftp {
            port,
            dir,
            client_key,
            encrypted_key,
            shared,
            runtime: Some(runtime),
        }
    }

    /// The tree, for filling and checking.
    pub fn tree(&self) -> MutexGuard<'_, Tree> {
        self.shared.tree.lock().unwrap()
    }

    /// The server's host key, as `ServerKey` knows it.
    pub fn host_key(&self) -> ServerKey {
        let key = self.shared.settings.lock().unwrap().host_key.clone();
        ServerKey::from_openssh(&key.public_key().to_openssh().unwrap()).unwrap()
    }

    /// Gives the server another host key: connections made from now on present it.
    pub fn rotate_host_key(&self) {
        self.shared.settings.lock().unwrap().host_key = Arc::new(new_key());
    }

    /// A private key file the server does not accept.
    pub fn stranger_key(&self) -> PathBuf {
        let path = self.dir.path().join("stranger");
        write_key(&new_key(), &path);
        path
    }

    /// Whether the server offers password logins too (only keys by default, as the OpenSSH the
    /// tests start). Affects connections made from now on.
    pub fn allow_passwords(&self, allow: bool) {
        self.shared.settings.lock().unwrap().passwords = allow;
    }

    /// Refuses every login, to every method, until it is set to `false` again.
    pub fn set_refuse_logins(&self, refuse: bool) {
        self.shared.refuse_logins.store(refuse, Ordering::SeqCst);
    }

    /// How many entries one `readdir` answers with (OpenSSH fills a packet: about a hundred).
    pub fn set_readdir_batch(&self, batch: usize) {
        self.shared.settings.lock().unwrap().batch = batch.max(1);
    }

    /// Cuts every open connection; new ones are accepted.
    pub fn drop_connections(&self) {
        for socket in self.shared.sockets.lock().unwrap().drain(..) {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }

    /// Cuts the connection that carries the request after the next `requests` ones, before
    /// answering it: a drop in the middle of a transfer.
    pub fn drop_after_requests(&self, requests: usize) {
        *self.shared.drop_after.lock().unwrap() = Some(requests);
    }

    /// Connections accepted so far.
    pub fn connections(&self) -> usize {
        self.shared.connections.load(Ordering::SeqCst)
    }

    /// SFTP requests answered so far (or cut).
    pub fn requests(&self) -> usize {
        self.shared.requests.load(Ordering::SeqCst)
    }

    /// A proxy in front of the server that delays each direction by half of `round_trip`.
    pub fn proxy(&self, round_trip: std::time::Duration) -> Proxy {
        Proxy::start(self.port, round_trip)
    }
}

impl Drop for FakeSftp {
    fn drop(&mut self) {
        self.drop_connections();
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

async fn accept(listener: TcpListener, shared: Arc<Shared>) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        shared.connections.fetch_add(1, Ordering::SeqCst);
        let Ok(std_stream) = stream.into_std() else {
            continue;
        };
        let Ok(handle) = std_stream.try_clone() else {
            continue;
        };
        let Ok(cutter) = std_stream.try_clone() else {
            continue;
        };
        let Ok(stream) = tokio::net::TcpStream::from_std(std_stream) else {
            continue;
        };
        let _ = stream.set_nodelay(true);
        {
            let mut sockets = shared.sockets.lock().unwrap();
            // Connections that ended are forgotten, so a long run does not pile them up.
            sockets.retain(|socket| socket.peer_addr().is_ok());
            sockets.push(handle);
        }
        let config = {
            let settings = shared.settings.lock().unwrap();
            Config {
                keys: vec![(*settings.host_key).clone()],
                methods: if settings.passwords {
                    MethodSet::from(&[MethodKind::PublicKey, MethodKind::Password][..])
                } else {
                    MethodSet::from(&[MethodKind::PublicKey][..])
                },
                // The default answers every refusal a second late.
                auth_rejection_time: std::time::Duration::ZERO,
                auth_rejection_time_initial: Some(std::time::Duration::ZERO),
                ..Config::default()
            }
        };
        let handler = Connection {
            shared: shared.clone(),
            channel: None,
            cutter,
        };
        tokio::spawn(async move {
            if let Ok(session) = russh::server::run_stream(Arc::new(config), stream, handler).await
            {
                let _ = session.await;
            }
        });
    }
}

struct Connection {
    shared: Arc<Shared>,
    channel: Option<Channel<Msg>>,
    cutter: StdStream,
}

impl Handler for Connection {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        let accepted = !self.shared.refuse_logins.load(Ordering::SeqCst)
            && user == USER
            && password == PASSWORD;
        Ok(if accepted {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn auth_publickey(&mut self, user: &str, key: &PublicKey) -> Result<Auth, Self::Error> {
        let known = self
            .shared
            .settings
            .lock()
            .unwrap()
            .authorized
            .iter()
            .any(|authorized| authorized.key_data() == key.key_data());
        let accepted = !self.shared.refuse_logins.load(Ordering::SeqCst) && user == USER && known;
        Ok(if accepted {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channel = Some(channel);
        reply.accept().await;
        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _originator: &str,
        _originator_port: u32,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        // A jump host: the connection to the next server is carried over the channel.
        let Ok(mut target) = tokio::net::TcpStream::connect((host, port as u16)).await else {
            return Ok(());
        };
        reply.accept().await;
        tokio::spawn(async move {
            let mut stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut target).await;
        });
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        match (name, self.channel.take()) {
            ("sftp", Some(opened)) if opened.id() == channel => {
                session.channel_success(channel)?;
                let handler = Files {
                    shared: self.shared.clone(),
                    handles: HashMap::new(),
                    next: 0,
                    cutter: self.cutter.try_clone().expect("the socket clones"),
                };
                russh_sftp::server::run(opened.into_stream(), handler).await;
            }
            _ => session.channel_failure(channel)?,
        }
        Ok(())
    }
}

enum Open {
    File { path: String, write: bool },
    Dir { entries: Vec<File>, next: usize },
}

struct Files {
    shared: Arc<Shared>,
    handles: HashMap<String, Open>,
    next: u64,
    cutter: StdStream,
}

fn ok(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: "Ok".to_owned(),
        language_tag: "en-US".to_owned(),
    }
}

fn utf8(bytes: &mut &[u8]) -> Option<String> {
    let (length, rest) = bytes.split_first_chunk::<4>()?;
    let length = u32::from_be_bytes(*length) as usize;
    let text = rest.get(..length)?;
    *bytes = &rest[length..];
    String::from_utf8(text.to_vec()).ok()
}

impl Files {
    fn tree(&self) -> MutexGuard<'_, Tree> {
        self.shared.tree.lock().unwrap()
    }

    fn openssh(&self) -> bool {
        self.shared.settings.lock().unwrap().flavour == Flavour::OpenSsh
    }

    /// Counts a request, and cuts the connection when a test scripted it.
    fn tick(&self) -> Reply<()> {
        self.shared.requests.fetch_add(1, Ordering::SeqCst);
        let mut script = self.shared.drop_after.lock().unwrap();
        if let Some(left) = script.as_mut() {
            if *left == 0 {
                *script = None;
                let _ = self.cutter.shutdown(Shutdown::Both);
                return Err(StatusCode::ConnectionLost);
            }
            *left -= 1;
        }
        Ok(())
    }

    fn handle(&mut self, open: Open) -> String {
        self.next += 1;
        let handle = format!("h{}", self.next);
        self.handles.insert(handle.clone(), open);
        handle
    }

    fn apply(node: &mut Node, attrs: &FileAttributes) {
        if let (Some(size), Content::File(bytes)) = (attrs.size, &mut node.content) {
            bytes.resize(size as usize, 0);
        }
        if let Some(mode) = attrs.permissions {
            node.mode = mode & 0o7777;
        }
        if let Some(atime) = attrs.atime {
            node.atime = atime;
        }
        if let Some(mtime) = attrs.mtime {
            node.mtime = mtime;
        }
    }

    fn set_attributes(&self, path: &str, attrs: &FileAttributes, follow: bool) -> Reply<Status> {
        let mut tree = self.tree();
        let (real, _) = tree.existing(path, follow)?;
        Self::apply(tree.nodes.get_mut(&real).expect("it exists"), attrs);
        Ok(ok(0))
    }
}

impl russh_sftp::server::Handler for Files {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn init(
        &mut self,
        _version: u32,
        _extensions: HashMap<String, String>,
    ) -> Result<Version, Self::Error> {
        let mut version = Version::new();
        if self.openssh() {
            for (name, value) in [
                ("posix-rename@openssh.com", "1"),
                ("statvfs@openssh.com", "2"),
                ("fsync@openssh.com", "1"),
                ("lsetstat@openssh.com", "1"),
            ] {
                version.extensions.insert(name.to_owned(), value.to_owned());
            }
        }
        Ok(version)
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        self.tick()?;
        let write = pflags.intersects(OpenFlags::WRITE | OpenFlags::APPEND);
        let path = {
            let mut tree = self.tree();
            let real = tree.resolve(&filename, true)?;
            match tree.nodes.get(&real) {
                // As `open(2)` does: a folder opens for reading and fails on the first read.
                Some(node) if node.is_dir() && write => return Err(StatusCode::Failure),
                Some(node) if node.is_dir() => {}
                Some(_) if pflags.contains(OpenFlags::CREATE | OpenFlags::EXCLUDE) => {
                    return Err(StatusCode::Failure)
                }
                Some(node) => {
                    if write && node.mode & 0o200 == 0 {
                        return Err(StatusCode::PermissionDenied);
                    }
                    if pflags.contains(OpenFlags::TRUNCATE) && write {
                        let node = tree.nodes.get_mut(&real).expect("it exists");
                        node.content = Content::File(Vec::new());
                        node.mtime = now();
                    }
                }
                None if pflags.contains(OpenFlags::CREATE) => {
                    tree.creatable(&real)?;
                    let mode = attrs.permissions.map_or(0o666, |mode| mode & 0o7777) & !0o022;
                    tree.nodes
                        .insert(real.clone(), Node::new(Content::File(Vec::new()), mode));
                }
                None => return Err(StatusCode::NoSuchFile),
            }
            real
        };
        let handle = self.handle(Open::File { path, write });
        Ok(Handle { id, handle })
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        self.tick()?;
        match self.handles.remove(&handle) {
            Some(Open::File { path, write: true }) => {
                if let Some(node) = self.tree().nodes.get_mut(&path) {
                    node.mtime = now();
                }
                Ok(ok(id))
            }
            Some(_) => Ok(ok(id)),
            None => Err(StatusCode::Failure),
        }
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        self.tick()?;
        let Some(Open::File { path, .. }) = self.handles.get(&handle) else {
            return Err(StatusCode::Failure);
        };
        let tree = self.tree();
        let Some(Node {
            content: Content::File(bytes),
            ..
        }) = tree.get(path)
        else {
            return Err(StatusCode::Failure);
        };
        let start = offset as usize;
        if start >= bytes.len() {
            return Err(StatusCode::Eof);
        }
        let end = bytes.len().min(start + len as usize);
        Ok(Data {
            id,
            data: bytes[start..end].to_vec(),
        })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        self.tick()?;
        let Some(Open::File { path, write }) = self.handles.get(&handle) else {
            return Err(StatusCode::Failure);
        };
        if !write {
            return Err(StatusCode::PermissionDenied);
        }
        let mut tree = self.tree();
        let Some(Node {
            content: Content::File(bytes),
            ..
        }) = tree.nodes.get_mut(path)
        else {
            return Err(StatusCode::Failure);
        };
        let start = offset as usize;
        if bytes.len() < start + data.len() {
            bytes.resize(start + data.len(), 0);
        }
        bytes[start..start + data.len()].copy_from_slice(&data);
        Ok(ok(id))
    }

    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        self.tick()?;
        let tree = self.tree();
        let (_, node) = tree.existing(&path, false)?;
        Ok(Attrs {
            id,
            attrs: node.attrs(),
        })
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        self.tick()?;
        let tree = self.tree();
        let (_, node) = tree.existing(&path, true)?;
        Ok(Attrs {
            id,
            attrs: node.attrs(),
        })
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
        self.tick()?;
        let path = match self.handles.get(&handle) {
            Some(Open::File { path, .. }) => path.clone(),
            _ => return Err(StatusCode::Failure),
        };
        let tree = self.tree();
        let node = tree.get(&path).ok_or(StatusCode::NoSuchFile)?;
        Ok(Attrs {
            id,
            attrs: node.attrs(),
        })
    }

    async fn setstat(
        &mut self,
        id: u32,
        path: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        self.tick()?;
        self.set_attributes(&path, &attrs, true)?;
        Ok(ok(id))
    }

    async fn fsetstat(
        &mut self,
        id: u32,
        handle: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        self.tick()?;
        let Some(Open::File { path, .. }) = self.handles.get(&handle) else {
            return Err(StatusCode::Failure);
        };
        let path = path.clone();
        self.set_attributes(&path, &attrs, true)?;
        Ok(ok(id))
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        self.tick()?;
        let entries = {
            let tree = self.tree();
            let (real, node) = tree.existing(&path, true)?;
            if !node.is_dir() {
                return Err(StatusCode::Failure);
            }
            let mut entries = vec![
                File::new(".", node.attrs()),
                File::new("..", tree.get(parent_of(&real)).unwrap_or(node).attrs()),
            ];
            for name in tree.children(&real) {
                let attrs = tree.get(&join(&real, &name)).expect("listed").attrs();
                entries.push(File::new(name, attrs));
            }
            entries
        };
        let handle = self.handle(Open::Dir { entries, next: 0 });
        Ok(Handle { id, handle })
    }

    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, Self::Error> {
        self.tick()?;
        let batch = self.shared.settings.lock().unwrap().batch;
        let Some(Open::Dir { entries, next }) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::Failure);
        };
        if *next >= entries.len() {
            return Err(StatusCode::Eof);
        }
        let end = entries.len().min(*next + batch);
        let files = entries[*next..end].to_vec();
        *next = end;
        Ok(Name { id, files })
    }

    async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
        self.tick()?;
        let mut tree = self.tree();
        let real = tree.resolve(&filename, false)?;
        match tree.nodes.get(&real) {
            None => return Err(StatusCode::NoSuchFile),
            Some(node) if node.is_dir() => return Err(StatusCode::Failure),
            Some(_) => {}
        }
        tree.check_writable_folder(&real)?;
        tree.nodes.remove(&real);
        Ok(ok(id))
    }

    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        self.tick()?;
        let mut tree = self.tree();
        let real = tree.creatable(&path)?;
        if tree.nodes.contains_key(&real) {
            return Err(StatusCode::Failure);
        }
        let mode = attrs.permissions.map_or(0o777, |mode| mode & 0o7777) & !0o022;
        tree.nodes.insert(real, Node::new(Content::Dir, mode));
        Ok(ok(id))
    }

    async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
        self.tick()?;
        let mut tree = self.tree();
        let real = tree.resolve(&path, false)?;
        match tree.nodes.get(&real) {
            None => return Err(StatusCode::NoSuchFile),
            Some(node) if !node.is_dir() => return Err(StatusCode::Failure),
            Some(_) if !tree.children(&real).is_empty() => return Err(StatusCode::Failure),
            Some(_) => {}
        }
        tree.check_writable_folder(&real)?;
        tree.nodes.remove(&real);
        Ok(ok(id))
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        self.tick()?;
        let tree = self.tree();
        let (real, _) = tree.existing(&path, true)?;
        Ok(Name {
            id,
            files: vec![File::dummy(real)],
        })
    }

    async fn rename(
        &mut self,
        id: u32,
        oldpath: String,
        newpath: String,
    ) -> Result<Status, Self::Error> {
        self.tick()?;
        self.tree().rename(&oldpath, &newpath, false)?;
        Ok(ok(id))
    }

    async fn readlink(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        self.tick()?;
        let tree = self.tree();
        match tree.existing(&path, false)?.1 {
            Node {
                content: Content::Link(target),
                ..
            } => Ok(Name {
                id,
                files: vec![File::dummy(target.clone())],
            }),
            _ => Err(StatusCode::Failure),
        }
    }

    async fn symlink(
        &mut self,
        id: u32,
        linkpath: String,
        targetpath: String,
    ) -> Result<Status, Self::Error> {
        self.tick()?;
        // OpenSSH's server reads the two strings in the opposite order from the draft.
        let (link, target) = if self.openssh() {
            (targetpath, linkpath)
        } else {
            (linkpath, targetpath)
        };
        let mut tree = self.tree();
        let real = tree.creatable(&link)?;
        if tree.nodes.contains_key(&real) {
            return Err(StatusCode::Failure);
        }
        tree.nodes
            .insert(real, Node::new(Content::Link(target), 0o777));
        Ok(ok(id))
    }

    async fn extended(
        &mut self,
        id: u32,
        request: String,
        data: Vec<u8>,
    ) -> Result<Packet, Self::Error> {
        self.tick()?;
        if !self.openssh() {
            return Err(StatusCode::OpUnsupported);
        }
        let mut data = data.as_slice();
        match request.as_str() {
            "posix-rename@openssh.com" => {
                let from = utf8(&mut data).ok_or(StatusCode::BadMessage)?;
                let to = utf8(&mut data).ok_or(StatusCode::BadMessage)?;
                self.tree().rename(&from, &to, true)?;
                Ok(Packet::Status(ok(id)))
            }
            "fsync@openssh.com" => {
                let handle = utf8(&mut data).ok_or(StatusCode::BadMessage)?;
                self.handles
                    .contains_key(&handle)
                    .then(|| Packet::Status(ok(id)))
                    .ok_or(StatusCode::Failure)
            }
            "lsetstat@openssh.com" => {
                let path = utf8(&mut data).ok_or(StatusCode::BadMessage)?;
                let mut rest = Bytes::copy_from_slice(data);
                let attrs: FileAttributes =
                    russh_sftp::de::from_bytes(&mut rest).map_err(|_| StatusCode::BadMessage)?;
                self.set_attributes(&path, &attrs, false)?;
                Ok(Packet::Status(ok(id)))
            }
            "statvfs@openssh.com" => {
                let path = utf8(&mut data).ok_or(StatusCode::BadMessage)?;
                self.tree().existing(&path, true)?;
                let space = Statvfs {
                    block_size: 4096,
                    fragment_size: 4096,
                    blocks: 1_000_000,
                    blocks_free: 600_000,
                    blocks_avail: 550_000,
                    inodes: 100_000,
                    inodes_free: 90_000,
                    inodes_avail: 90_000,
                    fs_id: 1,
                    flags: 0,
                    name_max: 255,
                };
                let data = russh_sftp::ser::to_bytes(&space)
                    .map_err(|_| StatusCode::Failure)?
                    .to_vec();
                Ok(Packet::ExtendedReply(ExtendedReply { id, data }))
            }
            _ => Err(StatusCode::OpUnsupported),
        }
    }
}
