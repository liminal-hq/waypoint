// Throwaway real S3 servers run as the current user (`rclone serve s3`, and MinIO when its binary
// is given), and the small in-memory one, for the S3 provider's tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Server::start_all` serves a temporary folder as an S3 service on a free loopback port for each
//! server it can run: `rclone serve s3` when there is an `rclone` (set `WAYPOINT_RCLONE` to choose the
//! binary) and MinIO when `WAYPOINT_MINIO` names its binary. It prints why and returns nothing
//! when there is neither, or when `WAYPOINT_S3_TESTS=off`, so the tests skip instead of failing;
//! with `WAYPOINT_S3_REQUIRE=1` (CI's `remote-conformance` job) they fail instead.
//! `WAYPOINT_TEST_TMP` chooses where the folders go (use a disk, not a small tmpfs).
//!
//! rclone builds each answer in full and re-reads a folder for every page of a listing, so it sets
//! the floor for what the provider can do, as spike #278 measured; it cannot hold folder markers
//! (`dir/` becomes a file) and answers `CreateMultipartUpload` with the wrong XML root, so the
//! suite's folder and multipart checks run against MinIO and the in-memory server.

#![allow(dead_code)]

pub mod canned;
pub mod fake_s3;

use std::fs;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use waypoint_path::{ConnectionKey, VfsPath};
use waypoint_protocol::AuthPrompt;
use waypoint_provider_s3::{S3Config, S3Options, S3Provider};
use waypoint_vfs::{Credential, CredentialSource, Secret};

pub const KEY_ID: &str = "waypoint-test";
pub const SECRET: &str = "waypoint-test-secret";

/// A credential source with one fixed access key.
pub struct FixedKey {
    pub key_id: String,
    pub secret: String,
}

impl CredentialSource for FixedKey {
    fn credential(&self, _: &ConnectionKey, prompt: &AuthPrompt) -> Option<Credential> {
        matches!(prompt, AuthPrompt::AccessKey { .. }).then(|| Credential::AccessKey {
            key_id: self.key_id.clone(),
            secret: Secret::from(self.secret.as_str()),
            session_token: None,
        })
    }
}

/// A temporary folder under `WAYPOINT_TEST_TMP`, or the system's.
pub fn temp_dir() -> tempfile::TempDir {
    let builder = tempfile::Builder::new().prefix("wp-s3-").to_owned();
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `rclone serve s3`: a stand-in that cannot hold folder markers and answers a multipart
    /// upload with the wrong XML root.
    Rclone,
    /// MinIO: complete enough for the whole conformance suite.
    Minio,
}

pub struct Server {
    pub kind: Kind,
    child: Option<Child>,
    pub port: u16,
    /// The folder that is served: for rclone its folders are the buckets.
    pub data: PathBuf,
    _dir: tempfile::TempDir,
}

fn unavailable(why: &str) {
    // CI's real-server job sets this, so a missing server fails there instead of passing quietly.
    if std::env::var_os("WAYPOINT_S3_REQUIRE").is_some_and(|v| v == "1") {
        panic!("WAYPOINT_S3_REQUIRE is set and the real-server tests cannot run: {why}");
    }
    eprintln!("skipping: {why}");
}

