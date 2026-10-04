// The options of OpenSSH's client configuration a file manager needs: which address, user, port,
// key files and jump hosts a host name stands for.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::pattern::{list_matches, wildcard};

/// How deep `Include` may nest, as OpenSSH limits it.
const MAX_INCLUDE_DEPTH: usize = 16;

/// What the configuration says about one host name. A field is `None` (or empty) when nothing
/// sets it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostConfig {
    /// The real name or address to connect to.
    pub host_name: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    /// Key files, `~` and `%` tokens expanded, in the order given.
    pub identity_files: Vec<PathBuf>,
    /// The jump hosts, in order; empty for none (`ProxyJump none` included).
    pub proxy_jump: Vec<Jump>,
}

/// One jump host of `ProxyJump`: `[user@]host[:port]`, or `ssh://[user@]host[:port]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jump {
    pub user: Option<String>,
    pub host: String,
    pub port: Option<u16>,
}

/// Reads a `ProxyJump` value: comma-separated jump hosts, or `none`.
pub fn parse_jumps(value: &str) -> Vec<Jump> {
    if value.eq_ignore_ascii_case("none") {
        return Vec::new();
    }
    value
        .split(',')
        .filter_map(|spec| {
            let spec = spec.trim();
            let spec = spec.strip_prefix("ssh://").unwrap_or(spec);
            let (user, rest) = match spec.rsplit_once('@') {
                Some((user, rest)) => (Some(user.to_owned()), rest),
                None => (None, spec),
            };
            let (host, port) = if let Some(bracketed) = rest.strip_prefix('[') {
                let (host, after) = bracketed.split_once(']')?;
                (host, after.strip_prefix(':').and_then(|p| p.parse().ok()))
            } else {
                match rest.rsplit_once(':') {
                    Some((host, port)) if !host.contains(':') => (host, port.parse().ok()),
                    _ => (rest, None),
                }
            };
            (!host.is_empty()).then(|| Jump {
                user,
                host: host.to_owned(),
                port,
            })
        })
        .collect()
}

/// Which hosts a block of options applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Condition {
    /// Options before the first `Host` or `Match`.
    Always,
    Host(Vec<String>),
    /// `Match all`.
    MatchAll,
    /// Any other `Match`: not evaluated, so never applied.
    Unsupported,
}

#[derive(Debug, Clone)]
struct Block {
    condition: Condition,
    options: Vec<(String, String)>,
}

/// A parsed client configuration.
#[derive(Debug, Clone, Default)]
pub struct SshConfig {
    blocks: Vec<Block>,
}

/// Splits a line into its keyword and arguments: `Keyword value`, `Keyword=value`, and
/// arguments in double quotes.
fn split_line(line: &str) -> Option<(String, Vec<String>)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let end = line
        .find(|c: char| c.is_whitespace() || c == '=')
        .unwrap_or(line.len());
    let keyword = line[..end].to_ascii_lowercase();
    let mut rest = line[end..].trim_start();
    if let Some(after) = rest.strip_prefix('=') {
        rest = after.trim_start();
    }
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut started = false;
    for c in rest.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        args.push(current);
    }
    Some((keyword, args))
}

impl SshConfig {
    /// Parses `text`. An `Include` with a relative path is read from `include_dir` (`~/.ssh` for
    /// the user's file).
    pub fn parse(text: &str, include_dir: &Path) -> Self {
        let mut config = Self {
            blocks: vec![Block {
                condition: Condition::Always,
                options: Vec::new(),
            }],
        };
        config.read(text, include_dir, 0);
        config
    }

    /// Reads the file at `path`; relative includes are read from its folder.
    pub fn load(path: &Path) -> io::Result<Self> {
        let text = fs::read_to_string(path)?;
        let dir = path.parent().unwrap_or(Path::new("."));
        Ok(Self::parse(&text, dir))
    }

    /// `~/.ssh/config`, or an empty configuration when there is none.
    pub fn for_user() -> Self {
        let Some(home) = std::env::home_dir() else {
            return Self::default();
        };
        let dir = home.join(".ssh");
        match fs::read_to_string(dir.join("config")) {
            Ok(text) => Self::parse(&text, &dir),
            Err(_) => Self::default(),
        }
    }

