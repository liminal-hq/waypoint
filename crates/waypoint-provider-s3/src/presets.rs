// The S3-compatible services Waypoint knows the shape of: their addresses and what they need.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// One service's URL shape and the settings a connection to it needs. The Connect dialog fills the
/// endpoint from a preset; the provider reads the same table to pick the addressing style and the
/// signing region of an endpoint it is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    /// The endpoint with `{value}` where the person's input goes, or `None` for AWS (which has
    /// no endpoint) and for a service whose address is entirely the person's (MinIO).
    pub endpoint_template: Option<&'static str>,
    /// What `{value}` is, as the dialog asks for it: `region`, `account id` or `host`.
    pub input: Option<&'static str>,
    /// The signing region when the address does not say one.
    pub default_region: &'static str,
    /// Whether the bucket goes in the path (`host/bucket/key`) instead of the host name: needed by
    /// a service with no wildcard DNS or certificate for buckets, and safe wherever it is accepted.
    pub path_style: bool,
    /// Whether a conditional `If-None-Match: *` write is honoured, so an exclusive create is
    /// atomic on the server instead of a check and a write.
    pub conditional_write: bool,
    /// The end of the host names of this service, to recognise an endpoint typed by hand.
    host_suffixes: &'static [&'static str],
}

pub const AWS: Preset = Preset {
    id: "aws",
    name: "Amazon S3",
    endpoint_template: None,
    input: None,
    default_region: "us-east-1",
    path_style: false,
    conditional_write: true,
    host_suffixes: &[".amazonaws.com", ".amazonaws.com.cn"],
};

pub const BACKBLAZE_B2: Preset = Preset {
    id: "b2",
    name: "Backblaze B2",
    endpoint_template: Some("https://s3.{value}.backblazeb2.com"),
    input: Some("region"),
    default_region: "us-west-004",
    path_style: true,
    conditional_write: false,
    host_suffixes: &[".backblazeb2.com"],
};

pub const CLOUDFLARE_R2: Preset = Preset {
    id: "r2",
    name: "Cloudflare R2",
    endpoint_template: Some("https://{value}.r2.cloudflarestorage.com"),
    input: Some("account id"),
    default_region: "auto",
    path_style: true,
    conditional_write: true,
    host_suffixes: &[".r2.cloudflarestorage.com"],
};

pub const WASABI: Preset = Preset {
    id: "wasabi",
    name: "Wasabi",
    endpoint_template: Some("https://s3.{value}.wasabisys.com"),
    input: Some("region"),
    default_region: "us-east-1",
    path_style: true,
    conditional_write: false,
    host_suffixes: &[".wasabisys.com"],
};

pub const MINIO: Preset = Preset {
    id: "minio",
    name: "MinIO",
    endpoint_template: None,
    input: Some("host"),
    default_region: "us-east-1",
    path_style: true,
    conditional_write: true,
    host_suffixes: &[],
};

pub const DIGITALOCEAN_SPACES: Preset = Preset {
    id: "spaces",
    name: "DigitalOcean Spaces",
    endpoint_template: Some("https://{value}.digitaloceanspaces.com"),
    input: Some("region"),
    default_region: "nyc3",
    path_style: true,
    conditional_write: false,
    host_suffixes: &[".digitaloceanspaces.com"],
};

/// What an endpoint that matches no preset is assumed to be: an S3-compatible server of unknown
/// make (MinIO, Ceph, `rclone serve s3`), addressed by path and without conditional writes.
pub const GENERIC: Preset = Preset {
    id: "custom",
    name: "S3-compatible service",
    endpoint_template: None,
    input: Some("host"),
    default_region: "us-east-1",
    path_style: true,
    conditional_write: false,
    host_suffixes: &[],
};

/// Every preset the Connect dialog offers, in the order it shows them.
pub const PRESETS: [Preset; 6] = [
    AWS,
    BACKBLAZE_B2,
    CLOUDFLARE_R2,
    WASABI,
    MINIO,
    DIGITALOCEAN_SPACES,
];

