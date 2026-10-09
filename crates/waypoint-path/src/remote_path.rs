// Locations on a server (`sftp`, `smb`, `dav`, `davs` and `s3`): parsing, the canonical URI, the
// form people read and the key of the login a location belongs to.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};

use crate::encoding::encode_into;
use crate::segments::{self, Segment};
use crate::{CaseRule, PathError};

/// The protocol of a server location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RemoteScheme {
    Sftp,
    Smb,
    /// WebDAV over plain HTTP.
    Dav,
    /// WebDAV over HTTPS.
    Davs,
    S3,
}

impl RemoteScheme {
    pub const ALL: [RemoteScheme; 5] = [
        RemoteScheme::Sftp,
        RemoteScheme::Smb,
        RemoteScheme::Dav,
        RemoteScheme::Davs,
        RemoteScheme::S3,
    ];

    /// The scheme as it is written in a canonical URI.
    pub fn as_str(self) -> &'static str {
        match self {
            RemoteScheme::Sftp => "sftp",
            RemoteScheme::Smb => "smb",
            RemoteScheme::Dav => "dav",
            RemoteScheme::Davs => "davs",
            RemoteScheme::S3 => "s3",
        }
    }

    /// Reads a scheme name in any letter case; `webdav` and `webdavs` are other spellings of `dav`
    /// and `davs`.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "sftp" => Some(RemoteScheme::Sftp),
            "smb" => Some(RemoteScheme::Smb),
            "dav" | "webdav" => Some(RemoteScheme::Dav),
            "davs" | "webdavs" => Some(RemoteScheme::Davs),
            "s3" => Some(RemoteScheme::S3),
            _ => None,
        }
    }

    /// The port a URI leaves out. S3 has none: its endpoint carries one.
    pub fn default_port(self) -> Option<u16> {
        match self {
            RemoteScheme::Sftp => Some(22),
            RemoteScheme::Smb => Some(445),
            RemoteScheme::Dav => Some(80),
            RemoteScheme::Davs => Some(443),
            RemoteScheme::S3 => None,
        }
    }

    /// How the protocol's servers usually compare names. A provider reports what its server really
    /// does; this is the common case.
    pub fn case_rule(self) -> CaseRule {
        match self {
            RemoteScheme::Smb => CaseRule::Insensitive,
            _ => CaseRule::Sensitive,
        }
    }
}

impl fmt::Display for RemoteScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A server's address.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Host {
    /// A name, lower-cased (an S3 bucket is kept as written). A name that is not ASCII stays
    /// Unicode; converting it to punycode is the provider's job.
    Name(String),
    Ipv4(Ipv4Addr),
    /// An IPv6 address with its zone (`fe80::1%eth0`), if any.
    Ipv6 {
        addr: Ipv6Addr,
        zone: Option<String>,
    },
}

impl Host {
    fn render(&self, out: &mut String) {
        match self {
            Host::Name(name) => encode_into(out, name.as_bytes(), &[]),
            Host::Ipv4(addr) => out.push_str(&addr.to_string()),
            Host::Ipv6 { addr, zone } => {
                out.push('[');
                out.push_str(&addr.to_string());
                if let Some(zone) = zone {
                    out.push_str("%25");
                    encode_into(out, zone.as_bytes(), &[]);
                }
                out.push(']');
            }
        }
    }

    fn display(&self) -> String {
        match self {
            Host::Name(name) => name.clone(),
            Host::Ipv4(addr) => addr.to_string(),
            Host::Ipv6 {
                addr,
                zone: Some(zone),
            } => format!("[{addr}%{zone}]"),
            Host::Ipv6 { addr, zone: None } => format!("[{addr}]"),
        }
    }
}

/// Who logs in where: the user (for SMB `domain;user`), the host and a port other than the
/// scheme's default. Never a password.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Authority {
    pub user: Option<String>,
    pub host: Host,
    pub port: Option<u16>,
}

impl Authority {
    fn render(&self, out: &mut String) {
        if let Some(user) = &self.user {
            encode_into(out, user.as_bytes(), b";");
            out.push('@');
        }
        self.host.render(out);
        if let Some(port) = self.port {
            out.push(':');
            out.push_str(&port.to_string());
        }
    }

