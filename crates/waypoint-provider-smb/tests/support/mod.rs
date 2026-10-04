// A throwaway Samba server run as the current user, for the SMB provider's tests against a real
// server.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Smbd::start` writes a configuration that needs no root (a private state directory, a `tdbsam`
//! password database holding the current user, shares in a temporary folder) and runs
//! `smbd -F` on a free loopback port, as spike #278 did. It returns `None`, after printing why,
//! when there is no `smbd` or `pdbedit` (always on Windows), or when `WAYPOINT_SMB_TESTS=off`, so
//! the tests skip instead of failing; with `WAYPOINT_SMB_REQUIRE=1` (CI's `remote-conformance`
//! job) they fail instead. Set `WAYPOINT_SMBD` and `WAYPOINT_PDBEDIT` to choose the binaries and
//! `WAYPOINT_TEST_TMP` to choose where the temporary folders go, and `WAYPOINT_SMB_KEEP=1` to keep
//! a server's folder (with its logs) after the test.
//!
//! An unprivileged `smbd` serves its files as the user who started it, so the one account is that
//! user. Three shares exist: `data` (writable), `readonly`, and `locked` (nobody may enter).

#![allow(dead_code)]

use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use waypoint_path::VfsPath;
use waypoint_protocol::AuthPrompt;
use waypoint_provider_smb::{SmbConfig, SmbOptions, SmbProvider};
use waypoint_vfs::{ConnectAnswer, Credential, CredentialSource, Secret};

/// The account's password.
pub const PASSWORD: &str = "correct horse battery staple";

fn skip(why: &str) -> Option<Smbd> {
    // CI's real-server job sets this, so a missing server fails there instead of passing quietly.
    if std::env::var_os("WAYPOINT_SMB_REQUIRE").is_some_and(|v| v == "1") {
        panic!("WAYPOINT_SMB_REQUIRE is set and the real-server tests cannot run: {why}");
    }
    eprintln!("skipping: {why}");
    None
}

fn first_existing(candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
}

/// A temporary folder under `WAYPOINT_TEST_TMP`, or the system's. Samba's sockets need the path to
/// stay short.
pub fn temp_dir() -> tempfile::TempDir {
    let builder = tempfile::Builder::new().prefix("wp-smb-").to_owned();
    match std::env::var_os("WAYPOINT_TEST_TMP") {
        Some(base) => {
            fs::create_dir_all(&base).unwrap();
            builder.tempdir_in(base).unwrap()
        }
        None => builder.tempdir().unwrap(),
    }
}

/// A port nothing listens on yet.
pub fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

pub fn user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .expect("the tests know the local user")
}

/// A credential source with one password for every connection.
pub struct Password(pub String);

impl CredentialSource for Password {
    fn credential(
        &self,
        _: &waypoint_path::ConnectionKey,
        prompt: &AuthPrompt,
    ) -> Option<Credential> {
        match prompt {
            AuthPrompt::Password { .. } => Some(Credential::Password {
                user: None,
                password: Secret::from(self.0.as_str()),
            }),
            _ => None,
        }
    }
}

pub struct Smbd {
    child: Child,
    pub port: u16,
    /// Configuration, state and the server's log.
    pub dir: tempfile::TempDir,
    /// The folder the `data` share serves.
    pub data: PathBuf,
    /// The folder the `readonly` share serves.
    pub readonly: PathBuf,
    pub user: String,
}

impl Smbd {
    pub fn start() -> Option<Smbd> {
        Self::start_with("")
    }

    /// Starts a server with `extra` appended to its `[global]` section.
    pub fn start_with(extra: &str) -> Option<Smbd> {
        Self::start_with_data(extra, "")
    }

