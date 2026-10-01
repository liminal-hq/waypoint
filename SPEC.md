# Waypoint — Product & Design Spec

Status: draft v0.1 · 2026-09-29 · Owner: Liminal HQ
Related: `docs/*` (goals, decisions, interactions, plugins, theming, shared components, prototype plan, open questions)

---

## 1. Summary

Waypoint is a tabbed Linux file manager for local files, remote servers and virtual locations (archives, git repos, cloud drives). It's built to be extended with plugins. It takes Nemo's calm, capable desktop feel and adds:

- **Tabs as first-class objects:** tear off, merge, pin, group or colour them, and split any one of them.
- **Super drag and drop:** a persistent Shelf, spring-loaded folders and tabs, drops onto remotes, a modifier action picker, and drags out to the terminal and other apps.
- **Every location is the same:** SFTP, SMB, WebDAV, S3, Git, archives and cloud drives all browse like local folders.
- **Plugins:** they add columns, actions, location providers, thumbnailers, previewers and panels.
- **Designed for the desktop it runs on:** light and dark modes, OS accent and icon theme, native-feeling frames on GNOME, Cinnamon, KDE and Windows 11, and optional window transparency.
- **Family fit:** the shared Liminal HQ title bar and context menu, the same type and tokens, and the same settings patterns as Emoji Nook, Jar and Threshold.

Assumed stack, matching the family: Tauri v2, React and Rust. The spec is written for UI and UX. Engineering notes are marked **[eng]**.

## 2. Users & jobs

| Persona            | Top jobs                                                                                   |
| ------------------ | ------------------------------------------------------------------------------------------ |
| Desktop generalist | Find, open and tidy files; move photos off a phone; empty the trash                        |
| Developer          | Jump between repos, see git status, open a terminal in the current folder, SFTP to servers |
| Homelab / sysadmin | Manage SMB shares, S3 buckets and remote logs; do bulk ops with verification               |
| Creative           | Browse by thumbnail, batch rename, convert and resize images, tag work                     |
| Power user         | Keyboard-only use, Vim nav, command palette, custom plugins                                |

## 3. Scope

### In scope (v1 design)

1. Main browser: tabs, sidebar, path bar, toolbar, views, split pane, status bar
2. Remote connect and browse
3. Drag and drop system, including the Shelf
4. Command palette
5. File operations queue, conflict resolver and verification
6. Properties, preview and Quick Look
7. Plugin manager
8. Settings, including Appearance and Transparency
9. Developer options
10. Supporting features: tags, smart folders, terminal drawer, disk usage, trash, bookmarks and workspaces, session restore, devices and encrypted volumes, Flow integration, batch rename

### Non-goals (v1)

