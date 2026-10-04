// What a saved connection is: the fields a person fills in, their checks, and the location they lead to
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;
use waypoint_path::{ConnectionKey, Host, RemotePath, RemoteScheme, VfsPath};
use waypoint_protocol::Location;

/// The longest name a connection may have, in characters.
pub const MAX_NAME_CHARS: usize = 200;

/// The longest host, user, key file, jump host or start folder, in bytes.
pub const MAX_FIELD_BYTES: usize = 4096;

/// The shortest and longest idle timeout, request count and refresh interval a connection may set.
pub const REFRESH_SECONDS_MIN: u32 = 10;
pub const REFRESH_SECONDS_MAX: u32 = 3600;
pub const TIMEOUT_SECONDS_MIN: u32 = 5;
pub const TIMEOUT_SECONDS_MAX: u32 = 600;
pub const REQUESTS_MAX: u32 = 1024;
pub const WINDOW_KIB_MIN: u32 = 64;
pub const WINDOW_KIB_MAX: u32 = 64 * 1024;

/// How a connection logs in. Whatever is chosen, the SSH agent and key files are never used for
/// anything but SSH, and no secret is part of the choice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum AuthMethod {
    /// What the provider finds on its own first (for SSH the agent, then the key files), then a
    /// password asked for or remembered.
    #[default]
    Auto,
    /// A password, asked for or remembered.
    Password,
    /// The key file the connection names, with its passphrase asked for or remembered.
    KeyFile,
}

/// The tuning and choices of one connection (A81). Every field has a provider default, so `None`
/// means "the default" and a document from an older build reads as the defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ConnectionOptions {
    /// Previews of the server's files (off by default, D14).
    pub thumbnails: bool,
    /// Refresh a shown folder every so many seconds; `None` refreshes only by the rules of D150.
    pub refresh_seconds: Option<u32>,
    /// How long a request or connecting may go unanswered (30 s by default).
    pub timeout_seconds: Option<u32>,
    /// Requests a listing keeps in flight (SFTP: 64).
    pub listing_requests: Option<u32>,
    /// Requests a transfer keeps in flight (SFTP: 64).
    pub transfer_requests: Option<u32>,
    /// The SSH channel window in KiB (8 MiB by default).
    pub window_kib: Option<u32>,
}

/// What a person fills in for a connection: the Connect dialog sends it to add or change one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ConnectionDraft {
    /// What the Network section calls it; blank takes the server's address.
    pub name: String,
    /// `sftp`, `smb`, `davs`, `dav` or `s3`.
    pub scheme: String,
    /// A host name or an IP address (IPv6 with or without its brackets).
    pub host: String,
    /// A port other than the scheme's default.
    pub port: Option<u16>,
    /// The user to log in as (`domain;user` on SMB); `None` lets the provider choose (for SSH,
    /// `~/.ssh/config` or the local account).
    pub user: Option<String>,
    pub auth: AuthMethod,
    /// The private key file, for `AuthMethod::KeyFile`.
    pub key_file: Option<String>,
    /// A jump host, as `ProxyJump` writes it (`[user@]host[:port]`, several separated by commas).
    pub jump_host: Option<String>,
    /// The folder the connection opens at, absolute on the server; `None` opens at the root.
    pub start_folder: Option<String>,
    pub options: ConnectionOptions,
}

/// A saved connection: its id and what was filled in. It never holds a secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SavedConnection {
    /// Stable for the life of the connection, never reused (`c1`, `c2`, …).
    pub id: String,
    #[serde(flatten)]
    pub draft: ConnectionDraft,
}

/// A saved connection as the page shows it: the fields, and what Rust derives from them so the page
/// never builds an address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ConnectionEntry {
    pub connection: SavedConnection,
    /// The name shown: the saved name, or the address when it has none.
    pub label: String,
    /// The login it belongs to (`sftp://me@nas.lan`), the key of its state events.
    pub key: String,
    /// Where opening it goes: the start folder, or the server's root.
    pub location: Location,
}

/// A server connected to without being saved, for the Network section's Recent servers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct RecentServer {
    /// The login (`sftp://me@nas.lan`).
    pub key: String,
    /// The server's root, to open it again.
    pub location: Location,
    /// When it last connected, in milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub at_ms: u64,
}