    fn read(&mut self, text: &str, include_dir: &Path, depth: usize) {
        for line in text.lines() {
            let Some((keyword, args)) = split_line(line) else {
                continue;
            };
            match keyword.as_str() {
                "host" => self.blocks.push(Block {
                    condition: Condition::Host(args),
                    options: Vec::new(),
                }),
                "match" => {
                    let all = args.len() == 1 && args[0].eq_ignore_ascii_case("all");
                    self.blocks.push(Block {
                        condition: if all {
                            Condition::MatchAll
                        } else {
                            Condition::Unsupported
                        },
                        options: Vec::new(),
                    });
                }
                "include" if depth < MAX_INCLUDE_DEPTH => {
                    // An included file's `Host` lines do not leak out: what follows the `Include`
                    // belongs to the block it was in, as OpenSSH restores it.
                    let around = self.blocks.len();
                    let condition = self.blocks.last().map(|b| b.condition.clone());
                    for pattern in &args {
                        for path in expand_include(pattern, include_dir) {
                            if let Ok(text) = fs::read_to_string(&path) {
                                self.read(&text, include_dir, depth + 1);
                            }
                        }
                    }
                    if let (true, Some(condition)) = (self.blocks.len() > around, condition) {
                        self.blocks.push(Block {
                            condition,
                            options: Vec::new(),
                        });
                    }
                }
                _ => {
                    if let (Some(block), Some(value)) = (self.blocks.last_mut(), args.first()) {
                        let value = if keyword == "proxyjump" {
                            args.join(",")
                        } else {
                            value.clone()
                        };
                        block.options.push((keyword, value));
                    }
                }
            }
        }
    }

    /// The options that apply to `alias` (the host name as written in a location).
    pub fn host(&self, alias: &str) -> HostConfig {
        let mut config = HostConfig::default();
        let mut jump_set = false;
        let mut identity_files = Vec::new();
        for block in &self.blocks {
            let applies = match &block.condition {
                Condition::Always | Condition::MatchAll => true,
                Condition::Host(patterns) => {
                    list_matches(patterns.iter().map(String::as_str), alias)
                }
                Condition::Unsupported => false,
            };
            if !applies {
                continue;
            }
            for (keyword, value) in &block.options {
                match keyword.as_str() {
                    "hostname" if config.host_name.is_none() => {
                        config.host_name = Some(value.replace("%h", alias));
                    }
                    "user" if config.user.is_none() => config.user = Some(value.clone()),
                    "port" if config.port.is_none() => config.port = value.parse().ok(),
                    "identityfile" => identity_files.push(value.clone()),
                    "proxyjump" if !jump_set => {
                        jump_set = true;
                        config.proxy_jump = parse_jumps(value);
                    }
                    _ => {}
                }
            }
        }
        let host = config.host_name.clone().unwrap_or_else(|| alias.to_owned());
        config.identity_files = identity_files
            .iter()
            .filter(|file| !file.eq_ignore_ascii_case("none"))
            .map(|file| expand_tokens(file, &host, config.user.as_deref(), config.port))
            .collect();
        config
    }
}

fn home() -> PathBuf {
    std::env::home_dir().unwrap_or_default()
}

fn local_user() -> String {
    ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .unwrap_or_default()
}

/// Expands `~` and the `%` tokens OpenSSH allows in `IdentityFile`.
fn expand_tokens(text: &str, host: &str, user: Option<&str>, port: Option<u16>) -> PathBuf {
    let text = match text.strip_prefix("~/") {
        Some(rest) => format!("{}/{rest}", home().display()),
        None => text.to_owned(),
    };
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('d') => out.push_str(&home().display().to_string()),
            Some('u') => out.push_str(&local_user()),
            Some('h') => out.push_str(host),
            Some('r') => out.push_str(&user.map_or_else(local_user, str::to_owned)),
            Some('p') => out.push_str(&port.unwrap_or(22).to_string()),
            Some(other) => {
                out.push('%');
                out.push(other);
            }
            None => out.push('%'),
        }
    }
    PathBuf::from(out)
}

