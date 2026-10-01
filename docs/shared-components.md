# Shared Components (Liminal family)

## Unified title bar

The copies have drifted. Spindle, Cadence, Threshold and Jar each have their own title bar. Waypoint defines a single standard that should be pushed back into the others.

| Capability                                                         | Source    | Unified behaviour                                                                |
| ------------------------------------------------------------------ | --------- | -------------------------------------------------------------------------------- |
| App mark and menu button, with the menu openable from the keyboard | Jar       | Left side. Alt or F10 opens the menu.                                            |
| Drag region on the bar and its non-interactive children            | Jar       | `data-tauri-drag-region` on all passive nodes                                    |
| Maximised-state hook (changes the restore glyph and radius)        | Jar       | Square corners when maximised                                                    |
| Always on Top toggle                                               | Threshold | Optional slot, on in Waypoint                                                    |
| App content in the bar (search, title, status)                     | Spindle   | Centre slot. Waypoint shows the location title only. Tabs stay in their own row. |
| Double-click to maximise                                           | All       | Standard                                                                         |
| Platform control layout                                            | New       | GNOME/KDE button layout from the OS, Win11 caption style, macOS-style unused     |
| Transparency-aware                                                 | New       | Follows the Transparency "Title bar" region                                      |

Slots: `start` (mark and menu) · `center` (title and app content) · `end` (app actions) · `controls` (window buttons).

## Context menu

Source: `ScottMorris/liminal-notes` (Editor ContextMenu) and `liminal-hq/jar` (ContextMenu module).

- Item types: action, checkbox, submenu, separator, section label, danger.
- Anatomy: a 16 px icon, a label, a right-aligned shortcut in a muted monospace font, and a submenu chevron.
- Icons: every action, checkbox and submenu item has an icon, so labels line up and the menu reads the same everywhere. Colour items show their swatch, and a checked checkbox shows the check mark in place of its icon. Icons are `aria-hidden` (the label always carries the meaning) and take the secondary text colour, or the row's own colour when it is highlighted or a danger item.
- Where icons live: chrome menus use `packages/chrome/src/icons/icons.tsx`; the app's menus use `apps/waypoint/src/icons/MenuIcons.tsx` (and `AppIcons.tsx`). The item-to-icon mapping sits in each menu's item builder, and each builder's test calls `expectEveryItemHasIcon` (`@liminal-hq/waypoint-chrome/ContextMenu/expectEveryItemHasIcon`), which walks nested submenus, so a new item cannot ship without one.
- Behaviour: positioned to stay in the viewport, arrow keys, Enter and Esc work, typing a letter jumps to an item, submenus open on hover after 150 ms.
- Styling: a raised surface, a subtle border, a 8 px radius, 4 px padding, rows 28 px high and 6 px row radius, accent hover.

## Settings shell

Same as Emoji Nook's SettingsPanel: a side-nav of sections with grouped rows (label, description, control), so every Liminal app shares one settings pattern.
