// From `s3://bucket/key?endpoint=…` locations to the bucket, the key and the service they name.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_path::{ConnectionKey, Endpoint, Host, RemotePath, RemoteScheme, VfsPath};
use waypoint_protocol::{Location, VfsError};

/// The longest key S3 accepts, in bytes.
pub(crate) const MAX_KEY_BYTES: usize = 1024;

/// What one `s3://` location names: a bucket on a service, and a key (or the prefix a folder is) in
/// it. The key has no trailing `/`; the root of a bucket has an empty one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Address {
    pub bucket: String,
    pub key: String,
    /// The service's origin (`https://minio.lan:9000`), or `None` for AWS.
    pub endpoint: Option<String>,
    pub connection: ConnectionKey,
    pub location: Location,
}

impl Address {
    pub(crate) fn is_bucket(&self) -> bool {
        self.key.is_empty()
    }

    /// The prefix the entries of this folder have: empty for the bucket, else the key and a `/`.
    pub(crate) fn prefix(&self) -> String {
        if self.key.is_empty() {
            String::new()
        } else {
            format!("{}/", self.key)
        }
    }

    /// The marker object that stands for this folder, when it is a folder.
    pub(crate) fn marker(&self) -> String {
        self.prefix()
    }

    /// The name of the entry: its last segment, or the bucket.
    pub(crate) fn name(&self) -> &str {
        if self.key.is_empty() {
            &self.bucket
        } else {
            self.key.rsplit('/').next().unwrap_or(&self.key)
        }
    }

    /// Whether `other` is on the same service (so a server-side copy can reach it).
    pub(crate) fn same_service(&self, other: &Address) -> bool {
        self.endpoint == other.endpoint
    }
}

/// The origin of an endpoint, as the SDK wants it.
pub fn endpoint_origin(endpoint: &Endpoint) -> String {
    let mut out = String::from(if endpoint.secure {
        "https://"
    } else {
        "http://"
    });
    match &endpoint.host {
        Host::Name(name) => out.push_str(name),
        Host::Ipv4(addr) => out.push_str(&addr.to_string()),
        Host::Ipv6 { addr, .. } => {
            out.push('[');
            out.push_str(&addr.to_string());
            out.push(']');
        }
    }
    if let Some(port) = endpoint.port {
        out.push(':');
        out.push_str(&port.to_string());
    }
    out
}

pub(crate) fn remote(path: &VfsPath) -> Result<&RemotePath, VfsError> {
    match path {
        VfsPath::Remote(remote) if remote.scheme() == RemoteScheme::S3 => Ok(remote),
        _ => Err(VfsError::Unsupported {
            what: format!("{} over S3", path.display()),
        }),
    }
}

/// The address of `path`, or `Unsupported` for another scheme and `InvalidName` for a key S3
/// cannot hold (not UTF-8, or longer than 1,024 bytes).
pub(crate) fn address(path: &VfsPath) -> Result<Address, VfsError> {
    let remote = remote(path)?;
    let bucket = match &remote.authority().host {
        Host::Name(name) => name.clone(),
        Host::Ipv4(addr) => addr.to_string(),
        Host::Ipv6 { addr, .. } => addr.to_string(),
    };
    let mut key = String::new();
    for segment in remote.segments() {
        let name = std::str::from_utf8(segment).map_err(|_| VfsError::InvalidName {
            name: String::from_utf8_lossy(segment).into_owned(),
            reason: "S3 keys that are not UTF-8 are not supported".to_owned(),
        })?;
        if !key.is_empty() {
            key.push('/');
        }
        key.push_str(name);
    }
    if key.len() > MAX_KEY_BYTES {
        return Err(VfsError::InvalidName {
            name: key.rsplit('/').next().unwrap_or(&key).to_owned(),
            reason: "an S3 key can be at most 1,024 bytes".to_owned(),
        });
    }
    Ok(Address {
        bucket,
        key,
        endpoint: remote.endpoint().map(endpoint_origin),
        connection: remote.connection_key(),
        location: path.to_location(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(uri: &str) -> Address {
        address(&VfsPath::from_uri(uri).unwrap()).unwrap()
    }

    #[test]
    fn a_location_names_a_bucket_a_key_and_a_service() {
        let a = at("s3://Photos/2026/a%20b.jpg");
        assert_eq!(a.bucket, "Photos");
        assert_eq!(a.key, "2026/a b.jpg");
        assert_eq!(a.endpoint, None);
        assert_eq!(a.prefix(), "2026/a b.jpg/");
        assert_eq!(a.name(), "a b.jpg");
        let m = at("s3://b/x?endpoint=http://minio.lan:9000");
        assert_eq!(m.endpoint.as_deref(), Some("http://minio.lan:9000"));
        let r = at("s3://b?endpoint=r2.example.com");
        assert_eq!(r.endpoint.as_deref(), Some("https://r2.example.com"));
        assert!(r.is_bucket());
        assert_eq!(r.prefix(), "");
        assert_eq!(r.name(), "b");
        assert!(!m.same_service(&a));
        assert!(a.same_service(&at("s3://other/k")));
    }

    #[test]
    fn keys_that_s3_cannot_hold_are_invalid_names() {
        let odd = VfsPath::from_uri("s3://b/%FF").unwrap();
        assert!(matches!(address(&odd), Err(VfsError::InvalidName { .. })));
        let long = VfsPath::from_uri(&format!("s3://b/{}", "a".repeat(1025))).unwrap();
        assert!(matches!(address(&long), Err(VfsError::InvalidName { .. })));
        let local = VfsPath::from_uri("file:///tmp").unwrap();
        assert!(matches!(address(&local), Err(VfsError::Unsupported { .. })));
    }
}
