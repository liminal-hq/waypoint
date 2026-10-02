# @liminal-hq/plugin-trash

Moves files to the trash, lists the trash, restores items, deletes them for good, empties the trash and expires old items. On Linux it works on the freedesktop.org Trash itself, so what one file manager trashes shows up in another (Nemo, Nautilus, Dolphin). On Windows it uses the Recycle Bin.

The plugin knows nothing about the app around it. It reports what works through `getStatus()`, so an interface can hide what does not.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-trash = "0.1"

# Alternatively with Git:
tauri-plugin-trash = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_trash::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Grant the windows that use it `trash:default` in a capability file, or the individual `trash:allow-trash`, `trash:allow-list`, `trash:allow-restore`, `trash:allow-delete`, `trash:allow-empty` and `trash:allow-get-status`.

### JavaScript

```bash
pnpm add @liminal-hq/plugin-trash
```

## Usage

### JavaScript

```typescript
import {
	deleteItem,
	empty,
	getStatus,
	hasFeature,
	isTrashError,
	list,
	restore,
	trash,
} from '@liminal-hq/plugin-trash';

const status = await getStatus();
if (hasFeature(status, 'trash')) {
	// One outcome per path, in order; one failure does not stop the others.
	const outcomes = await trash(['/home/me/a.txt', '/home/me/old-folder']);
	for (const outcome of outcomes) {
		if (outcome.status === 'trashed') remember(outcome.receipt);
		else report(outcome.error); // { kind: 'notFound' | 'permissionDenied' | 'trashUnavailable' | 'refused' | … }
	}
}

if (hasFeature(status, 'list')) {
	const items = await list(); // oldest first: { receipt, name, originalPath, deletedAt, size, isDir }
	try {
		await restore(items[0].receipt); // back where it was
	} catch (error) {
		if (isTrashError(error) && error.kind === 'originExists') {
			// Never overwritten: offer another place.
			await restore(items[0].receipt, { kind: 'path', path: '/home/me/restored-a.txt' });
		}
	}
	await deleteItem(items[1].receipt); // gone for good
	const report = await empty(30); // only items trashed 30 or more days ago; `empty()` empties everything
}
```

### Rust

The same operations are on the handle:

```rust
use tauri_plugin_trash::{RestoreTarget, TrashExt};

let results = app.trash().trash(vec!["/home/me/a.txt".into()]).await; // Vec<Result<TrashReceipt, TrashError>>
let items = app.trash().list().await?;
app.trash().restore(&items[0].receipt, RestoreTarget::Original).await?;
```

## API

| Function                                                  | What it does                                                                                                                                                                                |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `getStatus()`                                             | `{ available, reason, flavour, features }`; see below.                                                                                                                                      |
| `trash(paths)`                                            | Moves absolute paths to the trash as one batch and returns a `TrashOutcome` for each: `{ status: 'trashed', receipt }` or `{ status: 'failed', error }`. A link is trashed, not its target. |
| `list()`                                                  | Every item in the trash, oldest first.                                                                                                                                                      |
| `restore(receipt, target?)`                               | Puts an item back at its original path (`{ kind: 'original' }`, the default) or at a full path (`{ kind: 'path', path }`). Never overwrites.                                                |
| `deleteItem(receipt)`                                     | Removes one item for good. (`delete` in Rust and in the command names.)                                                                                                                     |
| `empty(olderThanDays?)`                                   | Empties the trash, or only items trashed that many days ago or earlier. Returns `{ removed, failed }`; an item that cannot be removed stays and is listed in `failed`.                      |
| `hasFeature(status, name)`, `featureReason(status, name)` | Read the status.                                                                                                                                                                            |

A `TrashReceipt` is `{ trashId, originalPath, deletedAt }` (`deletedAt` in Unix seconds). `trashId` is stable across restarts and across other apps' changes until the item leaves the trash, so a receipt can be kept (for an undo journal, say) and used later. The plugin only acts on receipts that name a trash it knows, never on an arbitrary path.

Errors are `TrashError` objects with a `kind`: `notFound`, `permissionDenied`, `trashUnavailable` (with `reason`: no usable trash for that file; nothing was moved), `refused` (with `reason`: the home folder, a mount point, a drive root or something in the trash), `originExists` and `originMissingParent` (with `path`), `unsupported` and `io` (with `message`).

## Features and status

`getStatus()` returns `{ available, reason, flavour, features }`, where each feature is `{ name, available, reason }`. Decide behaviour from the features, never from the platform.

| Feature      | Meaning                                                                              |
| ------------ | ------------------------------------------------------------------------------------ |
| `trash`      | Files can be moved to the trash.                                                     |
| `list`       | The trash can be read.                                                               |
| `restore`    | Items can be put back.                                                               |
| `empty`      | The trash can be emptied and single items deleted.                                   |
| `expiry`     | `empty(olderThanDays)` can remove items by age.                                      |
| `per-volume` | Files on other drives or volumes are trashed on their own volume, not copied across. |

`flavour` is `freedesktop`, `portal`, `windows` or `unsupported`.

## Platform notes

### Linux (`freedesktop`)

