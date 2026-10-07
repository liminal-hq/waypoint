# Interactions

## 1. Mouse

| Input                | Target                           | Result                                                            |
| -------------------- | -------------------------------- | ----------------------------------------------------------------- |
| Click                | Item                             | Select (single-click-to-open is an optional setting)              |
| Click                | Empty space in a folder          | Clear the selection (Shift and Ctrl clicks leave it alone)        |
| Double-click         | Item                             | Open or enter                                                     |
| Middle-click         | Folder, sidebar item, breadcrumb | Open in a new background tab                                      |
| Ctrl+middle-click    | Same                             | Open in a new window                                              |
| Middle-click         | Tab                              | Close (except pinned tabs)                                        |
| Right-click          | Item, empty space, tab, sidebar  | Context menu (Liminal pattern)                                    |
| Drag on empty space  | View                             | Rubber-band select (Ctrl adds to the selection, Shift extends it) |
| Double-click         | Title bar                        | Maximise or restore                                               |
| Double-click         | Split divider                    | Reset to 50/50                                                    |
| Back/forward buttons | Anywhere                         | History                                                           |
| Two-finger swipe     | View                             | Back or forward, with an edge arrow preview                       |
| Pinch                | Grid                             | Thumbnail size                                                    |

## 2. Keyboard (default "Waypoint" preset)

| Keys                      | Action                                                                                                   |
| ------------------------- | -------------------------------------------------------------------------------------------------------- |
| Ctrl+T / Ctrl+W           | New tab / close tab                                                                                      |
| Ctrl+Shift+T              | Reopen closed tab                                                                                        |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab (most recently used)                                                                 |
| Alt+1…9                   | Go to tab                                                                                                |
| Ctrl+Shift+N              | New window (one tab at Home)                                                                             |
| F10 / lone Alt            | Open the application menu with its first row focused; again, close it and return focus                   |
| Alt+F / E / V / W / H     | Open the application menu's File, Edit, View, Window or Help menu                                        |
| F10 / lone Alt (menu bar) | With the menu bar on: focus its first menu (again or Esc: back); Left / Right, Down / Enter / Space, Esc |
| F7 / Shift+F7             | New folder / new file                                                                                    |
| Ctrl+Shift+D              | Duplicate                                                                                                |
| Menu key / Shift+F10      | On a tab: its menu (Move to New Window, Move to Window ▸)                                                |
| Ctrl+Shift+P              | Command palette (commands, and the undo history); works from a text field too                            |
| Ctrl+L                    | Edit path                                                                                                |
| Ctrl+F                    | Search · `/` inline filter                                                                               |
| F2                        | Rename · Ctrl+F2 batch rename                                                                            |
| F3                        | Toggle split                                                                                             |
| F4                        | Terminal drawer                                                                                          |
| F5 / Shift+F5             | Copy / move to the other pane (Copy To… / Move To… when there is no pair or the other pane is read-only) |
| F6                        | Switch the active pane                                                                                   |
| F9                        | Sidebar · F11 inspector (shows and hides it; reopens on the tab last used; the same in a text field)     |
| Space                     | Quick Look (Space or Esc closes; Left/Right, or Up/Down in the grid, step through the items)             |
| Ctrl+C / Ctrl+X / Ctrl+V  | Copy / cut / paste files (the shared and system clipboard)                                               |
| Ctrl+Z / Ctrl+Shift+Z     | Undo / redo                                                                                              |
| Ctrl+H                    | Hidden files                                                                                             |
| Ctrl+1…5                  | Grid, List, Columns, Compact, Disk usage                                                                 |
| Ctrl+D                    | Bookmark                                                                                                 |
| Ctrl+B                    | Toggle the Shelf                                                                                         |
| Delete / Shift+Delete     | Trash / delete permanently                                                                               |
| Alt+Enter                 | Properties window (selected item or folder; four at most)                                                |

