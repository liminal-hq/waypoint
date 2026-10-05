// What a connection to an S3 service is configured with, beyond its address.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use aws_sdk_s3::config::SharedHttpClient;
use waypoint_vfs::{CredentialSource, Secret};

/// The smallest part S3 accepts in a multipart upload (every part but the last).
pub const MIN_PART_SIZE: u64 = 5 * 1024 * 1024;
/// The part size used when nothing says otherwise.
pub const DEFAULT_PART_SIZE: u64 = 8 * 1024 * 1024;
/// The largest object `CopyObject` copies in one request; bigger ones are copied in parts.
pub const MAX_SINGLE_COPY: u64 = 5 * 1024 * 1024 * 1024 - 1;

/// The tuning and the extra login details of one connection. Everything has a default, so a
/// connection that sets nothing works against AWS and against most S3-compatible services.
#[derive(Debug, Clone, Default)]
pub struct S3Options {
    /// The signing region. Without it AWS buckets start in `us-east-1` and follow the region the
    /// service names, and another service's preset (or `us-east-1`) is used.
    pub region: Option<String>,
    /// The session token of temporary credentials. It goes with the access key the credential
    /// source gives; it is never stored by this crate.
    pub session_token: Option<Secret>,
    /// Addresses buckets in the path instead of the host name; `None` follows the service's preset
    /// (everything but AWS is addressed by path).
    pub path_style: Option<bool>,
    /// Send no credentials at all, for a public bucket.
    pub anonymous: bool,
    /// Retry a request in the region the service names when the bucket is elsewhere. `None`
    /// follows the preset (AWS does; the other services have one region).
    pub follow_region_redirects: Option<bool>,
    /// Whether the service honours `If-None-Match: *` on a write; `None` follows the preset.
    pub conditional_writes: Option<bool>,
    /// The size of the parts of a multipart upload (at least 5 MiB; larger uploads grow it so
    /// they stay under S3's 10,000 parts). Default 8 MiB.
    pub part_size: Option<u64>,
    /// Parts of one upload sent at once. Default 4.
    pub parts_in_flight: Option<usize>,
    /// Attempts per request, retries included. Default 4.
    pub max_attempts: Option<u32>,
    /// Objects above this many bytes are copied on the server in parts. Default: the 5 GiB
    /// limit of `CopyObject`.
    pub copy_part_threshold: Option<u64>,
}

/// What an `S3Provider` is built from.
#[derive(Clone)]
pub struct S3Config {
    /// Where the access key and secret come from (the app's session cache and keyring).
    pub credentials: Arc<dyn CredentialSource>,
    /// The options of a connection that has none of its own.
    pub options: S3Options,
    /// Offer the AWS environment variables and shared files (`ambient`) to AWS endpoints when the
    /// source has no credential. Off unless the app turns it on.
    pub ambient_credentials: bool,
    pub(crate) http_client: Option<SharedHttpClient>,
}

impl S3Config {
    pub fn new(credentials: Arc<dyn CredentialSource>) -> Self {
        Self {
            credentials,
            options: S3Options::default(),
            ambient_credentials: false,
            http_client: None,
        }
    }

    pub fn with_options(mut self, options: S3Options) -> Self {
        self.options = options;
        self
    }

    /// Also read the AWS environment and `~/.aws` files for AWS endpoints (see `ambient`).
    pub fn with_ambient_credentials(mut self) -> Self {
        self.ambient_credentials = true;
        self
    }

    /// Replaces the HTTP client, which a test uses to replay recorded responses.
    #[doc(hidden)]
    pub fn with_http_client(mut self, client: SharedHttpClient) -> Self {
        self.http_client = Some(client);
        self
    }
}

impl S3Options {
    pub(crate) fn part_size(&self) -> u64 {
        self.part_size
            .unwrap_or(DEFAULT_PART_SIZE)
            .max(MIN_PART_SIZE)
    }

    pub(crate) fn parts_in_flight(&self) -> usize {
        self.parts_in_flight.unwrap_or(4).clamp(1, 16)
    }
}

/// The size of part number `index` (counting from zero): the base size, doubled for every thousand
/// parts, so 10,000 parts reach at least 4 TiB of the 5 TiB S3 allows and an ordinary file stays in
/// the base size.
pub(crate) fn part_size_for(base: u64, index: u64) -> u64 {
    base.saturating_mul(1 << (index / 1000).min(9))
        .min(5 * 1024 * 1024 * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_grow_so_the_part_limit_is_never_the_cap() {
        assert_eq!(part_size_for(DEFAULT_PART_SIZE, 0), DEFAULT_PART_SIZE);
        assert_eq!(part_size_for(DEFAULT_PART_SIZE, 999), DEFAULT_PART_SIZE);
        assert_eq!(
            part_size_for(DEFAULT_PART_SIZE, 1000),
            2 * DEFAULT_PART_SIZE
        );
        let mut total = 0u64;
        for index in 0..10_000 {
            total += part_size_for(DEFAULT_PART_SIZE, index);
        }
        assert!(total > 4 * 1024 * 1024 * 1024 * 1024);
    }

    #[test]
    fn a_part_is_never_smaller_than_s3_allows() {
        let options = S3Options {
            part_size: Some(1),
            ..S3Options::default()
        };
        assert_eq!(options.part_size(), MIN_PART_SIZE);
        assert_eq!(S3Options::default().part_size(), DEFAULT_PART_SIZE);
    }
}
