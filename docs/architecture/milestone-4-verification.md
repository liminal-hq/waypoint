# Milestone 4 verification

What was checked by hand on the merged milestone 4 build (commit `eb83500`), how, what was found, and what is still open. The headless suites cover the engine, the journal and the pages; this records the passes that need a real desktop, a real file system or a real Recycle Bin.

Status: verified on Linux and Windows 11 with the gaps in §5 · slice 17 (#104)

## 1. How it was run

| Pass       | Where                                                         | How                                                                                                                                                                    |
| ---------- | ------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Linux live | GNOME on Wayland (XWayland available), WebKitGTK              | `bun run tauri:dev` driven over the MCP bridge against `scripts/ops-fixture.sh`, with private `XDG_*` directories so the real Trash and settings were never touched    |
| Windows 11 | The `WinDev2407Eval` VM, a cross-built release `waypoint.exe` | Keyboard and a window-shift pointer script (the guest pointer cannot move, so there is no drag), results read back with PowerShell and the Recycle Bin shell namespace |
| Large tree | 16 cores, 30 GB RAM, btrfs on NVMe and tmpfs                  | `cargo test --release -p waypoint-ops --test large_tree -- --ignored --nocapture`                                                                                      |

## 2. Results

| Check                                                                                            | Linux                                    | Windows                                          |
| ------------------------------------------------------------------------------------------------ | ---------------------------------------- | ------------------------------------------------ |
| New folder (F7), new file (Shift+F7), rename (F2), duplicate (Ctrl+Shift+D)                      | pass                                     | pass                                             |
| Delete to the Trash, Ctrl+Z restores; the item is in the real Trash or Recycle Bin               | pass (`.trashinfo` written)              | pass (Recycle Bin lists the original path)       |
| The Trash view: original path and date, Restore, Delete Permanently, Empty Trash (each confirms) | pass                                     | pass                                             |
| Copy and move of a tree plus a 24 MiB file; the checksum matches                                 | pass                                     | pass                                             |
| Conflict dialog: never defaults to Replace; Keep Both, Replace and a folder merge                | pass                                     | pass (Keep Both)                                 |
| Cancel mid-copy leaves no `.waypoint-partial-*` file                                             | not caught live (see the large-tree row) | pass (4 GiB copy)                                |
| Clipboard: Waypoint to the system and back                                                       | not tested                               | pass (FileDrop, FileNameW, Preferred DropEffect) |
| Settings, app menu, Action bar, palette with undo history, Shelf, batch rename                   | pass                                     | pass (Settings, palette)                         |
| Operations ring and popover, progress, announcements                                             | pass                                     | pass                                             |
| Dialogs on WebKitGTK: modal backdrop, nothing clipped                                            | pass                                     | not applicable                                   |
| Accessibility of the dialogs and popover: names, focus in, Esc, live regions                     | pass, with the contrast finding below    | not tested                                       |
| Drag and drop, outbound drags, paste into an Explorer window, a cross-drive copy                 | not tested                               | not testable on the VM                           |

## 3. Performance on a large tree

100,003 files (100,000 small files in 1,000 folders, plus three 64 MiB files; 745 MB), journalled runs on `LocalProvider`.

| Operation (the whole job)     | btrfs on NVMe                        | tmpfs          |
| ----------------------------- | ------------------------------------ | -------------- |
| Plan                          | 0.12 to 0.18 s                       | 0.12 to 0.18 s |
| Copy, same volume             | 13.1 to 13.7 s (about 7,500 files/s) | 7.96 s         |
| Copy with BLAKE3 verification | **954 s**                            | 10.9 s         |
| Move, same volume             | 0.14 s                               | 0.12 s         |
| Delete (permanent)            | 4.7 s                                | 1.3 s          |
| Undo of the copy              | 4.7 to 5.8 s                         | 1.5 s          |

Peak resident memory was 68 MiB for the whole test, and each job sent 4 to 6 events, so a big tree does not flood the page. A cancel 40% of the way through a copy ended the job in 1.78 s on NVMe (0.45 s on tmpfs), left it `Cancelled` and left no `.waypoint-partial-*` file. The verified copy's cost is a sync of every file before its rename, not the hashing (on tmpfs it is 1.4× the plain copy); it is #134.

## 4. Found and fixed

| Finding                                                                                                                                      | Fix                                                                                                              |
| -------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Finishing a Delete Permanently threw a `TypeError` in the announcer (no `ops.done.delete.*` messages) and the operations UI stopped updating | The messages, and a test that words every kind in both tenses                                                    |
| Restoring or deleting from the Trash read "Restoring /…/Trash\|name"                                                                         | The queue names a trashed source by the name it was trashed under                                                |
| On Windows a file a copy was still writing showed in the folder at its full preallocated size                                                | `.waypoint-partial-…` is listed as hidden on every platform                                                      |
| The Trash view had no keyboard selection (focus stayed on the page body when it opened), and Restore had no key or palette command           | The list takes focus when the view opens; `Ctrl+Shift+R` and palette commands for Restore and Delete Permanently |
| After a typed path was accepted, the list had no keyboard focus                                                                              | The list takes focus once the navigation finishes                                                                |
| After a conflict or error dialog closed, focus fell to the page body                                                                         | Focus goes to the operations button when there is nowhere to return to                                           |
| A nested-conflict dialog named the job's destination, not the folder the clashes are in                                                      | It names the folder the clashing entries share                                                                   |

## 5. Open

- **Verified copy of many small files** takes 16 minutes for 100,000 on btrfs (#134).
- **White on the orange accent** is 2.80:1, below the 4.5:1 AA minimum (#135).
- **Undoing a move to the Trash does not reselect the restored item.** The undo job and the journal summary carry no names or folder, so a fix needs a record tied to the journal id.
- **The undo history still offers "Undo Move X to the Trash" after X was deleted permanently.** The undo fails gracefully with "X is no longer in the Trash"; greying the entry needs the journal to learn of the delete.
- **An F5 conflict that did not open its dialog by itself** was seen once on Linux and not reproduced in later runs (F5, Ctrl+V, the palette and a nested conflict all opened it); its cause is unknown.
- **The release build wrote an empty `Waypoint.log` on Windows** in a whole session. It logs at `Info` and up, so it may simply log little; it has not been looked into.
- **Not verified anywhere:** drag and drop in both directions on Windows, Explorer pastes, a cross-drive copy and cross-volume move on Windows, the per-volume Trash on a second mounted volume, KDE, X11 sessions and fractional scaling.