    fn display(&self) -> String {
        let mut out = String::new();
        if let Some(user) = &self.user {
            out.push_str(user);
            out.push('@');
        }
        out.push_str(&self.host.display());
        if let Some(port) = self.port {
            out.push(':');
            out.push_str(&port.to_string());
        }
        out
    }
}

/// Where an S3-compatible service is, when it is not AWS: an origin such as
/// `https://minio.lan:9000`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Endpoint {
    /// HTTPS, the default, or plain HTTP.
    pub secure: bool,
    pub host: Host,
    /// A port other than 443 (HTTPS) or 80 (HTTP).
    pub port: Option<u16>,
}

impl Endpoint {
    fn origin(&self) -> String {
        let mut out = String::from(if self.secure { "https://" } else { "http://" });
        self.host.render(&mut out);
        if let Some(port) = self.port {
            out.push(':');
            out.push_str(&port.to_string());
        }
        out
    }

    fn display(&self) -> String {
        let mut out = String::new();
        if !self.secure {
            out.push_str("http://");
        }
        out.push_str(&self.host.display());
        if let Some(port) = self.port {
            out.push(':');
            out.push_str(&port.to_string());
        }
        out
    }
}

/// The part of a server location that names one login: `scheme://[user@]host[:port]`, plus the
/// endpoint for S3. It keys the session pool, the saved connection and the credential in the
/// keyring, and holds no secret.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectionKey(String);

impl ConnectionKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The one connection to the elevated helper: there is a single helper per app, whatever the
    /// path, so every `admin:` location shares this key.
    pub fn elevated() -> Self {
        Self(format!("{}:", crate::ELEVATED_SCHEME))
    }
}

impl fmt::Display for ConnectionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A location on a server. The path is a list of names (bytes, so a name that is not UTF-8
/// survives); for SMB the first name is the share, and an empty path is the share browser.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RemotePath {
    scheme: RemoteScheme,
    authority: Authority,
    segments: Vec<Segment>,
    endpoint: Option<Endpoint>,
}

/// What reading text found besides the path.
pub(crate) struct Parsed {
    pub path: RemotePath,
    /// A password was written in the text and has been dropped.
    pub had_password: bool,
}

fn invalid(why: &'static str) -> PathError {
    PathError::InvalidUri(why)
}

fn decode_utf8(text: &str, lenient: bool, why: &'static str) -> Result<String, PathError> {
    let bytes = segments::decode_text(text, lenient)?;
    let text = String::from_utf8(bytes).map_err(|_| invalid(why))?;
    if text.chars().any(char::is_control) {
        return Err(invalid(why));
    }
    Ok(text)
}

fn parse_port(text: &str, default: Option<u16>) -> Result<Option<u16>, PathError> {
    if text.is_empty() {
        return Ok(None);
    }
    let port: u16 = text
        .parse()
        .ok()
        .filter(|port| *port != 0 && text.bytes().all(|b| b.is_ascii_digit()))
        .ok_or(invalid("the port is not a number from 1 to 65535"))?;
    Ok((Some(port) != default).then_some(port))
}

