// The pure model of UDisks2: a snapshot of its object tree mapped to volumes, and the plans for the actions
//
// `org.freedesktop.DBus.ObjectManager.GetManagedObjects` answers with every object UDisks2 knows and the properties of each of its interfaces. The Linux backend converts that answer into a [`Snapshot`] and everything else here is plain data in, plain data out, so it is tested from a JSON fixture and never needs a bus or a device.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::error::VolumesError;
use crate::models::{Volume, VolumeKind};

pub const BLOCK: &str = "org.freedesktop.UDisks2.Block";
pub const FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
pub const ENCRYPTED: &str = "org.freedesktop.UDisks2.Encrypted";
pub const LOOP: &str = "org.freedesktop.UDisks2.Loop";
pub const DRIVE: &str = "org.freedesktop.UDisks2.Drive";

/// The object path prefix of block devices, and the prefix of a volume id this module makes.
pub const BLOCK_PREFIX: &str = "/org/freedesktop/UDisks2/block_devices/";
pub const ID_PREFIX: &str = "udisks2:";

/// Mount points no one should offer to unmount.
const PROTECTED_MOUNTS: [&str; 5] = ["/", "/boot", "/boot/efi", "/usr", "/var"];

/// One property value, reduced to what the mapping reads. Byte strings (`ay`, `aay`) arrive as text and lists of text, with the trailing NUL dropped.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Prop {
    Bool(bool),
    Uint(u64),
    Str(String),
    Strs(Vec<String>),
}

pub type Props = BTreeMap<String, Prop>;

/// The answer of `GetManagedObjects`: object path to interface name to properties.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Snapshot {
    pub objects: BTreeMap<String, BTreeMap<String, Props>>,
}

impl Snapshot {
    fn prop(&self, path: &str, interface: &str, name: &str) -> Option<&Prop> {
        self.objects.get(path)?.get(interface)?.get(name)
    }

    fn string(&self, path: &str, interface: &str, name: &str) -> Option<&str> {
        match self.prop(path, interface, name)? {
            Prop::Str(text) => Some(text),
            _ => None,
        }
    }

    fn flag(&self, path: &str, interface: &str, name: &str) -> bool {
        matches!(self.prop(path, interface, name), Some(Prop::Bool(true)))
    }

    fn number(&self, path: &str, interface: &str, name: &str) -> Option<u64> {
        match self.prop(path, interface, name)? {
            Prop::Uint(number) => Some(*number),
            _ => None,
        }
    }

    fn strings(&self, path: &str, interface: &str, name: &str) -> Vec<&str> {
        match self.prop(path, interface, name) {
            Some(Prop::Strs(items)) => items.iter().map(String::as_str).collect(),
            _ => Vec::new(),
        }
    }

    fn has(&self, path: &str, interface: &str) -> bool {
        self.objects
            .get(path)
            .is_some_and(|ifaces| ifaces.contains_key(interface))
    }

    fn block_paths(&self) -> impl Iterator<Item = &str> {
        self.objects
            .iter()
            .filter(|(path, ifaces)| path.starts_with(BLOCK_PREFIX) && ifaces.contains_key(BLOCK))
            .map(|(path, _)| path.as_str())
    }

    /// The object a property names, or `None` for the empty reference UDisks2 writes as `/`.
    fn reference(&self, path: &str, interface: &str, name: &str) -> Option<&str> {
        self.string(path, interface, name)
            .filter(|target| !target.is_empty() && *target != "/")
    }

    /// The block device holding this one's ciphertext, when this is an unlocked volume.
    fn backing(&self, path: &str) -> Option<&str> {
        self.reference(path, BLOCK, "CryptoBackingDevice")
    }

    /// The block device that unlocks to this one, when this is an encrypted volume that is unlocked.
    fn cleartext_of(&self, path: &str) -> Option<&str> {
        self.block_paths()
            .find(|candidate| self.backing(candidate) == Some(path))
    }

