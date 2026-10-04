// Throwaway WebDAV servers run as the current user (Apache `httpd` with `mod_dav`, and `rclone serve
// webdav`) for the WebDAV provider's tests against a real server, and the shared helpers of every
// test file.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Each `start` returns `None`, after printing why, when its server is not installed (or on Windows,
//! or with `WAYPOINT_WEBDAV_TESTS=off`), so the tests skip instead of failing; with
//! `WAYPOINT_WEBDAV_REQUIRE=1` (CI's `remote-conformance` job) they fail instead. Set
//! `WAYPOINT_HTTPD` and `WAYPOINT_RCLONE` to choose the binaries and `WAYPOINT_TEST_TMP` to choose
//! where the temporary folders go.

#![allow(dead_code)]

pub mod mock;

use std::fs;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ring::digest;
use waypoint_path::{ConnectionKey, VfsPath};
use waypoint_protocol::AuthPrompt;
use waypoint_provider_webdav::{WebDavConfig, WebDavOptions, WebDavProvider};
use waypoint_vfs::{Credential, CredentialSource, Secret};

pub const USER: &str = "alice";
pub const PASSWORD: &str = "s3cret pass";

/// Answers every password question with one login, and counts the questions and the refusals.
#[derive(Default)]
pub struct Logins {
    pub login: Option<(String, String)>,
    pub asked: Mutex<Vec<AuthPrompt>>,
    pub rejected: Mutex<usize>,
    pub token: Option<String>,
}

impl Logins {
    pub fn with(user: &str, password: &str) -> Arc<Self> {
        Arc::new(Self {
            login: Some((user.to_owned(), password.to_owned())),
            ..Self::default()
        })
    }

    pub fn token(token: &str) -> Arc<Self> {
        Arc::new(Self {
            token: Some(token.to_owned()),
            ..Self::default()
        })
    }
}

impl CredentialSource for Logins {
    fn credential(&self, _: &ConnectionKey, prompt: &AuthPrompt) -> Option<Credential> {
        self.asked.lock().unwrap().push(prompt.clone());
        match prompt {
            AuthPrompt::Password { .. } => {
                self.login
                    .as_ref()
                    .map(|(user, password)| Credential::Password {
                        user: Some(user.clone()),
                        password: Secret::from(password.as_str()),
                    })
            }
            AuthPrompt::Passphrase { .. } => self
                .token
                .as_ref()
                .map(|token| Credential::Passphrase(Secret::from(token.as_str()))),
            _ => None,
        }
    }

    fn rejected(&self, _: &ConnectionKey, _: &AuthPrompt) {
        *self.rejected.lock().unwrap() += 1;
    }
}

/// Options for a test: loopback servers are never reached through a proxy named in the environment.
pub fn options() -> WebDavOptions {
    WebDavOptions::default()
        .with_system_proxy(false)
        .with_timeout(Duration::from_secs(10))
}

pub fn config(logins: Option<Arc<Logins>>) -> WebDavConfig {
    let config = WebDavConfig::new().with_options(options());
    match logins {
        Some(logins) => config.with_credentials(logins),
        None => config,
    }
}

pub fn plain(logins: Option<Arc<Logins>>) -> WebDavProvider {
    WebDavProvider::dav(config(logins))
}