/// Reads `host[:port]`, `[v6]` or `[v6%25zone]:port`.
fn parse_host_port(
    text: &str,
    lenient: bool,
    verbatim: bool,
    default_port: Option<u16>,
) -> Result<(Host, Option<u16>), PathError> {
    if let Some(inner) = text.strip_prefix('[') {
        let close = inner
            .find(']')
            .ok_or(invalid("an IPv6 address is not closed with `]`"))?;
        let (literal, after) = (&inner[..close], &inner[close + 1..]);
        let port = match after.strip_prefix(':') {
            Some(port) => parse_port(port, default_port)?,
            None if after.is_empty() => None,
            None => return Err(invalid("text follows an IPv6 address")),
        };
        let (addr, zone) = match literal.find("%25").or_else(|| {
            // Typed text may write the zone with a bare `%`.
            lenient.then(|| literal.find('%')).flatten()
        }) {
            Some(at) => {
                let skip = if literal[at..].starts_with("%25") {
                    3
                } else {
                    1
                };
                let zone = decode_utf8(&literal[at + skip..], lenient, "the zone is not text")?;
                if zone.is_empty() {
                    return Err(invalid("an IPv6 zone is empty"));
                }
                (&literal[..at], Some(zone))
            }
            None => (literal, None),
        };
        let addr: Ipv6Addr = addr.parse().map_err(|_| invalid("not an IPv6 address"))?;
        return Ok((Host::Ipv6 { addr, zone }, port));
    }
    let (host, port) = match text.rsplit_once(':') {
        Some((host, port)) => (host, parse_port(port, default_port)?),
        None => (text, None),
    };
    let name = decode_utf8(host, lenient, "the host is not text")?;
    if name.is_empty() {
        return Err(invalid("a server location needs a host"));
    }
    if name
        .chars()
        .any(|c| c.is_whitespace() || "/?#@[]:\\".contains(c))
    {
        return Err(invalid("the host holds a character a host cannot"));
    }
    if !verbatim {
        if let Ok(addr) = name.parse::<Ipv4Addr>() {
            return Ok((Host::Ipv4(addr), port));
        }
        return Ok((Host::Name(name.to_lowercase()), port));
    }
    Ok((Host::Name(name), port))
}

fn parse_authority(
    text: &str,
    scheme: RemoteScheme,
    lenient: bool,
) -> Result<(Authority, bool), PathError> {
    let (userinfo, host_port) = match text.rsplit_once('@') {
        Some((userinfo, host_port)) => (Some(userinfo), host_port),
        None => (None, text),
    };
    let mut had_password = false;
    let user = match userinfo {
        None => None,
        Some(userinfo) => {
            let (user, password) = match userinfo.split_once(':') {
                Some((user, password)) => (user, Some(password)),
                None => (userinfo, None),
            };
            if password.is_some_and(|password| !password.is_empty()) {
                if !lenient {
                    return Err(PathError::PasswordInUri);
                }
                had_password = true;
            }
            let user = decode_utf8(user, lenient, "the user name is not text")?;
            if user.is_empty() {
                return Err(invalid("the user name is empty"));
            }
            Some(user)
        }
    };
    let verbatim = scheme == RemoteScheme::S3;
    let (host, port) = parse_host_port(host_port, lenient, verbatim, scheme.default_port())?;
    if scheme == RemoteScheme::S3 && (user.is_some() || port.is_some()) {
        return Err(invalid(
            "an S3 location is a bucket; its user and port belong to the connection",
        ));
    }
    Ok((Authority { user, host, port }, had_password))
}

fn parse_endpoint(text: &str) -> Result<Endpoint, PathError> {
    let lower = text.to_ascii_lowercase();
    let (secure, rest) = if lower.starts_with("https://") {
        (true, &text[8..])
    } else if lower.starts_with("http://") {
        (false, &text[7..])
    } else {
        (true, text)
    };
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    if rest.contains('/') || rest.contains('@') {
        return Err(invalid(
            "an S3 endpoint is an origin: scheme, host and port",
        ));
    }
    let default = if secure { 443 } else { 80 };
    let (host, port) = parse_host_port(rest, true, false, Some(default))?;
    Ok(Endpoint { secure, host, port })
}

impl RemotePath {
    /// The root of a server: `/` on SFTP and WebDAV, the share browser on SMB, the bucket on S3.
    pub fn root(scheme: RemoteScheme, authority: Authority) -> Self {
        Self {
            scheme,
            authority,
            segments: Vec::new(),
            endpoint: None,
        }
    }

    /// Reads a canonical URI (or any well-formed one) strictly: a password, a fragment or an
    /// unknown query is refused.
    pub fn from_uri(uri: &str) -> Result<Self, PathError> {
        Self::parse(uri, false).map(|parsed| parsed.path)
    }