    /// The drive a block device sits on (through its backing device when it is unlocked).
    fn drive_of(&self, path: &str) -> Option<&str> {
        self.reference(path, BLOCK, "Drive")
            .or_else(|| {
                let backing = self.backing(path)?;
                self.reference(backing, BLOCK, "Drive")
            })
            .filter(|drive| self.has(drive, DRIVE))
    }

    fn mount_points(&self, path: &str) -> Vec<&str> {
        self.strings(path, FILESYSTEM, "MountPoints")
    }
}

/// The name of a block device's object: `sda1` for `/org/freedesktop/UDisks2/block_devices/sda1`.
fn block_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The id of the volume made from a block device.
pub fn id_of(path: &str) -> String {
    format!("{ID_PREFIX}{}", block_name(path))
}

/// The object path a volume id stands for, or `None` for an id that is not one of this module's (a `mount:` id of the mount table, for example) or that cannot name an object.
pub fn path_of(id: &str) -> Option<String> {
    let name = id.strip_prefix(ID_PREFIX)?;
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return None;
    }
    Some(format!("{BLOCK_PREFIX}{name}"))
}

/// The mount point to show: when a device is mounted several times (btrfs subvolumes, bind mounts), the shortest path, which is the one nearest the root.
fn primary_mount<'a>(points: &[&'a str]) -> Option<&'a str> {
    points
        .iter()
        .copied()
        .min_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)))
}

