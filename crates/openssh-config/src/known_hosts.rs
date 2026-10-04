// OpenSSH's `known_hosts`: checking a server's key, appending a trusted one and replacing a
// changed one.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use data_encoding::BASE64;
use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;

use crate::pattern::wildcard;

/// How `known_hosts` names a server: the host alone on port 22, `[host]:port` otherwise.
pub fn host_label(host: &str, port: u16) -> String {
    if port == 22 {
        host.to_owned()
    } else {
        format!("[{host}]:{port}")
    }
}

/// A key as recorded: its type and its base64 blob.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecordedKey {
    pub algorithm: String,
    pub base64: String,
}

/// What the files say about a server's key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    /// The key is recorded for the server.
    Known,
    /// Nothing is recorded for the server and this key type.
    Unknown,
    /// Another key of the same type is recorded for the server.
    Changed { recorded: RecordedKey },
    /// The key is marked `@revoked`.
    Revoked,
}

/// The user's `known_hosts`, which is read and written, and system files that are only read.
#[derive(Debug, Clone)]
pub struct KnownHostsFile {
    user: PathBuf,
    system: Vec<PathBuf>,
}

struct Line<'a> {
    marker: Option<&'a str>,
    hosts: &'a str,
    algorithm: &'a str,
    base64: &'a str,
}

fn parse(line: &str) -> Option<Line<'_>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let mut words = line.split_whitespace();
    let mut first = words.next()?;
    let marker = if first.starts_with('@') {
        let marker = first;
        first = words.next()?;
        Some(marker)
    } else {
        None
    };
    Some(Line {
        marker,
        hosts: first,
        algorithm: words.next()?,
        base64: words.next()?,
    })
}

/// Whether a line's host list names `label`: plain names and wildcards, `!` negation, and
/// `|1|salt|hash` names hashed with HMAC-SHA1.
fn hosts_match(hosts: &str, label: &str) -> bool {
    let mut matched = false;
    for entry in hosts.split(',') {
        let (negated, entry) = match entry.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, entry),
        };
        let hit = match entry.strip_prefix("|1|") {
            Some(hashed) => hashed_matches(hashed, label),
            None => wildcard(entry, label),
        };
        if hit && negated {
            return false;
        }
        matched |= hit;
    }
    matched
}

fn hashed_matches(hashed: &str, label: &str) -> bool {
    let Some((salt, hash)) = hashed.split_once('|') else {
        return false;
    };
    let (Ok(salt), Ok(hash)) = (
        BASE64.decode(salt.as_bytes()),
        BASE64.decode(hash.as_bytes()),
    ) else {
        return false;
    };
    let Ok(mut mac) = <Hmac<Sha1> as KeyInit>::new_from_slice(&salt) else {
        return false;
    };
    mac.update(label.as_bytes());
    mac.verify_slice(&hash).is_ok()
}

impl KnownHostsFile {
    /// The user's file at `user`, with no system files.
    pub fn new(user: impl Into<PathBuf>) -> Self {
        Self {
            user: user.into(),
            system: Vec::new(),
        }
    }

    /// Also reads `path`, which is never written.
    pub fn with_system(mut self, path: impl Into<PathBuf>) -> Self {
        self.system.push(path.into());
        self
    }

    /// `~/.ssh/known_hosts`, and on Linux `/etc/ssh/ssh_known_hosts` (on Windows,
    /// `%PROGRAMDATA%\ssh\ssh_known_hosts`).
    pub fn for_user() -> Option<Self> {
        let home = std::env::home_dir()?;
        let file = Self::new(home.join(".ssh").join("known_hosts"));
        let system = if cfg!(windows) {
            std::env::var_os("PROGRAMDATA")
                .map(|data| PathBuf::from(data).join("ssh").join("ssh_known_hosts"))
        } else {
            Some(PathBuf::from("/etc/ssh/ssh_known_hosts"))
        };
        Some(match system {
            Some(system) => file.with_system(system),
            None => file,
        })
    }

    /// The user's file.
    pub fn path(&self) -> &Path {
        &self.user
    }