impl Preset {
    /// The preset called `id`.
    pub fn by_id(id: &str) -> Option<&'static Preset> {
        PRESETS.iter().find(|preset| preset.id == id)
    }

    /// The endpoint for the person's `value` (a region, an account id), or `None` when this
    /// preset has no template.
    pub fn endpoint_for(&self, value: &str) -> Option<String> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        self.endpoint_template
            .map(|template| template.replace("{value}", value))
    }

    /// The preset an endpoint's host belongs to, or `GENERIC` when none does. `None` is AWS.
    pub fn for_host(host: Option<&str>) -> &'static Preset {
        let Some(host) = host else { return &AWS };
        let host = host.to_ascii_lowercase();
        PRESETS
            .iter()
            .find(|preset| {
                preset
                    .host_suffixes
                    .iter()
                    .any(|suffix| host.ends_with(suffix))
            })
            .unwrap_or(&GENERIC)
    }

    /// The signing region an endpoint host says (`s3.eu-central-1.wasabisys.com`), if it does.
    pub fn region_in_host(&self, host: &str) -> Option<String> {
        let host = host.to_ascii_lowercase();
        let suffix = self
            .host_suffixes
            .iter()
            .find(|suffix| host.ends_with(**suffix))?;
        let head = host.strip_suffix(suffix)?;
        match self.id {
            "b2" | "wasabi" => head.strip_prefix("s3.").map(str::to_owned),
            "spaces" => (!head.is_empty() && !head.contains('.')).then(|| head.to_owned()),
            "r2" => Some("auto".to_owned()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_build_their_endpoints() {
        assert_eq!(
            BACKBLAZE_B2.endpoint_for("us-west-004").as_deref(),
            Some("https://s3.us-west-004.backblazeb2.com")
        );
        assert_eq!(
            CLOUDFLARE_R2.endpoint_for(" abc123 ").as_deref(),
            Some("https://abc123.r2.cloudflarestorage.com")
        );
        assert_eq!(
            WASABI.endpoint_for("eu-central-1").as_deref(),
            Some("https://s3.eu-central-1.wasabisys.com")
        );
        assert_eq!(
            DIGITALOCEAN_SPACES.endpoint_for("ams3").as_deref(),
            Some("https://ams3.digitaloceanspaces.com")
        );
        assert_eq!(AWS.endpoint_for("x"), None);
        assert_eq!(MINIO.endpoint_for("x"), None);
        assert_eq!(WASABI.endpoint_for(""), None);
        assert_eq!(Preset::by_id("r2"), Some(&CLOUDFLARE_R2));
        assert_eq!(Preset::by_id("nope"), None);
    }

    #[test]
    fn an_endpoint_host_finds_its_preset_and_region() {
        assert_eq!(Preset::for_host(None).id, "aws");
        assert_eq!(
            Preset::for_host(Some("s3.eu-west-1.amazonaws.com")).id,
            "aws"
        );
        assert_eq!(Preset::for_host(Some("minio.lan")).id, "custom");
        assert!(Preset::for_host(Some("minio.lan")).path_style);
        let b2 = Preset::for_host(Some("S3.us-west-004.backblazeb2.com"));
        assert_eq!(b2.id, "b2");
        assert_eq!(
            b2.region_in_host("s3.us-west-004.backblazeb2.com")
                .as_deref(),
            Some("us-west-004")
        );
        let spaces = Preset::for_host(Some("nyc3.digitaloceanspaces.com"));
        assert_eq!(
            spaces
                .region_in_host("nyc3.digitaloceanspaces.com")
                .as_deref(),
            Some("nyc3")
        );
        assert_eq!(
            CLOUDFLARE_R2
                .region_in_host("abc.r2.cloudflarestorage.com")
                .as_deref(),
            Some("auto")
        );
        assert_eq!(GENERIC.region_in_host("minio.lan"), None);
    }
}