fn drive_description(snapshot: &Snapshot, drive: &str) -> Option<String> {
    let vendor = snapshot.string(drive, DRIVE, "Vendor").unwrap_or("").trim();
    let model = snapshot.string(drive, DRIVE, "Model").unwrap_or("").trim();
    let text = format!("{vendor} {model}").trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn size_description(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 || value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// The name to show for a block device: its label, the label of its ciphertext, the hint the system gives, or a description of the size.
fn label_of(snapshot: &Snapshot, path: &str, locked: bool) -> String {
    let own = |path: &str| {
        snapshot
            .string(path, BLOCK, "IdLabel")
            .filter(|label| !label.is_empty())
            .or_else(|| {
                snapshot
                    .string(path, BLOCK, "HintName")
                    .filter(|name| !name.is_empty())
            })
            .map(str::to_string)
    };
    if let Some(label) = own(path) {
        return label;
    }
    if let Some(label) = snapshot.backing(path).and_then(own) {
        return label;
    }
    if let Some(description) = snapshot
        .drive_of(path)
        .and_then(|drive| drive_description(snapshot, drive))
    {
        return description;
    }
    let size = snapshot.number(path, BLOCK, "Size").unwrap_or(0);
    let what = if locked { "Encrypted" } else { "Volume" };
    if size > 0 {
        format!("{} {what}", size_description(size))
    } else {
        what.to_string()
    }
}

fn kind_of(snapshot: &Snapshot, path: &str, encrypted: bool) -> VolumeKind {
    if encrypted {
        return VolumeKind::Encrypted;
    }
    let device = snapshot.string(path, BLOCK, "Device").unwrap_or("");
    if snapshot.has(path, LOOP) || device.starts_with("/dev/loop") {
        return VolumeKind::Loop;
    }
    let drive = snapshot.drive_of(path);
    if let Some(drive) = drive {
        if snapshot.flag(drive, DRIVE, "Optical") {
            return VolumeKind::Optical;
        }
        let bus = snapshot.string(drive, DRIVE, "ConnectionBus").unwrap_or("");
        let plugged_in = matches!(bus, "usb" | "sdio" | "ieee1394");
        if snapshot.flag(drive, DRIVE, "Removable")
            || (plugged_in && !snapshot.flag(path, BLOCK, "HintSystem"))
        {
            return VolumeKind::Removable;
        }
    }
    VolumeKind::Internal
}

/// Maps a snapshot of UDisks2's objects to the volumes a person would see. Hidden partitions (`HintIgnore`), partitions with no file system and nothing to unlock, empty drives and the ciphertext of an unlocked volume are left out.
pub fn volumes_from_snapshot(snapshot: &Snapshot) -> Vec<Volume> {
    let mut volumes = Vec::new();
    for path in snapshot.block_paths() {
        if snapshot.flag(path, BLOCK, "HintIgnore") {
            continue;
        }
        let has_filesystem = snapshot.has(path, FILESYSTEM);
        let is_encrypted_backing = snapshot.has(path, ENCRYPTED);
        // An unlocked volume is shown once, as its cleartext device.
        if is_encrypted_backing && snapshot.cleartext_of(path).is_some() {
            continue;
        }
        let locked = is_encrypted_backing;
        if !has_filesystem && !locked {
            continue;
        }
        let is_cleartext = snapshot.backing(path).is_some();
        let kind = kind_of(snapshot, path, locked || is_cleartext);
        let mount_points = snapshot.mount_points(path);
        let mount_point = primary_mount(&mount_points);
        let drive = snapshot.drive_of(path);
        let hint_system = snapshot.flag(path, BLOCK, "HintSystem");
        let (ejectable, power_off) = match drive {
            Some(drive) if !hint_system => (
                snapshot.flag(drive, DRIVE, "Ejectable"),
                snapshot.flag(drive, DRIVE, "CanPowerOff"),
            ),
            _ => (false, false),
        };
        let protected = mount_points.iter().any(|m| PROTECTED_MOUNTS.contains(m));
        let file_system = snapshot
            .string(path, BLOCK, "IdType")
            .filter(|name| !name.is_empty())
            .map(str::to_string);
        volumes.push(Volume {
            id: id_of(path),
            label: label_of(snapshot, path, locked),
            kind,
            file_system,
            mount_point: mount_point.map(str::to_string),
            uri: None,
            total: snapshot
                .number(path, BLOCK, "Size")
                .filter(|size| *size > 0),
            free: None,
            can_mount: has_filesystem && mount_points.is_empty() && !locked,
            can_unmount: !mount_points.is_empty() && !protected,
            can_eject: ejectable || power_off,
            can_power_off: power_off,
            locked,
            is_system: hint_system,
            device: snapshot.string(path, BLOCK, "Device").map(str::to_string),
        });
    }
    volumes
}

/// The file system a volume id names, with its mount point.
pub struct FilesystemTarget {
    pub path: String,
    pub mount_point: Option<String>,
}

fn require_block(snapshot: &Snapshot, id: &str) -> Result<String, VolumesError> {
    let path = path_of(id).ok_or(VolumesError::Unsupported)?;
    if snapshot.has(&path, BLOCK) {
        Ok(path)
    } else {
        Err(VolumesError::NotFound)
    }
}

/// The target of `mount` and `unmount`.
pub fn filesystem_target(snapshot: &Snapshot, id: &str) -> Result<FilesystemTarget, VolumesError> {
    let path = require_block(snapshot, id)?;
    if !snapshot.has(&path, FILESYSTEM) {
        return Err(VolumesError::Unsupported);
    }
    let mount_point = primary_mount(&snapshot.mount_points(&path)).map(str::to_string);
    Ok(FilesystemTarget { path, mount_point })
}

/// The block device `unlock` acts on; it must be an encrypted volume that is not unlocked yet.
pub fn unlock_target(snapshot: &Snapshot, id: &str) -> Result<String, VolumesError> {
    let path = require_block(snapshot, id)?;
    if !snapshot.has(&path, ENCRYPTED) {
        return Err(VolumesError::Unsupported);
    }
    Ok(path)
}

/// Everything `eject` has to do, in order: unmount what is mounted on the drive, lock what is unlocked, then eject and power the drive off as it allows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EjectPlan {
    /// Block devices to unmount, with the mount point of each (to name what holds it when it is busy).
    pub unmount: Vec<(String, String)>,
    /// Encrypted devices to lock once their cleartext is unmounted.
    pub lock: Vec<String>,
    pub eject: Option<String>,
    pub power_off: Option<String>,
}

/// Plans the eject of the drive a volume sits on. Every volume of that drive is unmounted, as removing the drive removes them all.
pub fn plan_eject(snapshot: &Snapshot, id: &str) -> Result<EjectPlan, VolumesError> {
    let path = require_block(snapshot, id)?;
    let drive = snapshot
        .drive_of(&path)
        .ok_or(VolumesError::Unsupported)?
        .to_string();
    if snapshot.flag(&path, BLOCK, "HintSystem") {
        return Err(VolumesError::Unsupported);
    }
    let ejectable = snapshot.flag(&drive, DRIVE, "Ejectable");
    let can_power_off = snapshot.flag(&drive, DRIVE, "CanPowerOff");
    if !ejectable && !can_power_off {
        return Err(VolumesError::Unsupported);
    }
    let mut plan = EjectPlan {
        eject: ejectable.then(|| drive.clone()),
        power_off: can_power_off.then(|| drive.clone()),
        ..EjectPlan::default()
    };
    for block in snapshot.block_paths() {
        if snapshot.drive_of(block) != Some(drive.as_str()) {
            continue;
        }
        for mount_point in snapshot.mount_points(block) {
            plan.unmount
                .push((block.to_string(), mount_point.to_string()));
        }
        if snapshot.has(block, ENCRYPTED) && snapshot.cleartext_of(block).is_some() {
            plan.lock.push(block.to_string());
        }
    }
    // A mount on the cleartext device must go before its ciphertext is locked; the mounts of the plain devices have no order among themselves.
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/udisks2-snapshot.json");

    fn snapshot() -> Snapshot {
        serde_json::from_str(FIXTURE).expect("the fixture is a snapshot")
    }

    fn volume(id: &str) -> Volume {
        volumes_from_snapshot(&snapshot())
            .into_iter()
            .find(|volume| volume.id == id)
            .unwrap_or_else(|| panic!("no volume {id}"))
    }

    fn ids() -> Vec<String> {
        volumes_from_snapshot(&snapshot())
            .into_iter()
            .map(|volume| volume.id)
            .collect()
    }

    #[test]
    fn an_internal_disk_is_internal_system_and_cannot_be_ejected() {
        let root = volume("udisks2:nvme0n1p2");
        assert_eq!(root.kind, VolumeKind::Internal);
        assert_eq!(root.label, "fedora");
        assert_eq!(root.file_system.as_deref(), Some("btrfs"));
        assert_eq!(root.device.as_deref(), Some("/dev/nvme0n1p2"));
        assert!(root.is_system);
        assert!(!root.can_eject && !root.can_power_off && !root.can_mount);
        assert_eq!(root.total, Some(500_000_000_000));
        assert_eq!(root.free, None);
    }

    #[test]
    fn the_mount_nearest_the_root_is_the_one_shown_and_the_system_cannot_be_unmounted() {
        let root = volume("udisks2:nvme0n1p2");
        // Mounted at `/home` and `/` (a btrfs subvolume each): `/` wins.
        assert_eq!(root.mount_point.as_deref(), Some("/"));
        assert!(!root.can_unmount);
        let efi = volume("udisks2:nvme0n1p1");
        assert_eq!(efi.mount_point.as_deref(), Some("/boot/efi"));
        assert!(!efi.can_unmount);
    }

    #[test]
    fn a_mounted_usb_stick_is_removable_and_can_be_unmounted_and_ejected() {
        let stick = volume("udisks2:sdb1");
        assert_eq!(stick.kind, VolumeKind::Removable);
        assert_eq!(stick.label, "KINGSTON");
        assert_eq!(
            stick.mount_point.as_deref(),
            Some("/run/media/scott/KINGSTON")
        );
        assert!(stick.can_unmount && !stick.can_mount);
        assert!(stick.can_eject && stick.can_power_off);
        assert!(!stick.is_system && !stick.locked);
    }

    #[test]
    fn an_unmounted_usb_disk_can_be_mounted_and_is_removable_by_its_bus() {
        let disk = volume("udisks2:sdc1");
        // The drive says it is not removable (a USB hard disk), but it is plugged in.
        assert_eq!(disk.kind, VolumeKind::Removable);
        assert!(disk.can_mount && !disk.can_unmount);
        assert_eq!(disk.mount_point, None);
        // It cannot be ejected, but it can be powered off, which `eject` does.
        assert!(disk.can_eject && disk.can_power_off);
        assert_eq!(disk.total, Some(2_000_000_000_000));
    }

    #[test]
    fn a_locked_luks_volume_can_only_be_unlocked() {
        let locked = volume("udisks2:sdd1");
        assert_eq!(locked.kind, VolumeKind::Encrypted);
        assert!(locked.locked);
        assert!(!locked.can_mount && !locked.can_unmount);
        assert_eq!(locked.file_system.as_deref(), Some("crypto_LUKS"));
        assert!(locked.can_eject);
    }

    #[test]
    fn an_unlocked_luks_volume_is_shown_once_as_its_cleartext_device() {
        let all = ids();
        assert!(!all.contains(&"udisks2:sde1".to_string()));
        let clear = volume("udisks2:dm_2d0");
        assert_eq!(clear.kind, VolumeKind::Encrypted);
        assert!(!clear.locked);
        assert_eq!(clear.label, "Vault");
        assert_eq!(clear.file_system.as_deref(), Some("ext4"));
        assert_eq!(clear.mount_point.as_deref(), Some("/run/media/scott/Vault"));
        // The drive belongs to the ciphertext device.
        assert!(clear.can_eject);
    }

    #[test]
    fn an_optical_disc_is_optical_and_ejectable() {
        let disc = volume("udisks2:sr0");
        assert_eq!(disc.kind, VolumeKind::Optical);
        assert_eq!(disc.label, "Install Disc");
        assert!(disc.can_eject && !disc.can_power_off);
        assert!(disc.can_unmount);
    }

    #[test]
    fn a_drive_with_no_media_has_no_volume() {
        assert!(!ids().contains(&"udisks2:sr1".to_string()));
    }

    #[test]
    fn a_loop_device_is_a_loop_volume_that_cannot_be_ejected() {
        let image = volume("udisks2:loop0");
        assert_eq!(image.kind, VolumeKind::Loop);
        assert!(image.can_mount && !image.can_eject);
        assert!(!image.is_system);
    }

    #[test]
    fn hidden_and_unformatted_devices_are_left_out() {
        let all = ids();
        // `HintIgnore` (a snap's loop device and a recovery partition), and swap, which has no file system.
        assert!(!all.contains(&"udisks2:loop1".to_string()));
        assert!(!all.contains(&"udisks2:nvme0n1p4".to_string()));
        assert!(!all.contains(&"udisks2:nvme0n1p3".to_string()));
    }

    #[test]
    fn an_unnamed_device_is_described_by_its_drive() {
        let unnamed = volume("udisks2:sdf1");
        assert_eq!(unnamed.label, "SanDisk Ultra");
    }

    #[test]
    fn the_whole_fixture_maps_to_a_stable_list() {
        assert_eq!(
            ids(),
            [
                "udisks2:dm_2d0",
                "udisks2:loop0",
                "udisks2:nvme0n1p1",
                "udisks2:nvme0n1p2",
                "udisks2:sdb1",
                "udisks2:sdc1",
                "udisks2:sdd1",
                "udisks2:sdf1",
                "udisks2:sr0",
            ]
        );
    }

    #[test]
    fn ids_round_trip_to_object_paths_and_foreign_ids_are_refused() {
        assert_eq!(
            path_of("udisks2:dm_2d0").as_deref(),
            Some("/org/freedesktop/UDisks2/block_devices/dm_2d0")
        );
        assert_eq!(path_of("mount:/mnt/x"), None);
        assert_eq!(path_of("udisks2:../x"), None);
        assert_eq!(path_of("udisks2:"), None);
    }

    #[test]
    fn mount_targets_report_the_mount_point_and_refuse_the_wrong_kinds() {
        let target = filesystem_target(&snapshot(), "udisks2:sdb1").unwrap();
        assert_eq!(
            target.mount_point.as_deref(),
            Some("/run/media/scott/KINGSTON")
        );
        assert_eq!(
            filesystem_target(&snapshot(), "udisks2:sdd1").err(),
            Some(VolumesError::Unsupported)
        );
        assert_eq!(
            filesystem_target(&snapshot(), "udisks2:nosuch").err(),
            Some(VolumesError::NotFound)
        );
        assert_eq!(
            filesystem_target(&snapshot(), "mount:/mnt/nfs").err(),
            Some(VolumesError::Unsupported)
        );
    }

    #[test]
    fn only_an_encrypted_device_can_be_unlocked() {
        assert!(unlock_target(&snapshot(), "udisks2:sdd1").is_ok());
        assert_eq!(
            unlock_target(&snapshot(), "udisks2:sdb1").err(),
            Some(VolumesError::Unsupported)
        );
    }

    #[test]
    fn ejecting_a_stick_unmounts_it_then_ejects_and_powers_off() {
        let plan = plan_eject(&snapshot(), "udisks2:sdb1").unwrap();
        assert_eq!(
            plan.unmount,
            [(
                format!("{BLOCK_PREFIX}sdb1"),
                "/run/media/scott/KINGSTON".to_string()
            )]
        );
        assert!(plan.lock.is_empty());
        assert_eq!(
            plan.eject.as_deref(),
            Some("/org/freedesktop/UDisks2/drives/Kingston_DataTraveler")
        );
        assert!(plan.power_off.is_some());
    }

    #[test]
    fn ejecting_an_unlocked_volume_unmounts_the_cleartext_and_locks_the_ciphertext() {
        let plan = plan_eject(&snapshot(), "udisks2:dm_2d0").unwrap();
        assert_eq!(plan.unmount.len(), 1);
        assert_eq!(plan.unmount[0].0, format!("{BLOCK_PREFIX}dm_2d0"));
        assert_eq!(plan.lock, [format!("{BLOCK_PREFIX}sde1")]);
    }

    #[test]
    fn a_disk_that_can_only_power_off_is_powered_off_without_an_eject() {
        let plan = plan_eject(&snapshot(), "udisks2:sdc1").unwrap();
        assert_eq!(plan.eject, None);
        assert!(plan.power_off.is_some());
        assert!(plan.unmount.is_empty());
    }

    #[test]
    fn system_loop_and_unknown_volumes_cannot_be_ejected() {
        for id in ["udisks2:nvme0n1p2", "udisks2:loop0"] {
            assert_eq!(
                plan_eject(&snapshot(), id).err(),
                Some(VolumesError::Unsupported),
                "{id}"
            );
        }
        assert_eq!(
            plan_eject(&snapshot(), "udisks2:gone").err(),
            Some(VolumesError::NotFound)
        );
    }

    #[test]
    fn an_empty_snapshot_has_no_volumes() {
        assert!(volumes_from_snapshot(&Snapshot::default()).is_empty());
    }

    #[test]
    fn sizes_are_described_in_decimal_units() {
        assert_eq!(size_description(16_000_000_000), "16.0 GB");
        assert_eq!(size_description(500_000_000_000), "500 GB");
        assert_eq!(size_description(512), "512 B");
    }
}