/// A temporary folder under `WAYPOINT_TEST_TMP`, or the system's.
pub fn temp_dir() -> tempfile::TempDir {
    let builder = tempfile::Builder::new()
        .prefix("waypoint-webdav-")
        .to_owned();
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

fn skip<T>(why: &str) -> Option<T> {
    // CI's real-server job sets this, so a missing server fails there instead of passing quietly.
    if std::env::var_os("WAYPOINT_WEBDAV_REQUIRE").is_some_and(|v| v == "1") {
        panic!("WAYPOINT_WEBDAV_REQUIRE is set and the real-server tests cannot run: {why}");
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

fn on_path(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|candidate| candidate.is_file())
}

fn wait_for_port(port: u16, child: &mut Child, what: &str, log: &Path) {
    let started = Instant::now();
    loop {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        if let Ok(Some(status)) = child.try_wait() {
            let said = fs::read_to_string(log).unwrap_or_default();
            panic!("{what} exited before it listened: {status}\n{said}");
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "{what} did not listen on port {port}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn stop(child: &mut Child) {
    // `httpd` has worker processes that only a graceful stop reaches.
    let _ = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status();
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// How a server wants callers to log in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Login {
    Anonymous,
    Basic,
    Digest,
}

/// A running server and the folder it serves.
pub struct Server {
    child: Child,
    pub name: &'static str,
    pub port: u16,
    /// Configuration and the server's log.
    pub dir: tempfile::TempDir,
    /// The folder served; the tests may fill it.
    pub data: PathBuf,
    /// The URL path the data is served at: `/dav`, or empty for the server's root.
    pub base: &'static str,
    pub login: Login,
}

impl Drop for Server {
    fn drop(&mut self) {
        stop(&mut self.child);
    }
}

impl Server {
    /// The URI of the served folder, naming the user for a server that has logins.
    pub fn root_uri(&self) -> String {
        let user = match self.login {
            Login::Anonymous => String::new(),
            _ => format!("{USER}@"),
        };
        format!("dav://{user}127.0.0.1:{}{}", self.port, self.base)
    }

    pub fn root(&self) -> VfsPath {
        VfsPath::from_uri(&self.root_uri()).unwrap()
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}{}", self.port, self.base)
    }

    /// A provider with the right login for this server.
    pub fn provider(&self) -> WebDavProvider {
        match self.login {
            Login::Anonymous => plain(None),
            _ => plain(Some(Logins::with(USER, PASSWORD))),
        }
    }

    pub fn log(&self) -> String {
        fs::read_to_string(self.dir.path().join("error.log")).unwrap_or_default()
    }
}

fn md5_hex(text: &str) -> String {
    use md5::{Digest, Md5};
    Md5::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn sha1_base64(text: &str) -> String {
    use base64::Engine as _;
    let sum = digest::digest(&digest::SHA1_FOR_LEGACY_USE_ONLY, text.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(sum.as_ref())
}

fn checked_start() -> Option<()> {
    if !cfg!(unix) {
        return skip("the real-server tests run on Linux only");
    }
    if std::env::var("WAYPOINT_WEBDAV_TESTS").as_deref() == Ok("off") {
        return skip("WAYPOINT_WEBDAV_TESTS=off");
    }
    Some(())
}

/// Apache's `httpd` with `mod_dav` and `mod_dav_fs`, serving a folder at `/dav`.
pub fn apache(login: Login) -> Option<Server> {
    checked_start()?;
    let httpd = match std::env::var_os("WAYPOINT_HTTPD") {
        Some(path) => Some(PathBuf::from(path)),
        None => on_path(&["httpd", "apache2"])
            .or_else(|| first_existing(&["/usr/sbin/apache2", "/usr/sbin/httpd"])),
    };
    let Some(httpd) = httpd else {
        return skip("no Apache httpd (set WAYPOINT_HTTPD)");
    };
    let Some(modules) = [
        "/usr/lib/httpd/modules",
        "/usr/lib/apache2/modules",
        "/usr/lib64/httpd/modules",
        "/usr/libexec/apache2",
    ]
    .iter()
    .map(Path::new)
    .find(|dir| dir.join("mod_dav.so").is_file()) else {
        return skip("Apache has no mod_dav");
    };

    let dir = temp_dir();
    let root = dir.path();
    let data = root.join("data");
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(root.join("run")).unwrap();
    let port = free_port();

    // What is built into the server cannot be loaded again.
    let built_in = Command::new(&httpd)
        .arg("-l")
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default();
    let mut conf = format!(
        "ServerRoot \"{root}\"\nPidFile \"{root}/httpd.pid\"\nDefaultRuntimeDir \"{root}/run\"\n\
         Mutex file:{root}/run\nListen 127.0.0.1:{port}\nServerName localhost\n\
         ErrorLog \"{root}/error.log\"\nLogLevel warn\nDavLockDB \"{root}/davlock\"\n\
         MaxRequestWorkers 25\n",
        root = root.display()
    );
    let mut wanted = vec![
        "mpm_event",
        "unixd",
        "authn_core",
        "authz_core",
        "dav",
        "dav_fs",
        "alias",
    ];
    if login != Login::Anonymous {
        wanted.extend(["authn_file", "authz_user", "auth_basic", "auth_digest"]);
    }
    for name in wanted {
        if !built_in.contains(&format!("mod_{name}.c")) {
            conf.push_str(&format!(
                "LoadModule {name}_module {}/mod_{name}.so\n",
                modules.display()
            ));
        }
    }
    let auth = match login {
        Login::Anonymous => "Require all granted\n".to_owned(),
        Login::Basic => {
            fs::write(
                root.join("users"),
                format!("{USER}:{{SHA}}{}\n", sha1_base64(PASSWORD)),
            )
            .unwrap();
            format!(
                "AuthType Basic\nAuthName \"waypoint\"\nAuthBasicProvider file\n\
                 AuthUserFile \"{}/users\"\nRequire valid-user\n",
                root.display()
            )
        }
        Login::Digest => {
            fs::write(
                root.join("users"),
                format!(
                    "{USER}:waypoint:{}\n",
                    md5_hex(&format!("{USER}:waypoint:{PASSWORD}"))
                ),
            )
            .unwrap();
            format!(
                "AuthType Digest\nAuthName \"waypoint\"\nAuthDigestProvider file\n\
                 AuthUserFile \"{}/users\"\nRequire valid-user\n",
                root.display()
            )
        }
    };
    conf.push_str(&format!(
        "Alias /dav \"{data}\"\n<Directory \"{data}\">\n  Dav On\n  {auth}</Directory>\n",
        data = data.display()
    ));
    fs::write(root.join("httpd.conf"), conf).unwrap();

    let mut child = Command::new(&httpd)
        .arg("-f")
        .arg(root.join("httpd.conf"))
        .arg("-DFOREGROUND")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(
            fs::File::create(root.join("stderr.log")).unwrap(),
        ))
        .spawn()
        .expect("httpd starts");
    wait_for_port(port, &mut child, "httpd", &root.join("stderr.log"));
    Some(Server {
        child,
        name: "apache",
        port,
        dir,
        data,
        base: "/dav",
        login,
    })
}

/// `rclone serve webdav`, serving a folder at the server's root.
pub fn rclone(login: Login) -> Option<Server> {
    checked_start()?;
    assert_ne!(login, Login::Digest, "rclone serves Basic only");
    let rclone = match std::env::var_os("WAYPOINT_RCLONE") {
        Some(path) => Some(PathBuf::from(path)),
        None => on_path(&["rclone"]),
    };
    let Some(rclone) = rclone else {
        return skip("no rclone (set WAYPOINT_RCLONE)");
    };
    let dir = temp_dir();
    let data = dir.path().join("data");
    fs::create_dir_all(&data).unwrap();
    let port = free_port();
    let mut command = Command::new(&rclone);
    command
        .args(["serve", "webdav"])
        .arg(&data)
        .arg("--addr")
        .arg(format!("127.0.0.1:{port}"))
        .arg("--config")
        .arg(dir.path().join("rclone.conf"))
        .arg("--cache-dir")
        .arg(dir.path().join("cache"));
    if login == Login::Basic {
        command.args(["--user", USER, "--pass", PASSWORD]);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(
            fs::File::create(dir.path().join("error.log")).unwrap(),
        ))
        .spawn()
        .expect("rclone starts");
    wait_for_port(port, &mut child, "rclone", &dir.path().join("error.log"));
    Some(Server {
        child,
        name: "rclone",
        port,
        dir,
        data,
        base: "",
        login,
    })
}