    /// Checks the key `algorithm base64` offered by `host` on `port`. A file that cannot be read
    /// counts as empty.
    pub fn check(&self, host: &str, port: u16, algorithm: &str, base64: &str) -> Lookup {
        let label = host_label(host, port);
        let mut known = false;
        let mut changed = None;
        for path in std::iter::once(&self.user).chain(&self.system) {
            let Ok(text) = fs::read_to_string(path) else {
                continue;
            };
            for line in text.lines().filter_map(parse) {
                if !hosts_match(line.hosts, &label) {
                    continue;
                }
                let same = line.base64 == base64;
                match line.marker {
                    Some("@revoked") if same => return Lookup::Revoked,
                    Some(_) => {}
                    None if same => known = true,
                    None if line.algorithm == algorithm && changed.is_none() => {
                        changed = Some(RecordedKey {
                            algorithm: line.algorithm.to_owned(),
                            base64: line.base64.to_owned(),
                        });
                    }
                    None => {}
                }
            }
        }
        match (known, changed) {
            (true, _) => Lookup::Known,
            (false, Some(recorded)) => Lookup::Changed { recorded },
            (false, None) => Lookup::Unknown,
        }
    }

    /// Appends `algorithm base64` for `host` on `port` to the user's file, creating it (and
    /// `~/.ssh`) if needed.
    pub fn append(&self, host: &str, port: u16, algorithm: &str, base64: &str) -> io::Result<()> {
        if let Some(parent) = self.user.parent() {
            create_private_dir(parent)?;
        }
        let existing = fs::read(&self.user).unwrap_or_default();
        let mut file = open_private_append(&self.user)?;
        if !existing.is_empty() && !existing.ends_with(b"\n") {
            file.write_all(b"\n")?;
        }
        writeln!(file, "{} {algorithm} {base64}", host_label(host, port))?;
        file.sync_all()
    }

    /// Replaces the `recorded` key of `host` on `port` with `algorithm base64`: the server is taken
    /// out of every line of the user's file that names it with the recorded key (a line naming
    /// only it is removed; one that also names other hosts keeps them, and a wildcard that covers
    /// it gets a negation), and the new key is appended on a line of its own. The file is rewritten through a temporary file and a rename. A recorded key that
    /// is only in a system file stays there, and the new key, appended to the user's file, is
    /// found first.
    pub fn replace(
        &self,
        host: &str,
        port: u16,
        recorded: &RecordedKey,
        algorithm: &str,
        base64: &str,
    ) -> io::Result<()> {
        let label = host_label(host, port);
        let text = match fs::read_to_string(&self.user) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error),
        };
        let mut kept = String::with_capacity(text.len());
        for line in text.lines() {
            let rewritten = match parse(line) {
                Some(parsed)
                    if parsed.marker.is_none()
                        && parsed.base64 == recorded.base64
                        && hosts_match(parsed.hosts, &label) =>
                {
                    without_host(line, parsed.hosts, &label)
                }
                _ => Some(line.to_owned()),
            };
            if let Some(line) = rewritten {
                kept.push_str(&line);
                kept.push('\n');
            }
        }
        kept.push_str(&format!("{label} {algorithm} {base64}\n"));
        if let Some(parent) = self.user.parent() {
            create_private_dir(parent)?;
        }
        let temporary = self.user.with_extension("waypoint-new");
        {
            let mut file = open_private_new(&temporary)?;
            file.write_all(kept.as_bytes())?;
            file.sync_all()?;
        }
        if let Ok(metadata) = fs::metadata(&self.user) {
            let _ = fs::set_permissions(&temporary, metadata.permissions());
        }
        fs::rename(&temporary, &self.user)
    }
}

