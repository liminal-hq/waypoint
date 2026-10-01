# Interactions

## 1. Mouse

| Input                | Target                           | Result                                                            |
| -------------------- | -------------------------------- | ----------------------------------------------------------------- |
| Click                | Item                             | Select (single-click-to-open is an optional setting)              |
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

| Keys                      | Action                                                    |
| ------------------------- | --------------------------------------------------------- |
| Ctrl+T / Ctrl+W           | New tab / close tab                                       |
| Ctrl+Shift+T              | Reopen closed tab                                         |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab (most recently used)                  |
| Alt+1…9                   | Go to tab                                                 |
| Ctrl+Shift+N              | New window (one tab at Home)                              |
| F7 / Shift+F7             | New folder / new file                                     |
| Ctrl+Shift+D              | Duplicate                                                 |
| Menu key / Shift+F10      | On a tab: its menu (Move to New Window, Move to Window ▸) |
| Ctrl+K or Ctrl+Shift+P    | Command palette (commands, and the undo history)          |
| Ctrl+L                    | Edit path                                                 |
| Ctrl+F                    | Search · `/` inline filter                                |
| F2                        | Rename · Ctrl+F2 batch rename                             |
| F3                        | Toggle split                                              |
| F4                        | Terminal drawer                                           |
| F5 / Shift+F5             | Copy / move to the other pane                             |
| F6                        | Switch the active pane                                    |
| F9                        | Sidebar · F11 inspector                                   |
| Space                     | Quick Look                                                |
| Ctrl+C / Ctrl+X / Ctrl+V  | Copy / cut / paste files (the system clipboard)           |
| Ctrl+Z / Ctrl+Shift+Z     | Undo / redo                                               |
| Ctrl+H                    | Hidden files                                              |
| Ctrl+1…5                  | Grid, List, Columns, Compact, Disk usage                  |
| Ctrl+D                    | Bookmark                                                  |
| Ctrl+B                    | Toggle the Shelf                                          |
| Delete / Shift+Delete     | Trash / delete permanently                                |
| Alt+Enter                 | Properties                                                |

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

### 3.2 Modifiers (held at release)

Ctrl copies. Shift moves. Ctrl+Shift links. **Alt opens the action picker**, and a right-drag also opens it on release. The picker lists Copy here, Move here, Link here, Compress here, Extract here (for archives), plus plugin drop actions (for example, "Convert to WebP here"). The cursor badge updates live with the action glyph, the count and the target name.

### 3.3 Spring loading

| Target             | Delay  | Visual                                                |
| ------------------ | ------ | ----------------------------------------------------- |
| Folder in a view   | 600 ms | A ring fills around the folder, then the folder opens |
| Tab                | 600 ms | The tab pulses, then activates                        |
| Sidebar item       | 600 ms | A ring, then the view navigates                       |
| Breadcrumb segment | 400 ms | Opens a dropdown of siblings                          |
| Window edge        | 800 ms | Opens a new tab                                       |

Leaving a sprung target returns you to where you started unless you dropped. Esc cancels the whole drag.

### 3.4 Shelf

- Opens with Ctrl+B or the toolbar button, or pops up automatically when a drag pauses near the right edge. It is docked at first; a floating Shelf comes later. Its items persist across restarts.
- It holds references grouped by origin (local or remote). Each item has a thumbnail and its origin path.
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
- Dropping files onto a tab springs it open. Dropping onto the "+" opens a new tab at the dropped folder.

### 3.6 Feedback

- The drag ghost is a stack of up to 3 thumbnails with a count badge.
- The target highlight is an accent outline plus a tint.
- An invalid target (read-only, same folder) shows a not-allowed badge.
- After release, the ops queue indicator bumps and a toast offers Undo.

## 4. Selection & naming

- Type-ahead jumps to the first prefix match, and the timer resets after 800 ms. Typing `/` starts a filter chip in the path bar area.
- Inline rename (F2, or Rename in the item menu) selects the basename only, and the whole name of a folder or a dotfile. Enter renames, Escape or a click elsewhere cancels, and a bad or taken name stays in the field with the reason under it. If the extension changes, it asks "Change the extension from .jpg to .png?" with Keep .jpg (the default) or Use .png. With several items selected F2 renames the focused one; Ctrl+F2 is batch rename.
- Batch rename (Ctrl+F2) is a dialog of rule stacks with a live preview table. Rules can be added, removed, reordered and changed to another type, and the table follows a moment after the last keystroke. Rows that clash are highlighted and say why in words (the highlight is a bar and bold text, not a colour alone), the summary line counts the problems, and the Apply button stays disabled until there are none and at least one name changes. Focus starts on the first rule's type, never on Apply; Esc cancels. A note appears when a rule changes a file extension. At most 500 rows are drawn, with a count of the rest.

