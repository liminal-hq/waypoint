// The platform-independent parts of the Windows backend: drive types, labels and remote names
//
// They live outside `windows.rs` so they are unit-tested on every platform, and so the Win32 code is only the thin part that needs a Windows machine.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![cfg_attr(not(any(windows, test)), allow(dead_code))]

use crate::models::VolumeKind;

/// The values `GetDriveTypeW` returns.
#[cfg(test)]
pub const DRIVE_UNKNOWN: u32 = 0;
#[cfg(test)]
pub const DRIVE_NO_ROOT_DIR: u32 = 1;
pub const DRIVE_REMOVABLE: u32 = 2;
pub const DRIVE_FIXED: u32 = 3;
pub const DRIVE_REMOTE: u32 = 4;
pub const DRIVE_CDROM: u32 = 5;
pub const DRIVE_RAMDISK: u32 = 6;

/// The kind of volume a drive type stands for; `None` for a drive that is not there (an unknown type, or a letter with no root).
pub fn kind_for_drive_type(drive_type: u32) -> Option<VolumeKind> {
    match drive_type {
        DRIVE_REMOVABLE => Some(VolumeKind::Removable),
        DRIVE_FIXED | DRIVE_RAMDISK => Some(VolumeKind::Internal),
        DRIVE_REMOTE => Some(VolumeKind::Network),
        DRIVE_CDROM => Some(VolumeKind::Optical),
        _ => None,
    }
}

/// The name Explorer shows: the volume label (or a default for the kind) and the drive letter, as `Data (D:)`.
pub fn display_label(label: &str, kind: VolumeKind, root: &str) -> String {
    let letter = root.trim_end_matches('\\');
    let name = if label.is_empty() {
        match kind {
            VolumeKind::Removable => "Removable Disk",
            VolumeKind::Optical => "CD Drive",
            VolumeKind::Network => "Network Drive",
            _ => "Local Disk",
        }
    } else {
        label
    };
    format!("{name} ({letter})")
}

/// The drive roots (`C:\`) a `GetLogicalDriveStringsW` buffer holds: NUL-separated, ended by an empty string.
pub fn split_drive_strings(buffer: &[u16]) -> Vec<String> {
    buffer
        .split(|unit| *unit == 0)
        .take_while(|part| !part.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

/// A URL for a mapped drive's remote name: `\\server\share` becomes `smb://server/share`. WebDAV names (`\\host@SSL\DavWWWRoot`) have no SMB URL and give `None`.
pub fn unc_to_uri(remote: &str) -> Option<String> {
    let rest = remote.strip_prefix("\\\\")?;
    let (host, share) = rest.split_once('\\')?;
    if host.is_empty() || share.is_empty() || host.contains('@') || share.starts_with("DavWWWRoot")
    {
        return None;
    }
    Some(format!("smb://{host}/{}", share.replace('\\', "/")))
}

/// Whether a drive root belongs to the volume the system was started from.
pub fn is_system_drive(root: &str, system_drive: Option<&str>) -> bool {
    system_drive.is_some_and(|system| {
        system
            .trim_end_matches('\\')
            .eq_ignore_ascii_case(root.trim_end_matches('\\'))
    })
}

/// The drive letters in a `GetLogicalDrives` bit mask, as roots (`C:\`).
pub fn roots_from_mask(mask: u32) -> Vec<String> {
    (0..26u8)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| format!("{}:\\", (b'A' + bit) as char))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_types_map_to_kinds() {
        assert_eq!(
            kind_for_drive_type(DRIVE_REMOVABLE),
            Some(VolumeKind::Removable)
        );
        assert_eq!(kind_for_drive_type(DRIVE_FIXED), Some(VolumeKind::Internal));
        assert_eq!(
            kind_for_drive_type(DRIVE_RAMDISK),
            Some(VolumeKind::Internal)
        );
        assert_eq!(kind_for_drive_type(DRIVE_REMOTE), Some(VolumeKind::Network));
        assert_eq!(kind_for_drive_type(DRIVE_CDROM), Some(VolumeKind::Optical));
        assert_eq!(kind_for_drive_type(DRIVE_UNKNOWN), None);
        assert_eq!(kind_for_drive_type(DRIVE_NO_ROOT_DIR), None);
        assert_eq!(kind_for_drive_type(99), None);
    }

    #[test]
    fn labels_follow_explorer() {
        assert_eq!(
            display_label("Data", VolumeKind::Internal, "D:\\"),
            "Data (D:)"
        );
        assert_eq!(
            display_label("", VolumeKind::Internal, "C:\\"),
            "Local Disk (C:)"
        );
        assert_eq!(
            display_label("", VolumeKind::Removable, "E:\\"),
            "Removable Disk (E:)"
        );
        assert_eq!(
            display_label("", VolumeKind::Optical, "F:\\"),
            "CD Drive (F:)"
        );
        assert_eq!(
            display_label("", VolumeKind::Network, "Z:\\"),
            "Network Drive (Z:)"
        );
    }

    #[test]
    fn a_drive_strings_buffer_splits_at_the_nuls() {
        let buffer: Vec<u16> = "C:\\\0D:\\\0Z:\\\0\0".encode_utf16().collect();
        assert_eq!(split_drive_strings(&buffer), ["C:\\", "D:\\", "Z:\\"]);
        assert!(split_drive_strings(&[0, 0]).is_empty());
        assert!(split_drive_strings(&[]).is_empty());
    }

    #[test]
    fn a_unc_name_becomes_an_smb_url_when_it_can() {
        assert_eq!(
            unc_to_uri("\\\\nas\\media").as_deref(),
            Some("smb://nas/media")
        );
        assert_eq!(
            unc_to_uri("\\\\nas\\media\\tv").as_deref(),
            Some("smb://nas/media/tv")
        );
        assert_eq!(unc_to_uri("\\\\host@SSL\\DavWWWRoot\\x"), None);
        assert_eq!(unc_to_uri("\\\\nas"), None);
        assert_eq!(unc_to_uri("C:\\x"), None);
    }

    #[test]
    fn the_system_drive_is_matched_without_regard_to_case_or_slash() {
        assert!(is_system_drive("C:\\", Some("c:")));
        assert!(is_system_drive("C:\\", Some("C:\\")));
        assert!(!is_system_drive("D:\\", Some("C:")));
        assert!(!is_system_drive("C:\\", None));
    }

    #[test]
    fn a_drive_mask_lists_its_letters() {
        assert_eq!(roots_from_mask(0b1101), ["A:\\", "C:\\", "D:\\"]);
        assert_eq!(roots_from_mask(1 << 25), ["Z:\\"]);
        assert!(roots_from_mask(0).is_empty());
    }
}
