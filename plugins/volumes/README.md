# @liminal-hq/plugin-volumes

Lists the volumes of the system (internal disks, USB sticks, optical discs, loop devices, encrypted volumes, network shares and mapped drives) with their free space, and mounts, unmounts, ejects and unlocks them. On Linux it talks to UDisks2 over the system bus, so polkit asks the person through the desktop's own agent, and it reads `/proc/self/mountinfo` for the mounts UDisks2 does not manage (NFS, SMB, `sshfs` and other FUSE mounts). On Windows it uses the drive and network APIs.

The plugin knows nothing about the app around it. It reports what works through `getStatus()`, so an interface can hide what does not.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-volumes = "0.1"

# Alternatively with Git:
tauri-plugin-volumes = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_volumes::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Grant the windows that use it `volumes:default` in a capability file, or the individual `volumes:allow-list`, `volumes:allow-refresh-space`, `volumes:allow-mount`, `volumes:allow-unmount`, `volumes:allow-eject`, `volumes:allow-unlock` and `volumes:allow-get-status`.

### JavaScript

```bash
pnpm add @liminal-hq/plugin-volumes
```

## Usage

### JavaScript

```typescript
import {
	eject,
	getStatus,
	hasFeature,
	isVolumesError,
	list,
	mount,
	onChanged,
	refreshSpace,
	unlock,
	unmount,
} from '@liminal-hq/plugin-volumes';

const status = await getStatus();
if (hasFeature(status, 'list')) {
	let volumes = await list(); // { id, label, kind, fileSystem, mountPoint, uri, total, free, canMount, … }

	// The whole list again on every change; ignore an event that is not newer than the last one seen.
	let seen = 0;
	const stop = await onChanged(({ revision, volumes: next }) => {
		if (revision <= seen) return;
		seen = revision;
		volumes = next;
	});

	try {
		await unmount(volumes[0].id);
	} catch (error) {
		if (isVolumesError(error) && error.kind === 'busy') {
			show(`In use by ${error.by ?? 'something'}`); // `by` names the program when it could be found
		}
	}
}

const mountPoint = await mount(id); // where it is mounted
const { id: unlocked, remember } = await unlock(lockedId, passphrase, true); // the id of the volume that appears (mount it next) and what became of the request to remember the passphrase
await eject(id); // unmounts everything on the drive, then ejects or powers it off
const nas = await refreshSpace(networkId); // network volumes are measured only when asked
```

### Rust

The same operations are on the handle:

```rust
use tauri_plugin_volumes::VolumesExt;

let volumes = app.volumes().list(false).await?;
let mount_point = app.volumes().mount(&volumes[0].id).await?;
```

## API

| Function                                                      | What it does                                                                                                                                                                                                                                                                                    |
| ------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `getStatus()`                                                 | `{ available, reason, message, flavour, features }`; see below.                                                                                                                                                                                                                                 |
| `list(measure?)`                                              | Every volume now. Mounted local volumes are measured within the timeout; with `measure` true network and FUSE mounts are too.                                                                                                                                                                   |
| `refreshSpace(id)`                                            | Measures one volume of any kind within the timeout and returns it.                                                                                                                                                                                                                              |
| `mount(id)`                                                   | Mounts a volume and returns its mount point.                                                                                                                                                                                                                                                    |
| `unmount(id)`                                                 | Unmounts a volume.                                                                                                                                                                                                                                                                              |
| `eject(id)`                                                   | Unmounts every volume on the drive, locks what is unlocked, then ejects the drive and powers it off as it allows.                                                                                                                                                                               |
| `unlock(id, passphrase, remember?)`                           | Unlocks an encrypted volume and returns `{ id, remember }`: the id of the volume that appears and what became of a request to remember the passphrase (`notAsked`, `remembered`, or `failed` with a reason and a sentence). The passphrase is sent once and is never kept by the plugin itself. |
| `forget(id)`                                                  | Forgets the passphrase kept for an encrypted volume; true when there was one.                                                                                                                                                                                                                   |
| `onChanged(handler)`                                          | Listens for `volumes://changed`, which carries `{ revision, volumes }`. Revisions start at 1 and only grow. The event is sent when the list changes, not when only free space does, and a burst of changes is announced once, 100 ms after it settles.                                          |
| `hasFeature(status, name)`, `featureReason`, `featureMessage` | Read the status.                                                                                                                                                                                                                                                                                |

A `Volume` is `{ id, label, kind, fileSystem, mountPoint, uri, total, free, canMount, canUnmount, canEject, canPowerOff, locked, isSystem, device }`. `kind` is `internal`, `removable`, `optical`, `network`, `loop` or `encrypted`. `id` is opaque: use it only to name the volume in the commands. `total` and `free` are bytes or `null`. `isSystem` marks a fixed, internal device. A hidden partition (`HintIgnore` in UDisks2) is not listed.

Errors are `VolumesError` objects with a `kind`: `busy` (with `by`, the program that holds the volume, or `null`), `notAuthorised` (polkit refused, or the person dismissed its prompt), `wrongPassphrase`, `unsupported`, `notFound` and `io` (with `message`).

### Free space never blocks the list

Each measurement (`statvfs`, `GetDiskFreeSpaceExW`) runs on a thread of its own with a timeout (2 seconds by default; `Options::space_timeout`), all within one shared deadline. A volume that does not answer is listed with `free: null` and the plugin does not start another measurement for it while the first is still stuck. Network and FUSE mounts, which are the ones that stall, are not measured at all unless asked.

## Features and status

`getStatus()` returns `{ available, reason, message, flavour, features }`, where each feature is `{ name, available, reason, message }`. Decide behaviour from the features, never from the platform.