/// Why a draft cannot be saved. Each names the field, so the dialog can put the message under it.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum DraftError {
    #[error("the name is longer than {MAX_NAME_CHARS} characters or holds control characters")]
    Name,
    #[error("`{scheme}` is not a server protocol")]
    Scheme { scheme: String },
    #[error("the host is missing or is not a host name or an address")]
    Host,
    #[error("the user name cannot be used")]
    User,
    #[error("a password does not belong in the address")]
    Password,
    #[error("the port must be between 1 and 65535")]
    Port,
    #[error("the key file must be an absolute path or start with `~/`")]
    KeyFile,
    #[error("the jump host is not `[user@]host[:port]`")]
    JumpHost,
    #[error("the start folder must be an absolute path on the server")]
    StartFolder,
    #[error("an option is out of range: {option}")]
    Option { option: String },
}

/// A draft brought to the one form it is saved in: trimmed, blank fields `None`, the host as the
/// canonical address writes it and the default port dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    pub draft: ConnectionDraft,
    pub root: RemotePath,
    pub start: RemotePath,
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

fn too_long(value: &Option<String>) -> bool {
    value
        .as_ref()
        .is_some_and(|v| v.len() > MAX_FIELD_BYTES || v.chars().any(char::is_control))
}

/// Percent-encodes what would end or confuse the user part of an address: `%`, `@`, `:`, `/`,
/// `?`, `#` and spaces. The lenient parser decodes it back.
fn encode_user(user: &str) -> String {
    let mut out = String::with_capacity(user.len());
    for c in user.chars() {
        match c {
            '%' | '@' | ':' | '/' | '?' | '#' | ' ' | '[' | ']' => {
                let mut buf = [0u8; 4];
                for byte in c.encode_utf8(&mut buf).bytes() {
                    out.push_str(&format!("%{byte:02X}"));
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// An IPv6 address written without its brackets gets them.
fn bracket_host(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    }
}

fn check_range(value: Option<u32>, min: u32, max: u32, option: &str) -> Result<(), DraftError> {
    match value {
        Some(v) if v < min || v > max => Err(DraftError::Option {
            option: option.to_owned(),
        }),
        _ => Ok(()),
    }
}

fn check_options(options: &ConnectionOptions) -> Result<(), DraftError> {
    check_range(
        options.refresh_seconds,
        REFRESH_SECONDS_MIN,
        REFRESH_SECONDS_MAX,
        "refreshSeconds",
    )?;
    check_range(
        options.timeout_seconds,
        TIMEOUT_SECONDS_MIN,
        TIMEOUT_SECONDS_MAX,
        "timeoutSeconds",
    )?;
    check_range(options.listing_requests, 1, REQUESTS_MAX, "listingRequests")?;
    check_range(
        options.transfer_requests,
        1,
        REQUESTS_MAX,
        "transferRequests",
    )?;
    check_range(
        options.window_kib,
        WINDOW_KIB_MIN,
        WINDOW_KIB_MAX,
        "windowKib",
    )?;
    Ok(())
}

/// Whether `jump` reads as `ProxyJump` does: comma-separated `[user@]host[:port]` hops.
fn valid_jump(jump: &str) -> bool {
    jump.split(',').all(|hop| {
        let hop = hop.trim();
        let host = hop.rsplit_once('@').map_or(hop, |(_, host)| host);
        let host = match host.rsplit_once(':') {
            Some((name, port)) if !name.contains(':') || name.ends_with(']') => {
                if port.parse::<u16>().map_or(true, |p| p == 0) {
                    return false;
                }
                name
            }
            _ => host,
        };
        !host.is_empty() && !host.contains(char::is_whitespace) && !host.contains('/')
    })
}

/// Checks a draft and brings it to its saved form. The address is read by `waypoint-path`, so a
/// draft saves only what the canonical form of D149 can write.
pub fn check_draft(draft: &ConnectionDraft) -> Result<Checked, DraftError> {
    let name = draft.name.trim().to_owned();
    if name.chars().count() > MAX_NAME_CHARS || name.chars().any(char::is_control) {
        return Err(DraftError::Name);
    }
    let scheme_name = draft.scheme.trim().to_ascii_lowercase();
    let scheme = RemoteScheme::from_name(&scheme_name).ok_or_else(|| DraftError::Scheme {
        scheme: scheme_name.clone(),
    })?;
    let host = draft.host.trim().to_owned();
    if host.is_empty()
        || host.len() > MAX_FIELD_BYTES
        || host.contains(|c: char| c.is_whitespace() || c.is_control() || "/@?#".contains(c))
    {
        return Err(DraftError::Host);
    }
    let user = blank_to_none(draft.user.clone());
    if too_long(&user) {
        return Err(DraftError::User);
    }
    if draft.port == Some(0) {
        return Err(DraftError::Port);
    }
    let key_file = blank_to_none(draft.key_file.clone());
    if too_long(&key_file)
        || key_file.as_ref().is_some_and(|path| {
            !(path.starts_with("~/") || std::path::Path::new(path).is_absolute())
        })
    {
        return Err(DraftError::KeyFile);
    }
    let jump_host = blank_to_none(draft.jump_host.clone());
    if too_long(&jump_host) || jump_host.as_deref().is_some_and(|jump| !valid_jump(jump)) {
        return Err(DraftError::JumpHost);
    }
    let start_folder = blank_to_none(draft.start_folder.clone());
    if too_long(&start_folder) || start_folder.as_ref().is_some_and(|f| !f.starts_with('/')) {
        return Err(DraftError::StartFolder);
    }
    check_options(&draft.options)?;

    let mut text = format!("{scheme_name}://");
    if let Some(user) = &user {
        text.push_str(&encode_user(user));
        text.push('@');
    }
    text.push_str(&bracket_host(&host));
    if let Some(port) = draft.port {
        text.push_str(&format!(":{port}"));
    }
    text.push('/');
    let (root, dropped) = match VfsPath::parse_input_reporting(&text) {
        Ok((VfsPath::Remote(root), dropped)) => (root, dropped),
        _ => return Err(DraftError::Host),
    };
    if dropped {
        return Err(DraftError::Password);
    }
    if root.endpoint().is_some() || root.scheme() != scheme {
        return Err(DraftError::Host);
    }
    let start = match &start_folder {
        Some(folder) => root.join(folder).map_err(|_| DraftError::StartFolder)?,
        None => root.clone(),
    };
    // The saved fields are the canonical ones, so two drafts of one server save the same.
    let authority = root.authority();
    let canonical = ConnectionDraft {
        name,
        scheme: scheme.as_str().to_owned(),
        host: host_text(&authority.host),
        port: authority.port,
        user: authority.user.clone(),
        auth: draft.auth,
        key_file,
        jump_host,
        start_folder: if start.is_root() {
            None
        } else {
            Some(server_path(&start))
        },
        options: draft.options.clone(),
    };
    Ok(Checked {
        draft: canonical,
        root,
        start,
    })
}

/// A host as the saved form writes it: a name lower-cased, an IPv6 address in brackets.
pub(crate) fn host_text(host: &Host) -> String {
    match host {
        Host::Name(name) => name.clone(),
        Host::Ipv4(addr) => addr.to_string(),
        Host::Ipv6 {
            addr,
            zone: Some(zone),
        } => format!("[{addr}%{zone}]"),
        Host::Ipv6 { addr, zone: None } => format!("[{addr}]"),
    }
}

/// The path part of a server location, for a saved start folder (`/srv/media`).
fn server_path(path: &RemotePath) -> String {
    let mut out = String::new();
    for segment in path.segments() {
        out.push('/');
        out.push_str(&String::from_utf8_lossy(segment));
    }
    if out.is_empty() {
        out.push('/');
    }
    out
}

impl SavedConnection {
    /// The connection's root and start locations. A saved connection was checked when it was
    /// saved, so this only fails for a document edited by hand, which loading leaves out.
    pub fn checked(&self) -> Result<Checked, DraftError> {
        check_draft(&self.draft)
    }

    pub fn key(&self) -> Option<ConnectionKey> {
        self.checked()
            .ok()
            .map(|checked| checked.root.connection_key())
    }

    /// The page's view of the connection.
    pub fn entry(&self) -> Option<ConnectionEntry> {
        let checked = self.checked().ok()?;
        let label = if self.draft.name.is_empty() {
            checked.root.root_label()
        } else {
            self.draft.name.clone()
        };
        Some(ConnectionEntry {
            connection: self.clone(),
            label,
            key: checked.root.connection_key().as_str().to_owned(),
            location: VfsPath::Remote(checked.start).to_location(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(host: &str) -> ConnectionDraft {
        ConnectionDraft {
            scheme: "sftp".into(),
            host: host.into(),
            ..ConnectionDraft::default()
        }
    }

    #[test]
    fn a_draft_saves_in_the_canonical_form() {
        let checked = check_draft(&ConnectionDraft {
            name: "  NAS ".into(),
            scheme: "SFTP".into(),
            host: "NAS.lan".into(),
            port: Some(22),
            user: Some(" me ".into()),
            start_folder: Some("/srv/./media/".into()),
            ..ConnectionDraft::default()
        })
        .unwrap();
        assert_eq!(checked.draft.name, "NAS");
        assert_eq!(checked.draft.scheme, "sftp");
        assert_eq!(checked.draft.host, "nas.lan");
        assert_eq!(checked.draft.port, None, "the default port is dropped");
        assert_eq!(checked.draft.user.as_deref(), Some("me"));
        assert_eq!(checked.draft.start_folder.as_deref(), Some("/srv/media"));
        assert_eq!(checked.root.connection_key().as_str(), "sftp://me@nas.lan");
        assert_eq!(
            VfsPath::Remote(checked.start).to_uri(),
            "sftp://me@nas.lan/srv/media"
        );
    }

    #[test]
    fn awkward_users_and_ipv6_hosts_round_trip() {
        let checked = check_draft(&ConnectionDraft {
            user: Some("a@b:c".into()),
            port: Some(2222),
            ..draft("fe80::1")
        })
        .unwrap();
        assert_eq!(checked.draft.user.as_deref(), Some("a@b:c"));
        assert_eq!(checked.draft.host, "[fe80::1]");
        assert_eq!(checked.draft.port, Some(2222));
        assert_eq!(
            checked.root.connection_key().as_str(),
            "sftp://a%40b%3Ac@[fe80::1]:2222"
        );
        // Saving the saved form again changes nothing.
        assert_eq!(check_draft(&checked.draft).unwrap().draft, checked.draft);
    }

    #[test]
    fn bad_fields_are_named() {
        assert!(matches!(check_draft(&draft("")), Err(DraftError::Host)));
        assert!(matches!(check_draft(&draft("a b")), Err(DraftError::Host)));
        assert!(matches!(
            check_draft(&ConnectionDraft {
                scheme: "ftp".into(),
                ..draft("h")
            }),
            Err(DraftError::Scheme { .. })
        ));
        assert!(matches!(
            check_draft(&ConnectionDraft {
                port: Some(0),
                ..draft("h")
            }),
            Err(DraftError::Port)
        ));
        assert!(matches!(
            check_draft(&ConnectionDraft {
                key_file: Some("id_rsa".into()),
                ..draft("h")
            }),
            Err(DraftError::KeyFile)
        ));
        assert!(check_draft(&ConnectionDraft {
            key_file: Some("~/.ssh/id_ed25519".into()),
            ..draft("h")
        })
        .is_ok());
        assert!(matches!(
            check_draft(&ConnectionDraft {
                jump_host: Some("bastion:0".into()),
                ..draft("h")
            }),
            Err(DraftError::JumpHost)
        ));
        assert!(check_draft(&ConnectionDraft {
            jump_host: Some("me@bastion:2200, [::1]:22".into()),
            ..draft("h")
        })
        .is_ok());
        assert!(matches!(
            check_draft(&ConnectionDraft {
                start_folder: Some("srv".into()),
                ..draft("h")
            }),
            Err(DraftError::StartFolder)
        ));
        assert!(matches!(
            check_draft(&ConnectionDraft {
                options: ConnectionOptions {
                    refresh_seconds: Some(1),
                    ..ConnectionOptions::default()
                },
                ..draft("h")
            }),
            Err(DraftError::Option { .. })
        ));
    }

    #[test]
    fn an_entry_derives_its_label_key_and_location() {
        let saved = SavedConnection {
            id: "c1".into(),
            draft: check_draft(&ConnectionDraft {
                user: Some("me".into()),
                start_folder: Some("/srv".into()),
                ..draft("nas.lan")
            })
            .unwrap()
            .draft,
        };
        let entry = saved.entry().unwrap();
        assert_eq!(entry.label, "me@nas.lan");
        assert_eq!(entry.key, "sftp://me@nas.lan");
        assert_eq!(entry.location.uri, "sftp://me@nas.lan/srv");
    }

    #[test]
    fn the_wire_form_is_flat_and_has_no_secret() {
        let saved = SavedConnection {
            id: "c1".into(),
            draft: draft("h"),
        };
        let json = serde_json::to_value(&saved).unwrap();
        assert_eq!(json["id"], "c1");
        assert_eq!(json["host"], "h");
        assert_eq!(json["auth"], "auto");
        assert!(json.get("password").is_none());
    }
}