    /// Starts a server with `extra` appended to its `[global]` section and `data_extra` to the
    /// `data` share's.
    pub fn start_with_data(extra: &str, data_extra: &str) -> Option<Smbd> {
        if !cfg!(unix) {
            return skip("the real-server tests run on Linux only");
        }
        if std::env::var("WAYPOINT_SMB_TESTS").as_deref() == Ok("off") {
            return skip("WAYPOINT_SMB_TESTS=off");
        }
        let smbd = match std::env::var_os("WAYPOINT_SMBD") {
            Some(path) => Some(PathBuf::from(path)),
            None => first_existing(&["/usr/sbin/smbd", "/usr/bin/smbd"]),
        };
        let Some(smbd) = smbd else {
            return skip("no smbd (set WAYPOINT_SMBD)");
        };
        let pdbedit = match std::env::var_os("WAYPOINT_PDBEDIT") {
            Some(path) => Some(PathBuf::from(path)),
            None => first_existing(&["/usr/bin/pdbedit", "/usr/sbin/pdbedit"]),
        };
        let Some(pdbedit) = pdbedit else {
            return skip("no pdbedit (set WAYPOINT_PDBEDIT)");
        };
        let dir = temp_dir();
        let root = dir.path();
        for name in [
            "priv", "lock", "state", "cache", "run", "data", "readonly", "locked",
        ] {
            fs::create_dir(root.join(name)).unwrap();
        }
        let (data, readonly) = (root.join("data"), root.join("readonly"));
        let user = user();
        let port = free_port();
        let conf = root.join("smb.conf");
        let text = format!(
            "[global]\n\
             \x20 ncalrpc dir = {r}/run\n  winbindd socket directory = {r}/run\n  nmbd:socket dir = {r}/run\n\
             \x20 workgroup = WORKGROUP\n  server min protocol = SMB2\n  smb ports = {port}\n\
             \x20 interfaces = lo\n  bind interfaces only = yes\n  private dir = {r}/priv\n\
             \x20 lock directory = {r}/lock\n  state directory = {r}/state\n  cache directory = {r}/cache\n\
             \x20 pid directory = {r}/run\n  passdb backend = tdbsam:{r}/priv/passdb.tdb\n\
             \x20 security = user\n  map to guest = never\n  log file = {r}/log.%m\n\
             \x20 disable netbios = yes\n  load printers = no\n  printing = bsd\n\
             \x20 printcap name = /dev/null\n  disable spoolss = yes\n  ntlm auth = ntlmv2-only\n\
             \x20 {extra}\n\
             [data]\n  path = {r}/data\n  read only = no\n  valid users = {user}\n  {data_extra}\n\
             [readonly]\n  path = {r}/readonly\n  read only = yes\n  valid users = {user}\n\
             [locked]\n  path = {r}/locked\n  read only = no\n  valid users = nobody-here\n",
            r = root.display(),
        );
        fs::write(&conf, text).unwrap();
        let added = Command::new(&pdbedit)
            .args(["-a", "-u", &user, "-t", "-s"])
            .arg(&conf)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|mut child| {
                child
                    .stdin
                    .take()
                    .unwrap()
                    .write_all(format!("{PASSWORD}\n{PASSWORD}\n").as_bytes())?;
                child.wait()
            });
        if !added.map(|status| status.success()).unwrap_or(false) {
            return skip("pdbedit could not add the account");
        }
        let log = fs::File::create(root.join("smbd.out")).unwrap();
        let child = Command::new(&smbd)
            .args(["-F", "--no-process-group", "-s"])
            .arg(&conf)
            .arg("-l")
            .arg(root)
            .arg("--debug-stdout")
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .expect("smbd starts");
        let server = Smbd {
            child,
            port,
            dir,
            data,
            readonly,
            user,
        };
        if !wait_for_port(port) {
            let log = fs::read_to_string(server.dir.path().join("smbd.out")).unwrap_or_default();
            panic!("smbd did not answer on port {port}: {log}");
        }
        Some(server)
    }

    /// A provider that logs in with the account's password.
    pub fn provider(&self) -> SmbProvider {
        self.provider_with(SmbOptions::default())
    }

    pub fn provider_with(&self, options: SmbOptions) -> SmbProvider {
        SmbProvider::new(
            SmbConfig::new()
                .with_credentials(Arc::new(Password(PASSWORD.to_owned())))
                .with_options(options),
        )
    }

    /// The location of `path` through another port (a proxy's).
    pub fn location_at(&self, port: u16, path: &str) -> VfsPath {
        VfsPath::from_uri(&format!("smb://{}@127.0.0.1:{port}/{path}", self.user)).unwrap()
    }

    /// A provider with no credential source: every login asks.
    pub fn provider_asking(&self) -> SmbProvider {
        SmbProvider::new(SmbConfig::new())
    }

    /// The location of `path` (a share and what is in it) as the account.
    pub fn location(&self, path: &str) -> VfsPath {
        VfsPath::from_uri(&format!(
            "smb://{}@127.0.0.1:{}/{path}",
            self.user, self.port
        ))
        .unwrap()
    }

    /// The server's root, the share browser.
    pub fn shares_location(&self) -> VfsPath {
        self.location("")
    }

    /// The `data` share's location.
    pub fn data_location(&self) -> VfsPath {
        self.location("data")
    }

    /// The answer a person gives when asked for the password.
    pub fn answer(&self, password: &str) -> ConnectAnswer {
        ConnectAnswer::Credential(Credential::Password {
            user: None,
            password: Secret::from(password),
        })
    }
}

