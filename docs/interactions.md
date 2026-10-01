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

| Keys                      | Action                                   |
| ------------------------- | ---------------------------------------- |
| Ctrl+T / Ctrl+W           | New tab / close tab                      |
| Ctrl+Shift+T              | Reopen closed tab                        |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab (most recently used) |
| Alt+1…9                   | Go to tab                                |
| Ctrl+Shift+N              | New window                               |
| Ctrl+K or Ctrl+Shift+P    | Command palette                          |
| Ctrl+L                    | Edit path                                |
| Ctrl+F                    | Search · `/` inline filter               |
| F2                        | Rename · Ctrl+F2 batch rename            |
| F3                        | Toggle split                             |
| F4                        | Terminal drawer                          |
| F5 / Shift+F5             | Copy / move to the other pane            |
| F6                        | Switch the active pane                   |
| F9                        | Sidebar · F11 inspector                  |
| Space                     | Quick Look                               |
| Ctrl+Z / Ctrl+Shift+Z     | Undo / redo                              |
| Ctrl+H                    | Hidden files                             |
| Ctrl+1…5                  | Grid, List, Columns, Compact, Disk usage |
| Ctrl+D                    | Bookmark                                 |
| Ctrl+B                    | Toggle the Shelf                         |
| Delete / Shift+Delete     | Trash / delete permanently               |
| Alt+Enter                 | Properties                               |

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

- Opens with Ctrl+B or the toolbar button, or pops up automatically when a drag pauses near the right edge. The shelf can float or be docked.
- It holds references grouped by origin (local or remote). Each item has a thumbnail and its origin path.
- You can drag the whole Shelf or a subset out of it. The per-item menu has Remove, Reveal, Copy path, and "Send to…".
- You can drop items onto it from other apps.

### 3.5 Tabs

> Native window implementation for Tauri: see `tauri-tear-off.md`.

- Drag a tab within the strip to reorder it. A drag starts after 4 px of movement. Drag it more than 24 px out of the strip to tear it off; a ghost window follows the cursor (where the platform allows it).
- Dropping a tab onto another window's strip merges it there. Dropping it onto a group chip joins the group.
- Dropping files onto a tab springs it open. Dropping onto the "+" opens a new tab at the dropped folder.

### 3.6 Feedback

- The drag ghost is a stack of up to 3 thumbnails with a count badge.
- The target highlight is an accent outline plus a tint.
- An invalid target (read-only, same folder) shows a not-allowed badge.
- After release, the ops queue indicator bumps and a toast offers Undo.

## 4. Selection & naming

- Type-ahead jumps to the first prefix match, and the timer resets after 800 ms. Typing `/` starts a filter chip in the path bar area.
- Inline rename selects the basename only. If the extension changes, it asks "Change .jpg to .png?" with Keep or Use .png.
- Batch rename (Ctrl+F2) is a dialog of rule stacks with a live preview table. Rows that clash are highlighted and the Apply button stays disabled until they're resolved.

## 5. Path bar

- Breadcrumb mode by default. Click empty space or press Ctrl+L to switch to a text field. Esc returns to breadcrumbs.
- Autocomplete draws on child folders, bookmarks, remotes and history. Tab completes.
- Each segment has a chevron that opens a sibling list, and every segment is a drop target.

## 6. Context menu (Liminal Notes and Jar pattern)

Sections separated by rules, with an icon, a label, a right-aligned shortcut and a submenu chevron. Items can be checkboxes, and danger items (such as Delete) come last and in red. It's positioned to stay within the viewport and fully navigable by keyboard. Plugin actions go in a labelled section.

Item context menu: Open · Preview (Quick Look) · Open in New Tab · Open in New Window · Open With ▸ | Cut · Copy · Copy Path · Add to Shelf | Rename · Duplicate · Compress ▸ | Tags ▸ | _Plugin section_ | Properties | Move to Trash.

## 7. Tab groups and pairs

| Input                    | Target                         | Result                                                                                                                                     |
| ------------------------ | ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Click                    | Group label                    | Collapse or expand                                                                                                                         |
| Double-click             | Group label                    | Rename in place                                                                                                                            |
| Right-click              | Group label                    | Group menu (see SPEC §5.2)                                                                                                                 |
| Drag                     | Group label                    | Move the group along the strip; out of the window tears it off                                                                             |
| Drag tab onto            | Group label                    | Add the tab (and its pair) to the group                                                                                                    |
| Drag tab out             | Beyond the group's span        | Remove it from the group                                                                                                                   |
| Drag folder onto         | Group label                    | Open it as a new tab in the group                                                                                                          |
| Hold tab over            | Middle of another tab (450 ms) | Split with it (joined pair)                                                                                                                |
| Drag tab to              | Content edge                   | Split left, right, top or bottom                                                                                                           |
| Drag                     | Pair joint                     | Move both; out of the window tears off both                                                                                                |
| Double-click             | Pair joint                     | Reset pane sizes                                                                                                                           |
| Right-click              | Pair joint                     | Separate, Swap Panes, Layout, Reset Sizes, Sync Navigation, Compare Folders, Pin, Colour, Group, Duplicate, Move to New Window, Close Both |
| F3                       | Anywhere                       | Split with a new tab, or separate if already split                                                                                         |
| F6                       | Split                          | Move focus to the next pane                                                                                                                |
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