Ctrl+Tab opens a switcher: while Ctrl is held, each Tab press moves the highlight down the most-recently-used list (the active tab first), Shift+Tab moves it up, and releasing Ctrl activates the highlighted tab once, so the tabs passed on the way do not enter the MRU list. Escape cancels. Menu key or Shift+F10 on a focused tab, or on the + button, opens its menu; pressing and holding the + button opens its menu too. Delete or Ctrl+W close a pinned tab; a middle-click does not.

### Vim mode (optional)

`h j k l` to move, `gg` and `G` for top and bottom, `Enter` or `l` to open, `h` or `-` to go up, `yy` to yank to the Shelf, `p` to paste, `dd` to trash, `cw` to rename, `v` for visual select, `/` to filter, `gt` and `gT` for tabs, `:` for the palette. A mode indicator appears in the status bar.

## 3. Drag & drop rules

### 3.1 Default action

| Source → Target            | Default                   |
| -------------------------- | ------------------------- |
| Same volume                | Move                      |
| Different volume or remote | Copy (upload or download) |
| Into an archive            | Add to the archive        |
| Onto an app or executable  | Open with                 |
| Onto the terminal          | Insert quoted paths       |
| Onto a Shelf slot          | Add a reference           |

**Built in milestone 4 (slices 12 to 14): drags inside the window and beyond.** The Source → Target table above holds for same volume and different volume (a volume is what the planner says; until it answers the pill reads "Move or copy 3 items to Documents" and a drop copies). The "Always copy" and "Always ask" settings replace the default. Drops on the Shelf are built (slice 14, §3.4), and drops to and from other applications (slice 13); drops on archives are built (#299, D170: a drop into an open archive that can be changed adds copies, never moves, and rewrites the archive; the picker also offers Compress Here… and Extract Here, D171); drops on apps and the terminal arrive with their own slices. Drops on servers are built (#305, D164): a server folder, a server tab and a saved or recent server in the Network section are targets; a move within one login counts as one volume (a move by default), anything else reaching a server copies; and the pill names the server ("Upload 3 items to Documents on NAS", "Download report.pdf from NAS to Documents", "Move report.pdf to Documents on NAS", "Copy … to … on NAS" from another server), read out as "Over Documents on NAS: will upload". A drag of a server's files out of the window says "Downloading report.pdf for the drag", downloads them into the app's cache and hands the copies to the system as copies only; a folder from a server or more than 64 MB stays in the window with "… could not be downloaded for the drag" and the reason.

- **Pill text** (read out as "Over Documents: will move", and "Dragging 3 items" at the start): "Copy 3 items to Documents", "Move report.pdf to Documents", "Link 3 items in Documents", "Choose what to do with 3 items in Documents", "Move 3 items to the Trash", "Open in a new tab", "Open in a split pair" (Alt over +), "Open Documents in a new left pane" (a split region, below), "Open in a new tab in Work" (a chip), and "Not allowed: already in Documents" / "…a folder cannot go into itself" / "…Documents cannot be changed" / "…use the Trash in the sidebar" / "…this tab is already split" for a refusal, which also draws the target with a dashed edge, shakes the pill (not under Reduce motion) and is said again as a notice on release.
- **Targets:** a folder row (into that folder), a pane's file area (into its folder; none over the pane the files are in), a sidebar place, favourite or folder, a breadcrumb segment, a tab (into its folder), the Trash (move to Trash), the + button (a new tab at each dropped folder, or at the parent of a dropped file) and a group chip (the same, in that group).
- **Starting:** a press on a row moves 4 px; a selected row drags the whole selection, an unselected one is selected first; a right-button press holds its context menu until the release so it can become a drag. Esc cancels.
- **Split regions (D174):** the file area's four regions (`tabs/splitRegions.ts`, the same thirds and halves a dragged tab uses) are a drop target for two file drags, drawn by the same overlay (`tabs/SplitZones.tsx`) and measured on every move from the pane area. A **Places or Folders item** is a drag source (pointer events, `dnd/fileDrag.ts` `pressLocations` with `place`): over the file area it shows the regions and a release on one opens that folder in a new pane on that side; it is dropped on nothing else (the pointer anywhere else just carries it, pill "Dragging Documents"), and where the tab on show is already in a pair the pill refuses with "Not allowed: this tab is already split", said again on release. Favourites keep the platform's drag, which reorders them, and the Trash and Overview are not dragged. A **folder dragged from a list or grid** keeps its meaning (a drop on a pane copies or moves it in); holding **Alt** over the file area, with exactly one folder dragged by the primary button, swaps that for the regions and a release opens the folder in a new pane. The pill reads "Open docs in a new left pane" and follows the key at each change. Without a split on offer (a pair on show) Alt keeps opening the picker. Without a pointer: Open in Split Pane in a sidebar item's menu and in a folder's item menu (new pane on the right).

**Built in milestone 4 (slice 13): drags to and from other applications.**

- **Files dragged in** (from another application or another Waypoint window) feed the same targets, pill, spring-loading and default rule as an in-page drag; the pill counts the files and names one ("Copy 3 items to Documents"), with plain page icons since nothing is known about them. The Trash refuses them ("these items cannot be trashed from here"), `+` and chips open the dropped folders (or a file's parent), and a move onto the folder the files are already in is refused. Under the by-volume default a drop from another application is a copy even on the same volume, because the drag carries no promise about what its source allows (an archive manager, a mail attachment or a browser's temporary file offers copies only); Shift moves (on Wayland through the action the compositor negotiated, below), and the picker (right button, Alt or "Always ask") offers Move everywhere. A drag that left this window and comes back is this window's own and keeps its own default. The drop becomes a copy, move or link job over the exact URIs, with conflicts, errors and Undo from the queue. Text and links dragged in are ignored. A drag leaving the window clears the highlight.
- **Modifiers:** read from the events on X11 and Windows. A Wayland compositor takes the keyboard during a drag and tells the application nothing, so the keys read as released. The compositor does read them: it chooses the drag's action from the keys (Shift for Move, Ctrl for Copy) among the actions the source offers and tells the toolkit, and the native drag and drop plugin reports that as the `action` of each enter, over and drop event, kept to what the source offers. Where the keys are unavailable the action stands in for them: a negotiated move is Shift (so it moves, and copies where the sources cannot be moved) and a negotiated link is Ctrl+Shift; a negotiated copy is also what no key gives, so it adds nothing and the default rule, "Always ask" and the picker keep their say. Ctrl+Alt and Alt (ask) are not reported as actions there, so "Always ask" or the right button is how to open the picker. The action is read at each pointer motion, so a key pressed or released with the pointer still takes effect at the next motion. On X11 and Windows the keys alone decide, as before. A Move from another application is carried out as a move job with every protection of one (conflict and error dialogs, optional verification and the undo journal).
- **Dragging out:** when the pointer leaves the visible window with a drag of rows (the primary button), the drag continues as the system's, offering copy, move (not from a read-only folder) and link (local items; Wayland ignores it). The in-page drag ends and says "Dragging notes.txt out of the window". When it ends the result is read out ("Dropped…", "Moved … to another application", "Linked…", "Drag cancelled"); a move leaves the originals to the application that took them, so no job runs here and the list follows the folder's watcher. If the system refuses (or the items are not local), the drag stays in the window with a notice and tries again only after the pointer has been back inside. Where the system cannot drag out, leaving the window does nothing, as before. A right-button drag stays in the window.
- **Between Waypoint windows** a drag out is an ordinary drop in the other window (the receiving window runs the job). Dropped back on the window it came from it keeps what it was (the folder it came from, and that it cannot be moved from a read-only one) and the end of the drag says nothing more, since the job does.
- **Without a pointer:** Paste adopts what another application copied (D107), so Copy and Cut there and Paste here are the paths for drops; Copy and Cut here and Paste there are the paths for drags out.

### 3.2 Modifiers (held at release)

Ctrl copies. Shift moves. Ctrl+Shift links. **Alt opens the action picker**, and a right-drag also opens it on release. The one exception is a lone folder dragged from a view over the file area, where Alt opens the folder in a new pane instead (§3.1, D174), as it opens a split pair on the + button; the picker is then the right button or the "Always ask" rule. The picker lists Copy here, Move here, Link here, Compress here, Extract here (for archives; Copy reads "Add to archive" when the target is an archive), plus plugin drop actions (for example, "Convert to WebP here"). The cursor badge updates live with the action glyph, the count and the target name.

### 3.3 Spring loading

| Target             | Delay  | Visual                                                |
| ------------------ | ------ | ----------------------------------------------------- |
| Folder in a view   | 600 ms | A ring fills around the folder, then the folder opens |
| Tab                | 600 ms | The tab pulses, then activates                        |
| Sidebar item       | 600 ms | A ring, then the view navigates                       |
| Breadcrumb segment | 400 ms | Opens a dropdown of siblings                          |
| Window edge        | 800 ms | Opens a new tab                                       |

Built in milestone 4: folder rows, tabs and sidebar places spring after the delay in Settings (200 to 2000 ms, 600 by default; the target draws a ring that fills over it, or a steady dotted edge under Reduce motion). A folder row opens in the pane it is in, a place in the active pane and a tab comes to the front. Breadcrumb segments and the window edge do not spring yet. After a spring opens something, the next one waits for the pointer to move.

Leaving a sprung target returns you to where you started unless you dropped. Esc cancels the whole drag. A drag near the top or bottom edge of a list scrolls it.

### 3.4 Shelf

- Opens with Ctrl+B, the Shelf button in the status bar or View ▸ Shelf (the pop-up when a drag pauses near an edge is later). It is docked along the bottom of the content column, under the pane or panes and beside the sidebar (which keeps the full height), above the status bar, leaving the right edge to the Inspector, and you resize it by its top edge; it can also be undocked into its own window (below). Its items persist across restarts unless Settings → Drag & drop turns that off, and every window shows the same list.
- **Built in milestone 4 (slice 14).** Keys in the dock, which is a strip of tiles (each group's chip, then its tiles): Left and Right move along the strip, Up and Down move between its lines when it wraps, Home and End go to the ends (Shift extends the selection), Enter opens (a folder in the pane; a file is shown in its folder) or toggles a group (so does Space on a group's chip), Space selects (Ctrl or Shift toggles), Ctrl+A selects all, Esc clears the selection, Delete (or Backspace) removes the selected items from the Shelf and never touches a file, Ctrl+C copies the files for a paste, and the Menu key or Shift+F10 opens the item menu. Add to Shelf has no key; use the menu or the palette. Dropping on the dock adds references with no file operation and cannot be refused except by the 500-item cap; dragging items out follows the default action rule, and a move that finishes removes the entries whose files left. Auto-hide, thumbnails, drops from other applications and Send to… are later.
- It holds references grouped by origin (local or remote). Each item has a thumbnail and its origin path.
- **Built: the Shelf window.** Undock is the button on the dock's header, View ▸ Undock Shelf and the palette's Undock Shelf; Dock is the button on the Shelf window's header, View ▸ Dock Shelf, the palette's Dock Shelf, or closing the window. With the Shelf undocked, the Main windows keep no dock, and Ctrl+B and the status bar button raise the Shelf window, or hide it when it is on screen and focused (Ctrl+B inside the Shelf window hides it). Focus Shelf raises it. Everything in the dock works in the window: the same keys (Left, Right, Up, Down, Home, End, Enter, Space, Ctrl+A, Esc, Delete, Ctrl+C, the Menu key), the item menu, drops (from other applications too) and drags out; the window is resized at its edges instead of by a divider. Its title bar has Always on Top where the system can do it. Position, size and Always on Top are remembered and come back with the session. Dragging the dock's header out of the window to undock it, and dropping the Shelf window on a Main window to dock it, are later. While files from another window are held over the Shelf window, the whole window is the drop target: an accent edge round it and a “Drop to add to the Shelf” hint, which go when they leave or are dropped.
- You can drag the whole Shelf or a subset out of it. The per-item menu has Remove, Reveal, Copy path, and "Send to…".
- You can drop items onto it from other apps.
- Every drag has a path without a pointer: copy, cut and paste, Copy To… and Move To…, F5 and Shift+F5 for the other pane, and Add to Shelf in the menu.

### 3.5 Tabs

> Native window implementation for Tauri: see `tauri-tear-off.md`.

- Drag a tab within the strip to reorder it. A drag starts after 4 px of movement. Drag it out of the window to tear it off (the pointer leaves the client area); a ghost window follows the cursor (where the platform allows it). Inside the window nothing tears off: the file area's four split regions win over it, and the strip, toolbar, sidebar and status bar are reorder.
- One pill follows the pointer with “Esc to cancel” and is announced as “Drag: release to …”. Final texts: “Release to move {tab} to position {n} of {count}”, “Release to move group {name}”, “Release to split with {tab}”, “Release to start a new group”, “Release to add to {group}”, “Release to leave {group}”, “Split left with the current view” (or right, top, bottom), “Release to separate the split”. The new-window phase shows no pill until tear-off arrives.
- Thresholds (`tabs/dragTiming.ts`): 4 px to start, out of the window for the new-window phase (no pixel threshold), 450 ms hold over the middle half of a tab to split, 800 ms rest in a slot to group (a move of more than 6 px restarts either hold), 140 ms of motion (0 under Reduce motion). The file area's split regions are its left and right thirds and the upper and lower halves of the centre column, measured once when the drag starts, with 6 px of slack to stop the borders flickering. The new pane's half is tinted; the active tab may be dragged to them (it splits itself, as F3 does), and they are offered only while neither tab is in a pair.
- Priority on each move: the new-window phase (only when the pointer has left the window), the file area's split regions, a group chip under the pointer, a tab body under the pointer (split hold), then the slot (reorder, leave, or the rest-to-group hold).
- Dropping a tab onto another window's strip merges it there. Dropping it onto a group chip joins the group.
- Dropping files onto a tab springs it open. Dropping onto the "+", or onto the strip's empty space (not a tab, a group chip or a scroll arrow), opens a new tab at the dropped folder, with the same cue and pill.

### 3.6 Feedback

- The drag ghost is a stack of up to 3 thumbnails with a count badge.
- The target highlight is an accent outline plus a tint.
- An invalid target (read-only, same folder) shows a not-allowed badge.
- After release, the ops queue indicator bumps and a toast offers Undo.

## 4. Selection & naming

- **Group headers** (a listing grouped by the empty-space menu's Group by submenu or the Action bar's Sort menu): click a header to fold the group shut or open. The arrow keys, Home, End, Page Up and Page Down step over the header of an open group and stop on the header of a folded one, which has no rows to land on. On an entry in the list, or the first cell of a group in the grid, Left steps up onto the group's header; on a header Left folds the group, Right opens it, Enter or Space does either, and Down or Up leaves it. What a folded group hides is not selectable: folding deselects it, and Select All, Invert Selection and a Shift range skip it. Right-clicking a header opens the empty-space menu.
- Type-ahead jumps to the first prefix match, and the timer resets after 800 ms. Typing `/` starts a filter chip in the path bar area.
- Inline rename (F2, or Rename in the item menu) selects the basename only, and the whole name of a folder or a dotfile. Enter renames, Escape or a click elsewhere cancels, and a bad or taken name stays in the field with the reason under it. If the extension changes, it asks "Change the extension from .jpg to .png?" with Keep .jpg (the default) or Use .png. With several items selected F2 renames the focused one; Ctrl+F2 is batch rename.
- Batch rename (Ctrl+F2) is a dialog of rule stacks with a live preview table. Rules can be added, removed, reordered and changed to another type, and the table follows a moment after the last keystroke. Rows that clash are highlighted and say why in words (the highlight is a bar and bold text, not a colour alone), the summary line counts the problems, and the Apply button stays disabled until there are none and at least one name changes. Focus starts on the first rule's type, never on Apply; Esc cancels. A note appears when a rule changes a file extension. At most 500 rows are drawn, with a count of the rest.

## 5. Path bar

- Breadcrumb mode by default. Click empty space or press Ctrl+L to switch to a text field. Esc returns to breadcrumbs.
- Autocomplete draws on child folders, bookmarks, remotes and history. Tab completes.
- Each segment has a chevron that opens a sibling list, and every segment is a drop target.

### 5.1 Panel toggles

- Right of the path bar: Sidebar (F9) and Split View (F3); the Shelf has its button in the status bar. Each is a toggle button (`aria-pressed` is the panel's state) whose tooltip is its name and key, and a press runs the same registry command as the menu, the palette and the key.
- Each button is a tab stop in the toolbar's order, like back, forward and up. Inspector and Terminal drawer toggles join them when those panels exist.
- On a narrow toolbar (under 560 px) they collapse into one More button; its menu has the same commands as checkable rows.

## 6. Context menu (Liminal Notes and Jar pattern)

Sections separated by rules, with an icon, a label, a right-aligned shortcut and a submenu chevron. Items can be checkboxes, and danger items (such as Delete) come last and in red. It's positioned to stay within the viewport and fully navigable by keyboard. Plugin actions go in a labelled section.

Item context menu: Open · Preview (Quick Look) · Open in New Tab · Open in New Window · Open With ▸ | Cut · Copy · Copy Path · Add to Shelf | Rename · Duplicate · Compress ▸ | Tags ▸ | _Plugin section_ | Properties | Move to Trash. Entries arrive with the feature behind them. Empty-space menu (as built): New ▸ (Folder F7) · Properties in a Window (Alt+Enter; File Shift+F7) | Undo _what it would undo_ (Ctrl+Z), Redo (Ctrl+Shift+Z) | Sort by ▸ · Group by ▸ · Show hidden files | Properties (the Inspector's Properties tab, for the folder); Paste joins after New. As built, the item menu is Open · Open in New Tab · Open in New Window · Open With ▸ (the default application, the recommended ones with their icons, Other Application…; only where the system can offer it and the selection is of one type) | Cut (Ctrl+X) · Copy (Ctrl+C) · Paste (Ctrl+V; Paste Into Folder on a folder) · Add to Favourites (folders) · Add to Shelf · Copy Path | Rename (F2) · Rename Selected… (Ctrl+F2, several selected) · Duplicate (Ctrl+Shift+D) · Compress… (a name and a format, in a dialog) | Extract Here · Extract To… (on an archive, which also has Open in New Tab and Open in New Window as a folder does) | Copy To… · Move To… · Copy to Other Pane (F5) · Move to Other Pane (Shift+F5; the last two only in a pair) | Move to Trash (Delete) · Delete Permanently (Shift+Delete, always confirmed), the last two in the danger style | Properties (the Inspector's Properties tab; always offered, last, where the window can open one). Cut, Paste, Move To… and the other-pane move are hidden in a read-only location while Copy, Copy To… and Copy to Other Pane stay; Paste and Paste Into Folder are disabled while the clipboard is empty, and the other-pane items while the other pane's folder cannot be written to (F5 then opens the dialog). Paste is in the empty-space menu directly after New. Write items are hidden, not disabled, in a read-only location (the Trash, an archive that cannot be changed); in an archive that can be changed (D170) New, Rename, Paste and Move to Trash / Delete Permanently are offered (a rename and a delete ask first, saying that the archive is rewritten and how large it is) and Duplicate, Cut, Move To…, Compress… and Extract Here are not. In the Trash: Restore · Delete Permanently · Empty Trash.

## 7. Tab groups and pairs

| Input                    | Target                         | Result                                                                                                                                     |
| ------------------------ | ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Click                    | Group label                    | Collapse or expand                                                                                                                         |
| Double-click             | Group label                    | Rename in place                                                                                                                            |
| Right-click              | Group label                    | Group menu (see SPEC §5.2)                                                                                                                 |
| Enter, Space             | Focused group label            | Collapse or expand                                                                                                                         |
| F2                       | Focused group label            | Rename in place (Enter commits, Esc cancels, an empty name keeps the old one)                                                              |
| Menu key, Shift+F10      | Focused group label            | Group menu                                                                                                                                 |
| Ctrl+Shift+Left or Right | Focused group label            | Move the whole group one place along the strip                                                                                             |
| Drag                     | Group label                    | Move the group along the strip; out of the window tears it off                                                                             |
| Drag tab onto            | Group label                    | Add the tab (and its pair) to the group                                                                                                    |
| Drag tab out             | Beyond the group's span        | Remove it from the group                                                                                                                   |
| Drag folder onto         | Group label                    | Open it as a new tab in the group                                                                                                          |
| Hold tab over            | Middle of another tab (450 ms) | Split with it (joined pair)                                                                                                                |
| Drag tab to              | Content edge                   | Split left, right, top or bottom                                                                                                           |
| Drag sidebar item to     | Content edge                   | Open its folder in a new pane on that side (Open in Split Pane in its menu)                                                                |
| Alt-drag one folder to   | Content edge                   | Open it in a new pane on that side (Open in Split Pane in its menu); without Alt it is copied or moved into the pane                       |
| Drag                     | Pair joint                     | Move both; out of the window tears off both                                                                                                |
| Double-click             | Pair joint                     | Reset pane sizes                                                                                                                           |
| Right-click              | Pair joint                     | Separate, Swap Panes, Layout, Reset Sizes, Sync Navigation, Compare Folders, Pin, Colour, Group, Duplicate, Move to New Window, Close Both |
| F3                       | Anywhere                       | Split with a new tab (focused); on a toggled split closes the pane it made (Undo toast), on a joined pair separates                        |
| F6, Shift+F6             | Split                          | Move focus to the next, or previous, pane                                                                                                  |
| Arrow keys, Enter        | Pane divider (focused)         | Move it 2% (10% with Shift) along its axis; Enter resets to equal sizes                                                                    |
| Alt+Enter                | Selection or current folder    | Properties window (one per subject, four at most; Esc closes). Also the item menu, the palette and the Inspector's button                  |
| Right-click → Properties | Item or empty space            | Inspector on the Properties tab (the panel opens if it was closed; it shows the item, or the current folder from empty space)              |

## 8. + button, start page, terminal tab

| Input          | Target                                | Result                                             |
| -------------- | ------------------------------------- | -------------------------------------------------- |
| Click          | +                                     | New tab in the current folder                      |
| Middle-click   | +                                     | New tab at Home                                    |
| Right-click    | +                                     | New-tab menu (see SPEC §5.2)                       |
| Drop folder(s) | +                                     | One tab per folder                                 |
| Drop file      | +                                     | Tab in its parent folder                           |
| Alt-drop       | +                                     | Open as a split pair                               |
| Click          | Start-page card                       | Open in this tab                                   |
| Hover          | Start-page card                       | Open in new tab, Remove                            |
| Drag           | Start-page card                       | Reorder within Pinned, Bookmarks or Recent Folders |
| Drop folder    | Pinned, Bookmarks, Recent Folders     | Add it                                             |
| Drag handle    | Start-page section header (Customise) | Reorder sections                                   |
| Double-click   | Terminal tab                          | Rename                                             |
| Drop files     | Terminal pane                         | Insert paths (Alt: cd)                             |

## 9. Navigation, selection and errors

| Input                     | Target                                 | Result                                                                         |
| ------------------------- | -------------------------------------- | ------------------------------------------------------------------------------ |
| Right-click or long-press | Back, Forward                          | History list                                                                   |
| Ctrl+Shift+G              | Anywhere                               | Go to…                                                                         |
| Ctrl+S                    | File view                              | Select by pattern                                                              |
| Ctrl+I                    | File view                              | Invert selection                                                               |
| Ctrl+D                    | Anywhere                               | Add the current folder to Favourites                                           |
| Right-click               | List header                            | Columns                                                                        |
| Drag, or Left and Right   | List column divider                    | Resize the column, for this folder (double-click or Backspace restores it)     |
| Right-click               | Sidebar Places, Favourites, servers    | Rename, move, remove                                                           |
| Drag                      | Sidebar Places, Favourites rows        | Reorder                                                                        |
| Drop folder               | Places or Favourites heading           | Add it                                                                         |
| Right-click               | Tree node                              | Expand, new folder, add to Places                                              |
| Menu                      | File → Copy To… / Move To…             | Destination dialog                                                             |
| Menu                      | File → Open With → Other Application…  | App chooser                                                                    |
| Menu or palette           | File → Open With…, Open With…          | App chooser listing every application, or the system's own for one file        |
| Click or Enter            | Sidebar Places → Overview              | Opens Overview in this tab (middle-click: a new tab beside it)                 |
| Palette                   | Open Overview                          | Opens Overview in the active tab                                               |
| Click or Enter            | Overview → a volume's name             | Opens that volume in this tab                                                  |
| Click or Enter            | Overview → Measure                     | Measures that network volume, which is not measured by default                 |
| Click or Enter            | Overview → Unlock                      | Passphrase dialog, as in the sidebar's Devices                                 |
| Click or Enter            | Overview → Measure now                 | Measures Home (the button reads Cancel while it runs; Cancel stops it at once) |
| Enter                     | Overview → a folder in Biggest folders | Opens that folder in this tab                                                  |
| Click                     | Status bar → Measuring Home            | Shows Overview in the active tab; its × cancels the scan                       |
| Click or Enter            | Overview → Open Trash                  | Opens the Trash in this tab                                                    |
| Click or Enter            | Overview → Empty Trash                 | The Trash's own confirmation, then empties it                                  |

## 10. Operations and tools

| Input                | Target                 | Result                                     |
| -------------------- | ---------------------- | ------------------------------------------ |
| Right-click          | A job in Operations    | Priority, pause, run now, schedule, cancel |
| Click                | Speed in Operations    | Set the transfer speed limit               |
| Right-click          | Folder                 | Share Over Network, Find Duplicates        |
| Right-click          | .iso file              | Mount Disk Image                           |
| Share ▾ (action bar) | Selection              | Nearby Devices, Share Over Network         |
| Right-click          | SANDISK in the sidebar | Format…                                    |
| Right-click          | Empty space            | New Encrypted Vault…                       |

### 10.1 Answering a waiting job

The conflict and error dialogs open by themselves in the window that started the job; Resolve… on the job's row (the ring's popover or the Operations window) opens them from any window. For two files the conflict dialog also shows a thumbnail of each, which is newer or larger, an "Identical contents" notice that says Skip is probably wanted (nothing is chosen for you) and, for text, a compact diff; none of it changes the keys, and a comparison that cannot be made shows nothing.

| Input                                                 | Target          | Result                                                                  |
| ----------------------------------------------------- | --------------- | ----------------------------------------------------------------------- |
| Choice in a row                                       | A conflict      | Answers that clash; only the choices that can be carried out are listed |
| Apply to all remaining                                | Conflict dialog | Answers every clash it can; a row's own choice wins                     |
| Continue                                              | Conflict dialog | Sends the answers; off until every clash is answered                    |
| Cancel the operation                                  | Conflict dialog | Stops the job, after a question once anything was answered              |
| Esc                                                   | Conflict dialog | Closes it and leaves the job waiting                                    |
| Compare the files, Show differences                   | A file clash    | Loads the two files' comparison, then opens its line diff               |
| Decide later                                          | Either dialog   | Closes it and leaves the job waiting                                    |
| Esc                                                   | Error dialog    | Closes it and leaves the job waiting                                    |
| Retry, Skip, Skip all like this, Cancel the operation | Error dialog    | Tells the job what to do with the item that failed                      |

## 11. Customisation and basics

| Input       | Target                            | Result                                                        |
| ----------- | --------------------------------- | ------------------------------------------------------------- |
| F1          | Anywhere in a main window         | Help (a few keys, then the shortcut list, the tour and About) |
| ?           | Anywhere but a text field         | Keyboard Shortcuts: every key the window offers               |
| Click       | A shortcut in Settings → Keyboard | Record a new one                                              |
| Right-click | Pair joint                        | Compare and Sync…                                             |
| Right-click | Item in the floating window       | Copy, Cut, Paste on the shared clipboard                      |
| Menu        | Window Layouts…                   | Save or restore a layout                                      |
