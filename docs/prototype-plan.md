# Prototype Plan

> **Historical:** this plan describes the Claude Design prototype (see `docs/ui-mockups/README.md`). The prototype is reference only; `SPEC.md` and `docs/architecture/` are the source of truth.

## Deliverables

| File                        | What                                                                                                                                                                                                                       | Depth    |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| `Waypoint.dc.html`          | The main window prototype: tabs, sidebar, path bar, views, split, inspector, shelf, drag and drop, context menus, palette, ops queue, conflict resolver, Quick Look, rename, batch rename, terminal drawer, connect dialog | **Deep** |
| `Waypoint Settings.dc.html` | The Settings window, including Transparency (with live preview), Plugins and Developer                                                                                                                                     | Medium   |
| `Waypoint Desktops.dc.html` | A canvas showing the window in GNOME, Cinnamon, KDE and Win11 frames, light and dark                                                                                                                                       | Visual   |

## Tweaks (props on the main DC)

- `desktop`: gnome · cinnamon · kde · win11 · liminal
- `scheme`: light · dark
- `transparency`: on/off, plus `opacity`
- `density`: comfortable · compact
- `sidebarStyle`: full · rail
- `showShelf`: on/off
- `viewMode`: grid · list · columns

## Mock data

- Home tree: Documents, Downloads (with a photos batch), Pictures/2026 Trip, Projects/waypoint (a git repo with modified, added and untracked files), Music, Videos, and an archive `assets.zip`.
- Remotes: `sftp://deploy@homelab` (Connected), `smb://nas/media` (Connected), `s3://liminal-backups` (Idle), `davs://cloud.liminal` (Nextcloud plugin).
- Ops queue: an upload to homelab at 62 %, a verify pass, and a completed extraction.
- Tabs: pinned Home, a group called "Site" (sftp, local repo), Downloads, and Pictures.

## Build order

1. Main window shell with the unified title bar and tab strip
2. File views plus selection, rubber-band and keyboard
3. Drag and drop engine: spring-load, action picker, shelf, tab drop
4. Split, inspector and Quick Look
5. Overlays: palette, context menu, ops queue, conflict, rename
6. Settings window
7. Desktop showcase

## Status (latest)

- Built: main window, Settings (with plugin manager and developer options), four-desktop showcase.
- Tabs: pin, colour, groups (with full menu), joined pairs (split view), reorder, tear off, floating second window, merge back.
- Views: list, grid, columns, disk usage (switcher in the footer). Sort and group-by from the empty-space menu.
- Files: archives open in a tab, conflicts, batch rename, undo and redo, trash, tags, Quick Look, inspector, shelf, terminal drawer, operations queue, command palette, keyboard shortcuts dialog.
- Also built: + button menu, start page (card grid, customise mode), terminal tab pairs, startup tabs and last-tab behaviour in Settings.
- Phase 1 and 2 built: search bar, folder tree and sidebar editing, recent locations, history menus, Go to, Copy To and Move To, Open With, select tools, per-folder views, custom columns, and error states (permission denied, not enough space, invalid names, failed jobs).
- Operations and tools built: queue controls, duplicate finder, network sharing, nearby devices, format, disk images, encrypted vaults, read-only errors.
- Customisation and product basics built: see SPEC §13b. Earlier list (now done): customisation (shortcut editor, context-menu editor, icon packs, saved layouts, cross-window clipboard, compare and sync, previous versions, per-workspace terminal history) and product basics (first run, help, about, privacy, touch mode, RTL, accessibility).
- Not yet (beyond the prototype): a real file system, a plugin runtime, native windows, and a screen reader test pass.
- Old note: real file system, plugin runtime, multi-window beyond the one floating window, drag-and-drop between separate OS windows.
- Bundled plugins: seven features from the operations and tools phase are now plugins with on/off switches in Settings → Plugins. Turning one off hides its menu items and palette commands in the prototype.