/// `line` with `label` taken out of its host list `hosts`, so the line keeps vouching for every
/// other host: an entry naming exactly `label` (plainly or hashed) is removed, and a wildcard that
/// covers it gets `!label` beside it. `None` when no other host is left.
fn without_host(line: &str, hosts: &str, label: &str) -> Option<String> {
    let mut entries: Vec<String> = Vec::new();
    let mut needs_negation = false;
    for entry in hosts.split(',') {
        let exact = if entry.starts_with('!') {
            false
        } else if let Some(hashed) = entry.strip_prefix("|1|") {
            hashed_matches(hashed, label)
        } else if entry.contains(['*', '?']) {
            needs_negation |= wildcard(entry, label);
            false
        } else {
            entry.eq_ignore_ascii_case(label)
        };
        if !exact {
            entries.push(entry.to_owned());
        }
    }
    if !entries.iter().any(|entry| !entry.starts_with('!')) {
        return None;
    }
    if needs_negation {
        entries.push(format!("!{label}"));
    }
    let start = line.len() - line.trim_start().len();
    let rest = &line[start + hosts.len()..];
    Some(format!("{}{}{rest}", &line[..start], entries.join(",")))
}

fn create_private_dir(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        return Ok(());
    }
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(path)
}

fn open_private_append(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.append(true).create(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

fn open_private_new(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ED: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIBCb8PDBXvzERLPgkKWxNbNUSsz4pysfnT/WkQUYHuQH";
    const ED2: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIH8a5MQ6ySq4iqqSL2z5fjBOnvjOZ2f6VHcrgaahwM7P";
    const ECDSA: &str = "AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBEXAMPLE";

    fn file(text: &str) -> (tempfile::TempDir, KnownHostsFile) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("known_hosts");
        fs::write(&path, text).unwrap();
        (dir, KnownHostsFile::new(path))
    }

    #[test]
    fn plain_wildcard_and_bracketed_hosts_are_found() {
        let (_dir, hosts) = file(&format!(
            "# comment\n\nnas.lan,10.0.0.5 ssh-ed25519 {ED}\n*.work ssh-ed25519 {ED2} me@laptop\n[nas.lan]:2222 ssh-ed25519 {ED2}\n"
        ));
        assert_eq!(hosts.check("nas.lan", 22, "ssh-ed25519", ED), Lookup::Known);
        assert_eq!(
            hosts.check("10.0.0.5", 22, "ssh-ed25519", ED),
            Lookup::Known
        );
        assert_eq!(
            hosts.check("build.work", 22, "ssh-ed25519", ED2),
            Lookup::Known
        );
        assert_eq!(
            hosts.check("nas.lan", 2222, "ssh-ed25519", ED2),
            Lookup::Known
        );
        assert_eq!(hosts.check("other", 22, "ssh-ed25519", ED), Lookup::Unknown);
        assert_eq!(
            hosts.check("nas.lan", 22, "ssh-ed25519", ED2),
            Lookup::Changed {
                recorded: RecordedKey {
                    algorithm: "ssh-ed25519".into(),
                    base64: ED.into()
                }
            }
        );
        // Another key type of a known server is unknown, not changed.
        assert_eq!(
            hosts.check("nas.lan", 22, "ecdsa-sha2-nistp256", ECDSA),
            Lookup::Unknown
        );
    }

    #[test]
    fn negation_revocation_and_hashed_names() {
        // `|1|salt|hash` for "nas.lan", made with `ssh-keygen -H`'s scheme.
        let salt = [7u8; 20];
        let mut mac = <Hmac<Sha1> as KeyInit>::new_from_slice(&salt).unwrap();
        mac.update(b"nas.lan");
        let hashed = format!(
            "|1|{}|{}",
            BASE64.encode(&salt),
            BASE64.encode(&mac.finalize().into_bytes())
        );
        let (_dir, hosts) = file(&format!(
            "{hashed} ssh-ed25519 {ED}\n*.lan,!secret.lan ssh-ed25519 {ED2}\n@revoked * ssh-ed25519 {ECDSA}\n@cert-authority *.lan ssh-ed25519 {ED}\n"
        ));
        assert_eq!(hosts.check("nas.lan", 22, "ssh-ed25519", ED), Lookup::Known);
        assert_eq!(hosts.check("tv.lan", 22, "ssh-ed25519", ED2), Lookup::Known);
        assert_eq!(
            hosts.check("secret.lan", 22, "ssh-ed25519", ED2),
            Lookup::Unknown
        );
        assert_eq!(
            hosts.check("any", 22, "ssh-ed25519", ECDSA),
            Lookup::Revoked
        );
        // A certificate authority line is not a host key.
        assert_eq!(
            hosts.check("tv.lan", 22, "ssh-ed25519", ED),
            Lookup::Changed {
                recorded: RecordedKey {
                    algorithm: "ssh-ed25519".into(),
                    base64: ED2.into()
                }
            }
        );
    }

    #[test]
    fn appending_creates_the_file_and_keeps_what_is_there() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ssh").join("known_hosts");
        let hosts = KnownHostsFile::new(&path);
        assert_eq!(hosts.check("h", 22, "ssh-ed25519", ED), Lookup::Unknown);
        hosts.append("h", 2222, "ssh-ed25519", ED).unwrap();
        fs::write(
            &path,
            format!(
                "{}other ssh-ed25519 {ED2}",
                fs::read_to_string(&path).unwrap()
            ),
        )
        .unwrap();
        hosts.append("h", 22, "ssh-ed25519", ED).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            format!("[h]:2222 ssh-ed25519 {ED}\nother ssh-ed25519 {ED2}\nh ssh-ed25519 {ED}\n")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn replacing_keeps_the_trust_of_every_other_host() {
        let salt = [9u8; 20];
        let mut mac = <Hmac<Sha1> as KeyInit>::new_from_slice(&salt).unwrap();
        mac.update(b"nas.lan");
        let hashed = format!(
            "|1|{}|{}",
            BASE64.encode(&salt),
            BASE64.encode(&mac.finalize().into_bytes())
        );
        let (_dir, hosts) = file(&format!(
            "nas.lan,10.0.0.5 ssh-ed25519 {ED} combined\n*.lan ssh-ed25519 {ED}\n{hashed},tv.lan ssh-ed25519 {ED}\n"
        ));
        let recorded = RecordedKey {
            algorithm: "ssh-ed25519".into(),
            base64: ED.into(),
        };
        hosts
            .replace("nas.lan", 22, &recorded, "ssh-ed25519", ED2)
            .unwrap();
        let text = fs::read_to_string(hosts.path()).unwrap();
        assert_eq!(
            text,
            format!(
                "10.0.0.5 ssh-ed25519 {ED} combined\n*.lan,!nas.lan ssh-ed25519 {ED}\ntv.lan ssh-ed25519 {ED}\nnas.lan ssh-ed25519 {ED2}\n"
            )
        );
        assert_eq!(
            hosts.check("nas.lan", 22, "ssh-ed25519", ED2),
            Lookup::Known
        );
        assert!(matches!(
            hosts.check("nas.lan", 22, "ssh-ed25519", ED),
            Lookup::Changed { .. }
        ));
        for other in ["10.0.0.5", "printer.lan", "tv.lan"] {
            assert_eq!(
                hosts.check(other, 22, "ssh-ed25519", ED),
                Lookup::Known,
                "{other}"
            );
        }
    }

    #[test]
    fn replacing_drops_the_old_line_and_keeps_the_rest() {
        let (_dir, hosts) = file(&format!(
            "keep ssh-ed25519 {ED}\nnas.lan ssh-ed25519 {ED}\n# note\n"
        ));
        let recorded = RecordedKey {
            algorithm: "ssh-ed25519".into(),
            base64: ED.into(),
        };
        hosts
            .replace("nas.lan", 22, &recorded, "ssh-ed25519", ED2)
            .unwrap();
        let text = fs::read_to_string(hosts.path()).unwrap();
        assert_eq!(
            text,
            format!("keep ssh-ed25519 {ED}\n# note\nnas.lan ssh-ed25519 {ED2}\n")
        );
        assert_eq!(
            hosts.check("nas.lan", 22, "ssh-ed25519", ED2),
            Lookup::Known
        );
        assert_eq!(hosts.check("keep", 22, "ssh-ed25519", ED), Lookup::Known);
    }
}
