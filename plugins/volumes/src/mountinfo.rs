// The pure parser of `/proc/self/mountinfo` and its mapping to volumes
//
// UDisks2 manages the mounts of block devices. Everything else a person sees as a volume (an NFS or SMB share, an `sshfs` folder, any other FUSE mount) is only in the kernel's mount table, and so is every mount when there is no UDisks2 to ask.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]

use std::collections::BTreeMap;

use crate::models::{Volume, VolumeKind};

/// The prefix of the id of a volume made from the mount table; the rest is the mount point.
pub const ID_PREFIX: &str = "mount:";

/// One line of `/proc/self/mountinfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountEntry {
    pub mount_id: u32,
    pub parent_id: u32,
    /// `major:minor` of the device.
    pub device: String,
    /// The part of the file system that is mounted (`/` for a whole file system, `/@home` for a btrfs subvolume, a folder for a bind mount).
    pub root: String,
    pub mount_point: String,
    pub options: String,
    pub fs_type: String,
    /// The device or remote the kernel was told to mount (`/dev/sda1`, `server:/export`, `//server/share`).
    pub source: String,
    pub super_options: String,
}

/// Undoes the kernel's octal escapes (`\040` for a space, `\011`, `\012` and `\134` for a tab, a newline and a backslash).
pub fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 4 <= bytes.len() {
            let digits = &bytes[i + 1..i + 4];
            if digits.iter().all(|d| (b'0'..=b'7').contains(d)) {
                let value = (u32::from(digits[0] - b'0') << 6)
                    | (u32::from(digits[1] - b'0') << 3)
                    | u32::from(digits[2] - b'0');
                if value <= 0xff {
                    out.push(value as u8);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Parses one line: `36 35 98:0 /mnt1 /mnt2 rw,noatime master:1 - ext3 /dev/root rw`. The optional fields between the options and the `-` separator are skipped; a line that does not have the shape is `None`.
pub fn parse_line(line: &str) -> Option<MountEntry> {
    let mut fields = line.split_ascii_whitespace();
    let mount_id = fields.next()?.parse().ok()?;
    let parent_id = fields.next()?.parse().ok()?;
    let device = fields.next()?.to_string();
    let root = unescape(fields.next()?);
    let mount_point = unescape(fields.next()?);
    let options = fields.next()?.to_string();
    // Zero or more optional fields (`shared:1`, `master:2`, …) end at a lone `-`.
    loop {
        if fields.next()? == "-" {
            break;
        }
    }
    let fs_type = fields.next()?.to_string();
    let source = unescape(fields.next()?);
    let super_options = fields.next().unwrap_or("").to_string();
    Some(MountEntry {
        mount_id,
        parent_id,
        device,
        root,
        mount_point,
        options,
        fs_type,
        source,
        super_options,
    })
}

/// Parses the whole table; lines that are not mount entries are skipped.
pub fn parse(text: &str) -> Vec<MountEntry> {
    text.lines().filter_map(parse_line).collect()
}

/// What a mount is, as far as a list of volumes cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// A remote or a FUSE mount.
    Network,
    /// A file system on a block device (or a ZFS dataset).
    Disk,
}

/// Network file systems the kernel mounts.
const NETWORK_FS: [&str; 15] = [
    "nfs",
    "nfs4",
    "cifs",
    "smb3",
    "smbfs",
    "9p",
    "ceph",
    "glusterfs",
    "lustre",
    "afs",
    "davfs",
    "ncpfs",
    "fuse",
    "ocfs2",
    "beegfs",
];

/// FUSE mounts that are plumbing of the desktop, not something a person mounted.
const PLUMBING_FUSE: [&str; 5] = [
    "fuse.gvfsd-fuse",
    "fuse.portal",
    "fuse.xwayland",
    "fuse.snapfuse",
    "fuse.squashfuse",
];

/// File systems of local disks. Others (`tmpfs`, `overlay`, `squashfs`, `proc`, …) are not volumes.
const DISK_FS: [&str; 19] = [
    "ext2", "ext3", "ext4", "btrfs", "xfs", "f2fs", "vfat", "exfat", "ntfs", "ntfs3", "fuseblk",
    "hfsplus", "udf", "iso9660", "jfs", "reiserfs", "zfs", "bcachefs", "msdos",
];

fn classify(entry: &MountEntry) -> Option<Class> {
    let fs = entry.fs_type.as_str();
    // An AppImage mounts itself with FUSE under `/tmp/.mount_…` while it runs.
    if PLUMBING_FUSE.contains(&fs) || entry.source.ends_with(".AppImage") {
        return None;
    }
    if NETWORK_FS.contains(&fs) || fs.starts_with("fuse.") {
        return Some(Class::Network);
    }
    if DISK_FS.contains(&fs) && (entry.source.starts_with("/dev/") || fs == "zfs") {
        return Some(Class::Disk);
    }
    None
}

/// Percent-encodes a path for a URL, keeping `/` and the characters a URL path allows.
fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Splits a `host:/path` source into host and path.
fn split_host_path(source: &str) -> Option<(&str, &str)> {
    let (host, path) = source.split_once(':')?;
    if host.is_empty() || host.contains('/') {
        return None;
    }
    Some((host, path))
}

/// The URL a network mount can be reached by, for the file systems that have a standard scheme.
pub fn network_uri(fs_type: &str, source: &str) -> Option<String> {
    match fs_type {
        "nfs" | "nfs4" => {
            let (host, path) = split_host_path(source)?;
            Some(format!("nfs://{host}{}", encode_path(&absolute(path))))
        }
        "cifs" | "smb3" | "smbfs" => {
            let rest = source.strip_prefix("//")?;
            let (host, share) = rest.split_once('/')?;
            (!host.is_empty() && !share.is_empty())
                .then(|| format!("smb://{host}/{}", encode_path(share)))
        }
        "fuse.sshfs" | "fuse" => {
            let (host, path) = split_host_path(source)?;
            Some(format!("sftp://{host}{}", encode_path(&absolute(path))))
        }
        "davfs" | "fuse.davfs" => (source.starts_with("http://") || source.starts_with("https://"))
            .then(|| source.to_string()),
        _ => None,
    }
}

fn absolute(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

/// A name for a network mount: the share and where it is (`export on server`), or the folder it is mounted at.
fn network_label(entry: &MountEntry) -> String {
    let last = |path: &str| {
        path.trim_end_matches('/')
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .map(str::to_string)
    };
    let remote = match entry.fs_type.as_str() {
        "cifs" | "smb3" | "smbfs" => entry.source.strip_prefix("//").and_then(|rest| {
            let (host, share) = rest.split_once('/')?;
            Some(format!("{} on {host}", last(share)?))
        }),
        _ => split_host_path(&entry.source).map(|(host, path)| {
            let host = host.rsplit('@').next().unwrap_or(host);
            match last(path) {
                Some(name) => format!("{name} on {host}"),
                None => host.to_string(),
            }
        }),
    };
    remote
        .or_else(|| last(&entry.mount_point))
        .unwrap_or_else(|| entry.source.clone())
}

fn disk_label(entry: &MountEntry) -> String {
    if entry.mount_point == "/" {
        return "File System".to_string();
    }
    entry
        .mount_point
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| entry.source.clone())
}

fn volume_id(entry: &MountEntry) -> String {
    format!("{ID_PREFIX}{}", entry.mount_point)
}

fn network_volume(entry: &MountEntry) -> Volume {
    Volume {
        id: volume_id(entry),
        label: network_label(entry),
        kind: VolumeKind::Network,
        file_system: Some(entry.fs_type.clone()),
        mount_point: Some(entry.mount_point.clone()),
        uri: network_uri(&entry.fs_type, &entry.source),
        total: None,
        free: None,
        can_mount: false,
        can_unmount: false,
        can_eject: false,
        can_power_off: false,
        locked: false,
        is_system: false,
        device: Some(entry.source.clone()),
        uuid: None,
        remembered: false,
    }
}

fn disk_volume(entry: &MountEntry) -> Volume {
    let mount = entry.mount_point.as_str();
    let kind = if entry.source.starts_with("/dev/loop") {
        VolumeKind::Loop
    } else if mount.starts_with("/run/media/") || mount.starts_with("/media/") {
        VolumeKind::Removable
    } else {
        VolumeKind::Internal
    };
    Volume {
        id: volume_id(entry),
        label: disk_label(entry),
        kind,
        file_system: Some(entry.fs_type.clone()),
        mount_point: Some(entry.mount_point.clone()),
        uri: None,
        total: None,
        free: None,
        can_mount: false,
        can_unmount: false,
        can_eject: false,
        can_power_off: false,
        locked: false,
        is_system: kind == VolumeKind::Internal,
        device: Some(entry.source.clone()),
        uuid: None,
        remembered: false,
    }
}

/// Maps the mount table to volumes. Remote and FUSE mounts are always listed. Mounts of block devices are listed only when `include_disks` is true (there is no UDisks2 to describe them), once for each device: a btrfs file system mounted as several subvolumes, or a device bind-mounted elsewhere, shows at the mount nearest the root.
pub fn volumes_from_mounts(entries: &[MountEntry], include_disks: bool) -> Vec<Volume> {
    let mut volumes = Vec::new();
    let mut disks: BTreeMap<&str, &MountEntry> = BTreeMap::new();
    for entry in entries {
        match classify(entry) {
            Some(Class::Network) => volumes.push(network_volume(entry)),
            Some(Class::Disk) if include_disks => {
                let slot = disks.entry(entry.source.as_str()).or_insert(entry);
                let nearer = (entry.mount_point.len(), &entry.mount_point)
                    < (slot.mount_point.len(), &slot.mount_point);
                if nearer {
                    *slot = entry;
                }
            }
            _ => {}
        }
    }
    volumes.extend(disks.values().map(|entry| disk_volume(entry)));
    // A mount point is listed once, however many times it is stacked.
    volumes.sort_by(|a, b| a.id.cmp(&b.id));
    volumes.dedup_by(|a, b| a.id == b.id);
    volumes
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = include_str!("../tests/fixtures/mountinfo.txt");

    fn entries() -> Vec<MountEntry> {
        parse(TABLE)
    }

    fn by_id(volumes: &[Volume], id: &str) -> Volume {
        volumes
            .iter()
            .find(|volume| volume.id == id)
            .unwrap_or_else(|| {
                panic!(
                    "no volume {id} in {:?}",
                    volumes.iter().map(|v| &v.id).collect::<Vec<_>>()
                )
            })
            .clone()
    }

    #[test]
    fn a_documented_line_parses_into_its_fields() {
        let entry = parse_line(
            "36 35 98:0 /mnt1 /mnt2 rw,noatime master:1 - ext3 /dev/root rw,errors=continue",
        )
        .unwrap();
        assert_eq!(entry.mount_id, 36);
        assert_eq!(entry.parent_id, 35);
        assert_eq!(entry.device, "98:0");
        assert_eq!(entry.root, "/mnt1");
        assert_eq!(entry.mount_point, "/mnt2");
        assert_eq!(entry.options, "rw,noatime");
        assert_eq!(entry.fs_type, "ext3");
        assert_eq!(entry.source, "/dev/root");
        assert_eq!(entry.super_options, "rw,errors=continue");
    }

    #[test]
    fn optional_fields_may_be_absent_or_many() {
        let none = parse_line("1 0 8:1 / /a rw - ext4 /dev/sda1 rw").unwrap();
        assert_eq!(none.fs_type, "ext4");
        let many = parse_line(
            "1 0 8:1 / /a rw shared:1 master:2 propagate_from:3 unbindable - ext4 /dev/sda1 rw",
        )
        .unwrap();
        assert_eq!(many.fs_type, "ext4");
        assert_eq!(many.source, "/dev/sda1");
    }

    #[test]
    fn escaped_spaces_tabs_and_backslashes_are_decoded() {
        assert_eq!(
            unescape("/run/media/scott/My\\040Passport"),
            "/run/media/scott/My Passport"
        );
        assert_eq!(unescape("a\\011b\\012c\\134d"), "a\tb\nc\\d");
        // Not an escape: left alone.
        assert_eq!(unescape("a\\9z"), "a\\9z");
        assert_eq!(unescape("trailing\\04"), "trailing\\04");
        let entry = parse_line("5 1 8:17 / /mnt/with\\040space rw - ext4 /dev/sdb1 rw").unwrap();
        assert_eq!(entry.mount_point, "/mnt/with space");
    }

    #[test]
    fn malformed_lines_are_skipped() {
        assert_eq!(
            parse("\nnonsense\n1 0 8:1 / /a rw\n1 0 8:1 / /a rw shared:1\n").len(),
            0
        );
    }

    #[test]
    fn the_fixture_table_parses_completely() {
        assert_eq!(
            entries().len(),
            TABLE.lines().filter(|l| !l.trim().is_empty()).count()
        );
    }

    #[test]
    fn network_and_fuse_mounts_are_network_volumes() {
        let volumes = volumes_from_mounts(&entries(), false);
        let nfs = by_id(&volumes, "mount:/mnt/nas");
        assert_eq!(nfs.kind, VolumeKind::Network);
        assert_eq!(nfs.label, "media on nas.local");
        assert_eq!(nfs.uri.as_deref(), Some("nfs://nas.local/export/media"));
        assert_eq!(nfs.file_system.as_deref(), Some("nfs4"));
        assert_eq!(nfs.device.as_deref(), Some("nas.local:/export/media"));
        assert!(!nfs.can_unmount && !nfs.can_eject && !nfs.is_system);

        let smb = by_id(&volumes, "mount:/mnt/smb share");
        assert_eq!(smb.kind, VolumeKind::Network);
        assert_eq!(smb.label, "Photos on fileserver");
        assert_eq!(smb.uri.as_deref(), Some("smb://fileserver/Photos"));

        let ssh = by_id(&volumes, "mount:/home/scott/remote");
        assert_eq!(ssh.kind, VolumeKind::Network);
        assert_eq!(ssh.label, "src on build.example.org");
        assert_eq!(
            ssh.uri.as_deref(),
            Some("sftp://scott@build.example.org/home/scott/src")
        );

        let rclone = by_id(&volumes, "mount:/home/scott/drive");
        assert_eq!(rclone.kind, VolumeKind::Network);
        assert_eq!(rclone.uri, None);
        assert_eq!(rclone.label, "remote");
    }

    #[test]
    fn desktop_plumbing_and_pseudo_file_systems_are_not_volumes() {
        let volumes = volumes_from_mounts(&entries(), true);
        for id in [
            "mount:/proc",
            "mount:/sys",
            "mount:/dev",
            "mount:/run",
            "mount:/tmp",
            "mount:/run/user/1000/gvfs",
            "mount:/run/user/1000/doc",
            "mount:/snap/core22/1",
            "mount:/tmp/.mount_Jan_0.PniOEN",
            "mount:/var/lib/docker/overlay2/x/merged",
        ] {
            assert!(volumes.iter().all(|v| v.id != id), "{id} listed");
        }
    }

    #[test]
    fn local_disks_are_left_to_udisks_unless_it_is_missing() {
        let with = volumes_from_mounts(&entries(), false);
        assert!(with.iter().all(|v| v.kind == VolumeKind::Network));
        let without = volumes_from_mounts(&entries(), true);
        assert!(without.iter().any(|v| v.kind == VolumeKind::Internal));
    }

    #[test]
    fn btrfs_subvolumes_of_one_device_are_one_volume_at_the_mount_nearest_the_root() {
        let volumes = volumes_from_mounts(&entries(), true);
        let root = by_id(&volumes, "mount:/");
        assert_eq!(root.kind, VolumeKind::Internal);
        assert_eq!(root.label, "File System");
        assert_eq!(root.file_system.as_deref(), Some("btrfs"));
        assert_eq!(root.device.as_deref(), Some("/dev/nvme0n1p2"));
        assert!(root.is_system);
        for id in ["mount:/home", "mount:/var/log"] {
            assert!(
                volumes.iter().all(|v| v.id != id),
                "{id} is a subvolume of the root device"
            );
        }
    }

    #[test]
    fn removable_and_loop_disks_are_guessed_from_where_and_what() {
        let volumes = volumes_from_mounts(&entries(), true);
        let stick = by_id(&volumes, "mount:/run/media/scott/My Passport");
        assert_eq!(stick.kind, VolumeKind::Removable);
        assert_eq!(stick.label, "My Passport");
        assert!(!stick.is_system);
        let image = by_id(&volumes, "mount:/mnt/image");
        assert_eq!(image.kind, VolumeKind::Loop);
    }

    #[test]
    fn a_bind_mount_of_a_device_is_not_listed_again() {
        let volumes = volumes_from_mounts(&entries(), true);
        assert!(volumes.iter().all(|v| v.id != "mount:/srv/bind"));
    }

    #[test]
    fn uris_are_made_only_for_known_schemes_and_are_percent_encoded() {
        assert_eq!(
            network_uri("nfs", "host:/a b").as_deref(),
            Some("nfs://host/a%20b")
        );
        assert_eq!(
            network_uri("cifs", "//h/share/sub").as_deref(),
            Some("smb://h/share/sub")
        );
        assert_eq!(network_uri("cifs", "//h"), None);
        assert_eq!(network_uri("9p", "x"), None);
        assert_eq!(
            network_uri("davfs", "https://dav.example.org/remote.php").as_deref(),
            Some("https://dav.example.org/remote.php")
        );
        assert_eq!(
            network_uri("fuse.sshfs", "host:rel").as_deref(),
            Some("sftp://host/rel")
        );
    }

    #[test]
    fn an_empty_table_has_no_volumes() {
        assert!(volumes_from_mounts(&[], true).is_empty());
    }
}
