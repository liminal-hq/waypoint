// What the SFTP provider's tests run against: an in-process server (`fake`) on every platform, a
// throwaway OpenSSH server run as the current user, and a TCP proxy that delays, stalls or severs
// connections to either. `Backend` is what a test needs from a server, so one body serves both.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `FakeSftp::start` needs nothing installed; see `fake.rs`. `on_both!` declares each test body
//! twice, as `name::in_process` and `name::openssh`.
//!
//! `Sshd::start` generates a host key and client keys in a temporary folder, writes a
//! configuration that needs no root (`UsePAM no`, `StrictModes no`) and runs `sshd -D` on a free
//! loopback port, as spike #278 did. It returns `None`, after printing why, when there is no
//! `sshd`, `sftp-server` or `ssh-keygen` (always on Windows), or when `WAYPOINT_SFTP_TESTS=off`,
//! so the tests skip instead of failing; with `WAYPOINT_SFTP_REQUIRE=1` (CI's `remote-conformance`
//! job) they fail instead. Set `WAYPOINT_SSHD` to choose the `sshd` binary and `WAYPOINT_TEST_TMP`
//! to choose where the temporary folders go.

#![allow(dead_code)]

use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use waypoint_provider_sftp::ServerKey;

pub mod backend;
pub mod fake;

pub use backend::Backend;
pub use fake::FakeSftp;

/// Declares, for each function that takes a `&dyn Backend`, a test against the in-process server
/// (every platform) and one against OpenSSH (skipped without `sshd`).
#[allow(unused_macros)]
macro_rules! on_both {
    ($($name:ident),+ $(,)?) => {$(
        mod $name {
            #[test]
            fn in_process() {
                let server = crate::support::FakeSftp::start();
                super::$name(&server);
            }

            #[test]
            fn openssh() {
                let Some(server) = crate::support::Sshd::start() else {
                    return;
                };
                super::$name(&server);
            }
        }
    )+};
}

/// The passphrase of the encrypted client key.
pub const PASSPHRASE: &str = "correct horse battery staple";

fn skip(why: &str) -> Option<Sshd> {
    // CI's real-server job sets this, so a missing server fails there instead of passing quietly.
    if std::env::var_os("WAYPOINT_SFTP_REQUIRE").is_some_and(|v| v == "1") {
        panic!("WAYPOINT_SFTP_REQUIRE is set and the real-server tests cannot run: {why}");
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

/// A temporary folder under `WAYPOINT_TEST_TMP`, or the system's.
pub fn temp_dir() -> tempfile::TempDir {
    let builder = tempfile::Builder::new().prefix("waypoint-sftp-").to_owned();
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

fn keygen(path: &Path, passphrase: &str) {
    let status = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-C", "", "-N", passphrase, "-f"])
        .arg(path)
        .status()
        .expect("ssh-keygen runs");
    assert!(status.success(), "ssh-keygen made {}", path.display());
}

pub struct Sshd {
    child: Child,
    pub port: u16,
    /// Keys, configuration and the server's log.
    pub dir: tempfile::TempDir,
    /// A folder the tests may fill, served at the same path.
    pub data: PathBuf,
    pub client_key: PathBuf,
    pub encrypted_key: PathBuf,
    pub host_key: ServerKey,
}

impl Sshd {
    pub fn start() -> Option<Sshd> {
        Self::start_with("")
    }

    /// Starts a server with `extra` appended to its configuration.
    pub fn start_with(extra: &str) -> Option<Sshd> {
        if !cfg!(unix) {
            return skip("the real-server tests run on Linux only");
        }
        if std::env::var("WAYPOINT_SFTP_TESTS").as_deref() == Ok("off") {
            return skip("WAYPOINT_SFTP_TESTS=off");
        }
        let sshd = match std::env::var_os("WAYPOINT_SSHD") {
            Some(path) => Some(PathBuf::from(path)),
            None => first_existing(&["/usr/sbin/sshd", "/usr/bin/sshd"]),
        };
        let Some(sshd) = sshd else {
            return skip("no sshd (set WAYPOINT_SSHD)");
        };
        let Some(sftp_server) = first_existing(&[
            "/usr/lib/ssh/sftp-server",
            "/usr/lib/openssh/sftp-server",
            "/usr/libexec/openssh/sftp-server",
            "/usr/libexec/sftp-server",
        ]) else {
            return skip("no sftp-server");
        };
        if Command::new("ssh-keygen").arg("-?").output().is_err() {
            return skip("no ssh-keygen");
        }
        let dir = temp_dir();
        let host = dir.path().join("host_ed25519");
        let client_key = dir.path().join("client_ed25519");
        let encrypted_key = dir.path().join("client_encrypted");
        keygen(&host, "");
        keygen(&client_key, "");
        keygen(&encrypted_key, PASSPHRASE);
        let mut authorized = fs::read_to_string(client_key.with_extension("pub")).unwrap();
        authorized.push_str(&fs::read_to_string(encrypted_key.with_extension("pub")).unwrap());
        fs::write(dir.path().join("authorized_keys"), authorized).unwrap();
        let data = dir.path().join("data");
        fs::create_dir(&data).unwrap();
        let port = free_port();
        let config = dir.path().join("sshd_config");
        let mut text = format!(
            "ListenAddress 127.0.0.1\nHostKey {host}\nAuthorizedKeysFile {dir}/authorized_keys\n\
             PidFile none\nPasswordAuthentication no\nKbdInteractiveAuthentication no\n\
             PubkeyAuthentication yes\nUsePAM no\nStrictModes no\nAllowTcpForwarding yes\n\
             Subsystem sftp {sftp}\nLogLevel ERROR\n{extra}\n",
            host = host.display(),
            dir = dir.path().display(),
            sftp = sftp_server.display(),
        );
        // OpenSSH 9.8 and later slow down a source that fails to log in, which the tests do on
        // purpose; older servers do not know the option.
        let with_penalties = format!("{text}PerSourcePenalties no\n");
        fs::write(&config, &with_penalties).unwrap();
        let checked = Command::new(&sshd)
            .args(["-t", "-f"])
            .arg(&config)
            .stderr(Stdio::null())
            .status();
        if checked.map(|s| s.success()).unwrap_or(false) {
            text = with_penalties;
        }
        fs::write(&config, &text).unwrap();
        let log = fs::File::create(dir.path().join("sshd.log")).unwrap();
        let child = Command::new(&sshd)
            .args(["-D", "-e", "-f"])
            .arg(&config)
            .args(["-p", &port.to_string()])
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .expect("sshd starts");
        let host_key =
            ServerKey::from_openssh(&fs::read_to_string(host.with_extension("pub")).unwrap())
                .unwrap();
        let server = Sshd {
            child,
            port,
            dir,
            data,
            client_key,
            encrypted_key,
            host_key,
        };
        if !wait_for_banner(port) {
            let log = fs::read_to_string(server.dir.path().join("sshd.log")).unwrap_or_default();
            panic!("sshd did not answer on port {port}: {log}");
        }
        Some(server)
    }
}

impl Drop for Sshd {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn wait_for_banner(port: u16) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
            let mut banner = [0u8; 4];
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            if stream.read_exact(&mut banner).is_ok() && &banner == b"SSH-" {
                return true;
            }
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