    /// Reads text a person typed: a password is dropped (see `had_password`), a `%` that does not
    /// start an escape is kept, and `#` and `?` are part of names unless `?endpoint=` follows an S3
    /// path.
    pub(crate) fn parse(text: &str, lenient: bool) -> Result<Parsed, PathError> {
        let (scheme_name, rest) = text
            .split_once("://")
            .ok_or(invalid("a server location starts with `scheme://`"))?;
        let scheme = RemoteScheme::from_name(scheme_name)
            .ok_or_else(|| PathError::UnsupportedScheme(scheme_name.to_ascii_lowercase()))?;
        let (rest, query) = if lenient {
            match rest.rfind("?endpoint=") {
                Some(at) if scheme == RemoteScheme::S3 => (&rest[..at], Some(&rest[at + 1..])),
                _ => (rest, None),
            }
        } else {
            if rest.contains('#') {
                return Err(invalid("a location has no fragment"));
            }
            match rest.split_once('?') {
                Some((rest, query)) => (rest, Some(query)),
                None => (rest, None),
            }
        };
        let (authority, path) = match rest.find('/') {
            Some(at) => (&rest[..at], &rest[at..]),
            None => (rest, ""),
        };
        let (authority, had_password) = parse_authority(authority, scheme, lenient)?;
        let segments = segments::parse_path(path, lenient)?;
        let endpoint = match query {
            None => None,
            Some(_) if scheme != RemoteScheme::S3 => {
                return Err(invalid("this location takes no query"));
            }
            Some(query) => {
                let value = query
                    .strip_prefix("endpoint=")
                    .filter(|value| !value.contains('&'))
                    .ok_or(invalid("an S3 location takes only `?endpoint=`"))?;
                let value = decode_utf8(value, lenient, "the endpoint is not text")?;
                Some(parse_endpoint(&value)?)
            }
        };
        Ok(Parsed {
            path: Self {
                scheme,
                authority,
                segments,
                endpoint,
            },
            had_password,
        })
    }

    pub fn scheme(&self) -> RemoteScheme {
        self.scheme
    }

    pub fn authority(&self) -> &Authority {
        &self.authority
    }

    /// The S3 service, when it is not AWS.
    pub fn endpoint(&self) -> Option<&Endpoint> {
        self.endpoint.as_ref()
    }

    /// The names from the root, as bytes.
    pub fn segments(&self) -> &[Vec<u8>] {
        &self.segments
    }

    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    /// The containing folder, or `None` at the root.
    pub fn parent(&self) -> Option<Self> {
        let (_, rest) = self.segments.split_last()?;
        Some(Self {
            segments: rest.to_vec(),
            ..self.clone()
        })
    }

    /// Joins a relative name or path (`..` allowed); one starting with `/` replaces the path.
    pub fn join(&self, child: impl AsRef<OsStr>) -> Result<Self, PathError> {
        let child = segments::os_bytes(child.as_ref())?;
        Ok(Self {
            segments: segments::join(&self.segments, &child)?,
            ..self.clone()
        })
    }

    /// The last name, or `None` at the root.
    pub fn file_name(&self) -> Option<OsString> {
        self.segments.last().map(|name| segments::os_string(name))
    }

    /// The login this location belongs to.
    pub fn connection_key(&self) -> ConnectionKey {
        let mut out = format!("{}://", self.scheme);
        self.authority.render(&mut out);
        self.render_query(&mut out);
        ConnectionKey(out)
    }

    fn render_query(&self, out: &mut String) {
        if let Some(endpoint) = &self.endpoint {
            out.push_str("?endpoint=");
            encode_into(out, endpoint.origin().as_bytes(), &[]);
        }
    }

    /// The canonical URI.
    pub fn to_uri(&self) -> String {
        let mut out = format!("{}://", self.scheme);
        self.authority.render(&mut out);
        out.push_str(&segments::render(&self.segments));
        self.render_query(&mut out);
        out
    }

    /// The server and folder for people, without a password (there never is one) and with the
    /// names decoded.
    pub fn display(&self) -> String {
        let mut out = format!(
            "{}://{}{}",
            self.scheme,
            self.authority.display(),
            segments::display(&self.segments)
        );
        if let Some(endpoint) = &self.endpoint {
            out.push_str(&format!(" ({})", endpoint.display()));
        }
        out
    }