fn wait_for(port: u16, what: &str, mut child: Option<&mut Child>) -> bool {
    let start = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        if let Some(Ok(Some(status))) = child.as_mut().map(|child| child.try_wait()) {
            unavailable(&format!(
                "{what} exited with {status} (rclone needs 1.65 or later for `serve s3`)"
            ));
            return false;
        }
        if start.elapsed() > Duration::from_secs(30) {
            unavailable(&format!("{what} did not start serving in time"));
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    true
}

impl Server {
    /// Every real server this machine can run: `rclone serve s3` when there is an `rclone`, and
    /// MinIO when `WAYPOINT_MINIO` names its binary. Empty (after saying why) when there is none.
    pub fn start_all() -> Vec<Server> {
        if std::env::var("WAYPOINT_S3_TESTS").is_ok_and(|v| v == "off") {
            unavailable("WAYPOINT_S3_TESTS=off");
            return Vec::new();
        }
        let servers: Vec<Server> = [Self::start_rclone(), Self::start_minio()]
            .into_iter()
            .flatten()
            .collect();
        if servers.is_empty() {
            unavailable("there is neither `rclone` nor a MinIO binary (WAYPOINT_MINIO)");
        }
        servers
    }

    fn start_rclone() -> Option<Self> {
        let rclone = std::env::var("WAYPOINT_RCLONE").unwrap_or_else(|_| "rclone".to_owned());
        if Command::new(&rclone)
            .arg("version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            eprintln!("no `rclone` (set WAYPOINT_RCLONE)");
            return None;
        }
        let dir = temp_dir();
        let data = dir.path().join("data");
        fs::create_dir(&data).unwrap();
        let port = free_port();
        let child = Command::new(&rclone)
            .args(["serve", "s3"])
            .arg(&data)
            .args(["--addr", &format!("127.0.0.1:{port}")])
            .args(["--auth-key", &format!("{KEY_ID},{SECRET}")])
            .arg("--cache-dir")
            .arg(dir.path().join("cache"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("rclone starts");
        let mut server = Self {
            kind: Kind::Rclone,
            child: Some(child),
            port,
            data,
            _dir: dir,
        };
        wait_for(port, "rclone", server.child.as_mut()).then_some(server)
    }

    /// MinIO already running (a container: `WAYPOINT_MINIO_URL=http://127.0.0.1:9000`, with
    /// `MINIO_ROOT_USER` and `MINIO_ROOT_PASSWORD` set to this file's key and secret), or started
    /// from the binary `WAYPOINT_MINIO` names.
    fn start_minio() -> Option<Self> {
        if let Some(url) = std::env::var("WAYPOINT_MINIO_URL")
            .ok()
            .filter(|u| !u.is_empty())
        {
            let port: u16 = url
                .rsplit(':')
                .next()
                .and_then(|p| p.trim_end_matches('/').parse().ok())?;
            let dir = temp_dir();
            let data = dir.path().to_owned();
            let server = Self {
                kind: Kind::Minio,
                child: None,
                port,
                data,
                _dir: dir,
            };
            return wait_for(port, "minio", None).then_some(server);
        }
        let minio = std::env::var_os("WAYPOINT_MINIO")?;
        let dir = temp_dir();
        let data = dir.path().join("data");
        fs::create_dir(&data).unwrap();
        let port = free_port();
        let child = Command::new(minio)
            .arg("server")
            .arg(&data)
            .args(["--address", &format!("127.0.0.1:{port}")])
            .args(["--console-address", &format!("127.0.0.1:{}", free_port())])
            .arg("--quiet")
            .env("MINIO_ROOT_USER", KEY_ID)
            .env("MINIO_ROOT_PASSWORD", SECRET)
            .env("MINIO_BROWSER", "off")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("minio starts");
        let mut server = Self {
            kind: Kind::Minio,
            child: Some(child),
            port,
            data,
            _dir: dir,
        };
        wait_for(port, "minio", server.child.as_mut()).then_some(server)
    }

    pub fn name(&self) -> &'static str {
        match self.kind {
            Kind::Rclone => "rclone",
            Kind::Minio => "minio",
        }
    }

    pub fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Makes a bucket and returns its root location.
    pub fn bucket(&self, name: &str) -> VfsPath {
        // A MinIO that outlives a test run (a container) is shared, and the conformance suite wants
        // an empty bucket: every call makes a fresh one.
        let unique;
        let name = match self.kind {
            Kind::Minio => {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos());
                unique = format!("{name}{}{}", std::process::id(), nanos % 1_000_000_000);
                unique.as_str()
            }
            Kind::Rclone => name,
        };
        let location = self.location(name);
        match self.kind {
            Kind::Rclone => fs::create_dir_all(self.data.join(name)).unwrap(),
            Kind::Minio => {
                // The server may still be settling just after it listens.
                let provider = self.provider();
                let mut last = None;
                for _ in 0..40 {
                    match provider.create_bucket(&location) {
                        Ok(()) | Err(waypoint_protocol::VfsError::AlreadyExists { .. }) => {
                            return location
                        }
                        Err(error) => last = Some(error),
                    }
                    std::thread::sleep(Duration::from_millis(250));
                }
                panic!("cannot make the bucket {name}: {last:?}");
            }
        }
        location
    }

    pub fn location(&self, bucket: &str) -> VfsPath {
        VfsPath::from_uri(&format!(
            "s3://{bucket}?endpoint=http%3A%2F%2F127.0.0.1%3A{}",
            self.port
        ))
        .unwrap()
    }

    pub fn provider(&self) -> S3Provider {
        self.provider_with(S3Options::default())
    }

    pub fn provider_with(&self, options: S3Options) -> S3Provider {
        S3Provider::new(
            S3Config::new(Arc::new(FixedKey {
                key_id: KEY_ID.to_owned(),
                secret: SECRET.to_owned(),
            }))
            .with_options(options),
        )
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