/// The files an `Include` names: `~` expanded, relative to `dir`, with `*` and `?` in the file
/// name matched against the folder's entries (in name order, as `glob(3)` sorts them).
fn expand_include(pattern: &str, dir: &Path) -> Vec<PathBuf> {
    let pattern = match pattern.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None => PathBuf::from(pattern),
    };
    let pattern = if pattern.is_absolute() {
        pattern
    } else {
        dir.join(pattern)
    };
    let Some(name) = pattern.file_name().and_then(|n| n.to_str()) else {
        return Vec::new();
    };
    if !name.contains(['*', '?']) {
        return vec![pattern];
    }
    let Some(parent) = pattern.parent() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|file| !file.starts_with('.') && wildcard(name, file))
        })
        .map(|entry| entry.path())
        .collect();
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_value_wins_and_key_files_add_up() {
        let config = SshConfig::parse(
            "# defaults later\n\
             Host nas\n  HostName 10.0.0.5\n  User media\n  Port=2222\n  IdentityFile /keys/nas\n\
             Host nas *.lan\n  User other\n  IdentityFile \"/keys/with space\"\n\
             Host *\n  User fallback\n  IdentityFile /keys/%h-%r-%p\n",
            Path::new("/nowhere"),
        );
        let nas = config.host("nas");
        assert_eq!(nas.host_name.as_deref(), Some("10.0.0.5"));
        assert_eq!(nas.user.as_deref(), Some("media"));
        assert_eq!(nas.port, Some(2222));
        assert_eq!(
            nas.identity_files,
            [
                PathBuf::from("/keys/nas"),
                PathBuf::from("/keys/with space"),
                PathBuf::from("/keys/10.0.0.5-media-2222"),
            ]
        );
        let other = config.host("tv.lan");
        assert_eq!(other.host_name, None);
        assert_eq!(other.user.as_deref(), Some("other"));
        assert_eq!(config.host("x").user.as_deref(), Some("fallback"));
    }

    #[test]
    fn negated_patterns_match_all_and_unsupported_matches() {
        let config = SshConfig::parse(
            "Host *.lan !secret.lan\n  User lan\n\
             Match host foo exec \"true\"\n  User never\n\
             Match all\n  Port 2200\n",
            Path::new("/nowhere"),
        );
        assert_eq!(config.host("nas.lan").user.as_deref(), Some("lan"));
        assert_eq!(config.host("secret.lan").user, None);
        assert_eq!(config.host("foo").user, None);
        assert_eq!(config.host("foo").port, Some(2200));
    }

    #[test]
    fn jump_hosts_are_read_in_order() {
        assert_eq!(
            parse_jumps("me@bastion:2222,ssh://inner,[::1]:22"),
            [
                Jump {
                    user: Some("me".into()),
                    host: "bastion".into(),
                    port: Some(2222)
                },
                Jump {
                    user: None,
                    host: "inner".into(),
                    port: None
                },
                Jump {
                    user: None,
                    host: "::1".into(),
                    port: Some(22)
                },
            ]
        );
        assert!(parse_jumps("none").is_empty());
        let config = SshConfig::parse(
            "Host a\n  ProxyJump none\nHost *\n  ProxyJump gw\n",
            Path::new("/"),
        );
        assert!(config.host("a").proxy_jump.is_empty());
        assert_eq!(config.host("b").proxy_jump[0].host, "gw");
    }

    #[test]
    fn includes_are_read_in_place_with_globs() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("conf.d")).unwrap();
        fs::write(
            dir.path().join("conf.d/20-b.conf"),
            "Host b\n  User from-b\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("conf.d/10-a.conf"),
            "Host a\n  User from-a\nHost b\n  Port 7\n",
        )
        .unwrap();
        fs::write(dir.path().join("conf.d/.hidden"), "Host a\n  User hidden\n").unwrap();
        fs::write(dir.path().join("self"), "Include self\n").unwrap();
        fs::write(
            dir.path().join("config"),
            "Host a b c\n  Include conf.d/*.conf\n  Port 9\nInclude self\nHost *\n  User default\n",
        )
        .unwrap();
        let config = SshConfig::load(&dir.path().join("config")).unwrap();
        assert_eq!(config.host("a").user.as_deref(), Some("from-a"));
        assert_eq!(config.host("b").user.as_deref(), Some("from-b"));
        assert_eq!(config.host("b").port, Some(7));
        assert_eq!(config.host("c").user.as_deref(), Some("default"));
        // What follows an `Include` stays in the block that held it.
        assert_eq!(config.host("a").port, Some(9));
        assert_eq!(config.host("z").port, None);
    }
}