    /// The label of the root's breadcrumb: `user@host[:port]`, or the bucket.
    pub fn root_label(&self) -> String {
        self.authority.display()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canon(text: &str) -> String {
        RemotePath::parse(text, true).unwrap().path.to_uri()
    }

    #[test]
    fn each_scheme_has_one_canonical_form() {
        assert_eq!(
            canon("SFTP://Me@NAS.lan:22/srv/./media/"),
            "sftp://Me@nas.lan/srv/media"
        );
        assert_eq!(canon("sftp://nas.lan:2222"), "sftp://nas.lan:2222/");
        assert_eq!(
            canon("smb://WORK;me@Files:445/Projects/a b"),
            "smb://WORK;me@files/Projects/a%20b"
        );
        assert_eq!(
            canon("webdavs://cloud.example.com:443/remote.php/dav/"),
            "davs://cloud.example.com/remote.php/dav"
        );
        assert_eq!(canon("webdav://h:80/x"), "dav://h/x");
        assert_eq!(canon("davs://h:80/x"), "davs://h:80/x");
        assert_eq!(canon("s3://Photos/2026/"), "s3://Photos/2026");
        assert_eq!(
            canon("s3://photos/2026?endpoint=http://MinIO.lan:9000/"),
            "s3://photos/2026?endpoint=http%3A%2F%2Fminio.lan%3A9000"
        );
        assert_eq!(
            canon("s3://b?endpoint=r2.example.com:443"),
            "s3://b/?endpoint=https%3A%2F%2Fr2.example.com"
        );
    }

    #[test]
    fn canonical_uris_read_back_to_the_same_path() {
        for text in [
            "sftp://me@nas.lan/srv/media",
            "sftp://[fe80::1%25eth0]:2222/a",
            "sftp://[2001:db8::1]/",
            "sftp://10.0.0.7/x%23y%3Fz%25",
            "smb://WORK;me@files/Projects",
            "smb://files/",
            "dav://h/x",
            "davs://cloud.example.com/remote.php/dav/files/me",
            "s3://photos/2026?endpoint=https%3A%2F%2Fminio.lan%3A9000",
            "sftp://h/caf%E9/%FF%FE",
            "sftp://us%40er@h/",
        ] {
            let path = RemotePath::from_uri(text).unwrap();
            assert_eq!(path.to_uri(), text, "{text}");
            assert_eq!(RemotePath::from_uri(&path.to_uri()).unwrap(), path);
        }
    }

    #[test]
    fn hosts_are_written_in_a_standard_form() {
        assert_eq!(
            canon("sftp://[2001:DB8:0:0:0:0:0:1]/"),
            "sftp://[2001:db8::1]/"
        );
        assert_eq!(canon("sftp://[fe80::1%eth0]/"), "sftp://[fe80::1%25eth0]/");
        assert_eq!(canon("sftp://192.168.1.1:22/"), "sftp://192.168.1.1/");
        assert_eq!(
            canon("sftp://BÜCHER.example/"),
            "sftp://b%C3%BCcher.example/"
        );
        let path = RemotePath::from_uri("sftp://[fe80::1%25eth0]:2222/a").unwrap();
        assert_eq!(path.display(), "sftp://[fe80::1%eth0]:2222/a");
    }

    #[test]
    fn a_password_is_dropped_from_typed_text_and_refused_in_a_uri() {
        let parsed = RemotePath::parse("sftp://me:hunter2@host/x", true).unwrap();
        assert!(parsed.had_password);
        assert_eq!(parsed.path.to_uri(), "sftp://me@host/x");
        assert!(!parsed.path.display().contains("hunter2"));
        assert!(!parsed.path.connection_key().as_str().contains("hunter2"));
        // A password with `@` in it still never survives.
        let parsed = RemotePath::parse("sftp://me:p@ss@host/x", true).unwrap();
        assert!(parsed.had_password);
        assert!(!parsed.path.to_uri().contains("ss"));
        assert_eq!(
            RemotePath::from_uri("sftp://me:hunter2@host/x"),
            Err(PathError::PasswordInUri)
        );
        // An empty password is no password.
        assert_eq!(
            RemotePath::from_uri("sftp://me:@host/x").unwrap().to_uri(),
            "sftp://me@host/x"
        );
    }

    #[test]
    fn typed_text_keeps_awkward_characters_as_names() {
        let parsed = RemotePath::parse("sftp://h/100%/what?#.txt", true).unwrap();
        assert_eq!(parsed.path.to_uri(), "sftp://h/100%25/what%3F%23.txt");
        assert_eq!(parsed.path.display(), "sftp://h/100%/what?#.txt");
        // Strictly, the same text is malformed.
        assert!(RemotePath::from_uri("sftp://h/what?x").is_err());
        assert!(RemotePath::from_uri("sftp://h/a#b").is_err());
        assert!(RemotePath::from_uri("sftp://h/100%").is_err());
    }

    #[test]
    fn nonsense_is_refused() {
        for text in [
            "sftp://",
            "sftp:///x",
            "sftp://@h/",
            "sftp://h:0/",
            "sftp://h:99999/",
            "sftp://h:2x/",
            "sftp://[::1/",
            "sftp://[nonsense]/",
            "sftp://[::1]x/",
            "sftp://h%20x/",
            "s3://me@bucket/",
            "s3://bucket:9000/",
            "s3://bucket/?region=x",
            "s3://bucket/?endpoint=https%3A%2F%2Fh%2Fpath",
            "smb://h/a%2Fb",
            "smb://h/a%00",
        ] {
            assert!(RemotePath::from_uri(text).is_err(), "{text}");
        }
        assert_eq!(
            RemotePath::from_uri("ftp://h/"),
            Err(PathError::UnsupportedScheme("ftp".to_owned()))
        );
    }

    #[test]
    fn walking_up_and_down_stays_on_the_server() {
        let path = RemotePath::from_uri("smb://files/Projects/a").unwrap();
        let share = path.parent().unwrap();
        assert_eq!(share.to_uri(), "smb://files/Projects");
        let browser = share.parent().unwrap();
        assert!(browser.is_root());
        assert_eq!(browser.parent(), None);
        assert_eq!(browser.join("Projects/a").unwrap(), path);
        assert_eq!(
            path.join("../b").unwrap().to_uri(),
            "smb://files/Projects/b"
        );
        assert_eq!(path.join("/Other").unwrap().to_uri(), "smb://files/Other");
        assert_eq!(path.file_name().unwrap(), "a");
        assert_eq!(browser.file_name(), None);
    }

    #[test]
    fn the_connection_key_names_the_login_only() {
        let a = RemotePath::from_uri("sftp://me@nas/srv/a").unwrap();
        let b = RemotePath::parse("SFTP://me@NAS:22/home", true)
            .unwrap()
            .path;
        assert_eq!(a.connection_key(), b.connection_key());
        assert_eq!(a.connection_key().as_str(), "sftp://me@nas");
        let other_user = RemotePath::from_uri("sftp://you@nas/srv/a").unwrap();
        assert_ne!(a.connection_key(), other_user.connection_key());
        let s3 = RemotePath::from_uri("s3://b/k?endpoint=https%3A%2F%2Fm%3A9000").unwrap();
        assert_eq!(
            s3.connection_key().as_str(),
            "s3://b?endpoint=https%3A%2F%2Fm%3A9000"
        );
    }

    #[test]
    fn people_read_the_server_and_the_folder() {
        let path = RemotePath::from_uri("smb://WORK;me@files/My%20Projects").unwrap();
        assert_eq!(path.display(), "smb://WORK;me@files/My Projects");
        assert_eq!(path.root_label(), "WORK;me@files");
        let s3 = RemotePath::parse("s3://photos/2026?endpoint=minio.lan:9000", true)
            .unwrap()
            .path;
        assert_eq!(s3.display(), "s3://photos/2026 (minio.lan:9000)");
        assert_eq!(s3.root_label(), "photos");
    }

    #[test]
    fn schemes_know_their_ports_and_case_rules() {
        for scheme in RemoteScheme::ALL {
            assert_eq!(RemoteScheme::from_name(scheme.as_str()), Some(scheme));
        }
        assert_eq!(RemoteScheme::Smb.case_rule(), CaseRule::Insensitive);
        assert_eq!(RemoteScheme::Sftp.case_rule(), CaseRule::Sensitive);
        assert_eq!(RemoteScheme::S3.default_port(), None);
        assert_eq!(RemoteScheme::from_name("WebDAVs"), Some(RemoteScheme::Davs));
    }
}