- Desktop icon management and wallpaper
- A built-in text or code editor (Waypoint only previews)
- Sync engines (cloud plugins mount; they don't sync offline)
- Anything AI-driven (rename rules are deterministic)

## 4. Information architecture

```
Window
├─ Title bar (shared Liminal component; app menu, title, window controls)
├─ Tab strip (own row, below the title bar)
├─ Toolbar row: back/forward/up · path bar · search · view switcher · panel toggles
├─ Body
│  ├─ Sidebar (collapsible): Favourites · Workspaces · Places · Devices · Network/Remotes · Smart folders · Tags · Plugins
│  ├─ Pane A │ Pane B (optional split, per tab)
│  └─ Inspector (optional): Preview · Properties · Plugin panels
├─ Terminal drawer (optional, bottom, follows the active pane's cwd)
├─ Shelf (floating or docked; holds dragged items across tabs and windows)
└─ Status bar: selection summary · free space · ops queue indicator · plugin status items
```

Overlays: command palette, Quick Look, ops queue popover, conflict resolver, connect dialog, batch rename, context menus, Settings window, Plugin manager, Developer options.

## 5. Core surfaces

### 5.1 Title bar

- Uses the **unified Liminal title bar** (see `docs/shared-components.md`). Tabs are **not** in it; they get their own row.
- Contents: app mark and menu button (left), window title with the current location (centre), Always on Top, Minimise, Maximise/Restore and Close (right).
- Double-click toggles maximise. The whole bar is a drag region except its controls.
- On Windows 11 the controls follow the Win11 order and style (see §9). GNOME and KDE button layouts follow the OS setting.

### 5.2 Tab strip

- Each tab shows an icon (folder, remote badge or plugin icon), a truncated title, a close button on hover, and an optional colour dot or group chip.
- **Pinned tabs:** icon only with a small pin mark, grouped on the left and kept in view when the strip scrolls. A pinned tab has no close button and a middle-click does not close it; Ctrl+W, Delete and the tab menu's Close Tab still do. A tab cannot be dragged or moved across the pinned boundary. Close Other Tabs and Close Tabs to the Right leave pinned tabs open.
- **Colours:** a tab can carry one of nine colours (red, orange, yellow, green, teal, blue, purple, pink, grey), shown as an accent along its top edge. The colour's name is in the tab's tooltip and accessible description, so it is never conveyed by colour alone.
- **Tab menu** (right-click, or the Menu key or Shift+F10 on a focused tab): Pin Tab or Unpin Tab, Colour ▸ (None and the palette), Duplicate Tab (the same location beside the tab, without its history), Move to New Window, Move to Window ▸, Close Tab, Close Other Tabs, Close Tabs to the Right, Reopen Closed Tab and Recently Closed ▸. Add to Group ▸ (the window's other groups, then New Group) and, for a grouped tab, Remove from Group; a grouped tab's Pin item reads “Pin Group “name”” because pinning a grouped tab pins its whole group. Split With ▸ (the tabs that are not already paired) on a single tab, or Separate Tabs, Swap Panes, Layout ▸, Reset Sizes and Close Both on either half of a pair.
- **Groups:** a named, coloured chip that collapses its tabs. Drag a tab onto a chip to join the group.
- **Split indicator:** a pair's tabs sit together as one bridged pill with a small split glyph (the joint) on the seam. Each half is still a tab with its own focus stop, and a click on a half activates that pane; the pill's accessible name is “Split: A and B”, and it shares pin and colour. The joint's right-click menu (Menu key or Shift+F10 when it has focus; the same actions are on either half's tab menu) has Separate, Swap Panes, Layout ▸ (Side by Side, Stacked), Reset Sizes, Pin or Unpin, Colour ▸, Duplicate Split, Move to New Window, Move to Window ▸ and Close Both. A plain click does nothing and a double-click resets the pane sizes.
- **Tear off:** drag a tab out of the strip to spawn a new window under the cursor. A ghost follows the pointer, even outside the app. Native pattern in `docs/tauri-tear-off.md`.
- **Merge:** drop a tab onto another window's strip. Drop it onto a tab's body to put it next to that tab.
- **Spring-load:** while you hover a file drag over a tab for 600 ms, that tab activates.
- Middle-click a folder to open it in a new tab beside the current one, without leaving the current tab. **Ctrl+middle-click** opens it in a new window, on a folder in the file view and on a place, favourite or tree folder in the sidebar; their menus have Open in New Window too. Middle-click a tab to close it.
- **New window:** Ctrl+Shift+N opens a window with one tab at Home. It opens the size of the window it came from, 30 px down and to the right of it on the same monitor on X11 and Windows (back at the monitor's corner when that would run off the screen); on Wayland the compositor places it. A notice warns from the eighth open window (“Many windows are open…”), and a thirteenth is refused with a notice (moving every tab of a window to a new window is not a thirteenth, because the window it leaves closes).
- **Move a tab to another window:** the tab's menu (right-click, the Menu key or Shift+F10) has **Move to New Window** and **Move to Window ▸**, which lists the other windows by their active folder and tab count (“Documents — 3 tabs”). The tab keeps its history, pin, colour and id; its scroll position and focused entry are sent just before the move and restored in the window that receives it. The window that receives it comes to the front, says “Moved Documents to this window”, and the window it left says “Moved to a new window” or “Moved Documents to Downloads”. Moving a window's last tab closes that window; a moved tab is not a closed tab and does not appear in Recently Closed.
- **Keyboard:** the strip is a tablist with roving focus. Left and Right move focus along the tabs (Home and End jump), Enter or Space activates the focused tab, Ctrl+Shift+Left or Right moves it, and Delete closes it. Ctrl+T opens a tab at the current folder, Ctrl+W closes the active tab, Ctrl+Shift+T reopens the last closed tab, Ctrl+Tab and Ctrl+Shift+Tab step through the tabs in most-recently-used order, committed when Ctrl is released (a small list of the tabs shows the candidate while Ctrl is held, Escape cancels, and the candidate is announced to screen readers), and Alt+1 to Alt+9 go to that tab. Dragging a tab along the strip reorders it, and Escape abandons the drag.
- **Last tab and last window:** closing the last tab of a window closes the window, and closing the last window quits Waypoint. Closed tabs, including the tabs of a closed window, go to Recently Closed (the last 10, kept with the session).
- Each tab keeps its selection and scroll position while it is open; a background tab gives up its cached rows after a short while and refreshes them when you return.

#### The + button

- **Click:** new tab in the current folder. **Middle-click:** new tab at Home. **Right-click, or press and hold for about half a second,** opens a menu (the Menu key or Shift+F10 does too, when the button has focus). Milestone 3 has New Tab, New Tab at Home, New Window (announced as “Opened a new window”), Reopen Closed Tab and Recently Closed ▸; the full list is: New Tab, New Tab at Home, New Empty Tab (start page), New Tab in Group, New Split Tab, New Tab from Clipboard Path, New Window, New Terminal Tab, Connect to Server, Reopen Closed Tab, Recently Closed Tabs (last 10), Open Bookmarks. The closed list is read afresh each time a menu opens.
- **Drop:** a folder opens a new tab there; a file opens its parent folder; several folders open several tabs; holding Alt opens a split pair.
- The button is fixed at the right end of the strip, next to the all-tabs list. Tabs scroll horizontally when they overflow, with arrows that appear only when needed.

#### Start page (New Tab)

- A tab whose location is the start page. Choosing a destination opens it **in the same tab**, which then becomes a normal folder tab. Its tab reads “New Tab” with a plus icon.
- **Layout:** card grid under a search box. Sections: Quick Actions, Pinned, Bookmarks, Recent Folders, Recent Files, Servers, Devices, Tags, Recently Closed.
- **Cards:** a folder-peek thumbnail, a live count or status (items, connected, free space), and a hover row with Open in new tab and Remove. Cards in Pinned, Bookmarks and Recent Folders can be dragged to reorder.
- **Sections:** collapsible (remembered). Customise mode adds a drag handle and an eye toggle to each section header, to reorder or hide it.
- **Drop:** dropping a folder on Pinned, Bookmarks or Recent Folders adds it.
- **Ways to open one alongside other tabs:** right-click the + button → New Empty Tab, Ctrl+Alt+N, the app menu, the tab's right-click menu, or the command palette (“New empty tab”).
- **Automatic use:** optionally shown when the last tab is closed (Settings → Tabs & Windows), or as a startup tab.

#### Terminal tab

- **New Terminal Tab** creates a real tab that joins the current tab as a pair, beside it. It stays in the folder where it started; `cd` inside it does not move the other pane.
- The tab shows a terminal icon and a title that follows the shell (the folder, or the running command while one is busy). Double-click to rename. Long titles truncate in the tab, and the full title shows in the tooltip and the pair header.
- Dropping files inserts their quoted paths; Alt-drop changes directory.

#### Startup

Settings → General → When Waypoint starts: **Restore last session** (default) or **Open startup tabs**, a list where each tab is the start page, Home, a folder you choose (path field with autocomplete and Browse), or the current workspace's folder. “Apply now” previews the choice in the open window.

#### Tab groups

- A group is a coloured label in the tab strip followed by its tabs. Groups belong to one window.
- **Size:** no hard limit. Past 8 tabs the label gets an amber outline, shows the count, and adding a tab shows a warning suggesting a split.
- **Pairs:** a joined pair (split view) always stays together inside a group. Adding, removing, pinning or colouring one half applies to both.
- **Label:** click collapses or expands (a collapsed label reads “Site · 3”). Double-click renames in place. A new group is auto-named “Group N” and enters rename immediately.
- **Right-click menu:** Rename Group, Change Colour, Collapse or Expand, Collapse All Other Groups, New Tab in Group, Pin or Unpin Group, Sort Tabs in Group (by name, by location, local first), Duplicate Group, Save Group as Workspace, Move Group to New Window, Move Group to Window ▸ (the other windows, as for a tab; the group keeps its name, colour and collapsed state), Ungroup, Close Group (closing every tab of a window this way closes the window, as closing the last tab does, and the tabs go to Recently Closed).
- **Collapsed groups and the active tab:** a collapsed group hides its tabs, and the chip stands in for them. When a hidden tab becomes active by any other route (Ctrl+Tab, Alt+digit, reopening, a neighbour closing) its group expands, since an active tab nobody can see is worse than an open group. Collapsing the group that holds the active tab is allowed: the tab stays active, the chip says it contains the active tab, and it becomes the strip's tab stop.
- **Soft limit:** past 8 tabs the chip's amber outline and count appear. When a tab joins a group that is over the limit, Waypoint says so in a short note under the strip and through the live region, and never refuses the tab.
- **Without a pointer:** every gesture has a menu or key path (drag arrives with its own milestone slice). Left and Right reach a chip like a tab; Enter or Space on it collapses or expands, F2 renames, the Menu key or Shift+F10 opens its menu, and Ctrl+Shift+Left or Right moves the whole group. Ctrl+Shift+Left or Right on a tab inside a group keeps it inside; leaving a group is Remove from Group. Saving a group as a workspace is disabled (“Arrives with workspaces”) until workspaces exist, and Move Group to New Window says so, without failing, when the window cannot be made.
- **Screen readers:** the chip is a button named “Site, tab group, 3 tabs” that reports `aria-expanded`; it is in the tablist's roving order but is not itself a tab. Its tabs stay `tab`s, and each one's description starts “Group: Site”. Creating, renaming, collapsing, colouring, adding, removing, ungrouping and closing a group are announced with the group's name and its tab count.
- **Drag and drop:** drag the label to move the whole group; drag a tab onto a label to add it; drag a tab out of the group's span to remove it; drag the label out of the window to tear the group off; drag a folder onto a label to open it as a new tab in that group.
- **Workspaces:** saving a group as a workspace adds its locations as a sidebar favourites set.
- Overflow: the strip scrolls and a tab list menu appears. Ctrl+Tab cycles tabs in most-recently-used order, committed when Ctrl is released.

### 5.3a Search

Clicking or focusing the search box (or pressing / or Ctrl+F) opens a search bar under the toolbar. Its chips set the **scope** (this folder, with subfolders, Home, the current workspace, open tabs, or a chosen place) and **filters** (type, date, size, tags, permissions), plus toggles for searching inside files, hidden files, match case and regular expressions. Results replace the folder view, with a Location column, highlighted matches in names and a snippet for content matches, live progress with a Stop button, and an optional grouping by folder. From the bar: save as a smart folder (appears in the sidebar), open in a new tab, or close (Esc). Right-click a result → Open Containing Folder.

### 5.3 Toolbar & path bar

- Back, forward and up, each with a long-press history menu. Up is disabled where the location has no parent. **Alt+Left, Alt+Right and Alt+Up** do the same from the keyboard.
- **Path bar:** breadcrumbs by default, each one navigates. Click empty space or press Ctrl+L to edit it as text with autocomplete (local paths, `sftp://`, bookmarks, recent). Enter goes, Escape (or clicking away) cancels, and text that is not a location shows a message under the field and keeps it open. Typed text is parsed by Rust (`~`, relative paths and `file://` are understood), never split or joined in the UI. Autocomplete and sibling dropdowns on the breadcrumbs come later. Breadcrumb segments are drop targets and have sibling dropdowns.
- Search: filter-as-you-type in the current folder, with Enter for recursive search and saved-search chips.
- The view switcher is not in the toolbar: per D23 the view icons live in the status bar footer (§5.8), and sort and group live in the empty-space context menu (§5.5).
- Toggles: sidebar, inspector, split, terminal drawer, shelf.

### 5.3b Navigation and selection

- **History menus:** right-click, long-press or press the menu key on Back and Forward to see the folders behind and ahead (nearest first), and jump to any of them. The history belongs to the tab and is kept by the session.
- **Go to…** (Ctrl+Shift+G): type a location, with recent, saved and matching folders below. Until those lists exist it opens the path bar's text editor, the same as Ctrl+L. A missing, unreadable or not-a-folder location shows its own error state, not a blank view, and leaves the tab's history intact so Back returns to where you were.
- **Copy To… / Move To…:** in the file menu; pick a destination from places, favourites, open tabs, drives and servers, or type one.
- **Open With…:** the file menu lists recommended apps, and _Other Application…_ opens a chooser with an “always use for .ext” option, which then shows in Properties.
- **Select tools:** Select by Pattern (Ctrl+S, with * and ? wildcards, select/add/deselect and a live match count), Invert Selection (Ctrl+I), Select Similar, Deselect All.
- **Per-folder view:** a folder remembers its view, sort and grouping (turn off in Settings → General); the View menu has _Reset This Folder’s View_.
- **Columns:** right-click the list header to show or hide Git, Media, Kind, Modified, Permissions, Owner and Created.
- **Errors:** permission denied (with _Open as Administrator_, an authentication prompt), not enough space (with _Choose Another Location_), invalid names (with the reason, for example FAT drives), name already in use, and failed jobs (Retry and Dismiss in Operations).

### 5.4 Sidebar

Sections can be reordered and collapsed:

- **Favourites:** pinned folders.
- **Workspaces:** named bookmark sets that switch the Favourites list.
- **Places:** Home, Desktop, Documents, Downloads, Pictures, Music, Videos, Trash.
- **Devices:** drives with usage bars and eject. Locked encrypted volumes show a lock and unlock inline.
- **Network:** saved remotes, each with a connection status dot.
- **Smart folders:** saved searches.
- **Tags:** colour labels.
- **Plugin sections:** added by plugins.

**Folders section:** an expandable tree that follows the current folder, loads children when opened, lists hidden folders when hidden files are on, and shows servers and archives as branches. Folders are drop targets that spring open. The _Places / Folders_ switch at the top turns the tree into the whole sidebar. **Editing:** right-click Places, Favourites and servers to rename, reorder (or drag), remove, add the current folder (Ctrl+D) or edit a connection; dropping a folder on the Places or Favourites heading adds it. A **Recent** section lists the last eight locations.

Every item is a drop target. Hovering an item during a drag springs it open after 600 ms.

**What milestone 2 ships of the sidebar:** a collapsible left panel (toggle button in the toolbar, **F9**) with the **Places / Folders** switch at its top, showing either **Places** with **Favourites**, or the **Folders** tree on its own; the other sections, drag and drop into the sidebar and the Recent list arrive with the features they depend on. The sidebar is a navigation landmark, and each section is a labelled group whose heading collapses it, and the switch is a pair of tabs (Left and Right move between them); the view and the collapsed sections last for the window (nothing is saved between runs yet).

- **Places** shows Home and the user folders that exist (the XDG user directories on Linux, Known Folders on Windows). **Favourites** are the freedesktop bookmarks file (`~/.config/gtk-3.0/bookmarks`), shared with other file managers, and **Ctrl+D** pins the current folder. A favourite whose folder is gone stays listed, and opening it shows the usual Folder not found state.
- A click opens the item in the active tab; a middle-click opens it in a background tab; the item of the current folder is marked. Right-click (or the menu key) shows Open and Open in New Tab, plus Rename, Move Up, Move Down and Remove from Favourites on a favourite and Add to Favourites on a folder. A folder in the file list has Add to Favourites in its menu too.
- **Favourites** are renamed in place (**F2**; an empty name restores the folder's own) and reordered by dragging a favourite onto another, with **Alt+Up** and **Alt+Down**, or from the menu. Up and Down move between the items of Places and Favourites.
- **Folders** (its own view, so choosing a place never moves it) is a tree that opens down to the current folder and follows the active tab. A folder's children are read only when it is expanded, and the tree lists hidden folders only while the tab shows hidden files. It follows the tree keyboard pattern: Up and Down move, Right expands (then enters), Left collapses (then goes to the parent), Home and End jump, Enter opens the folder in the tab, and typing the first letters of a name jumps to it. A folder with more than 2000 sub-folders shows the first 2000 and says so.

### 5.5 File view

- **View switcher:** List and Grid buttons at the right end of the status bar (Ctrl+2 and Ctrl+1; Columns, Compact and Disk usage take Ctrl+3 to Ctrl+5 when they exist). The choice is for the window's session and every folder shows it; a folder remembering its own view is deferred with the other per-folder settings (§5.3b).
- **Grid:** icons with a size slider (48–256 px, in the status bar beside the switcher while the grid is shown), names under them, and later thumbnails and folder peeks (a 2×2 mosaic of contents). The grid fits as many columns as the width allows and is virtualised by row. Arrow keys move one item sideways or one row up and down, Page Up and Page Down move by the visible rows, Home and End go to the ends; selection, type-ahead, Enter and the context menus are the list's. A folder too large for the webview's scroll height shows the same "Showing the first N of M" banner, counted in rows of columns.
- **Empty-space menu:** right-click the file area (or press the menu key with nothing focused) for Sort by (Name, Size, Modified, Kind), Descending, Folders first and Show hidden files (Ctrl+H). The sort applies to the open folder and carries to folders the tab opens next; Show hidden files applies to every open tab.
- **List:** sortable, resizable, reorderable columns. Plugin columns (git status, media info) sit beside built-ins.
- **Columns (Miller):** hierarchical navigation with a preview column at the end.
- **Disk usage:** a treemap or sunburst of the current folder, with list parity.
- Selection works by click, Shift/Ctrl, rubber-band, and type-ahead (jump to a match; type `/` for an inline filter).
- **Inline rename:** F2. The first selection stops before the extension. Changing the extension asks for confirmation.
- **Quick Look:** Space or hover-to-peek (an optional setting). The arrow keys move through files while it's open.
- **Empty states:** an empty folder, no search results, a remote disconnected (with reconnect), and permission denied (with "Open as administrator").

### 5.6 Split view (joined tabs)

A split is two or more tabs joined together. They sit in the tab strip as one bridged pill, and each half is still a normal tab that can be clicked. Each keeps its own history. Pinning, colour and group apply to the pair.

- **Create:** drag a tab onto the left, right, top or bottom of the content area; hold a tab over another tab; right-click → Split With; F3; or drag a folder to a content edge.
- **Toggle (F3 or the split button):** creates a paired tab on the same folder and focuses the new pane. Toggling again **closes the pane it created** (whichever pane has focus) and leaves the original tab, with an “Undo” toast that puts the pane back with its history and sizes (the toast stays about 8 seconds, waits while the pointer or focus is on it, and Escape dismisses it); the closed pane keeps its history for Reopen Closed Tab. A pair you joined by hand is only separated, never closed.
- **Separate:** drag a pane header up to the tab strip, right-click → Separate Tabs, F3 again, or close one half (the other stays as a normal tab).
- **Layout:** side by side or stacked, more than two panes, draggable dividers, double-click a divider to reset. A pair's panes each have a header (folder, a grip for the drag that comes with the tab drag engine, a close button, and “Active” on the focused pane). A divider is a separator: drag it, or use the arrow keys along its axis (2%, or 10% with Shift), Enter to reset; a pane never goes under 10%. Sizes are saved when the drag is released, and Escape abandons a drag.
- **Panes:** the focused pane is the active tab. Click in a pane, or F6 (next) and Shift+F6 (previous), to focus it; the path bar, sidebar, status bar and navigation follow it. Each pane keeps its own selection and scroll.
- **Tear off:** drag a pane header out of the window to move one half; drag the joint between the tabs to move both.
- **Close:** closing a half asks first if the other half is the target of a running operation.

### 5.6a Split pane (original notes)

- Available per tab, vertical or horizontal. The divider can be dragged, and double-clicking it resets to 50/50.
- F6 or clicking a pane makes it active, and the active pane gets an accent outline.
- **Pane sync:** a toggle that mirrors navigation in the other pane by relative path, so you can compare the same subtree on local and remote.
- F5 copies to the other pane and F6 (while held with Shift) moves to it. Keys are configurable.

### 5.7 Inspector

Tabs: **Preview** (a thumbnail or rendered preview with a short summary: name, kind and size), **Properties** (the full detail) and **plugin panels**.

- **Properties tab:** an editable name; kind; size (folders calculate in the background and show “Calculating…” while they update); location; modified; owner; free space (folders); default app; media info and Git status when those plugins are on; tags and a comment (editable); permissions with toggles and an “Apply to enclosed items” option for folders; and a checksum.
- **Type-specific details:** images show dimensions, megapixels, colour space, bit depth and camera details; video shows duration, resolution, codec and frame rate; audio shows bitrate, sample rate, artist and album; PDFs and documents show pages, words, author and encoding; code shows language and lines; archives show item count and compression; folders show what they contain. Media details come from the Media Info plugin. Preview shows the first three.
- **Nothing selected** shows Properties for the current folder.
- Right-click → Properties always jumps to the Properties tab. The panel button opens the last tab used. **Alt+Enter** opens a floating Properties window with extra detail (created, accessed, inode, file system, extended attributes).

### 5.8 Status bar

Shows the item count, the selection count and size, free space on the current volume, the List and Grid view switcher at the right end (D23), an ops queue ring (progress and count, click to open), and plugin status items such as the git branch and remote latency.

- The selection count is immediate; the size is the sum of the selected files (folders add nothing, it is not recursive) and is worked out in Rust, so it arrives a moment later and dims while a newer one is on its way. Free space is hidden where the volume cannot report it.
- The bar announces the selection count politely to screen readers, and reports a failure such as a file that would not open.
- **Open:** Enter or double-click opens a folder in place and a file in its default application. The read-only context menu on an entry offers Open, Open in New Tab (folders) and Copy Path (the path as Rust displays it, which may be lossy for names that are not valid UTF-8). Right-clicking an entry that is not selected selects it first, and the menu acts on the entry under the pointer. The menu key or Shift+F10 opens it for the focused entry.

## 6. Super drag and drop

Full rules are in `docs/interactions.md` §3. Summary:

| Feature                | Behaviour                                                                                                                                                                                                                            |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Shelf                  | A floating stash. Drop items on it, go anywhere, then drag them out. It persists across tabs, windows and restarts (as references, not copies).                                                                                      |
| Spring-loaded targets  | Folders, tabs, sidebar items and breadcrumb segments open after hovering for 600 ms (configurable). Leaving springs back unless you've dropped.                                                                                      |
| Remote drops           | Dropping onto a remote folder or saved remote queues an upload. A cursor badge shows "Upload to host".                                                                                                                               |
| Modifier action picker | Default rule: same volume moves, a different volume copies. Ctrl copies, Shift moves, Ctrl+Shift links, and Alt (or a right-drag) opens a radial or menu picker: Copy, Move, Link, Compress here, Extract here, plus plugin actions. |
| Out to apps            | Drag files to a terminal to insert quoted paths, or to other apps as URIs. Hold Alt while dragging to the terminal drawer to `cd` there instead.                                                                                     |
| Drag feedback          | A stack preview with a count, a live action badge, a target highlight and an invalid-target shake.                                                                                                                                   |

## 7. Remote & virtual locations

| Provider     | URI                                        | Notes                                                                      |
| ------------ | ------------------------------------------ | -------------------------------------------------------------------------- |
| SFTP/SSH     | `sftp://user@host:port/path`               | Key agent, known-hosts prompt, jump host, "Open terminal here" over SSH    |
| SMB          | `smb://host/share`                         | Domain auth, share browser                                                 |
| WebDAV       | `davs://host/path`                         | Nextcloud preset                                                           |
| S3           | `s3://bucket/prefix`                       | Endpoint preset (AWS, MinIO, R2), storage class column                     |
| Git          | `git+file://repo` or a local repo overlay  | Status column, branch in the status bar, stage and commit actions (plugin) |
| Archives     | Browsing into `*.zip`, `*.tar.*` or `*.7z` | Read-only, or read/write where the format allows                           |
| Cloud drives | Plugin-provided                            | Nextcloud and Google Drive samples                                         |

**Connect dialog:** pick a protocol, fill host, user and auth, test the connection, then save to the sidebar (optionally into a workspace). Thumbnails are off for remotes by default and can be turned on per remote.

## 8. Operations

- **Ops queue:** every copy, move, upload, extract, delete or verify is a job with progress, speed and an ETA. Jobs can be paused, resumed, cancelled, reordered and retried. The queue lives in a popover from the status bar and can be popped out into a window.
- **Conflict resolver:** one dialog for a batch. It compares each pair side by side (thumbnail, size, date and a diff for text) with per-file or apply-to-all choices: Replace, Skip, Keep both (rename), Merge folders, or Replace if newer.
- **Checksum verification:** an optional setting that verifies after each copy (BLAKE3 by default, or SHA-256). Results are recorded on the job.
- **Undo/redo:** Ctrl+Z and Ctrl+Shift+Z for rename, move, copy (deletes the copies), trash (restores) and tag changes. The history is visible in the Edit menu.
- **Trash:** restore to the original location, with "Empty items older than N days" and per-volume trash views.
- **Batch rename:** find/replace, regex, a counter, case, and date tokens, with a live before/after table and conflict highlighting. Rename rule plugins add tokens.

## 9. Theming & platforms

Full detail is in `docs/theming-and-platforms.md`.

- **Modes:** Light, Dark and System. There's also a Theme source: Liminal (the brand tokens) or OS (the portal accent, colour scheme and icon theme).
- **Icons:** the freedesktop icon theme from the OS (Adwaita, Breeze, Papirus…). Waypoint's own set fills in anything missing.
- **Transparency:** the main setting. It adds a translucent window background with an opacity slider, blur where the compositor supports it, and per-region control (sidebar, content, title bar). High contrast or reduced transparency turns it off automatically.
- **DE frames:** the prototype shows Waypoint on **GNOME** (libadwaita), **Cinnamon** (Mint-Y, Nemo's home), **KDE Plasma** (Breeze) and **Windows 11** (Mica/Fluent, as a portability check).

## 10. Plugins

Full detail is in `docs/plugins.md`. There are extension points for columns, context actions, drop actions, location providers, thumbnailers, previewers, inspector panels, sidebar sections, status items, palette commands and rename tokens. Plugins run sandboxed with declared permissions. The Plugin manager lets you browse, install, update, configure, grant permissions and see each plugin's health.

## 11. Settings

Settings open in a separate window using the same side-nav pattern as Emoji Nook.

1. **General:** startup (restore session or open Home), default view, single or double click, confirm on delete, hidden files.
2. **Appearance:** mode, theme source, accent, density, icon theme, thumbnail size, font scale.
3. **Transparency:** enable, opacity, blur, per-region toggles, "Solid when unfocused", and a live preview.
4. **Tabs & windows:** new-tab location, middle-click behaviour, tear-off, close last tab behaviour, groups.
5. **Drag & drop:** default action rule, spring-load delay, the Shelf (persist it, auto-hide), terminal drop behaviour.
6. **Previews & thumbnails:** file types, max file size, remote thumbnails, folder peeks, hover-to-peek.
7. **Operations:** verify after copy, algorithm, concurrency, undo history depth.
8. **Keyboard:** the shortcut editor, a Vim mode toggle and a keymap preset (Nemo, Dolphin, Finder-like).
9. **Integrations:** terminal emulator, Flow, "Open with" defaults.
10. **Plugins:** a link to the Plugin manager.
11. **Privacy:** recent files, search index scope.
12. **Developer:** see §12. It's hidden until you enable it (click the About version 5× or pass `--dev`).

## 12. Developer options

- **Inspector overlays:** show drop target outlines, drag regions and the focus ring trail.
- **Plugin dev:** load an unpacked plugin from a folder, hot reload, a log console per plugin, an API playground and the permission audit.
- **IPC and backend:** a live IPC log, FS watcher events and provider latency charts.
- **Theme tools:** a token inspector, forcing a DE frame or colour scheme, and a portal-values dump.
- **Feature flags:** experimental toggles.
- **Diagnostics:** export logs and reset settings.

## 13. Integrations

- **Terminal drawer:** a bottom drawer running the user's shell that follows the active pane's cwd (an optional setting). You can drop files into it and pop it out to an external terminal.
- **Flow:** logs your working context (current workspace, recently touched folders) to Liminal Flow. Opt-in.
- **Session restore:** the next start brings back each window with its size (and position, where the platform lets an app place its windows: not on Wayland), its tabs in order with the active tab and each tab's back and forward history, the list or grid view with its icon size and hidden-files choice, each tab's scroll position and focused entry, and Recently Closed. Closing a window waits about 150 ms before it goes, so the page can report each tab's scroll position and focused entry first and they are saved with the session. A folder that no longer exists opens as the usual "Folder not found" page with its history intact. A session file that cannot be read is kept aside, the previous run's copy is tried, and the status bar says "Your last session could not be restored" if neither works. Closing the last window by its own close button keeps that window's tabs for the next start; closing its last tab instead (or closing other windows) sends the tabs to Recently Closed. Groups, splits and selection are restored with their own features.
- **Devices:** mount, unmount, eject and unlock LUKS volumes (passphrase dialog, remember in keyring).

## 13a. OS integration

Full detail in `docs/os-integrations.md`. Portals first, so Waypoint works as a Flatpak. It shares Trash, thumbnails, bookmarks, recent files and tags with other Linux file managers, reads Nemo, KDE and Nautilus actions, and integrates with notifications, dock progress, a global shortcut, sleep inhibit and the default-file-manager setting. Windows 11 is a real target with its own equivalents (Recycle Bin, jump lists, Explorer menu, Quick Access, OneDrive placeholders). Options that don't work on the current system are hidden; Settings → Integrations has a Services status panel.

### 5.9 Action bar

A row below the toolbar (on by default, hideable) with common commands: New, Cut, Copy, Paste, Rename, Share, Delete, Sort, View, Group, Extract all (only when an archive is selected), Undo and Redo. Labels are shown by default. Right-click the bar to hide labels, customise items, or hide the bar; users can add, remove and reorder items. Plugins can add buttons. Also togglable from the View menu and the command palette.

## 13a. Bundled plugins

Duplicate finding, Compare and Sync, Previous Versions, Network Sharing, Nearby Devices, Disk Tools (Format and Mount Disk Image) and Encrypted Vaults ship as bundled plugins, listed in Settings → Plugins with their permissions. All are on by default except Encrypted Vaults. Turning one off removes its menu items, palette commands and dialogs. The terminal stays core. Details: `docs/plugins.md`.

## 13b. Customisation and basics

- **Keyboard shortcuts:** an editor lists 17 common actions; click one and press the new keys. Conflicts are caught and named. Start from a Waypoint, Nemo, Dolphin or Explorer set.
- **Context menu:** choose which file-menu items show and reorder them.
- **Icon style:** Waypoint, Outline, Filled or Duotone for Waypoint's own icons.
- **Window layouts:** save the tabs, pairs, groups and panes as a named layout and restore it later.
- **Shared clipboard:** copy or cut in one window and paste in another, including the floating window.
- **Compare and Sync:** for a pair of tabs, list files that differ or exist on one side, choose copy left, copy right or skip for each, then synchronise as background jobs.
- **Previous versions:** in Properties, restore a file from a snapshot as a copy.
- **Terminal history:** commands are remembered per workspace.
- **First run and help:** a five-step tour, ending with integration choices; F1 opens Help with tips, the shortcut list, the tour and About.
- **About and updates:** version, licence, stable or beta channel, check for updates, install and restart.
- **Privacy and crashes:** crash reports are offered, never sent silently, with three levels of detail; no usage analytics.
- **Touch mode, language and direction, text size, high contrast, focus ring, reduced motion:** Settings → Accessibility and Language & Region. See `docs/accessibility.md` for the review.

## 13c-0. Window menu

Right-clicking empty parts of the title bar or the tab strip opens the same window menu as Threshold, Cadence and Spindle: Restore or Maximize, Minimize, Move, Always on Top, and Close. The app-name button keeps its own menu, and tabs, groups, the + button and the pair joint keep theirs. The floating window's title bar has a smaller version with Merge Back.

## 13c. Tab drag language

Every tab drag shows one label pill beside the pointer saying what will happen, with “Esc to cancel”. Motion is 140 ms and turns into instant jumps with Reduce motion. The thresholds are one set: a drag starts after 4 px of movement, tears off after 24 px away from the strip, splits after a 450 ms hold over the middle of a tab, and starts a group after an 800 ms rest in a slot.

- **Reorder:** the tab follows the pointer inside the strip, the other tabs slide aside, and a thin accent line marks the open slot. On release the tab settles into the slot.
- **Split pair:** hold over the middle of another tab. A ring draws around it over 450 ms, then the two tabs bridge into one pill.
- **New group:** rest in a slot for 800 ms. A bracket draws under the slot and the pill says “Release to start a new group”. The new group opens with its name ready to edit.
- **Add to a group:** drag onto a group label; the pill says “Release to add to …”.
- **Leave a group:** drag the tab out of the group's span.
- **Split pane:** drag into the file area; the left, right, top or bottom area highlights and the pill says which.
- **New window:** drag away from the strip. The tab collapses out of the strip, a mini window preview follows the pointer, and the pill says “Release to open in a new window”. Dropping it on the other window's strip merges it in; moving it back over the strip cancels. Where the compositor allows no cursor position or window placement (Wayland), there is no floating ghost: the preview stays inside the window, the compositor places the new window, and a tab merges into another window from its menu.
- **Cancel:** Esc at any point slides the tab back to where it started.

## 14. Accessibility

Every action works from the keyboard, focus is always visible, and screen readers get labels and live regions for ops progress. Waypoint follows reduced motion (no spring animation), reduced transparency and high contrast. Hit targets are at least 28 px on desktop and at least 44 px in touch mode. Text contrast meets WCAG AA on every theme, including transparent windows, where text falls back to a solid backdrop when the measured contrast is too low.

## 15. Success criteria (design)

- Common tasks work without the mouse and without looking anything up.
- Moving a file from local to a remote folder in a different tab takes one drag, whether through a spring-loaded tab or the Shelf.
- A new user recognises Waypoint as belonging to the same family as Jar and Threshold.
- The same window feels native in all four DE frames.