The plugin implements the [freedesktop.org Trash specification](https://specifications.freedesktop.org/trash-spec/latest/) itself:

- **Home trash** at `$XDG_DATA_HOME/Trash` (`~/.local/share/Trash`), with `files/`, `info/` and `directorysizes`, for files on the same device as it. `.trashinfo` files hold an absolute, percent-encoded `Path` and a local-time `DeletionDate`.
- **Per-volume trashes** for files on other devices: `$topdir/.Trash/$uid` when `$topdir/.Trash` exists, is a real folder (not a link) and has the sticky bit, otherwise `$topdir/.Trash-$uid`, created with mode 0700. `Path` is relative to the top directory. Listing and emptying cover the home trash and every mounted volume's trash.
- A trash is **trusted only if this user owns it**: the trash directory, `files` and `info` must each be a real folder (not a link) owned by the user, with no group or other write permission. On a shared top directory another user can create `.Trash-$uid` or `.Trash/$uid` before this user does; such a folder is not used (the shared `.Trash` falls back to `.Trash-$uid`, and a refused per-user trash is `trashUnavailable`) and is neither listed nor emptied. Removal walks through directory descriptors with `O_NOFOLLOW`, so a folder swapped for a link mid-walk is unlinked and never followed.
- A **network mount** (NFS, SMB, `sshfs`, …) is not asked for its device until a file under it is trashed, and listing or emptying skips one that does not answer within two seconds, so one hung server does not stall the trash.
- Commands that change a trash (`trash`, `restore`, `delete`, `empty`) take turns within the process, so an `empty` cannot remove the `.trashinfo` of an item that a `trash` is still placing.
- A file is **never copied across devices**. If its volume has no usable trash, `trash` fails with `trashUnavailable` and the file stays where it is.
- Names that are taken get a number before the extension (`photo.2.jpg`). The `.trashinfo` is created first and exclusively, which reserves the name, and is removed again if the move fails.
- Paths that are not UTF-8 survive (every byte is percent-encoded). The JSON `originalPath` shows such bytes lossily; use the receipt to act on the item.
- `restore` refuses to overwrite and never creates folders. Restoring to another path must stay on the volume the item is trashed on.
- Refused on purpose: the home folder and its parents, mount points and folders holding one, the trash itself and anything in it.
- Emptying removes each item and then its `.trashinfo`, so an interruption never leaves a listed item without its origin. Items another tool left without a `.trashinfo` are removed when emptying everything.

### Flatpak (`portal`)

Inside a Flatpak sandbox (`/.flatpak-info` exists) the plugin trashes through `org.freedesktop.portal.Trash`, which can only trash. `list`, `restore`, `empty`, `expiry` and `per-volume` are reported unavailable with a reason, and their calls fail with `unsupported`. The portal path was written against the portal's documentation and is not exercised by the automated tests.

### Windows (`windows`)

- `trash` uses `IFileOperation` with `FOF_ALLOWUNDO`, `FOFX_RECYCLEONDELETE`, `FOF_SILENT`, `FOF_NOERRORUI` and `FOFX_EARLYFAILURE`, as one batch. `FOF_NOCONFIRMATION` is not set on purpose: together with `FOF_ALLOWUNDO` it answers "this file is too large for the Recycle Bin, delete it permanently?" with yes. In that rare case the shell asks instead.
- `list` reads the Recycle Bin shell folder, so every drive's bin is covered. An item's `trashId` is the path of its file in the bin (`C:\$Recycle.Bin\S-1-5-21-…\$R1A2B3C.txt`).
- `restore` checks the destination itself, then moves the item out with `IFileOperation::MoveItem`. (The shell's "undelete" verb answers conflicts with dialogs.)
- `empty()` uses `SHEmptyRecycleBinW` without prompts, progress or sound; the age sweep lists the bin and deletes the old items.
- Paths are resolved before they are judged: `.` and `..` are resolved lexically, and the folders above the last part are resolved through links and short names (the last part never is, so a link is trashed itself). `C:\Users\me\Documents\..` is therefore refused as the profile folder. Mount points (a volume mounted in a folder) are not recognised on Windows, only drive roots, the profile folder and its parents, and the Recycle Bin.
- The shell calls run on the main thread, which the plugin blocks for the duration of an operation. Walking a folder for its size does not: `list` reports folder sizes from a blocking thread after the shell calls return.
- **Caveats, not verified beyond the live test:** the flags above are meant to make an item that cannot be recycled fail rather than be deleted permanently, but if the shell does ask ("too large for the Recycle Bin, delete permanently?") the prompt is modal and blocks the main thread until answered. `restore` passes no owner window, so a prompt the shell raises has no parent. Run `live_recycle_bin` on Windows after changing the flags.

### Other systems

`unsupported`: every feature is unavailable and every call fails with `unsupported`.

## Tests

`cargo test -p tauri-plugin-trash` runs the freedesktop logic in temporary directories with fake mounts and devices (nothing touches the real trash), plus the plugin through Tauri's mock runtime. The Windows backend's pure parts are unit-tested everywhere. `cargo test -p tauri-plugin-trash live_recycle_bin -- --ignored --nocapture` drives the real Recycle Bin on Windows; set `TRASH_LIVE_EMPTY=1` to also empty it, which removes the user's own items.

## Security

`trash(paths)` and `restore` with `{ kind: 'path', path }` act on paths the caller builds. Grant `trash:default` only to the main windows, and let the operations engine be the caller, which applies the protected-path rules of its own before it reaches this plugin. A window that can run arbitrary script (an extension surface, a webview showing remote content) must not be given the capability.

## Types

All JSON is camelCase. TypeScript bindings are generated by ts-rs into `guest-js/bindings`.

## Graduating to `tauri-plugins-workspace`

- [ ] Copy `plugins/trash` into `plugins/` there and add it to `members`.
- [ ] Add a Rust and a JavaScript entry to `.changes/config.json`.
- [ ] Restore the `"prepare": "pnpm build"` script in `package.json` and run `pnpm install`.
- [ ] Run Prettier with that repository's settings (`trailingComma: es5`).
- [ ] Run `cargo clippy -D warnings`, `cargo test` and `pnpm build`.
- [ ] Check `ashpd` and `windows` resolve to the versions the other plugins there use.
- [ ] Switch the app from the path dependency to a versioned or tagged one and delete the in-repo copy.

## License

Apache-2.0 OR MIT