impl Drop for Smbd {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // A debugging aid: with `WAYPOINT_SMB_KEEP=1` the configuration and the server's logs stay.
        if std::env::var_os("WAYPOINT_SMB_KEEP").is_some_and(|v| v == "1") {
            let kept = std::mem::replace(&mut self.dir, temp_dir());
            eprintln!("kept {}", kept.path().display());
            std::mem::forget(kept);
        }
    }
}

fn wait_for_port(port: u16) -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
            // The server speaks only once it is spoken to; connecting is the whole check.
            let _ = stream.set_read_timeout(Some(Duration::from_millis(50)));
            let mut byte = [0u8; 1];
            let _ = stream.read(&mut byte);
            return true;
        }
        thread::sleep(Duration::from_millis(25));
    }
    false
}

/// A TCP proxy to a server: each direction delayed by half of `round_trip` (order kept, no
/// bandwidth limit), with switches to stall it and to cut every connection.
pub struct Proxy {
    pub port: u16,
    stalled: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    sockets: Arc<Mutex<Vec<TcpStream>>>,
    connections: Arc<AtomicUsize>,
}

impl Proxy {
    pub fn start(target: u16, round_trip: Duration) -> Proxy {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let stalled = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let sockets = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::new(AtomicUsize::new(0));
        let one_way = round_trip / 2;
        {
            let (stalled, stopped, sockets, connections) = (
                stalled.clone(),
                stopped.clone(),
                sockets.clone(),
                connections.clone(),
            );
            thread::spawn(move || {
                for client in listener.incoming() {
                    if stopped.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(client) = client else { continue };
                    connections.fetch_add(1, Ordering::SeqCst);
                    let Ok(server) = TcpStream::connect(("127.0.0.1", target)) else {
                        continue;
                    };
                    let _ = client.set_nodelay(true);
                    let _ = server.set_nodelay(true);
                    {
                        let mut sockets = sockets.lock().unwrap();
                        sockets.push(client.try_clone().unwrap());
                        sockets.push(server.try_clone().unwrap());
                    }
                    pipe(
                        client.try_clone().unwrap(),
                        server.try_clone().unwrap(),
                        one_way,
                        stalled.clone(),
                    );
                    pipe(server, client, one_way, stalled.clone());
                }
            });
        }
        Proxy {
            port,
            stalled,
            stopped,
            sockets,
            connections,
        }
    }

    /// How many connections have come through.
    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// Holds every byte until `resume`, as a server that stopped answering.
    pub fn stall(&self) {
        self.stalled.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.stalled.store(false, Ordering::SeqCst);
    }

    /// Cuts every open connection, as a dropped network does; new ones still go through.
    pub fn sever(&self) {
        for socket in self.sockets.lock().unwrap().drain(..) {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.sever();
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

/// Copies `from` to `to`, each chunk sent `delay` after it was read.
fn pipe(mut from: TcpStream, mut to: TcpStream, delay: Duration, stalled: Arc<AtomicBool>) {
    let (send, receive) = mpsc::channel::<(Instant, Vec<u8>)>();
    thread::spawn(move || {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match from.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if send
                        .send((Instant::now() + delay, buf[..n].to_vec()))
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
    });
    thread::spawn(move || {
        for (due, bytes) in receive {
            let now = Instant::now();
            if due > now {
                thread::sleep(due - now);
            }
            while stalled.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(10));
            }
            if to.write_all(&bytes).is_err() {
                break;
            }
        }
        let _ = to.shutdown(Shutdown::Write);
    });
}