## 5. Path bar

- Breadcrumb mode by default. Click empty space or press Ctrl+L to switch to a text field. Esc returns to breadcrumbs.
- Autocomplete draws on child folders, bookmarks, remotes and history. Tab completes.
- Each segment has a chevron that opens a sibling list, and every segment is a drop target.

## 6. Context menu (Liminal Notes and Jar pattern)

Sections separated by rules, with an icon, a label, a right-aligned shortcut and a submenu chevron. Items can be checkboxes, and danger items (such as Delete) come last and in red. It's positioned to stay within the viewport and fully navigable by keyboard. Plugin actions go in a labelled section.

Item context menu: Open · Preview (Quick Look) · Open in New Tab · Open in New Window · Open With ▸ | Cut · Copy · Copy Path · Add to Shelf | Rename · Duplicate · Compress ▸ | Tags ▸ | _Plugin section_ | Properties | Move to Trash. Entries arrive with the feature behind them. Empty-space menu (as built): New ▸ (Folder F7, File Shift+F7) | Undo _what it would undo_ (Ctrl+Z), Redo (Ctrl+Shift+Z) | Sort by · Show hidden files; Paste joins after New. As built, the item menu is Open · Open in New Tab · Open in New Window | Add to Favourites (folders) · Copy Path | Rename (F2) · Duplicate (Ctrl+Shift+D) | Move to Trash (Delete) · Delete Permanently (Shift+Delete, always confirmed), the last two in the danger style. Write items are hidden, not disabled, in a read-only location (the Trash, an archive). In the Trash: Restore · Delete Permanently · Empty Trash.

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
| Drag                     | Pair joint                     | Move both; out of the window tears off both                                                                                                |
| Double-click             | Pair joint                     | Reset pane sizes                                                                                                                           |
| Right-click              | Pair joint                     | Separate, Swap Panes, Layout, Reset Sizes, Sync Navigation, Compare Folders, Pin, Colour, Group, Duplicate, Move to New Window, Close Both |
| F3                       | Anywhere                       | Split with a new tab (focused); on a toggled split closes the pane it made (Undo toast), on a joined pair separates                        |
| F6, Shift+F6             | Split                          | Move focus to the next, or previous, pane                                                                                                  |
| Arrow keys, Enter        | Pane divider (focused)         | Move it 2% (10% with Shift) along its axis; Enter resets to equal sizes                                                                    |
| Alt+Enter                | Selection or current folder    | Floating Properties window                                                                                                                 |
| Right-click → Properties | Item or empty space            | Inspector on the Properties tab                                                                                                            |

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

| Input                     | Target                                | Result                               |
| ------------------------- | ------------------------------------- | ------------------------------------ |
| Right-click or long-press | Back, Forward                         | History list                         |
| Ctrl+Shift+G              | Anywhere                              | Go to…                               |
| Ctrl+S                    | File view                             | Select by pattern                    |
| Ctrl+I                    | File view                             | Invert selection                     |
| Ctrl+D                    | Anywhere                              | Add the current folder to Favourites |
| Right-click               | List header                           | Columns                              |
| Right-click               | Sidebar Places, Favourites, servers   | Rename, move, remove                 |
| Drag                      | Sidebar Places, Favourites rows       | Reorder                              |
| Drop folder               | Places or Favourites heading          | Add it                               |
| Right-click               | Tree node                             | Expand, new folder, add to Places    |
| Menu                      | File → Copy To… / Move To…            | Destination dialog                   |
| Menu                      | File → Open With → Other Application… | App chooser                          |

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

## 11. Customisation and basics

| Input       | Target                            | Result                                   |
| ----------- | --------------------------------- | ---------------------------------------- |
| F1          | Anywhere                          | Help                                     |
| Click       | A shortcut in Settings → Keyboard | Record a new one                         |
| Right-click | Pair joint                        | Compare and Sync…                        |
| Right-click | Item in the floating window       | Copy, Cut, Paste on the shared clipboard |
| Menu        | Window Layouts…                   | Save or restore a layout                 |
