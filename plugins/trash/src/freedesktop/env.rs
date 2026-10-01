// Describes the environment the freedesktop trash works in, and reads the real one
//
// Everything the trash logic needs from the machine arrives through `TrashEnv`, so tests run in a temporary directory with fake mounts and never touch the real `~/.local/share/Trash`. Only `default_env` reads the real machine, and only the plugin glue calls it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsString;
use std::fmt;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::NaiveDateTime;

/// The current local time, as the `DeletionDate` of a new `.trashinfo` file is written and as expiry measures age.
pub type Clock = Arc<dyn Fn() -> NaiveDateTime + Send + Sync>;

/// One mounted volume that can hold a trash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountInfo {
    /// The top directory of the volume.
    pub mount_point: PathBuf,
    /// The device number (`st_dev`) of files on the volume.
    pub device_id: u64,
    /// A network file system. Informational: it is trashed on like any other.
    pub is_network: bool,
    /// A removable or user-mounted volume. Informational.
    pub is_removable: bool,
}

/// Everything the freedesktop trash needs from the machine.
#[derive(Clone)]
pub struct TrashEnv {
    /// `$XDG_DATA_HOME`: the home trash is `data_home/Trash`.
    pub data_home: PathBuf,
    /// The user's home folder, which is never trashed.
    pub home_dir: PathBuf,
    pub uid: u32,
    /// The mounted volumes, used to find the top directory of a file's volume and to look for per-volume trashes.
    pub mounts: Vec<MountInfo>,
    pub now: Clock,
}

impl fmt::Debug for TrashEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrashEnv")
            .field("data_home", &self.data_home)
            .field("home_dir", &self.home_dir)
            .field("uid", &self.uid)
            .field("mounts", &self.mounts)
            .finish_non_exhaustive()
    }
}

/// A clock that reads the system's local time.
pub fn system_clock() -> Clock {
    Arc::new(|| chrono::Local::now().naive_local())
}

/// Reads the real environment: `$XDG_DATA_HOME` (or `~/.local/share`), the user id and the mount table from `/proc/self/mountinfo`.
pub fn default_env() -> TrashEnv {
    let home_dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| PathBuf::from("/"));
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home_dir.join(".local/share"));
    let mountinfo = std::fs::read(std::path::Path::new("/proc/self/mountinfo")).unwrap_or_default();
    let mounts = parse_mountinfo(&String::from_utf8_lossy(&mountinfo))
        .into_iter()
        .filter_map(|mut mount| {
            // The device number in `mountinfo` is not `st_dev` on every file system (btrfs subvolumes differ), so ask the volume itself.
            mount.device_id = std::fs::metadata(&mount.mount_point).ok()?.dev();
            Some(mount)
        })
        .collect();
    TrashEnv {
        data_home,
        home_dir,
        // SAFETY: `geteuid` takes no arguments, cannot fail and has no side effects.
        uid: unsafe { libc::geteuid() },
        mounts,
        now: system_clock(),
    }
}

/// File systems that are never a place to keep a trash.
const PSEUDO_FILESYSTEMS: &[&str] = &[
    "proc",
    "sysfs",
    "devtmpfs",
    "devpts",
    "cgroup",
    "cgroup2",
    "securityfs",
    "debugfs",
    "tracefs",
    "pstore",
    "bpf",
    "configfs",
    "fusectl",
    "mqueue",
    "hugetlbfs",
    "autofs",
    "binfmt_misc",
    "efivarfs",
    "selinuxfs",
    "nsfs",
    "rpc_pipefs",
    "fuse.portal",
    "fuse.gvfsd-fuse",
    "fuse.gvfs-fuse-daemon",
];

fn is_network_filesystem(fstype: &str) -> bool {
    matches!(
        fstype,
        "nfs" | "nfs4" | "cifs" | "smb3" | "smbfs" | "afs" | "ceph" | "9p" | "davfs" | "glusterfs"
    ) || fstype.starts_with("fuse.sshfs")
        || fstype.starts_with("fuse.rclone")
        || fstype.starts_with("fuse.davfs")
}

/// Parses `/proc/self/mountinfo` into mounts with a placeholder device id (the caller fills it in). Pseudo file systems are dropped.
pub fn parse_mountinfo(text: &str) -> Vec<MountInfo> {
    text.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(' ').collect();
            let separator = fields.iter().position(|field| *field == "-")?;
            let mount_point = unescape_mount_field(fields.get(4)?);
            let fstype = *fields.get(separator + 1)?;
            if PSEUDO_FILESYSTEMS.contains(&fstype) {
                return None;
            }
            let path = PathBuf::from(&mount_point);
            let removable = path.starts_with("/run/media") || path.starts_with("/media");
            Some(MountInfo {
                mount_point: path,
                device_id: 0,
                is_network: is_network_filesystem(fstype),
                is_removable: removable,
            })
        })
        .collect()
}

/// `mountinfo` writes a space, tab, newline or backslash in a path as a three-digit octal escape.
fn unescape_mount_field(field: &str) -> OsString {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\'
            && index + 3 < bytes.len()
            && bytes[index + 1..index + 4]
                .iter()
                .all(|b| (b'0'..=b'7').contains(b))
        {
            let value = bytes[index + 1..index + 4]
                .iter()
                .fold(0u32, |acc, b| acc * 8 + u32::from(b - b'0'));
            out.push((value & 0xFF) as u8);
            index += 4;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    OsString::from_vec(out)
}