| Feature    | Meaning                                                           |
| ---------- | ----------------------------------------------------------------- |
| `list`     | Volumes can be listed.                                            |
| `mount`    | A volume can be mounted.                                          |
| `unmount`  | A volume can be unmounted.                                        |
| `eject`    | A drive can be ejected (or powered off).                          |
| `unlock`   | An encrypted volume can be unlocked.                              |
| `watch`    | `volumes://changed` is sent when something is plugged.            |
| `remember` | The host can keep passphrases and read them back now (see below). |

`flavour` is `udisks2`, `mountinfo`, `windows` or `unsupported`. A `reason` is a code to branch on and `message` a sentence for people:

| Reason                 | Meaning                                                                                                                          |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `no-system-bus`        | There is no system D-Bus to reach.                                                                                               |
| `udisks2-missing`      | The bus is there, UDisks2 is not installed or cannot be started.                                                                 |
| `flatpak-sandbox`      | A Flatpak without access to the system bus (grant `--system-talk-name=org.freedesktop.UDisks2`).                                 |
| `udisks2-failed`       | UDisks2 answered but would not describe its objects.                                                                             |
| `unsupported-platform` | This operating system has no volume support in the plugin.                                                                       |
| `no-keyring`           | There is no keyring to keep passphrases in (`remember`).                                                                         |
| `keyring-locked`       | The keyring is locked and was not unlocked (`remember`).                                                                         |
| `disabled`             | The host has remembering turned off (`remember`).                                                                                |
| `not-configured`       | The host gave the plugin no place to keep passphrases (`remember`).                                                              |
| `not-supported`        | The system has no such operation: Windows mounts drives by itself, has no unmount apart from eject and unlocks BitLocker itself. |

## Platform notes

### Linux (`udisks2` and `mountinfo`)

- The plugin reads UDisks2's whole object tree (`GetManagedObjects`) and maps it to volumes in a pure module, so the mapping is tested from a recorded snapshot without a bus or a device. Block devices with a file system, locked encrypted devices (shown once, as the cleartext device when unlocked), loop devices and optical discs are volumes; hidden partitions (`HintIgnore`), swap, empty drives and the ciphertext of an unlocked volume are not.
- A device mounted several times (btrfs subvolumes, bind mounts) shows the mount nearest the root. The system's own mounts (`/`, `/boot`, `/boot/efi`, `/usr`, `/var`) cannot be unmounted.
- Every call passes `auth.no_user_interaction = false`, so polkit may ask. A refusal is `notAuthorised`.
- A busy device is `busy`; the plugin names the holder by looking for a process whose working folder or open files are inside the mount, in `/proc` (a process of another user is not visible, so the name can be missing but is never wrong).
- `eject` works on the drive: it unmounts every mount on it, locks unlocked encrypted volumes, calls `Drive.Eject` where the drive can eject and `Drive.PowerOff` where it can power off. Loop devices and fixed disks cannot be ejected.
- Changes come from UDisks2's signals (`InterfacesAdded`, `InterfacesRemoved`, `PropertiesChanged`) and from the kernel's notice that the mount table changed (`poll` on `/proc/self/mountinfo`), so a mount that UDisks2 never hears of still shows up.
- Mounts that are not UDisks2's are listed from the mount table: NFS, SMB, `9p`, WebDAV and every FUSE mount (`sshfs`, `rclone`, …) as `network`, with a `smb://`, `nfs://` or `sftp://` URL where the scheme is standard. Desktop plumbing (GVFS, the document portal, AppImage mounts), pseudo file systems, `tmpfs` and `squashfs` are not volumes. They have no actions.
- Without UDisks2 the list is the mount table alone (block devices included, once each) and `mount`, `unmount`, `eject` and `unlock` are unavailable with the reason.
- The mapping does not remember the passphrase of `unlock` or write it anywhere. Do not enable `trace` logging for `zbus`, which prints message bodies.

### Windows (`windows`)

- Volumes are the drive letters: fixed and RAM disks are `internal`, USB drives `removable`, CD and DVD drives `optical` and mapped drives `network` (with the remote name as `device` and an `smb://` URL where it is an SMB share). The label is shown as Explorer shows it, `Data (D:)`.
- The volume information of a drive with no disc, or the name of a stalled mapped drive, is asked on a worker thread with a timeout, so one dead drive does not stall the list; a missing disc is an error rather than a dialog.
- `eject` is the shell's `Eject` verb on the drive, as Safely Remove in Explorer does it, on a single-threaded apartment of its own. It cannot say what holds a drive open. A USB hard disk reports itself as a fixed drive and is not offered for ejecting.
- `mount`, `unmount` and `unlock` are reported unavailable (`not-supported`).
- Changes come from `WM_DEVICECHANGE` to a message-only window on a thread of its own, registered for volume interface notifications, plus a three-second comparison of the drive letters, because no notification announces a mapped drive.
- Cross-checked with `cargo xwin` and exercised in a Windows 11 virtual machine through the ignored `live_volumes` test.

### Other systems

Every feature is reported unavailable with `unsupported-platform` and the list is empty.

## Testing

`cargo nextest run -p tauri-plugin-volumes` runs headless: the UDisks2 mapping from a JSON snapshot, the mount-table parser from a fixture, the timeout with an injected slow measurement, the debounce over an injected clock and the commands through Tauri's mock runtime over a fake backend. Nothing mounts, unmounts, ejects or unlocks anything, and nothing needs a bus.

`cargo test -p tauri-plugin-volumes live_list -- --ignored --nocapture` (Linux) prints this machine's volumes. It only reads.
