# tauri-plugin-waypoint-vfs

Waypoint's file system plugin: it opens listings over the `waypoint-vfs` crate and serves them to the frontend by range, so the webview holds only the rows near the viewport (A9). It also owns places and favourites.

This is a domain plugin, private to Waypoint (see `docs/architecture/crates-and-plugins.md`). Its JavaScript API is the `@liminal-hq/waypoint-plugin-vfs` package in `guest-js/`; the app reaches the plugin only through it, and its wire types come from `@liminal-hq/waypoint-protocol`.

## Status

Built for local folders. `getStatus()` reports `listing`, `watch` and `places`, `trash-view` when the app has given the plugin a Trash that can be browsed, plus `polling-fallback` while any open listing is kept current by polling because the operating system's notifications are unavailable. Linux is exercised by tests; the Windows build is type-checked with clippy for `x86_64-pc-windows-gnu` but has not been run on Windows 11.

## Commands

Every command has a typed function in `guest-js`; the app never calls `invoke` itself. Rejections are `VfsError` objects (see Errors).

| Command (guest-js function)                                               | Does                                                                                                                                                                                                                                                                   |
| ------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `open_listing` (`openListing(location, options?)`)                        | Resolves with the first `ListingSnapshot` at once, in phase `scanning`; the scan runs in the background and reports through events. A missing location rejects with `notFound`, a file with `notADirectory`, unparseable text with `invalidLocation`.                  |
| `get_range` (`getRange(handle, start, count)`)                            | Reads `count` entries from view position `start`; shorter at the end.                                                                                                                                                                                                  |
| `set_sort` (`setSort(handle, sort)`)                                      | Re-sorts (and groups, through the sort's `groupBy`) and resolves with the new snapshot (a higher `revision`; cached pages are stale; `groups` holds the runs of a grouped view).                                                                                       |
| `set_filter` (`setFilter(handle, filter)`)                                | Changes what is hidden and resolves with the new snapshot.                                                                                                                                                                                                             |
| `close_listing` (`closeListing(handle)`)                                  | Closes a listing, cancels a scan in flight and stops its watcher. An unknown handle is not an error.                                                                                                                                                                   |
| `get_home` (`getHome()`)                                                  | The `Location` a window opens at first.                                                                                                                                                                                                                                |
| `list_places` (`listPlaces()`)                                            | `Places`: Home and the user folders that exist (Desktop, Documents, Downloads, Pictures, Music, Videos), then the favourites.                                                                                                                                          |
| `add_favourite`, `remove_favourite`, `rename_favourite`, `move_favourite` | Edit the favourites and resolve with the updated `Places`. Adding one that exists, or removing one that does not, is not an error.                                                                                                                                     |
| `get_status` (`getStatus()`)                                              | `PluginStatus` with the features above.                                                                                                                                                                                                                                |
| `check_folder` (`checkFolder(location)`)                                  | `FolderCheck` (`isFolder`, `writable`) for a destination picker: whether a location is a folder and can be written to (its provider writes and its permissions allow it). A missing location rejects with `notFound`, one that cannot be seen with `permissionDenied`. |
| `get_trash_info` (`getTrashInfo()`)                                       | `TrashInfo`: whether the Trash can be browsed here, why not, and how many items it holds, for the sidebar's Trash place. Reading it lists the Trash.                                                                                                                   |

The `trash` scheme (`trash:/`, `trash:/{percent-encoded receipt id}`) is served by a read-only provider over a `TrashSource` the app gives the plugin with `Vfs::set_trash_source` (plugins never call each other); without one, opening it is `unsupported`. Its snapshots are `readOnly` with layout `trash`, and `open_entry` refuses Trash items with `unsupported`.

Handles are numbered from 1 per plugin instance and belong to the window that opened them: another window's handle is `staleHandle`, and every listing of a window is closed (scan cancelled, watcher stopped) when that window is destroyed.

## Events

One event name, `waypoint-vfs://listing`, emitted only to the window that owns the listing. The payload is a `ListingEvent` (tagged by `kind`). `onListingEvent(handler)` listens on the current webview window and resolves to an unlisten function; subscribe first, then open listings.

| `kind`     | Payload                                           | When                                                                                                                                                                                          |
| ---------- | ------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `progress` | `handle`, `revision`, `phase`, `scanned`, `count` | During a scan about every 50 ms, counts only and never rows; once more with phase `ready` and the final count when the scan ends; with phase `rescanning` when the watcher asks for a rescan. |
| `changed`  | `handle`, `revision`, `count`, `ops`              | The view changed under a live watcher. Apply `ops` in order to cached pages.                                                                                                                  |
| `failed`   | `handle`, `error`                                 | The scan failed (for example permission denied) or the folder went away.                                                                                                                      |

No event is emitted for a listing after it is closed.

## Errors

Commands reject with a `waypoint_protocol::VfsError` serialised as its tagged object (`{ "kind": "notFound", "location": { … } }`), never a string, so `isVfsError` recognises it. A fault inside the plugin (a background task that panicked) arrives as `{ "kind": "io", "message": … }`.

## Places and favourites

- **Linux.** The user folders come from `~/.config/user-dirs.dirs` (`$XDG_CONFIG_HOME` respected) with `$HOME/Desktop` and the like as fallbacks; a folder that does not exist, or that the file points at `$HOME` (disabled), is left out. Favourites are the lines of `~/.config/gtk-3.0/bookmarks` (`file:///uri Optional Label`), the file GTK and other file managers share.
- **Windows.** The user folders are the Known Folders, read through the `dirs` crate. There is no shared bookmarks file, so favourites live in `%APPDATA%\Waypoint\favourites`, in the same line format.
- Edits keep every other line (other apps' bookmarks, comments, non-`file` URIs) exactly as found, and write by renaming a finished temporary file over the original, so a crash never leaves half a file.

## Permissions

`waypoint-vfs:default` grants every command above. Opening goes through the opener plugin's Rust API, so the webview needs no `opener:` permission. The main window and the window-chrome capability already include it.

## Adding a command

Each command is one function in `src/commands.rs` that resolves `(window, handle)` through `Registry::get` and calls the crate; register it in `lib.rs`, list it in `build.rs` and `permissions/default.toml`, and add a wrapper in `guest-js/index.ts`.
