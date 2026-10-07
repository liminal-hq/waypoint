# Search, terminal and tags

Status: **proposed** (2026-10-06; milestone 7, slice 0) · the contract every feature of milestone 7 builds on · decisions A120 to A127 in [`decisions.md`](decisions.md) and D210 to D216 in [`../decisions.md`](../decisions.md) · the search engine and its budgets from spike #315 ([`milestone-7-spikes.md`](milestone-7-spikes.md)); the terminal renderer and PTY approach **pending spike #316**

Milestone 7 adds search (SPEC §5.3a) with smart folders and an optional index, a built-in terminal (SPEC §5.2 Terminal tab, §13), tags and comments shared with other file managers (SPEC §5.4, §5.7, §13a), editing of tags, comments and permissions in the Inspector, and the Flow integration. This document fixes how they meet the rest of Waypoint: the query model, results as listings, `search:` locations and smart folders, the index and its settings, search over providers, where tags live, the PTY plugin's outline, the Flow boundary and the order of work.

## 1. Principles

- **Search is complete without an index.** The walk is the engine; an index, when a person turns it on, only makes it faster (the owner's decision, D210, A120). A search gives the same results with the index off, on, still building or out of date.
- **Results are a listing.** A search is opened like a folder and shown by the same list and grid, with the same selection, sorting, grouping, drag engine, operations, thumbnails and Inspector (SPEC §5.3a). Nothing in the file area learns a second way to show entries.
- **Never hydrate a cloud placeholder.** Search, the index, tag reads and snippets decide from the attributes the listing already holds and never open a file or folder whose data is not on this machine (A64, A70, `docs/os-integrations.md` Windows table).
- **Stoppable and honest.** Every search streams, reports progress, stops within 50 ms and says what it passed over (unreadable folders, binary and large files, placeholders, other volumes), in words.
- **Rust owns state.** Smart folders, the index, the tag catalogue and the sidecar have one writer each, a revision and events; the page renders them.
- **Reusable first.** The PTY is a reusable plugin with no Waypoint imports; the tag sidecar and the search engine are pure crates.

## 2. Search

### 2.1 Crates and plugins

| Part                           | Kind          | Owns                                                                                                                                                                                                                                                                                                                                                             |
| ------------------------------ | ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `waypoint-search`              | pure crate    | The query model and its `ts-rs` wire types; scope resolution; the local walker (`ignore`) and the walker over other providers (`Provider::list_batches`); name, glob and regex matching; content search (`grep-searcher`) with snippets; `SearchProvider`, the `waypoint_vfs::Provider` for the `search` scheme; the smart-folder store; the optional index (§4) |
| `tauri-plugin-waypoint-search` | domain plugin | Holds the `SearchService` (the search pool, the index service, the smart folders) behind one lock; commands for status, query validation, smart folders and the index; events; the Services panel entry (`get_status`)                                                                                                                                           |
| `waypoint-path`                | pure crate    | `VfsPath::Search(SearchPath)`, the canonical `search:` URI (§3)                                                                                                                                                                                                                                                                                                  |
| `waypoint-vfs`                 | pure crate    | `ListingLayout::Search` (a Location column), `ScannedEntry::found` (where a result really is), the scan's search progress and `stop_scan`                                                                                                                                                                                                                        |
| `src-tauri`                    | composition   | Builds the `SearchService` with the provider registry and the settings, registers its `SearchProvider` into the vfs registry (like the archive provider, A103), and gives the plugin the same service                                                                                                                                                            |

The search crate depends on `waypoint-vfs` and `waypoint-path`; the plugin does not call the vfs plugin. A search reaches the page through the vfs plugin's listing commands, because its results are a listing of a `search:` location served by `SearchProvider`. Issue #319's `start` and `stop` are therefore the vfs plugin's `list` of a `search:` location and its `stop_scan`; the search plugin keeps `get_status` and its own stores.

### 2.2 The query

```rust
// Illustrative shape; #318 fixes the names. All of it is ts-rs exported.
pub struct SearchQuery {
    pub scope: SearchScope,
    pub text: String,            // what was typed; empty with filters only lists what the filters match
    pub syntax: NameSyntax,      // Substring (default), Glob, Regex
    pub match_case: bool,        // false: ignore case (Unicode simple folding)
    pub hidden: bool,            // include hidden entries (the platform's rule)
    pub contents: bool,          // search inside files (local files only in milestone 7)
    pub ignore_files: bool,      // respect .gitignore, .ignore and the global Git excludes (off by default)
    pub filters: Vec<SearchFilter>, // all must hold
}
pub enum SearchScope {
    Folder { location: Location, subfolders: bool }, // "this folder" and "with subfolders"
    Home,
    Workspace { id: WorkspaceId },                   // the workspace's folders
    OpenTabs,                                        // the folders of this window's tabs
    Place { location: Location },                   // a chosen place, with its subfolders
}
pub enum SearchFilter {
    Kind(Vec<KindGroup>),        // folder, document, image, audio, video, archive, code, other (from IconGroup, D138)
    Modified(TimeRange),         // today, this week, this month, this year, a range
    Size(SizeRange),
    Tags(Vec<String>),           // arrives with #330/#331; every listed tag must be on the entry
    Permissions(PermissionFilter), // executable, read-only, writable by others
}
```

- **Scope resolution.** A scope becomes a set of root locations when the search starts: a workspace's folders and the window's tab folders are deduplicated and a root inside another is dropped. Roots on other schemes are searched through their providers (§5). "Open tabs" means the tabs of the window the search runs in.
- **Matching.** Names are matched as the entry's display name: a substring with `memchr::memmem`, a glob with `globset` (whole name, `*` does not cross `/`), a regex with `regex`. An invalid glob or regex is a typed error with the position, and `validate_query` lets the bar say so while typing, before Enter.
- **Contents.** `contents` searches each file's text with `grep-searcher` and `grep-regex` (the typed text as a literal, or the regex); a file is listed at its first match, with that line (up to 160 bytes, cut at a character boundary) as the snippet and the match's byte range for highlighting. Binary files (a NUL in what was read) and files over the size cap (`search.contentMaxFileMib`, 64 by default) are passed over and counted; UTF-16 files with a byte order mark are read through `encoding_rs_io`.
- **Filter-as-you-type in the current folder** is not a search: it stays the listing's own `Filter` (A33). Enter starts a search of the folder with its subfolders.

### 2.3 Results as a listing

A search is the listing of a `search:` location (§3). `SearchProvider::list_batches` resolves the scope, walks and hands over batches of `ScannedEntry` (at most 256 entries or 50 ms, whichever comes first), each with `found: Some(FoundIn { location, folder, snippet, ranges })`: the entry's real location, its folder as people read it (the Location column), and the snippet and highlighted ranges. The listing's index sorts and filters them like any other (A33, A115's `Index::extend`), so results arrive in sort order.

- **The real location.** A listing resolves an entry of a `search` listing through `found.location`, not through its folder and name, so opening, Open Containing Folder, drag and drop, the clipboard, operations, thumbnails (`thumb://` resolution, D130), the Inspector and Properties act on the file itself.
- **Layout.** `ListingLayout::Search` shows Name, Location, Size, Modified and Kind; grouping by folder uses the existing group headers (D128) with `GroupBy::Folder`.
- **Progress.** The scan's progress events carry `SearchProgress { folders, files, bytes, current_folder, skipped }`, where `skipped` counts unreadable folders, binary files, files over the cap, cloud placeholders and other volumes. They come at least every 250 ms while the search runs. The final event (A18) says `Complete`, `Stopped` or `Failed { error }` with the totals.
- **Stop.** `stop_scan(handle)` cancels the provider's scan and keeps what was found; closing the listing (the tab navigates or closes) cancels it too. Every worker checks the cancel between entries and reads files through a cancel check, so it stops within 50 ms (measured 1.3 to 3.4 ms, A120).
- **Changes after the search.** The results listing watches the folders its results are in, up to 1,024 folders, so a rename, trash or move of a result updates its row. Beyond that bound rows refresh on Refresh (F5), which reruns the search, and the status bar says the results may be out of date. New files that would match appear on Refresh, not by themselves (milestone 7).
- **Threads.** The `SearchService` owns a search pool of the available parallelism, at most 8 threads, apart from the operations pool, so a search never delays a copy and a copy never blocks a search. One search runs per tab; a new search in the tab cancels the old one.

### 2.4 Errors

The search provider reports listing errors as `VfsError` (A80) so the page's existing states apply: a scope that is gone is `NotFound`, a scope that cannot be read is `PermissionDenied`, a server that has gone is `Disconnected`. One variant is added: `InvalidQuery { reason, at }` (`reason`: `BadRegex`, `BadGlob`, `ContentsNotHere`, `EmptyQuery`) with a byte offset into the typed text where it applies. Problems inside the walk are counts (`skipped`), never errors: one unreadable folder does not fail a search.

### 2.5 What a search passes over

From the spike's policies (A120): symlinks are never followed (a link's own name can match); a search stays on the volume of each root (`same_file_system`), so a FUSE or network mount inside Home is searched only as a scope of its own; cloud placeholder files match by name and are never opened, and placeholder folders are not entered; the Trash's own folders (the freedesktop `Trash` folders, `$RECYCLE.BIN`) are skipped, and the Trash is searched as `trash:/` (by name); archives are files and are entered only when the scope is inside one (§5).

### 2.6 Budgets

The budgets are A120's: a warm name search of 500,000 entries gives its first result in under 100 ms and is done in under 1 s on 4 threads; with a complete index the first result is under 50 ms; content progress comes at least every 250 ms; every worker stops within 50 ms; a search uses under 64 MB above the app. The milestone's performance pass (#340) measures them through the whole pipeline, which the spike did not.

## 3. `search:` locations and smart folders

A search is a location, so tabs, history, Recently Closed, pairs, workspaces and session restore hold it like any other (A19, A71), and a restored tab reruns its search when it is shown.

| Form                 | Meaning                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `search:?{query}`    | An ad hoc search: the whole query in the URI. Keys in this order, each only when not the default: `v=1`, `in` (the scope: `home`, `tabs`, `workspace:{id}`, or a percent-encoded location URI), `sub=0` (this folder only), `q` (the text), `syntax=glob\|regex`, `case=1`, `hidden=1`, `contents=1`, `ignore=1`, and one key per filter (`kind=image,video`, `modified=2026-01-01..2026-06-30`, `size=1048576..`, `tag=red`, `perm=exec`) |
| `search:/saved/{id}` | A smart folder: its query is the stored one, so renaming or editing it keeps every tab, favourite and history entry that points at it                                                                                                                                                                                                                                                                                                      |

`waypoint-path` writes and reads both; the page never builds a `search:` URI by hand (it asks the search plugin to encode a `SearchQuery`). The display name is the smart folder's name, or "Search: {text} in {scope}".

**Smart folders** (D212) are a `SmartFolders` store in `waypoint-search`: an ordered list of `{ id, name, query, created_ms }` with a revision, held by the search plugin with its own lock and event (`waypoint-search://smart-folders`, carrying the revision and what changed), saved as `smart-folders.json` in the app data directory (versioned and rotated like `session.json`), shared by every window, and one more part of Export settings (the settings backup already exports folder views and saved connections as parts). Commands: `list_smart_folders`, `save_smart_folder(name, query) → id`, `rename_smart_folder`, `update_smart_folder(id, query)`, `move_smart_folder`, `remove_smart_folder`. A removed smart folder's `search:/saved/{id}` opens as "This smart folder was removed", with its query shown, so a tab that held it is not blank. The sidebar's Smart folders section lists them (rename with F2, reorder, remove), and Go to… (#322) lists them with recent and matching folders.

## 4. The optional index

### 4.1 What it is

One SQLite database (`rusqlite` with the bundled SQLite, FTS5 on) in `{app data}/search-index/` (a folder, so the write-ahead log sits with it), owned by the `SearchService`'s index thread. It holds **names and metadata only**: contents are never indexed in milestone 7.

```sql
-- schema version 1 (PRAGMA user_version = 1); illustrative, #430 fixes it
CREATE TABLE roots   (id INTEGER PRIMARY KEY, uri TEXT UNIQUE, state INT, entries INT, scanned_ms INT, reason TEXT);
CREATE TABLE dirs    (id INTEGER PRIMARY KEY, root INT, parent INT, uri TEXT UNIQUE, mtime_ns INT);
CREATE TABLE entries (id INTEGER PRIMARY KEY, dir INT, name TEXT, folded TEXT, kind INT, size INT, mtime_ms INT, hidden INT, placeholder INT);
CREATE VIRTUAL TABLE names USING fts5(folded, content='entries', content_rowid='id', tokenize='trigram', detail='none');
```

A version the app does not know is dropped and rebuilt, never migrated in place. The spike measured 71 bytes an entry (37.6 MiB for 552,501), a build of under a second after a 0.15 s warm walk, and queries of 0.1 to 26 ms (A120).

### 4.2 What it covers

The settings name the **indexed locations** (Home when it is first switched on) and the **excluded** ones. Never indexed: the contents of cloud placeholder folders (a placeholder file is indexed by name, from its attributes), other volumes inside an indexed location (add them as locations of their own), remote providers, archives and Git revisions, the Trash's folders, Waypoint's own data directory, and folders it cannot read (counted). Hidden entries are indexed; the query applies the hidden toggle.

### 4.3 Keeping it current

- **Watching.** On Linux, inotify with one watch per indexed folder, using at most half of the watches `fs.inotify.max_user_watches` leaves free; a location that would need more is **partial** (its status says so and why) and searches walk it. On Windows, `ReadDirectoryChangesW` on each location's root with `bWatchSubtree`, through `notify` (already a dependency); an overflow rescans the folders whose times changed.
- **Batches.** Events are gathered for about a second and applied in one transaction (the spike measured 157 µs a change), so a change reaches the index within 2 s.
- **Catch-up.** At start, and after a watcher overflow, each indexed folder's modified time is compared with the stored one and the folders that differ are relisted (10 ms warm, 1.4 s cold for 53,040 folders).
- **Priority.** Building and catching up run below the operations pool, on the search pool's lowest priority (and the idle I/O class on Linux, background mode on Windows, where available), and pause on battery when _Pause indexing on battery_ is on (default on; the power state comes from an existing reusable plugin, which #430 chooses).
- **Correctness.** A location is **ready** only after a full scan with its watches in place. A search over a ready location reads the index and `lstat`s each hit as it is listed, which refreshes its size and date and drops a hit that has gone; a location that is building, partial, paused or failed is walked. A search with filters the index cannot answer (tags, permissions, contents) uses the index for the list of candidates and checks each one.

### 4.4 Status, actions and settings

The index's state is `IndexStatus`, read with `index_status` and pushed as `waypoint-search://index` events (at most four a second while building):

| Field        | Meaning                                                                                                                           |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------- |
| `enabled`    | The switch                                                                                                                        |
| `state`      | `Off`, `Building { folders_done, folders_seen, entries }`, `Ready`, `Paused { reason }` (battery, on request), `Failed { error }` |
| `size_bytes` | What the index's files take on disk, measured from the files                                                                      |
| `entries`    | The number of entries, from the database                                                                                          |
| `locations`  | Per indexed location: its URI, state (ready, building, partial, failed), entries, last updated and the reason when not ready      |
| `excluded`   | The excluded locations                                                                                                            |
| `updated_ms` | When the index last changed                                                                                                       |
| `errors`     | The last few errors, with stable codes                                                                                            |
| `watches`    | Linux: watches used and the system limit                                                                                          |

Actions: `rebuild_index` (drops and builds again; searches walk meanwhile) and `delete_index` (removes the files and turns the switch off). Turning the switch off also removes the files, which frees the space (#430); both ask first, with Cancel the default ("Delete the search index? It takes 38 MB and can be built again.").

The settings live in `waypoint-settings` under `search`: `index.enabled` (off by default), `index.locations`, `index.excluded`, `index.pauseOnBattery` (on) and `contentMaxFileMib` (1 to 1,024, 64). `src-tauri` follows them and tells the service, as it does for the Experimental gate (A109).

**The Settings → Search page** (D210, #431) arrives in milestone 7: the switch with one line of explanation; while on, the status block (size, entries, locations with their states, last updated, a progress bar with "Indexing… 12,400 of about 53,000 folders" while building, errors in words), the indexed and excluded locations with Add and Remove, _Pause indexing on battery_, Rebuild index and Delete index…; and, whether or not the index is on, _Largest file to search inside_. With the index off the page shows only the switch, its line and the size cap. Where the index cannot work (no writable data directory, SQLite failed to open), the switch is dimmed with the reason the Services panel gives. Milestone 9's Privacy page shows the index scope as a row that opens this page.

## 5. Search across providers

- **Local first.** `file://` roots are walked with `ignore` directly, the measured engine; the placeholder check is `waypoint-vfs`'s own (`is_placeholder`, made public for the search crate).
- **Every other provider is searched by name through its listing.** Remote providers (SFTP, SMB, WebDAV, S3), archives and Git revisions are walked with `Provider::list_batches`, a folder at a time, with at most the provider's own requests in flight (four folders at once), and names, kinds, sizes and times from the listing only. Nothing is downloaded. Size, date and kind filters work where the listing reports them; tags and permissions filters do not apply to remotes in milestone 7.
- **No content search over remotes in milestone 7.** With a remote root in the scope, the Contents toggle is dimmed with the reason ("Searching inside files works on this computer's files only"), and a `contents` query over a remote root is `InvalidQuery { reason: ContentsNotHere }`.
- **Which scopes reach servers.** Home never does. A folder, a place, a workspace or the open tabs do when they are on a server, and only the connections already signed in: a search never connects or asks for a password; a root whose connection is not signed in is listed in the results' summary as not searched, with Sign In.
- **Server-side search later.** `Provider` may gain an optional `search(path, query)` that a provider with a server-side search (WebDAV `SEARCH`, a Nextcloud search, an S3 prefix listing without delimiters) implements; it is not part of milestone 7.

## 6. Tags and comments

### 6.1 Where they live (D214, A125)

| Option                                                     | For                                                                                                                                                                                          | Against                                                                                                                            | Chosen                                                       |
| ---------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| `user.xdg.tags` and `user.xdg.comment` extended attributes | The freedesktop convention; Dolphin (through KFileMetaData and Baloo) reads and writes them, so tags are shared; they travel with the file on ext4, btrfs, XFS and in `cp -a` and `rsync -X` | Not on FAT, exFAT, most network shares and some FUSE file systems; Flatpak needs only file access                                  | **Linux, where the file system takes `user.` attributes**    |
| A sidecar SQLite database in the app data directory        | Works on every file system and on Windows; transactional; one place for the tag catalogue too                                                                                                | Only Waypoint sees it; a file renamed or moved outside Waypoint must be found again (by its file identity)                         | **Where attributes fail, and always on Windows**             |
| NTFS alternate data streams (`file:waypoint.tags`)         | Travel with the file on NTFS                                                                                                                                                                 | Lost on a copy to FAT, a share or a cloud folder; stripped by some tools; flagged by some security software; invisible to Explorer | Rejected                                                     |
| The Windows property system (`System.Keywords`)            | Explorer's own Tags column                                                                                                                                                                   | Only for formats with a property handler (JPEG, Office), and writing them rewrites the file's contents                             | Not in milestone 7; reading them for display is a later idea |

- **Format.** `user.xdg.tags` is a comma-separated UTF-8 list and `user.xdg.comment` UTF-8 text, as Dolphin writes them; how a comma inside a tag is written, and whether the installed Nautilus reads them at all, is checked in #330 before the format is fixed. Tags are compared ignoring case and kept as first written.
- **The sidecar** is the pure crate `waypoint-tags`: `{app data}/tags/tags.sqlite`, rows keyed by the file's identity (device and inode on Linux; volume serial and 128-bit file id on Windows) with its last known location, so a rename or move inside the volume finds the row again; Waypoint's own operations update the location as they go, and a row whose file is gone is removed on the next read that misses it.
- **The tag catalogue** (each tag's colour and its order in the sidebar's Tags section) is Waypoint's own and lives in the same database, on every platform; a tag found on a file but not in the catalogue is shown without a colour until one is chosen.
- **Remote and virtual locations** have no tags in milestone 7: their providers report the `tags` capability false and the Inspector says "Tags are not available here".

### 6.2 How they are read and written

- `waypoint-vfs` gains `Provider::tags(path)`, `set_tags(path, tags)`, `comment(path)` and `set_comment(path, text)`, defaulting to `Unsupported`, and a `tags` capability with the reason when it is off. `LocalProvider` reads and writes the attributes on Linux and falls back to an injected `TagSidecar` (a trait in `waypoint-vfs`, implemented by `waypoint-tags`, given by `src-tauri`); on Windows it uses the sidecar only.
- **Undo.** `waypoint-ops` makes `SetTags`, `SetComment` and `SetPermissions` jobs with journal entries that hold the previous values, so Undo restores them (#330). "Apply to enclosed items" for permissions is one job over the folder's tree with progress and a cancel.
- **Copies keep them.** The copy engine copies `user.` attributes with the data where both ends take them (A50), and copies the sidecar row (to a new identity) where either end uses the sidecar; a move within a volume keeps both by itself.
- **The catalogue's commands.** The vfs plugin holds the tags store as it holds the saved connections (A81): `list_tags`, `set_tag_colour`, `move_tag` and `forget_tag` (out of the catalogue; files keep it). Renaming a tag on every file that has it would need a search of every file for it, so it is not in milestone 7.
- **Changes reach listings.** An attribute change is an `IN_ATTRIB` event to the watcher, which refreshes the entry; a sidecar or catalogue change is an event of the tags store (`waypoint-vfs://tags`) that open listings and the Inspector follow.
- **Search.** The tag filter reads tags while walking (the spike measured `lgetxattr` of 499,458 files in 113 to 229 ms warm and 2.1 s cold), and the sidebar's Tags section opens a tag as a search (`search:?in=home&tag=red`).

### 6.3 What the Inspector's editing needs (#332)

1. The selection's tags and comment in the entry details (A64 gains `tags`, `comment` and `tags_writable` with the reason when not), read a moment after the selection settles like the rest of the details.
2. The tag catalogue, for completion and colours, and its events.
3. The three undoable jobs above, run through the operations queue so a failure is the usual error dialog and Undo works from the menu and the palette.
4. For permissions, the provider's `PermissionModel` to decide which toggles show, and `set_permissions` with "Apply to enclosed items" for folders.
5. The default application's "always use for .ext" through `mime-apps`'s `set_default`.
6. Events, so a change made in another window, by Undo or by another program shows at once.

## 7. The terminal

**The renderer, the PTY library, the budgets, screen reader support and the Flatpak path are pending spike #316**, which records them in [`milestone-7-spikes.md`](milestone-7-spikes.md) under its own heading. What this contract fixes is the outline the spike fills in (A126, D216):

- **A reusable `pty` plugin** (`tauri-plugin-pty`, no Waypoint imports, graduating to the shared workspace): `spawn({ program?, args, cwd, env, cols, rows }, on_event: Channel<PtyEvent>) → PtyId`, `write(id, bytes)`, `resize(id, cols, rows)`, `kill(id)` and `get_status` (what works here, with reasons). `PtyEvent` is `Output(bytes)` (batched), `Title(text)` (OSC 0 and 2), `Cwd(path)` (OSC 7, with `/proc/{pid}/cwd` as the Linux fallback) and `Exit(code)`. The user's shell is the default program. On Linux it is a PTY (`portable-pty` is the candidate); on Windows, ConPTY; in a Flatpak, the host shell through `flatpak-spawn --host`. _Pending spike #316._
- **The terminal tab is a location.** Like Overview (A71), a terminal tab's location is a virtual `terminal:` location carrying its start folder (`terminal:?cwd={folder URI}`), so pairs, groups, pins, colours, tear-off and session restore work unchanged; it never navigates (a `cd` in it does not move the paired pane, SPEC §5.2). `waypoint-session` gains a per-tab custom title for the double-click rename; the title that follows the shell is runtime state. A restored terminal tab starts a new shell in its start folder; scrollback is not restored.
- **The drawer** (F4) is per window and follows the active pane's folder when _Follow the active folder_ is on, by writing a `cd` only when the shell is idle at a prompt (how idleness is known is _pending spike #316_).
- **SSH terminals** (#327) run over the SFTP connection's SSH session (`russh` channel with a PTY request) in `waypoint-provider-sftp`, behind the same event stream as `PtyEvent`, composed in `src-tauri`, so the terminal view does not know which it has.
- **History per workspace** (D75) is the shell's history file chosen per workspace (`HISTFILE` for bash and zsh, the PSReadLine history path for PowerShell); how far that reaches other shells is _pending spike #316_.

## 8. Flow

Liminal Flow is a separate application (`liminal-hq/flow`, the `flo` command): a working-memory sidecar for the shell with its own SQLite store. The boundary (D215, A127):

- **Opt-in and off by default.** Nothing is sent while the setting is off. The setting is a row of Settings → Integrations (#333, #334), and the Services panel says when Flow is not installed.
- **One subscriber in `src-tauri`** listens to the session and operations events and decides what to send; no plugin knows Flow.
- **The transport is Flow's command line**, the interface Flow publishes: `flo` found on `PATH` (in a Flatpak, through `flatpak-spawn --host`), run without a shell, with a timeout, at most one at a time and coalesced. Waypoint never writes Flow's database. Flow publishes Linux builds only, so on Windows the row is dimmed with "Flow is not available for Windows".
- **What is logged is open question 5** (`docs/open-questions.md`): paths, or only workspace names. **It is not decided here**, and #334 waits for the answer; this contract fixes only the boundary above.

## 9. Ids and the order of work

Decisions of this contract: **D210** (search without an index, the index and its page), **D211** (what a search passes over and its toggles), **D212** (smart folders), **D213** (search across providers), **D214** (tags and comments), **D215** (Flow's boundary), **D216** (the terminal tab and drawer, pending #316); **A120** (the search spike), **A121** (crate and plugin), **A122** (results as a listing), **A123** (`search:` URIs and the smart-folder store), **A124** (the index), **A125** (tag storage), **A126** (the `pty` plugin outline, pending #316), **A127** (the Flow subscriber and transport).

Slice 0 (#313) is this contract (#314) and the two spikes (#315, #316). Then, by dependency:

1. **Search engine** — #318 (`waypoint-search`: the query, `search:` paths, the walkers, content, `SearchProvider`, smart folders store; needs this contract), then #319 (the plugin, `ListingLayout::Search`, `found`, `stop_scan`, the Services entry).
2. **Search in the window** — #320 (the bar, chips and keys) and #321 (the results view), after #319, in parallel; then #322 (smart folders in the sidebar and Go to…).
3. **The index** — #430 (needs #318, can run beside #319 to #322), then #431 (the Settings → Search page).
4. **Tags** — #330 (storage, `waypoint-tags`, jobs and xattr preservation; needs this contract only), then #331 (tags in menus, the sidebar and the list; the tag filter of #320) and #332 (Inspector editing).
5. **Terminal** — after #316: #324 (PTY on Linux) and #325 (ConPTY), then #326 (the drawer), #327 (terminal tab and SSH) and #328 (drops and history).
6. **Integrations** — #335 (terminal emulator choice and Open in Terminal) and #336 (Open With defaults) at any time; #334 (Flow) when open question 5 is answered.
7. **Verification** — #338, #339 and #340 when everything above is in.
